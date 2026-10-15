//! Interakcje efektów wykryte w przeglądzie F4.

use crate::testkit::{Scenario, content, enchantment, spell, stats};
use crate::*;

fn cast(state: &mut DuelState, content: &Content<'_>, side: Side) -> EventBuf {
    let mut moves = Side::BOTH.map(|side| default_move(state, side));
    moves[side.index()].card = CardAction::Cast(0);
    let mut events = EventBuf::new();
    resolve_round(state, content, &moves, &mut events).unwrap();
    events
}

#[test]
fn setting_and_swapping_stats_account_for_active_bonuses_exactly_once() {
    let aura = enchantment(
        1,
        0,
        0,
        Trigger::OnRound,
        &[Effect::StatMod {
            target: Target::Caster,
            stat: Stat::PhysAtk,
            amount: 2,
            duration: Duration::WhileActive,
        }],
    );
    let cards = [
        aura.clone(),
        spell(
            2,
            0,
            0,
            &[Effect::SwapStats {
                target: Target::Caster,
                pair: StatPair::Attacks,
            }],
        ),
        spell(
            3,
            0,
            0,
            &[Effect::StatSet {
                target: Target::Caster,
                stat: Stat::PhysAtk,
                value: 0,
            }],
        ),
    ];
    let content = content(&cards);
    let mut state = Scenario::new(stats(6, 10, 3, 10, 100, 80), stats(0, 10, 0, 10, 100, 80))
        .gift(Side::A, &aura)
        .hand(Side::A, &[2, 2, 3])
        .build();
    assert_eq!(
        (state.stats(Side::A).phys_atk, state.stats(Side::A).mag_atk),
        (8, 3)
    );
    cast(&mut state, &content, Side::A);
    assert_eq!(
        (state.stats(Side::A).phys_atk, state.stats(Side::A).mag_atk),
        (3, 8)
    );
    cast(&mut state, &content, Side::A);
    assert_eq!(
        (state.stats(Side::A).phys_atk, state.stats(Side::A).mag_atk),
        (8, 3)
    );
    cast(&mut state, &content, Side::A);
    assert_eq!(state.stats(Side::A).phys_atk, 0);
}

#[test]
fn hash_covers_scaled_enchantment_effects_and_trigger() {
    let aura = enchantment(
        1,
        0,
        0,
        Trigger::OnRound,
        &[Effect::Heal {
            target: Target::Caster,
            amount: 2,
        }],
    );
    let cards = [aura.clone()];
    let content = content(&cards);
    let base = Scenario::new(stats(0, 0, 0, 0, 100, 80), stats(0, 0, 0, 0, 100, 80))
        .hp(Side::A, 50)
        .build();
    let (mut one, mut two) = (base.clone(), base);
    crate::enchant::register(&mut one, &mut EventBuf::new(), Side::A, &aura, 1);
    crate::enchant::register(&mut two, &mut EventBuf::new(), Side::A, &aura, 2);
    assert_ne!(state_hash(&one), state_hash(&two));
    let mut different_trigger = one.clone();
    different_trigger.enchantments[0].trigger = Trigger::OnHit;
    assert_ne!(state_hash(&one), state_hash(&different_trigger));
    for state in [&mut one, &mut two] {
        let moves = Side::BOTH.map(|side| default_move(state, side));
        resolve_round(state, &content, &moves, &mut EventBuf::new()).unwrap();
    }
    assert_eq!(
        (one.player(Side::A).hp(), two.player(Side::A).hp()),
        (52, 54)
    );
}

