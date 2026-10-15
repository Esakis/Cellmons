//! Offline session owns the full state; only PlayerView crosses the WASM boundary.
use engine::{
    CardAction, DuelState, Event, EventBuf, Kind, Move, Outcome, Phase, PlayerSetup, Side,
};
use engine_ai::{Bot, Level, training_profiles};
use protocol::{Message, game::v1 as wire};
use wasm_bindgen::prelude::wasm_bindgen;

struct Session {
    state: DuelState,
    bot: Bot,
    player_bot: Option<Bot>,
    opponent_avatar: u32,
    last: Vec<wire::DuelEvent>,
}

#[wasm_bindgen]
#[derive(Default)]
pub struct Training {
    session: Option<Session>,
}

#[wasm_bindgen]
impl Training {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

    /// Every request receives a correlated reply, including malformed input.
    pub fn dispatch(&mut self, bytes: &[u8]) -> Vec<u8> {
        let mut reply = wire::TrainingReply::default();
        match wire::TrainingRequest::decode(bytes) {
            Err(_) => reply.error = "invalid_request".into(),
            Ok(request) => {
                reply.seq = request.seq;
                if let Err(error) = self.handle(request, &mut reply) {
                    reply.error = error;
                }
            }
        }
        reply.encode_to_vec()
    }
}

impl Training {
    fn handle(
        &mut self,
        request: wire::TrainingRequest,
        reply: &mut wire::TrainingReply,
    ) -> Result<(), String> {
        use wire::training_request::Body;
        let body = request.body.ok_or("invalid_request")?;
        if let Body::Start(setup) = body {
            self.session = Some(start(setup)?);
        } else {
            let session = self.session.as_mut().ok_or("no_session")?;
            let content = engine_content::content();
            match body {
                Body::Play(input) => {
                    let mut mv = decode_move(&input)?;
                    if session.state.is_finished() {
                        return Err("finished".into());
                    }
                    if let Some(bot) = &mut session.player_bot {
                        mv = bot.choose(&engine::view(&session.state, Side::A), &content);
                    }
                    // Rejected drafts must not advance the opponent's RNG.
                    engine::validate_move(&session.state, &content, Side::A, &mv)
                        .map_err(move_error)?;
                    let other = session
                        .bot
                        .choose(&engine::view(&session.state, Side::B), &content);
                    let mut events = EventBuf::new();
                    engine::resolve_round(&mut session.state, &content, &[mv, other], &mut events)
                        .map_err(|_| "resolve_failed")?;
                    session.bot.observe(&engine::view(&session.state, Side::B));
                    if let Some(bot) = &mut session.player_bot {
                        bot.observe(&engine::view(&session.state, Side::A));
                    }
                    session.last = events.as_slice().iter().filter_map(event).collect();
                    reply.events = session.last.clone();
                }
                Body::Preview(input) => {
                    let mv = decode_move(&input)?;
                    fill_preview(session, mv, reply);
                    reply.view = Some(snapshot(session));
                    return Ok(());
                }
                Body::Forfeit(_) => {
                    if session.state.is_finished() {
                        reply.events = session.last.clone();
                        reply.view = Some(snapshot(session));
                        fill_preview(
                            session,
                            engine::default_move(&session.state, Side::A),
                            reply,
                        );
                        return Ok(());
                    }
                    let mut events = EventBuf::new();
                    engine::end_by(
                        &mut session.state,
                        Side::A,
                        engine::Termination::Forfeit,
                        &mut events,
                    )
                    .map_err(|_| "finished")?;
                    session.last = events.as_slice().iter().filter_map(event).collect();
                    reply.events = session.last.clone();
                }
                Body::Replay(_) => reply.events = session.last.clone(),
                Body::Start(_) => unreachable!(),
            }
        }
        let session = self.session.as_ref().ok_or("no_session")?;
        reply.view = Some(snapshot(session));
        fill_preview(
            session,
            engine::default_move(&session.state, Side::A),
            reply,
        );
        Ok(())
    }
}

