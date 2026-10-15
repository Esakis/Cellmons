//! Rodziny efektów (`docs/03-tresc-gry.md`, rozdz. 5.3; krok F2.2 planu).

use super::{NO_CARD, play};
use crate::card::CardDef;
use crate::effect::{AttackFilter, Duration, Effect, HandAction, Pool, StatPair, Target};
use crate::end::{Outcome, Reason};
use crate::event::{Event, EventBuf};
use crate::moves::CardAction::Cast;
use crate::moves::{MoveError, validate_move};
use crate::testkit::{Scenario, content, mv, spell};
use crate::types::{Kind, School, Schools, Side, Stat, Stats};

// Ustawienia domyślne: A ataki FFF, obrony MMM; B ataki MMM, obrony FFF.
const A: Stats = crate::testkit::stats(6, 1, 3, 2, 100, 50);
const B: Stats = crate::testkit::stats(4, 2, 7, 1, 100, 45);

/// Ruchy: A rzuca pierwszą kartę z ręki, obaj bez zmian ustawień (wszystkie ataki blokowane).
fn cast_a() -> [crate::moves::Move; 2] {
    [mv("FFF", "MMM", Cast(0)), mv("MMM", "FFF", NO_CARD)]
}

fn hits(events: &EventBuf, side: Side) -> Vec<(i32, i32)> {
    events
        .as_slice()
        .iter()
        .filter_map(|event| match *event {
            Event::AttackHit {
                attacker,
                multiplier,
                damage,
                ..
            } if attacker == side => Some((multiplier, damage)),
            _ => None,
        })
        .collect()
}

fn missed(events: &EventBuf, side: Side) -> usize {
    events
        .as_slice()
        .iter()
        .filter(|event| matches!(event, Event::AttackMissed { attacker, .. } if *attacker == side))
        .count()
}

fn one(id: u16, effect: Effect) -> CardDef {
    spell(id, 1, 0, &[effect])
}

#[test]
fn stat_set_fixes_the_value_and_later_changes_add_to_it() {
    let cards = [
        one(
            1,
            Effect::StatSet {
                target: Target::Caster,
                stat: Stat::PhysAtk,
                value: 0,
            },
        ),
        one(
            2,
            Effect::StatMod {
                target: Target::Caster,
                stat: Stat::PhysAtk,
                amount: 2,
                duration: Duration::Duel,
            },
        ),
    ];
    let content = content(&cards);
    let mut state = Scenario::new(A, B).hand(Side::A, &[1, 2]).build();
    play(&mut state, &content, cast_a());
    assert_eq!(state.stats(Side::A).phys_atk, 0);
    play(&mut state, &content, cast_a());
    assert_eq!(state.stats(Side::A).phys_atk, 2);
}

#[test]
fn swap_stats_exchanges_physical_and_magical_values() {
    let cards = [one(
        1,
        Effect::SwapStats {
            target: Target::Caster,
            pair: StatPair::Both,
        },
    )];
    let content = content(&cards);
    let mut state = Scenario::new(A, B).hand(Side::A, &[1]).build();
    play(&mut state, &content, cast_a());
    let stats = state.stats(Side::A);
    assert_eq!(
        (stats.phys_atk, stats.mag_atk, stats.phys_def, stats.mag_def),
        (3, 6, 2, 1)
    );
}

#[test]
fn heal_does_not_go_above_max_hp() {
    let cards = [one(
        1,
        Effect::Heal {
            target: Target::Caster,
            amount: 20,
        },
    )];
    let content = content(&cards);
    let mut state = Scenario::new(A, B).hand(Side::A, &[1]).build();
    state.player_mut(Side::A).hp = 90;
    let (summary, events) = play(&mut state, &content, cast_a());
    // Z-ENG-3: 90 + 20 zatrzymuje się na Ż_max 100.
    assert_eq!(summary.totals[0].hp, 100);
    assert!(events.as_slice().contains(&Event::Healed {
        side: Side::A,
        amount: 10,
        source: 1
    }));
}

#[test]
fn spell_damage_ignores_defense_and_can_knock_out() {
    let cards = [one(
        1,
        Effect::Damage {
            target: Target::Opponent,
            amount: 100,
        },
    )];
    let content = content(&cards);
    let mut state = Scenario::new(A, B).hand(Side::A, &[1]).build();
    let (summary, _) = play(&mut state, &content, cast_a());
    assert_eq!(summary.totals[1].hp, 0);
    assert_eq!(
        summary.outcome,
        Some(Outcome::Win {
            winner: Side::A,
            reason: Reason::Knockout
        })
    );
}

