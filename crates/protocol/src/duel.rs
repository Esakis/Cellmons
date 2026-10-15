//! Perspective-aware, language-neutral projection shared by offline and online play.
use crate::game::v1 as wire;
use engine::{CardAction, DuelState, Event, Kind, Move, Outcome, Phase, Side};

pub fn decode_move(input: &wire::DuelMove) -> Result<Move, String> {
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
pub fn encode_move(mv: &Move) -> wire::DuelMove {
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
pub fn move_error(error: engine::MoveError) -> String {
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
pub fn preview(p: engine::Preview) -> wire::MovePreview {
    wire::MovePreview {
        changes: u32::from(p.cost.changes),
        focus_cost: p.cost.focus,
        stamina_cost: p.cost.stamina,
        focus_after: p.focus_after,
        max_attack_stamina: p.max_attack_stamina,
        error: p.validity.err().map(move_error).unwrap_or_default(),
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
pub fn outcome(result: Outcome, viewer: Side) -> wire::DuelOutcome {
    let (winner, reason) = match result {
        Outcome::Win { winner, reason } => (u32::from(winner != viewer) + 1, reason),
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

pub fn event_for(input: &Event, viewer: Side) -> Option<wire::DuelEvent> {
    use Event::*;
    if matches!(input, CardDrawn { side, .. } if *side != viewer) {
        return None;
    }
    let mut out = wire::DuelEvent::default();
    // The opponent's draws stay private. All other card IDs here are public.
    out.kind = match *input {
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
    if viewer == Side::B {
        match out.kind.as_str() {
            "round" | "end" => {}
            "reveal" => {
                out.moves.reverse();
                std::mem::swap(&mut out.old_value, &mut out.new_value);
            }
            _ => out.side = 1 - out.side,
        }
    }
    Some(out)
}
pub fn snapshot(state: &DuelState, viewer: Side, opponent_avatar: u32) -> wire::TrainingView {
    let seen = engine::view(state, viewer);
    wire::TrainingView {
        round: u32::from(seen.round),
        total_rounds: u32::from(seen.rules.duel.rounds),
        initiative: u32::from(seen.initiative != viewer),
        me: Some(side(&seen.me)),
        opponent: Some(side(&seen.opponent)),
        hand: seen.hand.iter().map(|id| u32::from(*id)).collect(),
        enchantments: seen
            .enchantments
            .iter()
            .map(|(owner, id)| wire::DuelEnchantment {
                side: u32::from(*owner != viewer),
                card_id: u32::from(*id),
            })
            .collect(),
        outcome: match seen.phase {
            Phase::Finished(result) => Some(outcome(result, viewer)),
            _ => None,
        },
        opponent_avatar_id: opponent_avatar,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn each_view_has_only_its_own_hand_and_draws_with_reversed_public_sides() {
        let player = |index: usize| {
            let starter = &engine_content::STARTERS[index];
            engine::PlayerSetup {
                avatar: engine_content::avatar(starter.avatar).unwrap().snapshot(1),
                deck: starter.deck.iter().copied().collect(),
            }
        };
        let state = engine::new_duel(
            &engine::DuelSetup {
                rules: engine_content::rules(),
                seed: [17; 32],
                players: [player(0), player(1)],
            },
            &engine_content::content(),
        )
        .unwrap();
        let a = snapshot(&state, Side::A, 2);
        let b = snapshot(&state, Side::B, 1);
        assert_eq!(a.me, b.opponent);
        assert_eq!(a.opponent, b.me);
        assert_eq!(
            a.hand,
            engine::view(&state, Side::A)
                .hand
                .iter()
                .copied()
                .map(u32::from)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            b.hand,
            engine::view(&state, Side::B)
                .hand
                .iter()
                .copied()
                .map(u32::from)
                .collect::<Vec<_>>()
        );
        assert_ne!(a.hand, b.hand);
        let draw = Event::CardDrawn {
            side: Side::B,
            card: 7,
        };
        assert!(event_for(&draw, Side::A).is_none());
        let own = event_for(&draw, Side::B).unwrap();
        assert_eq!((own.side, own.card_id), (0, 7));
        assert_eq!(
            outcome(
                Outcome::Win {
                    winner: Side::A,
                    reason: engine::Reason::Forfeit
                },
                Side::B
            )
            .winner,
            2
        );
    }
}
