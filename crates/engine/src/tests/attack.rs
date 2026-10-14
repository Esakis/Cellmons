//! Faza ataków (R-ATK-01…07).

use super::{NO_CARD, attack_log, idle, play};
use crate::end::{Outcome, Reason};
use crate::event::Event;
use crate::testkit::{Scenario, classic_rules, content, mv, stats};
use crate::types::{Side, Stats};

// Ustawienia domyślne: A ataki FFF, obrony MMM; B ataki MMM, obrony FFF.
const A: Stats = stats(6, 1, 3, 2, 100, 50);
const B: Stats = stats(4, 2, 7, 1, 100, 45);

/// Trafienia strony: (mnożnik, obrażenia, koszt W).
fn hits(events: &crate::event::EventBuf, side: Side) -> Vec<(i32, i32, i32)> {
    events
        .as_slice()
        .iter()
        .filter_map(|event| match *event {
            Event::AttackHit {
                attacker,
                multiplier,
                damage,
                stamina_cost,
                ..
            } if attacker == side => Some((multiplier, damage, stamina_cost)),
            _ => None,
        })
        .collect()
}

#[test]
fn r_atk_01_six_attacks_alternate_starting_with_initiative() {
    let content = content(&[]);
    for first in Side::BOTH {
        let mut state = Scenario::new(A, B).initiative(first).build();
        let moves = idle(&state);
        let (_, events) = play(&mut state, &content, moves);
        let order: Vec<(Side, u8)> = attack_log(&events)
            .into_iter()
            .map(|(side, slot, _)| (side, slot))
            .collect();
        let second = first.other();
        assert_eq!(
            order,
            [
                (first, 0),
                (second, 0),
                (first, 1),
                (second, 1),
                (first, 2),
                (second, 2)
            ]
        );
    }
}

#[test]
fn r_atk_02_defense_of_the_same_kind_blocks() {
    let content = content(&[]);
    let mut state = Scenario::new(A, B).build();
    let moves = idle(&state);
    let (_, events) = play(&mut state, &content, moves);
    assert!(
        attack_log(&events)
            .iter()
            .all(|(_, _, multiplier)| multiplier.is_none())
    );
    assert_eq!(state.player(Side::A).hp(), 100);
    assert_eq!(state.player(Side::B).hp(), 100);
}

#[test]
fn r_atk_02_defense_of_the_other_kind_does_not_block() {
    let content = content(&[]);
    let mut state = Scenario::new(A, B).build();
    let (_, events) = play(
        &mut state,
        &content,
        [mv("FFF", "MMM", NO_CARD), mv("MMM", "MMM", NO_CARD)],
    );
    assert_eq!(hits(&events, Side::A).len(), 3);
}

#[test]
fn r_atk_02_block_requires_the_minimum_stamina() {
    let content = content(&[]);
    let mut rules = classic_rules();
    rules.duel.block_min_stamina = 10;
    for (stamina, blocked) in [(9, false), (10, true)] {
        let mut state = Scenario::new(A, B).rules(rules.clone()).build();
        state.player_mut(Side::B).stamina = stamina;
        let moves = idle(&state);
        let (_, events) = play(&mut state, &content, moves);
        // Pierwszy atak A (F) trafia w obronę F postaci B.
        assert_eq!(
            attack_log(&events)[0],
            (Side::A, 0, if blocked { None } else { Some(1) })
        );
    }
}

#[test]
fn r_atk_03_defender_without_stamina_is_always_hit() {
    let content = content(&[]);
    let mut state = Scenario::new(A, B).build();
    state.player_mut(Side::B).stamina = 0;
    let moves = idle(&state);
    let (_, events) = play(&mut state, &content, moves);
    assert_eq!(hits(&events, Side::A).len(), 3);
}