#[test]
fn drain_moves_what_the_opponent_has_to_the_caster() {
    let cards = [
        one(
            1,
            Effect::Drain {
                pool: Pool::Focus,
                amount: 3,
            },
        ),
        one(
            2,
            Effect::Drain {
                pool: Pool::Hp,
                amount: 10,
            },
        ),
        one(
            3,
            Effect::Drain {
                pool: Pool::Stamina,
                amount: 100,
            },
        ),
    ];
    let content = content(&cards);

    let mut state = Scenario::new(A, B).hand(Side::A, &[1]).build();
    let (summary, _) = play(&mut state, &content, cast_a());
    assert_eq!(summary.totals.map(|t| t.focus), [4 - 1 + 3, 4 - 3]);

    let mut state = Scenario::new(A, B).hand(Side::A, &[2]).build();
    state.player_mut(Side::A).hp = 95;
    let (summary, _) = play(&mut state, &content, cast_a());
    // A leczy się o zadane 10, ale najwyżej do Ż_max (Z-ENG-3).
    assert_eq!(summary.totals.map(|t| t.hp), [100, 90]);

    let mut state = Scenario::new(A, B).hand(Side::A, &[3]).build();
    state.player_mut(Side::A).stamina = 10;
    let (summary, _) = play(&mut state, &content, cast_a());
    // B traci całe 45 W, A zyskuje najwyżej do W_max (Z-ENG-5). Bez W postać B nie blokuje,
    // więc trzy trafienia A kosztują go jeszcze 3 × 6 W.
    assert_eq!(summary.totals.map(|t| t.stamina), [50 - 18, 0]);
}

#[test]
fn force_hit_and_unblockable_beat_matching_defenses() {
    for effect in [
        Effect::ForceHit {
            target: Target::Caster,
            filter: AttackFilter::default(),
        },
        Effect::Unblockable {
            target: Target::Caster,
            filter: AttackFilter::default(),
        },
    ] {
        let cards = [one(1, effect)];
        let content = content(&cards);
        let mut state = Scenario::new(A, B).hand(Side::A, &[1]).build();
        let (_, events) = play(&mut state, &content, cast_a());
        // Obrony B to F F F, więc bez efektu wszystkie ataki A byłyby zablokowane.
        assert_eq!(hits(&events, Side::A), [(1, 4), (2, 10), (4, 22)]);
    }
}

#[test]
fn force_miss_can_cover_only_the_first_attack_of_a_kind() {
    let filter = AttackFilter {
        kind: Some(Kind::Mag),
        slot: None,
        first_only: true,
    };
    let cards = [spell(
        1,
        1,
        0,
        &[Effect::ForceMiss {
            target: Target::Opponent,
            filter,
        }],
    )];
    let content = content(&cards);
    let mut state = Scenario::new(A, B).hand(Side::A, &[1]).build();
    // Obrony A F F F: magiczne ataki B normalnie trafiają.
    let (_, events) = play(
        &mut state,
        &content,
        [mv("FFF", "FFF", Cast(0)), mv("MMM", "FFF", NO_CARD)],
    );
    assert_eq!(missed(&events, Side::B), 1);
    // Chybienie zeruje serię, więc kolejne trafienia mają mnożniki 1 i 2.
    assert_eq!(
        hits(&events, Side::B)
            .iter()
            .map(|hit| hit.0)
            .collect::<Vec<_>>(),
        [1, 2]
    );
}

#[test]
fn forced_miss_beats_forced_hit() {
    let cards = [
        one(
            1,
            Effect::ForceHit {
                target: Target::Caster,
                filter: AttackFilter::default(),
            },
        ),
        one(
            2,
            Effect::ForceMiss {
                target: Target::Opponent,
                filter: AttackFilter::default(),
            },
        ),
    ];
    let content = content(&cards);
    let mut state = Scenario::new(A, B)
        .hand(Side::A, &[1])
        .hand(Side::B, &[2])
        .build();
    let (_, events) = play(
        &mut state,
        &content,
        [mv("FFF", "MMM", Cast(0)), mv("MMM", "FFF", Cast(0))],
    );
    // Z-ENG-1: chybienie wymuszone przez obrońcę wygrywa z wymuszonym trafieniem.
    assert_eq!(missed(&events, Side::A), 3);
    assert!(hits(&events, Side::A).is_empty());
}

#[test]
fn swap_choices_exchanges_attacks_and_defenses_for_the_round() {
    let cards = [one(
        1,
        Effect::SwapChoices {
            target: Target::Opponent,
        },
    )];
    let content = content(&cards);
    let mut state = Scenario::new(A, B).hand(Side::A, &[1]).build();
    let (_, events) = play(&mut state, &content, cast_a());
    // B atakuje F F F i broni się M M M: ataki obu stron trafiają.
    assert_eq!(hits(&events, Side::A).len(), 3);
    assert_eq!(hits(&events, Side::B).len(), 3);
    assert_eq!(state.player(Side::B).attacks(), &[Kind::Phys; 3]);
}

