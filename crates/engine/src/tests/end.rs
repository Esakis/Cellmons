//! Koniec pojedynku (R-KON-01…05, walkower z R-RUN-09).

use super::{NO_CARD, idle, play};
use crate::end::{Outcome, Reason, Termination, compare_hp_percent, end_by};
use crate::event::{Event, EventBuf};
use crate::round::{DuelError, resolve_round};
use crate::testkit::{Scenario, classic_rules, content, mv, stats};
use crate::types::{Side, Stats};

// Ustawienia domyślne: A ataki FFF, obrony MMM; B ataki MMM, obrony FFF.
const A: Stats = stats(6, 1, 3, 2, 100, 50);
const B: Stats = stats(4, 2, 7, 1, 100, 45);

fn win(winner: Side, reason: Reason) -> Option<Outcome> {
    Some(Outcome::Win { winner, reason })
}

#[test]
fn r_kon_01_player_left_with_zero_hp_loses() {
    let content = content(&[]);
    let mut state = Scenario::new(A, stats(4, 2, 7, 1, 4, 45)).build();
    // Tylko pierwsza obrona B jest magiczna: jedno trafienie A za 6 − 2 = 4 obrażenia.
    let (summary, _) = play(
        &mut state,
        &content,
        [mv("FFF", "MMM", NO_CARD), mv("MMM", "MFF", NO_CARD)],
    );
    assert_eq!(summary.totals[1].hp, 0);
    assert_eq!(summary.outcome, win(Side::A, Reason::Knockout));
    assert_eq!(state.outcome(), summary.outcome);
}

#[test]
fn r_kon_01_both_knocked_out_by_the_end_of_the_round_is_a_draw() {
    let content = content(&[]);
    let mut state = Scenario::new(stats(6, 1, 3, 2, 8, 50), stats(4, 2, 7, 1, 4, 45)).build();
    // A1 sprowadza B do 0 Ż, ale nokaut liczy się dopiero na końcu rundy, a do tego czasu
    // magiczne ataki B pokonują fizyczne obrony A.
    let (summary, _) = play(
        &mut state,
        &content,
        [mv("FFF", "FFF", NO_CARD), mv("MMM", "MFF", NO_CARD)],
    );
    assert!(summary.totals.iter().all(|totals| totals.hp <= 0));
    assert_eq!(
        summary.outcome,
        Some(Outcome::Draw {
            reason: Reason::Knockout
        })
    );
}

#[test]
fn r_kon_02_after_the_last_round_the_higher_hp_percent_wins() {
    let content = content(&[]);
    let mut rules = classic_rules();
    rules.duel.rounds = 1;
    let mut state = Scenario::new(A, B).rules(rules).build();
    let (summary, _) = play(
        &mut state,
        &content,
        [mv("FFF", "MMM", NO_CARD), mv("MMM", "MFF", NO_CARD)],
    );
    assert_eq!(summary.totals.map(|totals| totals.hp), [100, 96]);
    assert_eq!(summary.outcome, win(Side::A, Reason::Rounds));
}

#[test]
fn r_kon_03_hp_percent_is_compared_exactly() {
    let draw = Outcome::Draw {
        reason: Reason::Rounds,
    };
    let a_wins = Outcome::Win {
        winner: Side::A,
        reason: Reason::Rounds,
    };
    let b_wins = Outcome::Win {
        winner: Side::B,
        reason: Reason::Rounds,
    };

    // 50% przeciw 60%: decyduje procent, nie liczba punktów.
    assert_eq!(compare_hp_percent([50, 30], [100, 50]), b_wins);
    assert_eq!(compare_hp_percent([50, 25], [100, 50]), draw);
    // 66,67% przeciw 66,7%: po zaokrągleniu do pełnych procent byłby remis.
    assert_eq!(compare_hp_percent([2, 667], [3, 1000]), b_wins);
    // Ujemne Ż liczy się jako 0.
    assert_eq!(compare_hp_percent([-5, 0], [100, 100]), draw);
    assert_eq!(compare_hp_percent([-30, 1], [100, 100]), b_wins);
    // Procent może przekroczyć 100%.
    assert_eq!(compare_hp_percent([120, 100], [100, 100]), a_wins);
}

#[test]
fn r_kon_04_forfeit_is_a_loss() {
    let mut state = Scenario::new(A, B).build();
    let mut events = EventBuf::new();
    let outcome = end_by(&mut state, Side::A, Termination::Forfeit, &mut events);
    assert_eq!(outcome.ok(), win(Side::B, Reason::Forfeit));
    assert_eq!(state.outcome(), outcome.ok());
    assert_eq!(
        events.as_slice(),
        [Event::DuelEnded {
            outcome: outcome.expect("koniec")
        }]
    );
}

#[test]
fn r_kon_05_disconnect_without_return_is_a_loss() {
    let content = content(&[]);
    let mut state = Scenario::new(A, B).build();
    let moves = idle(&state);
    play(&mut state, &content, moves);
    let outcome = end_by(
        &mut state,
        Side::B,
        Termination::Disconnect,
        &mut EventBuf::new(),
    );
    assert_eq!(outcome.ok(), win(Side::A, Reason::Disconnect));
    assert_eq!(state.round(), 2);
}

#[test]
fn r_run_09_walkover_after_missed_moves_is_a_loss() {
    let mut state = Scenario::new(A, B).build();
    let outcome = end_by(
        &mut state,
        Side::A,
        Termination::Timeout,
        &mut EventBuf::new(),
    );
    assert_eq!(outcome.ok(), win(Side::B, Reason::Timeout));
}

#[test]
fn finished_duel_does_not_change_any_more() {
    let content = content(&[]);
    let mut state = Scenario::new(A, B).build();
    end_by(
        &mut state,
        Side::A,
        Termination::Forfeit,
        &mut EventBuf::new(),
    )
    .expect("pojedynek trwa");
    let finished = state.clone();
    let mut events = EventBuf::new();

    let moves = idle(&state);
    assert_eq!(
        resolve_round(&mut state, &content, &moves, &mut events),
        Err(DuelError::Finished)
    );
    assert_eq!(
        end_by(&mut state, Side::B, Termination::Disconnect, &mut events),
        Err(DuelError::Finished)
    );
    assert_eq!(state, finished);
    assert!(events.is_empty());
}