#[test]
fn r_atk_04_hits_build_a_combo_and_deal_multiplied_damage() {
    let content = content(&[]);
    let mut state = Scenario::new(A, B).build();
    let (summary, events) = play(
        &mut state,
        &content,
        [mv("FFF", "MMM", NO_CARD), mv("MMM", "MMM", NO_CARD)],
    );
    // Obrażenia: AF 6 × mnożnik − OF 2; koszt W to AF bez mnożnika.
    assert_eq!(hits(&events, Side::A), [(1, 4, 6), (2, 10, 6), (4, 22, 6)]);
    assert_eq!(summary.totals[1].hp, 100 - 36);
    assert_eq!(summary.totals[0].stamina, 50 - 18);
}

#[test]
fn r_atk_04_zero_damage_hit_still_costs_stamina_and_counts() {
    let content = content(&[]);
    let weak = stats(1, 1, 0, 2, 100, 50);
    let armored = stats(4, 9, 7, 1, 100, 45);
    let mut state = Scenario::new(weak, armored).build();
    let (_, events) = play(
        &mut state,
        &content,
        [mv("FFF", "MMM", NO_CARD), mv("MMM", "MMM", NO_CARD)],
    );
    assert_eq!(hits(&events, Side::A), [(1, 0, 1), (2, 0, 1), (4, 0, 1)]);
}

#[test]
fn r_atk_04_combo_counts_hits_of_both_kinds() {
    let content = content(&[]);
    let mut state = Scenario::new(A, B).build();
    let (_, events) = play(
        &mut state,
        &content,
        [mv("FMF", "MMM", NO_CARD), mv("MMM", "MFM", NO_CARD)],
    );
    let multipliers: Vec<i32> = hits(&events, Side::A).iter().map(|hit| hit.0).collect();
    assert_eq!(multipliers, [1, 2, 4]);
}

#[test]
fn r_atk_05_block_resets_the_combo_and_costs_nothing() {
    let content = content(&[]);
    let mut state = Scenario::new(A, B).build();
    let (summary, events) = play(
        &mut state,
        &content,
        [mv("FFF", "MMM", NO_CARD), mv("MMM", "MFM", NO_CARD)],
    );
    let a_attacks: Vec<Option<i32>> = attack_log(&events)
        .into_iter()
        .filter(|(side, _, _)| *side == Side::A)
        .map(|(_, _, multiplier)| multiplier)
        .collect();
    assert_eq!(a_attacks, [Some(1), None, Some(1)]);
    assert_eq!(summary.totals[0].stamina, 50 - 6 - 6);
    // Ataki B są blokowane przez A, więc B nie traci W.
    assert_eq!(summary.totals[1].stamina, 45);
}

#[test]
fn r_atk_06_all_six_attacks_happen_even_after_hp_drops_to_zero() {
    let content = content(&[]);
    let fragile = stats(4, 2, 7, 1, 3, 45);
    let mut state = Scenario::new(A, fragile).build();
    let (summary, events) = play(
        &mut state,
        &content,
        [mv("FFF", "MMM", NO_CARD), mv("MMM", "MMM", NO_CARD)],
    );
    assert_eq!(attack_log(&events).len(), 6);
    assert!(summary.totals[1].hp < 0);
    let outcome = Outcome::Win {
        winner: Side::A,
        reason: Reason::Knockout,
    };
    assert_eq!(summary.outcome, Some(outcome));
    assert_eq!(
        events.as_slice().last(),
        Some(&Event::DuelEnded { outcome })
    );
}

#[test]
fn r_atk_07_stamina_stops_at_zero_while_hp_can_go_negative() {
    let content = content(&[]);
    let tired = stats(6, 1, 3, 2, 100, 10);
    let fragile = stats(4, 2, 7, 1, 20, 45);
    let mut state = Scenario::new(tired, fragile).build();
    let (summary, events) = play(
        &mut state,
        &content,
        [mv("FFF", "MMM", NO_CARD), mv("MMM", "MMM", NO_CARD)],
    );
    assert_eq!(hits(&events, Side::A), [(1, 4, 6), (2, 10, 4), (4, 22, 0)]);
    assert_eq!(summary.totals[0].stamina, 0);
    assert_eq!(summary.totals[1].hp, 20 - 36);
}