#[test]
fn damage_taken_multiplier_scales_attack_and_spell_damage() {
    let double = |duration| Effect::DamageTakenMult {
        target: Target::Opponent,
        percent: 200,
        duration,
    };
    let cards = [
        one(1, double(Duration::Round)),
        spell(
            2,
            1,
            0,
            &[
                double(Duration::Duel),
                Effect::Damage {
                    target: Target::Opponent,
                    amount: 10,
                },
            ],
        ),
    ];
    let content = content(&cards);
    let mut state = Scenario::new(A, B).hand(Side::A, &[1]).build();
    let moves = [mv("FFF", "MMM", Cast(0)), mv("MMM", "MMM", NO_CARD)];
    let (_, events) = play(&mut state, &content, moves);
    assert_eq!(hits(&events, Side::A), [(1, 8), (2, 20), (4, 44)]);
    // Mnożnik „do końca rundy” wygasa.
    assert_eq!(state.damage_taken_pct(Side::B), 100);

    let mut state = Scenario::new(A, B).hand(Side::A, &[2]).build();
    let (summary, _) = play(&mut state, &content, cast_a());
    assert_eq!(summary.totals[1].hp, 100 - 20);
}

#[test]
fn school_bonus_unlocks_cards_and_stays_within_levels() {
    let cards = [
        one(
            1,
            Effect::SchoolMod {
                target: Target::Caster,
                school: School::Combat,
                amount: 2,
                duration: Duration::Duel,
            },
        ),
        CardDef {
            requires: Schools::NONE.with(School::Combat, 2),
            ..one(
                2,
                Effect::Focus {
                    target: Target::Caster,
                    amount: 0,
                },
            )
        },
        one(
            3,
            Effect::SchoolMod {
                target: Target::Caster,
                school: School::Combat,
                amount: 9,
                duration: Duration::Duel,
            },
        ),
    ];
    let content = content(&cards);
    let mut state = Scenario::new(A, B).hand(Side::A, &[2, 1, 3]).build();
    let cast_first = mv("FFF", "MMM", Cast(0));
    assert_eq!(
        validate_move(&state, &content, Side::A, &cast_first),
        Err(MoveError::RequirementsNotMet)
    );
    play(
        &mut state,
        &content,
        [mv("FFF", "MMM", Cast(1)), mv("MMM", "FFF", NO_CARD)],
    );
    assert!(validate_move(&state, &content, Side::A, &cast_first).is_ok());

    // Ręka A: [2, 3]. Poziom efektywny zatrzymuje się na 4.
    play(
        &mut state,
        &content,
        [mv("FFF", "MMM", Cast(1)), mv("MMM", "FFF", NO_CARD)],
    );
    assert_eq!(state.schools(Side::A).level(School::Combat), 4);
}

#[test]
fn hand_effects_discard_at_random_and_draw_up_to_the_hand_size() {
    let cards: Vec<CardDef> = (1..=7)
        .map(|id| match id {
            1 => one(
                1,
                Effect::Hand {
                    target: Target::Opponent,
                    action: HandAction::DiscardRandom,
                    count: 2,
                },
            ),
            2 => one(
                2,
                Effect::Hand {
                    target: Target::Caster,
                    action: HandAction::Draw,
                    count: 3,
                },
            ),
            _ => crate::testkit::noop_card(id, 0, 0),
        })
        .collect();
    let content = content(&cards);

    let scenario = Scenario::new(A, B)
        .hand(Side::A, &[1])
        .hand(Side::B, &[3, 4, 5, 6, 7]);
    let mut state = scenario.clone().build();
    let (_, events) = play(&mut state, &content, cast_a());
    let lost: Vec<u16> = events
        .as_slice()
        .iter()
        .filter_map(|event| match *event {
            Event::ForcedDiscard {
                side: Side::B,
                card,
                ..
            } => Some(card),
            _ => None,
        })
        .collect();
    assert_eq!(lost.len(), 2);
    assert_eq!(state.player(Side::B).hand().len(), 3);
    assert_eq!(state.player(Side::B).discard(), lost.as_slice());
    // To samo ziarno: te same karty.
    let mut again = scenario.build();
    play(&mut again, &content, cast_a());
    assert_eq!(again.player(Side::B).hand(), state.player(Side::B).hand());

    // Ręka A po rzuceniu: [3, 4, 5]; efekt dobiera tylko 2 karty do limitu 5, a zastępcza karta
    // na koniec rundy już się nie mieści.
    let mut state = Scenario::new(A, B)
        .hand(Side::A, &[2, 3, 4, 5])
        .deck(Side::A, &[6, 7, 1])
        .build();
    play(&mut state, &content, cast_a());
    assert_eq!(state.player(Side::A).hand(), [3, 4, 5, 1, 7]);
    assert_eq!(state.player(Side::A).deck(), [6]);
}
