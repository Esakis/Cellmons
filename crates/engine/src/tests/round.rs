//! Przebieg rundy (R-RUN-01…05, R-RUN-10…15).

use super::{NO_CARD, attack_log, idle, play};
use crate::card::CardId;
use crate::end::{Outcome, Reason};
use crate::event::Event;
use crate::moves::CardAction::{Cast, Discard};
use crate::testkit::{Scenario, classic_rules, content, mv, noop_card, stats};
use crate::types::{Side, Stats};

// Ustawienia domyślne: A ataki FFF, obrony MMM; B ataki MMM, obrony FFF. Przy nich wszystkie
// ataki są blokowane, więc rundy „bez zmian” nie zmieniają Ż ani W.
const A: Stats = stats(6, 1, 3, 2, 500, 500);
const B: Stats = stats(4, 2, 7, 1, 500, 500);

fn cards(ids: impl IntoIterator<Item = CardId>) -> Vec<crate::card::CardDef> {
    ids.into_iter().map(|id| noop_card(id, 1, 4)).collect()
}

#[test]
fn r_run_01_each_round_adds_focus() {
    let content = content(&[]);
    let mut state = Scenario::new(A, B).build();
    assert_eq!(state.player(Side::A).focus(), 4);

    let moves = idle(&state);
    let (summary, _) = play(&mut state, &content, moves);
    assert_eq!(summary.totals[0].focus, 4);
    assert_eq!(state.round(), 2);
    assert_eq!(state.player(Side::A).focus(), 8);
    assert_eq!(state.player(Side::B).focus(), 8);
}

#[test]
fn r_run_02_unspent_focus_carries_over_without_limit() {
    let content = content(&[]);
    let mut state = Scenario::new(A, B).build();
    for _ in 0..4 {
        let moves = idle(&state);
        play(&mut state, &content, moves);
    }
    assert_eq!(state.round(), 5);
    assert_eq!(state.player(Side::A).focus(), 20);
}

#[test]
fn r_run_02_focus_cap_limits_only_the_income() {
    let content = content(&[]);
    let mut rules = classic_rules();
    rules.duel.focus_cap = Some(10);
    let mut state = Scenario::new(A, B).rules(rules).build();
    for _ in 0..3 {
        let moves = idle(&state);
        play(&mut state, &content, moves);
    }
    assert_eq!(state.player(Side::A).focus(), 10);

    // K ponad limitem (w przyszłości z efektu karty) zostaje.
    state.player_mut(Side::A).focus = 15;
    let moves = idle(&state);
    play(&mut state, &content, moves);
    assert_eq!(state.player(Side::A).focus(), 15);
}

#[test]
fn r_run_03_combo_restarts_every_round() {
    let content = content(&[]);
    // Obrony A są fizyczne, więc magiczne ataki B zawsze trafiają.
    let mut state = Scenario::new(A, B).build();
    let a = mv("FFF", "FFF", NO_CARD);
    let b = mv("MMM", "FFF", NO_CARD);
    for _ in 0..2 {
        let (_, events) = play(&mut state, &content, [a, b]);
        let combo: Vec<Option<i32>> = attack_log(&events)
            .into_iter()
            .filter(|(side, _, _)| *side == Side::B)
            .map(|(_, _, multiplier)| multiplier)
            .collect();
        assert_eq!(combo, [Some(1), Some(2), Some(4)]);
    }
}

#[test]
fn r_run_04_move_sets_attacks_defenses_and_card_action() {
    let cards = cards([1, 2]);
    let content = content(&cards);
    let mut state = Scenario::new(A, B)
        .hand(Side::A, &[1])
        .hand(Side::B, &[2])
        .build();
    let a = mv("MFF", "FMM", Cast(0));
    let b = mv("FMF", "MFM", Discard(0));
    play(&mut state, &content, [a, b]);

    assert_eq!(state.player(Side::A).attacks(), &a.attacks);
    assert_eq!(state.player(Side::A).defenses(), &a.defenses);
    assert_eq!(state.player(Side::B).attacks(), &b.attacks);
    assert_eq!(state.player(Side::B).defenses(), &b.defenses);
    assert_eq!(state.player(Side::A).discard(), [1]);
    assert_eq!(state.player(Side::B).discard(), [2]);
}

#[test]
fn r_run_05_each_changed_slot_costs_focus() {
    let content = content(&[]);
    let mut state = Scenario::new(A, B).build();
    let b = mv("MMM", "FFF", NO_CARD);

    // Runda 1: dwie zmiany względem ustawień domyślnych FFF / MMM.
    let (summary, _) = play(&mut state, &content, [mv("FMF", "MFM", NO_CARD), b]);
    assert_eq!(summary.totals[0].focus, 4 - 2);
    // Runda 2: powrót do FFF / MMM to znowu dwie zmiany, liczone względem rundy 1.
    let (summary, _) = play(&mut state, &content, [mv("FFF", "MMM", NO_CARD), b]);
    assert_eq!(summary.totals[0].focus, 2 + 4 - 2);
    // Runda 3: bez zmian.
    let (summary, _) = play(&mut state, &content, [mv("FFF", "MMM", NO_CARD), b]);
    assert_eq!(summary.totals[0].focus, 4 + 4);
}

