//! Offline session owns the full state; only PlayerView crosses the WASM boundary.
use engine::{CardAction, DuelState, Event, EventBuf, Move, PlayerSetup, Side};
use engine_ai::{Bot, Level, training_profiles};
use protocol::duel::{decode_move, move_error, preview};
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
    let level = setup.custom.as_ref().map_or(1, |p| p.level);
    if !(1..=20).contains(&level) {
        return Err("invalid_setup".into());
    }
    let mut player = PlayerSetup {
        avatar: avatar.snapshot(level as u8),
        deck: Default::default(),
    };
    if let Some(custom) = setup.custom {
        if !custom.specialization.is_empty() || custom.gift_id != 0 {
            let path = engine_content::path(id).ok_or("invalid_setup")?;
            if !custom.specialization.is_empty() {
                player.avatar.specialization = Some(
                    *path
                        .specializations
                        .iter()
                        .find(|s| engine_content::specialization_key(**s) == custom.specialization)
                        .ok_or("invalid_setup")?,
                );
            }
            if custom.gift_id != 0 {
                let gift = u16::try_from(custom.gift_id).map_err(|_| "invalid_setup")?;
                if !path.gifts.contains(&gift) {
                    return Err("invalid_setup".into());
                }
                player.avatar.gift = Some(gift);
            }
        }

        if custom.spent.len() != 6 || custom.spent.iter().any(|v| *v > 100) {
            return Err("invalid_setup".into());
        }
        player.avatar.stats.phys_atk += custom.spent[0] as i32;
        player.avatar.stats.phys_def += custom.spent[1] as i32;
        player.avatar.stats.mag_atk += custom.spent[2] as i32;
        player.avatar.stats.mag_def += custom.spent[3] as i32;
        player.avatar.stats.hp_max += 7 * custom.spent[4] as i32;
        player.avatar.stats.stamina_max += 7 * custom.spent[5] as i32;
        for id in custom.deck {
            player
                .deck
                .try_push(u16::try_from(id).map_err(|_| "invalid_setup")?)
                .map_err(|_| "invalid_setup")?;
        }
    } else {
        if let Some(starter) = engine_content::STARTERS
            .iter()
            .find(|starter| starter.avatar == id)
        {
            player.deck = starter.deck.iter().copied().collect();
        } else {
            let rules = engine_content::rules().deck;
            for _ in 0..rules.max_copies {
                for card in content
                    .cards()
                    .iter()
                    .filter(|c| player.avatar.schools.meets(&c.requires))
                {
                    if player.deck.len() == usize::from(rules.size) {
                        break;
                    }
                    player.deck.push(card.id);
                }
            }
        }
    }
    let profiles = training_profiles();
    let profile = profiles
        .bots
        .iter()
        .find(|profile| profile.id == setup.profile_id)
        .ok_or("invalid_profile")?;
    let rotation = setup.rotation as usize % profile.avatar_pool.len();
    let opponent_avatar = u32::from(profile.avatar_pool[rotation]);
    let opponent = profile
        .player(level as u8, rotation)
        .ok_or("invalid_profile")?;
    let players = [player, opponent];
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
    protocol::duel::snapshot(&session.state, Side::A, session.opponent_avatar)
}
fn event(input: &Event) -> Option<wire::DuelEvent> {
    protocol::duel::event_for(input, Side::A)
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
            custom: None,
            content_version: engine_content::CONTENT_VERSION
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect(),
        }
    }
    #[test]
    fn every_nonstarter_class_can_start_local_training() {
        for avatar in engine_content::AVATARS
            .iter()
            .filter(|a| !engine_content::STARTERS.iter().any(|s| s.avatar == a.id))
        {
            let mut setup = setup();
            setup.avatar_id = u32::from(avatar.id);
            setup.autoplay = false;
            let reply = call(
                &mut Training::new(),
                1,
                wire::training_request::Body::Start(setup),
            );
            assert!(reply.error.is_empty(), "{}: {}", avatar.key, reply.error);
            assert!(!reply.view.unwrap().hand.is_empty());
        }
    }
    #[test]
    fn local_collection_deck_and_development_points_reach_the_engine() {
        let mut setup = setup();
        setup.autoplay = false;
        let ids = engine_content::STARTERS[0]
            .cards
            .iter()
            .take(5)
            .map(|(id, _)| u32::from(*id))
            .collect::<Vec<_>>();
        setup.custom = Some(wire::TrainingCustomPlayer {
            specialization: String::new(),
            gift_id: 0,
            level: 5,
            spent: vec![1, 0, 0, 0, 2, 0],
            deck: ids
                .iter()
                .flat_map(|id| std::iter::repeat_n(*id, 4))
                .collect(),
        });
        let reply = call(
            &mut Training::new(),
            1,
            wire::training_request::Body::Start(setup.clone()),
        );
        assert!(reply.error.is_empty());
        let view = reply.view.unwrap();
        let base = engine_content::avatar(1).unwrap().snapshot(5);
        assert_eq!(view.me.as_ref().unwrap().stats[0], base.stats.phys_atk + 1);
        assert_eq!(view.me.as_ref().unwrap().hp_start, base.stats.hp_max + 14);
        assert!(view.hand.iter().all(|id| ids.contains(id)));
        setup.custom.as_mut().unwrap().spent = vec![1];
        assert_eq!(
            call(
                &mut Training::new(),
                1,
                wire::training_request::Body::Start(setup)
            )
            .error,
            "invalid_setup"
        );
    }
    #[test]
    fn reincarnated_local_training_uses_its_path_specialization_and_free_gift() {
        let path = engine_content::PATHS
            .iter()
            .find(|p| p.base_avatar == 1)
            .unwrap();
        let mut setup = setup();
        setup.avatar_id = u32::from(path.avatar.id);
        setup.autoplay = false;
        setup.custom = Some(wire::TrainingCustomPlayer {
            level: 1,
            spent: vec![0; 6],
            deck: vec![],
            specialization: engine_content::specialization_key(path.specializations[0]).into(),
            gift_id: u32::from(path.gifts[0]),
        });
        let reply = call(
            &mut Training::new(),
            1,
            wire::training_request::Body::Start(setup.clone()),
        );
        assert!(reply.error.is_empty(), "{}", reply.error);
        assert!(
            reply
                .view
                .unwrap()
                .enchantments
                .iter()
                .any(|e| e.side == 0 && e.card_id == u32::from(path.gifts[0]))
        );
        setup.custom.as_mut().unwrap().gift_id = 1;
        assert_eq!(
            call(
                &mut Training::new(),
                1,
                wire::training_request::Body::Start(setup)
            )
            .error,
            "invalid_setup"
        );
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