fn start(setup: wire::TrainingSetup) -> Result<Session, String> {
    let content = engine_content::content();
    let version: String = content
        .version()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    if setup.content_version != version {
        return Err("content_mismatch".into());
    }
    let seed: [u8; 32] = setup.seed.try_into().map_err(|_| "invalid_seed")?;
    let id = u16::try_from(setup.avatar_id).map_err(|_| "invalid_avatar")?;
    let avatar = engine_content::avatar(id).ok_or("invalid_avatar")?;
    let starter = engine_content::STARTERS
        .iter()
        .find(|starter| starter.avatar == id)
        .ok_or("invalid_avatar")?;
    let profiles = training_profiles();
    let profile = profiles
        .bots
        .iter()
        .find(|profile| profile.id == setup.profile_id)
        .ok_or("invalid_profile")?;
    let rotation = setup.rotation as usize % profile.avatar_pool.len();
    let opponent_avatar = u32::from(profile.avatar_pool[rotation]);
    let opponent = profile.player(1, rotation).ok_or("invalid_profile")?;
    let players = [
        PlayerSetup {
            avatar: avatar.snapshot(1),
            deck: starter.deck.iter().copied().collect(),
        },
        opponent,
    ];
    let state = engine::new_duel(
        &engine::DuelSetup {
            rules: engine_content::rules(),
            seed,
            players,
        },
        &content,
    )
    .map_err(|_| "invalid_setup")?;
    let bot_seed = u64::from_le_bytes(seed[..8].try_into().expect("eight bytes"));
    Ok(Session {
        state,
        bot: profile.bot(bot_seed ^ 0xb07),
        player_bot: setup
            .autoplay
            .then(|| Bot::new(Level::L2, bot_seed ^ 0xa07)),
        opponent_avatar,
        last: Vec::new(),
    })
}

fn decode_move(input: &wire::DuelMove) -> Result<Move, String> {
    fn slots(values: &[u32]) -> Result<[Kind; 3], String> {
        let kinds: Vec<_> = values
            .iter()
            .map(|kind| match kind {
                0 => Ok(Kind::Phys),
                1 => Ok(Kind::Mag),
                _ => Err("invalid_move".to_owned()),
            })
            .collect::<Result<_, _>>()?;
        kinds.try_into().map_err(|_| "invalid_move".into())
    }
    let index = u8::try_from(input.hand_index).map_err(|_| "invalid_move")?;
    Ok(Move {
        attacks: slots(&input.attacks)?,
        defenses: slots(&input.defenses)?,
        card: match input.action {
            0 => CardAction::None,
            1 => CardAction::Cast(index),
            2 => CardAction::Discard(index),
            _ => return Err("invalid_move".into()),
        },
    })
}

fn kinds(values: &[Kind; 3]) -> Vec<u32> {
    values
        .iter()
        .map(|kind| u32::from(*kind == Kind::Mag))
        .collect()
}
fn encode_move(mv: &Move) -> wire::DuelMove {
    let (action, hand_index) = match mv.card {
        CardAction::None => (0, 0),
        CardAction::Cast(i) => (1, u32::from(i)),
        CardAction::Discard(i) => (2, u32::from(i)),
    };
    wire::DuelMove {
        attacks: kinds(&mv.attacks),
        defenses: kinds(&mv.defenses),
        action,
        hand_index,
    }
}
fn move_error(error: engine::MoveError) -> String {
    use engine::MoveError::*;
    match error {
        WrongPhase => "finished",
        InvalidHandIndex => "invalid_hand",
        RequirementsNotMet => "requirements",
        InterventionUsed => "intervention",
        NotEnoughFocus => "focus",
        NotEnoughStamina => "stamina",
        UnknownCard => "unknown_card",
    }
    .into()
}
fn preview(p: engine::Preview) -> wire::MovePreview {
    wire::MovePreview {
        changes: u32::from(p.cost.changes),
        focus_cost: p.cost.focus,
        stamina_cost: p.cost.stamina,
        focus_after: p.focus_after,
        max_attack_stamina: p.max_attack_stamina,
        error: p.validity.err().map(move_error).unwrap_or_default(),
    }
}
fn fill_preview(session: &Session, mv: Move, reply: &mut wire::TrainingReply) {
    let seen = engine::view(&session.state, Side::A);
    let content = engine_content::content();
    reply.preview = Some(preview(seen.preview(&content, &mv)));
    reply.cast_options = (0..seen.hand.len())
        .map(|index| {
            preview(seen.preview(
                &content,
                &Move {
                    card: CardAction::Cast(index as u8),
                    ..mv
                },
            ))
        })
        .collect();
}
fn snapshot(session: &Session) -> wire::TrainingView {
    let seen = engine::view(&session.state, Side::A);
    wire::TrainingView {
        round: u32::from(seen.round),
        total_rounds: u32::from(seen.rules.duel.rounds),
        initiative: seen.initiative.index() as u32,
        me: Some(side(&seen.me)),
        opponent: Some(side(&seen.opponent)),
        hand: seen.hand.iter().map(|id| u32::from(*id)).collect(),
        enchantments: seen
            .enchantments
            .iter()
            .map(|(owner, id)| wire::DuelEnchantment {
                side: owner.index() as u32,
                card_id: u32::from(*id),
            })
            .collect(),
        outcome: match seen.phase {
            Phase::Finished(result) => Some(outcome(result)),
            _ => None,
        },
        opponent_avatar_id: session.opponent_avatar,
    }
}
fn side(seen: &engine::SideView) -> wire::DuelSide {
    wire::DuelSide {
        hp: seen.hp,
        hp_start: seen.hp_start,
        stamina: seen.stamina,
        focus: seen.focus,
        stats: engine::Stat::ALL.map(|stat| seen.stats.get(stat)).to_vec(),
        schools: seen.schools.0.map(u32::from).to_vec(),
        attacks: kinds(&seen.attacks),
        defenses: kinds(&seen.defenses),
        deck_len: u32::from(seen.deck_len),
        hand_len: u32::from(seen.hand_len),
        discard: seen.discard.iter().map(|id| u32::from(*id)).collect(),
    }
}
fn outcome(result: Outcome) -> wire::DuelOutcome {
    let (winner, reason) = match result {
        Outcome::Win { winner, reason } => (winner.index() as u32 + 1, reason),
        Outcome::Draw { reason } => (0, reason),
    };
    wire::DuelOutcome {
        winner,
        reason: match reason {
            engine::Reason::Knockout => "knockout",
            engine::Reason::Rounds => "rounds",
            engine::Reason::Forfeit => "forfeit",
            engine::Reason::Disconnect => "disconnect",
            engine::Reason::Timeout => "timeout",
            engine::Reason::ConditionalWin => "conditional",
        }
        .into(),
    }
}

