//! Efekty czarów (R-CZA-01, R-RUN-12, Z-SPW, Z-ENG-2, Z-ENG-5).

use super::{NO_CARD, attack_log, idle, play};
use crate::effect::{Duration, Effect, Resource, Target};
use crate::event::Event;
use crate::moves::CardAction::Cast;
use crate::testkit::{Scenario, content, mv, noop_card, spell, stats};
use crate::types::{Side, Stat, Stats};

// Ustawienia domyślne: A ataki FFF, obrony MMM; B ataki MMM, obrony FFF.
const A: Stats = stats(6, 1, 3, 2, 100, 50);
const B: Stats = stats(4, 2, 7, 1, 100, 45);

fn stat_mod(target: Target, stat: Stat, amount: i16, duration: Duration) -> Effect {
    Effect::StatMod {
        target,
        stat,
        amount,
        duration,
    }
}

#[test]
fn r_cza_01_instant_spell_changes_stats_for_the_rest_of_the_duel() {
    let cards = [spell(
        1,
        1,
        4,
        &[stat_mod(Target::Caster, Stat::PhysAtk, 2, Duration::Duel)],
    )];
    let content = content(&cards);
    let mut state = Scenario::new(A, B).hand(Side::A, &[1]).build();
    // B broni się M M F: dwa pierwsze ataki F postaci A trafiają.
    let (summary, events) = play(
        &mut state,
        &content,
        [mv("FFF", "MMM", Cast(0)), mv("MMM", "MMF", NO_CARD)],
    );

    assert!(events.as_slice().contains(&Event::StatChanged {
        side: Side::A,
        stat: Stat::PhysAtk,
        old: 6,
        new: 8,
        source: 1
    }));
    // AF 8 zamiast 6: 8 − 2 = 6 i 16 − 2 = 14 obrażeń.
    assert_eq!(summary.totals[1].hp, 100 - 6 - 14);
    let moves = idle(&state);
    play(&mut state, &content, moves);
    assert_eq!(state.stats(Side::A).phys_atk, 8);
    assert_eq!(state.player(Side::A).base_stats().phys_atk, 6);
}

#[test]
fn change_until_the_end_of_the_round_expires() {
    let cards = [spell(
        1,
        1,
        4,
        &[stat_mod(
            Target::Opponent,
            Stat::PhysDef,
            -2,
            Duration::Round,
        )],
    )];
    let content = content(&cards);
    let mut state = Scenario::new(A, B).hand(Side::A, &[1]).build();
    let (summary, _) = play(
        &mut state,
        &content,
        [mv("FFF", "MMM", Cast(0)), mv("MMM", "MFF", NO_CARD)],
    );
    // OF postaci B spada w tej rundzie do 0, więc pierwsze trafienie zadaje pełne 6.
    assert_eq!(summary.totals[1].hp, 100 - 6);
    assert_eq!(state.stats(Side::B).phys_def, 2);
}

#[test]
fn r_run_12_spell_that_can_no_longer_be_paid_fizzles() {
    // Karta B zabiera A 2 K, a czar A kosztuje 3 K; karta B zabiera A 20 W, a czar A kosztuje 40 W.
    let cards = [
        spell(
            1,
            1,
            0,
            &[Effect::Focus {
                target: Target::Opponent,
                amount: -2,
            }],
        ),
        noop_card(2, 3, 0),
        spell(
            3,
            1,
            0,
            &[Effect::Stamina {
                target: Target::Opponent,
                amount: -20,
            }],
        ),
        noop_card(4, 0, 40),
    ];
    let content = content(&cards);
    for (b_card, a_card, focus_after, stamina_after) in [(1, 2, 2, 50), (3, 4, 4, 30)] {
        // Gdy B ma inicjatywę, jego czar działa pierwszy i czar A przepada bez kosztów (Z-SPW).
        let mut state = Scenario::new(A, B)
            .initiative(Side::B)
            .hand(Side::A, &[a_card])
            .hand(Side::B, &[b_card])
            .build();
        let moves = [mv("FFF", "MMM", Cast(0)), mv("MMM", "FFF", Cast(0))];
        let (summary, events) = play(&mut state, &content, moves);
        assert!(events.as_slice().contains(&Event::SpellFizzled {
            side: Side::A,
            card: a_card
        }));
        assert_eq!(summary.totals[0].focus, focus_after);
        assert_eq!(summary.totals[0].stamina, stamina_after);
        assert_eq!(state.player(Side::A).discard(), [a_card]);

        // Gdy inicjatywę ma A, jego czar zostaje opłacony przed czarem B.
        let mut state = Scenario::new(A, B)
            .hand(Side::A, &[a_card])
            .hand(Side::B, &[b_card])
            .build();
        let (_, events) = play(&mut state, &content, moves);
        assert!(events.as_slice().iter().any(
            |event| matches!(event, Event::SpellCast { side: Side::A, card, .. } if *card == a_card)
        ));
    }
}