#[test]
fn r_run_05_change_cost_comes_from_the_rules() {
    let content = content(&[]);
    let mut rules = classic_rules();
    rules.duel.change_cost = 2;
    let mut state = Scenario::new(A, B).rules(rules).build();
    let (summary, _) = play(
        &mut state,
        &content,
        [mv("FMF", "MFM", NO_CARD), mv("MMM", "FFF", NO_CARD)],
    );
    assert_eq!(summary.totals[0].focus, 0);
}

#[test]
fn r_run_10_moves_are_revealed_and_change_cost_is_charged() {
    let content = content(&[]);
    let mut state = Scenario::new(A, B).build();
    let moves = [mv("FMF", "MFM", NO_CARD), mv("MMF", "FMF", NO_CARD)];
    let (_, events) = play(&mut state, &content, moves);
    assert_eq!(
        events.as_slice()[0],
        Event::MovesRevealed {
            moves,
            change_cost: [2, 2]
        }
    );
}

#[test]
fn r_run_11_spells_resolve_in_initiative_order() {
    let cards = cards([1, 2]);
    let content = content(&cards);
    for first in Side::BOTH {
        let mut state = Scenario::new(A, B)
            .initiative(first)
            .hand(Side::A, &[1])
            .hand(Side::B, &[2])
            .build();
        let moves = [mv("FFF", "MMM", Cast(0)), mv("MMM", "FFF", Cast(0))];
        let (_, events) = play(&mut state, &content, moves);
        let casters: Vec<Side> = events
            .as_slice()
            .iter()
            .filter_map(|event| match *event {
                Event::SpellCast { side, .. } => Some(side),
                _ => None,
            })
            .collect();
        assert_eq!(casters, [first, first.other()]);
    }
}

#[test]
fn r_run_12_casting_pays_costs_and_moves_the_card_to_the_discard_pile() {
    let cards = [noop_card(1, 2, 8)];
    let content = content(&cards);
    let mut state = Scenario::new(A, B).hand(Side::A, &[1]).build();
    let (summary, events) = play(
        &mut state,
        &content,
        [mv("FFF", "MMM", Cast(0)), mv("MMM", "FFF", NO_CARD)],
    );

    assert!(events.as_slice().contains(&Event::SpellCast {
        side: Side::A,
        card: 1,
        focus: 2,
        stamina: 8
    }));
    assert_eq!(summary.totals[0].focus, 4 - 2);
    assert_eq!(summary.totals[0].stamina, 500 - 8);
    assert!(state.player(Side::A).hand().is_empty());
    assert_eq!(state.player(Side::A).discard(), [1]);
}

#[test]
fn r_run_14_played_card_is_replaced_from_the_top_of_the_deck() {
    let cards = cards(1..=7);
    let content = content(&cards);
    let mut state = Scenario::new(A, B)
        .hand(Side::A, &[1, 2, 3, 4, 5])
        .deck(Side::A, &[6, 7])
        .hand(Side::B, &[1, 2, 3, 4, 5])
        .deck(Side::B, &[6, 7])
        .build();
    let (_, events) = play(
        &mut state,
        &content,
        [mv("FFF", "MMM", Discard(1)), mv("MMM", "FFF", NO_CARD)],
    );

    let a = state.player(Side::A);
    assert_eq!(a.hand(), [1, 7, 3, 4, 5]);
    assert_eq!(a.deck(), [6]);
    assert_eq!(a.discard(), [2]);
    assert!(events.as_slice().contains(&Event::CardDrawn {
        side: Side::A,
        card: 7
    }));
    // B nie zagrał karty, więc nie dobiera.
    assert_eq!(state.player(Side::B).hand(), [1, 2, 3, 4, 5]);
    assert_eq!(state.player(Side::B).deck(), [6, 7]);
}

#[test]
fn r_run_14_empty_deck_means_no_draw() {
    let cards = cards([1, 2]);
    let content = content(&cards);
    let mut state = Scenario::new(A, B).hand(Side::A, &[1, 2]).build();
    let (_, events) = play(
        &mut state,
        &content,
        [mv("FFF", "MMM", Cast(0)), mv("MMM", "FFF", NO_CARD)],
    );
    assert_eq!(state.player(Side::A).hand(), [2]);
    assert!(
        !events
            .as_slice()
            .iter()
            .any(|event| matches!(event, Event::CardDrawn { .. }))
    );
}

#[test]
fn r_run_15_end_conditions_are_checked_after_each_round() {
    let content = content(&[]);
    let mut rules = classic_rules();
    rules.duel.rounds = 2;
    let mut state = Scenario::new(A, B).rules(rules).build();

    let moves = idle(&state);
    let (summary, events) = play(&mut state, &content, moves);
    assert_eq!(summary.outcome, None);
    assert_eq!(
        events.as_slice().last(),
        Some(&Event::RoundStarted {
            round: 2,
            focus: [8, 8]
        })
    );

    let moves = idle(&state);
    let (summary, events) = play(&mut state, &content, moves);
    let draw = Outcome::Draw {
        reason: Reason::Rounds,
    };
    assert_eq!(summary.outcome, Some(draw));
    assert_eq!(
        events.as_slice().last(),
        Some(&Event::DuelEnded { outcome: draw })
    );
    assert_eq!(state.round(), 2);
    assert_eq!(state.outcome(), Some(draw));
}