fn event(input: &Event) -> Option<wire::DuelEvent> {
    use Event::*;
    let mut out = wire::DuelEvent::default();
    // The opponent's draws stay private. All other card IDs here are public.
    out.kind = match *input {
        CardDrawn { side: Side::B, .. } => return None,
        RoundStarted { round, .. } => {
            out.amount = i32::from(round);
            "round"
        }
        MovesRevealed { moves, change_cost } => {
            out.moves = moves.iter().map(encode_move).collect();
            out.old_value = change_cost[0];
            out.new_value = change_cost[1];
            "reveal"
        }
        SpellCast {
            side,
            card,
            focus,
            stamina,
        } => {
            out.side = side.index() as u32;
            out.card_id = u32::from(card);
            out.old_value = focus;
            out.new_value = stamina;
            "cast"
        }
        SpellCountered {
            side,
            card,
            focus,
            stamina,
        } => {
            out.side = side.index() as u32;
            out.card_id = u32::from(card);
            out.old_value = focus;
            out.new_value = stamina;
            "countered"
        }
        SpellFailed { side, card } => {
            out.side = side.index() as u32;
            out.card_id = u32::from(card);
            "failed"
        }
        SpellFizzled { side, card } => {
            out.side = side.index() as u32;
            out.card_id = u32::from(card);
            "fizzled"
        }
        RandomSpell { side, drawn, .. } => {
            out.side = side.index() as u32;
            out.card_id = u32::from(drawn);
            "random"
        }
        CardDiscarded { side, card } | ForcedDiscard { side, card, .. } => {
            out.side = side.index() as u32;
            out.card_id = u32::from(card);
            "discard"
        }
        CardDrawn { side, card } => {
            out.side = side.index() as u32;
            out.card_id = u32::from(card);
            "draw"
        }
        EnchantmentAdded { side, card } => {
            out.side = side.index() as u32;
            out.card_id = u32::from(card);
            "enchant"
        }
        EnchantmentRemoved { side, card, .. } => {
            out.side = side.index() as u32;
            out.card_id = u32::from(card);
            "disenchant"
        }
        StatChanged {
            side,
            stat,
            old,
            new,
            source,
        } => {
            out.side = side.index() as u32;
            out.card_id = u32::from(source);
            out.old_value = old;
            out.new_value = new;
            out.detail = format!("{stat:?}");
            "stat"
        }
        ResourceChanged {
            side,
            resource,
            old,
            new,
            source,
        } => {
            out.side = side.index() as u32;
            out.card_id = u32::from(source);
            out.old_value = old;
            out.new_value = new;
            out.detail = format!("{resource:?}");
            "resource"
        }
        SchoolChanged {
            side,
            school,
            old,
            new,
            source,
        } => {
            out.side = side.index() as u32;
            out.card_id = u32::from(source);
            out.old_value = i32::from(old);
            out.new_value = i32::from(new);
            out.detail = format!("{school:?}");
            "school"
        }
        Healed {
            side,
            amount,
            source,
        } => {
            out.side = side.index() as u32;
            out.card_id = u32::from(source);
            out.amount = amount;
            "heal"
        }
        Damaged {
            side,
            amount,
            source,
        } => {
            out.side = side.index() as u32;
            out.card_id = u32::from(source);
            out.amount = amount;
            "damage"
        }
        ChoicesSwapped { side, source } => {
            out.side = side.index() as u32;
            out.card_id = u32::from(source);
            "swap"
        }
        AttackOverride {
            attacker,
            hit,
            source,
            ..
        } => {
            out.side = attacker.index() as u32;
            out.card_id = u32::from(source);
            out.amount = i32::from(hit);
            "override"
        }
        DamageTakenChanged {
            side,
            percent,
            source,
        } => {
            out.side = side.index() as u32;
            out.card_id = u32::from(source);
            out.new_value = percent as i32;
            "multiplier"
        }
        AttackHit {
            attacker,
            slot,
            kind,
            damage,
            stamina_cost,
            multiplier,
        } => {
            out.side = attacker.index() as u32;
            out.slot = u32::from(slot);
            out.attack_kind = u32::from(kind == Kind::Mag);
            out.amount = damage;
            out.old_value = multiplier;
            out.new_value = stamina_cost;
            "hit"
        }
        AttackBlocked {
            attacker,
            slot,
            kind,
        } => {
            out.side = attacker.index() as u32;
            out.slot = u32::from(slot);
            out.attack_kind = u32::from(kind == Kind::Mag);
            "block"
        }
        AttackMissed {
            attacker,
            slot,
            kind,
        } => {
            out.side = attacker.index() as u32;
            out.slot = u32::from(slot);
            out.attack_kind = u32::from(kind == Kind::Mag);
            "miss"
        }
        DuelEnded { .. } => "end",
    }
    .into();
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test as test;
    fn call(
        training: &mut Training,
        seq: u32,
        body: wire::training_request::Body,
    ) -> wire::TrainingReply {
        wire::TrainingReply::decode(
            training
                .dispatch(
                    &wire::TrainingRequest {
                        seq,
                        body: Some(body),
                    }
                    .encode_to_vec(),
                )
                .as_slice(),
        )
        .unwrap()
    }
    fn setup() -> wire::TrainingSetup {
        wire::TrainingSetup {
            avatar_id: 1,
            profile_id: "archmage".into(),
            seed: vec![42; 32],
            rotation: 2,
            autoplay: true,
            content_version: engine_content::CONTENT_VERSION
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect(),
        }
    }
    #[test]
    fn worker_protocol_finishes_a_deterministic_bot_duel_and_replays_without_mutation() {
        use wire::training_request::Body;
        let run = || {
            let mut training = Training::new();
            let mut reply = call(&mut training, 1, Body::Start(setup()));
            assert!(reply.error.is_empty());
            for seq in 2..=6 {
                let side = reply.view.as_ref().unwrap().me.as_ref().unwrap();
                let mv = wire::DuelMove {
                    attacks: side.attacks.clone(),
                    defenses: side.defenses.clone(),
                    ..Default::default()
                };
                let before = engine::state_hash(&training.session.as_ref().unwrap().state);
                let p = call(&mut training, 20, Body::Preview(mv.clone()));
                assert!(p.error.is_empty());
                assert_eq!(
                    before,
                    engine::state_hash(&training.session.as_ref().unwrap().state)
                );
                reply = call(&mut training, seq, Body::Play(mv));
                assert!(reply.error.is_empty(), "{}", reply.error);
                assert!(!reply.events.iter().any(|e| e.kind == "draw" && e.side == 1));
                if reply.view.as_ref().unwrap().outcome.is_some() {
                    break;
                }
            }
            assert!(reply.view.as_ref().unwrap().outcome.is_some());
            let replay = call(&mut training, 30, Body::Replay(wire::TrainingReplay {}));
            assert_eq!(reply.view, replay.view);
            assert_eq!(reply.events, replay.events);
            reply.view
        };
        assert_eq!(run(), run());
    }
    #[test]
    fn rejects_malformed_requests_and_keeps_session_on_bad_restart() {
        use wire::training_request::Body;
        let mut training = Training::new();
        assert!(
            !wire::TrainingReply::decode(training.dispatch(&[255]).as_slice())
                .unwrap()
                .error
                .is_empty()
        );
        call(&mut training, 1, Body::Start(setup()));
        let hash = engine::state_hash(&training.session.as_ref().unwrap().state);
        let mut wrong = setup();
        wrong.content_version.clear();
        assert_eq!(
            call(&mut training, 2, Body::Start(wrong)).error,
            "content_mismatch"
        );
        assert_eq!(
            hash,
            engine::state_hash(&training.session.as_ref().unwrap().state)
        );
        assert_eq!(
            call(&mut training, 3, Body::Play(wire::DuelMove::default())).error,
            "invalid_move"
        );
    }
}