#[test]
fn stacked_enchantments_do_not_drop_a_later_forced_miss() {
    let hit = Effect::ForceHit {
        target: Target::Both,
        filter: AttackFilter::default(),
    };
    let aura = enchantment(1, 0, 0, Trigger::OnRound, &[hit; 4]);
    let cards = [
        aura.clone(),
        spell(
            2,
            0,
            0,
            &[Effect::ForceMiss {
                target: Target::Opponent,
                filter: AttackFilter::default(),
            }],
        ),
    ];
    let content = content(&cards);
    let mut state = Scenario::new(stats(8, 0, 0, 0, 1000, 1000), stats(0, 0, 0, 0, 1000, 1000))
        .gift(Side::A, &aura)
        .hand(Side::A, &[1; 4])
        .hand(Side::B, &[2])
        .build();
    for _ in 0..4 {
        cast(&mut state, &content, Side::A);
    }
    let events = cast(&mut state, &content, Side::B);
    assert_eq!(
        events
            .as_slice()
            .iter()
            .filter(|event| matches!(
                event,
                Event::AttackMissed {
                    attacker: Side::A,
                    ..
                }
            ))
            .count(),
        3
    );
}

#[test]
fn every_override_filter_fits_and_a_consumed_first_hit_can_be_rearmed() {
    let mut state = Scenario::new(stats(0, 0, 0, 0, 100, 80), stats(0, 0, 0, 0, 100, 80)).build();
    let mut events = EventBuf::new();
    for side in Side::BOTH {
        for hit in [true, false] {
            for kind in [None, Some(Kind::Phys), Some(Kind::Mag)] {
                for slot in [None, Some(0), Some(1), Some(2)] {
                    for first_only in [true, false] {
                        let filter = AttackFilter {
                            kind,
                            slot,
                            first_only,
                        };
                        let effect = if hit {
                            Effect::ForceHit {
                                target: Target::Caster,
                                filter,
                            }
                        } else {
                            Effect::ForceMiss {
                                target: Target::Caster,
                                filter,
                            }
                        };
                        for _ in 0..3 {
                            crate::effect::EffectCtx {
                                state: &mut state,
                                events: &mut events,
                                caster: side,
                                source: 1,
                                scale: 1,
                                triggers: false,
                            }
                            .apply(&effect);
                        }
                    }
                }
            }
        }
    }
    assert_eq!(state.overrides.len(), crate::state::MAX_OVERRIDES);
    assert_eq!(state.take_override(Side::A, 0, Kind::Phys), Some(false));
    state.overrides.clear();
    let first = Effect::ForceHit {
        target: Target::Caster,
        filter: AttackFilter {
            first_only: true,
            ..Default::default()
        },
    };
    crate::effect::EffectCtx {
        state: &mut state,
        events: &mut events,
        caster: Side::A,
        source: 1,
        scale: 1,
        triggers: false,
    }
    .apply(&first);
    assert_eq!(state.take_override(Side::A, 0, Kind::Phys), Some(true));
    assert_eq!(state.take_override(Side::A, 1, Kind::Phys), None);
    crate::effect::EffectCtx {
        state: &mut state,
        events: &mut events,
        caster: Side::A,
        source: 1,
        scale: 1,
        triggers: false,
    }
    .apply(&first);
    assert_eq!(state.take_override(Side::A, 2, Kind::Phys), Some(true));
}

#[test]
fn repeated_win_conditions_leave_room_for_a_later_winning_condition() {
    let mut state = Scenario::new(stats(0, 0, 0, 0, 100, 80), stats(0, 0, 0, 0, 100, 80))
        .hp(Side::A, 50)
        .stamina(Side::B, 0)
        .build();
    let mut events = EventBuf::new();
    for condition in
        std::iter::repeat_n(Condition::HpPercentAhead, 100).chain([Condition::OpponentExhausted])
    {
        crate::effect::EffectCtx {
            state: &mut state,
            events: &mut events,
            caster: Side::A,
            source: 1,
            scale: 1,
            triggers: false,
        }
        .apply(&Effect::ConditionalWin { condition });
    }
    assert_eq!(state.win_checks.len(), 2);
    assert_eq!(
        crate::end::conditional_win(&state),
        Some(Outcome::Win {
            winner: Side::A,
            reason: Reason::ConditionalWin
        })
    );
}