#[test]
fn stats_do_not_drop_below_their_floor() {
    let cards = [spell(
        1,
        1,
        4,
        &[
            stat_mod(Target::Opponent, Stat::PhysDef, -10, Duration::Duel),
            stat_mod(Target::Opponent, Stat::HpMax, -500, Duration::Duel),
        ],
    )];
    let content = content(&cards);
    let mut state = Scenario::new(A, B).hand(Side::A, &[1]).build();
    play(
        &mut state,
        &content,
        [mv("FFF", "MMM", Cast(0)), mv("MMM", "FFF", NO_CARD)],
    );
    // Z-ENG-2: obrona nie spada poniżej 0, a Ż_max poniżej 1.
    assert_eq!(state.stats(Side::B).phys_def, 0);
    assert_eq!(state.stats(Side::B).hp_max, 1);
}

#[test]
fn stamina_and_focus_changes_stay_within_bounds() {
    let cards = [spell(
        1,
        1,
        4,
        &[
            Effect::Stamina {
                target: Target::Caster,
                amount: 20,
            },
            Effect::Focus {
                target: Target::Opponent,
                amount: -10,
            },
        ],
    )];
    let content = content(&cards);
    let mut state = Scenario::new(A, B).hand(Side::A, &[1]).build();
    let (summary, events) = play(
        &mut state,
        &content,
        [mv("FFF", "MMM", Cast(0)), mv("MMM", "FFF", NO_CARD)],
    );
    // Z-ENG-5: przywracanie W nie przekracza W_max (50 − 4 + 20 → 50); K nie spada poniżej 0.
    assert_eq!(summary.totals[0].stamina, 50);
    assert_eq!(summary.totals[1].focus, 0);
    assert!(events.as_slice().contains(&Event::ResourceChanged {
        side: Side::B,
        resource: Resource::Focus,
        old: 4,
        new: 0,
        source: 1
    }));
}

#[test]
fn effects_run_in_order_and_both_target_starts_with_the_caster() {
    let cards = [spell(
        1,
        1,
        4,
        &[
            stat_mod(Target::Both, Stat::MagDef, -1, Duration::Duel),
            Effect::Focus {
                target: Target::Caster,
                amount: 2,
            },
        ],
    )];
    let content = content(&cards);
    let mut state = Scenario::new(A, B)
        .initiative(Side::B)
        .hand(Side::A, &[1])
        .build();
    let (_, events) = play(
        &mut state,
        &content,
        [mv("FFF", "MMM", Cast(0)), mv("MMM", "FFF", NO_CARD)],
    );
    let changed: Vec<(Side, &str)> = events
        .as_slice()
        .iter()
        .filter_map(|event| match *event {
            Event::StatChanged { side, .. } => Some((side, "stat")),
            Event::ResourceChanged { side, .. } => Some((side, "resource")),
            _ => None,
        })
        .collect();
    assert_eq!(
        changed,
        [(Side::A, "stat"), (Side::B, "stat"), (Side::A, "resource")]
    );
    assert_eq!(state.stats(Side::A).mag_def, 1);
    assert_eq!(state.stats(Side::B).mag_def, 0);
    // Efekty działają przed fazą ataków, więc ataki wciąż się wykonują.
    assert_eq!(attack_log(&events).len(), 6);
}
