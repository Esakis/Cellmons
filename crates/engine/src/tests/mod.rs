//! Testy silnika. Testy reguł zaczynają się od identyfikatora reguły z `docs/02-zasady-gry.md`
//! (np. `r_atk_02_…`), co sprawdza `tests/rule_coverage.rs`.

mod attack;
mod content;
mod effects;
mod enchantments;
mod end;
mod examples;
mod families;
mod hash;
mod invariants;
mod keywords;
mod moves;
mod replay;
mod rng;
mod round;
mod rules;
mod setup;
mod view;

use crate::card::Content;
use crate::event::{Event, EventBuf};
use crate::moves::{CardAction, Move, default_move};
use crate::round::{RoundSummary, resolve_round};
use crate::state::DuelState;
use crate::types::Side;

/// Ruch bez akcji karty (krótszy zapis `CardAction::None`, który nie przesłania `Option::None`).
const NO_CARD: CardAction = CardAction::None;

/// Rozgrywa rundę poprawnymi ruchami stron A i B.
fn play(
    state: &mut DuelState,
    content: &Content<'_>,
    moves: [Move; 2],
) -> (RoundSummary, EventBuf) {
    let mut events = EventBuf::new();
    let summary = resolve_round(state, content, &moves, &mut events).expect("poprawne ruchy");
    (summary, events)
}

/// Ruchy „bez zmian” obu stron (R-RUN-09).
fn idle(state: &DuelState) -> [Move; 2] {
    Side::BOTH.map(|side| default_move(state, side))
}

/// Ataki w kolejności: atakujący, slot i mnożnik trafienia (`None` przy bloku).
fn attack_log(events: &EventBuf) -> Vec<(Side, u8, Option<i32>)> {
    events
        .as_slice()
        .iter()
        .filter_map(|event| match *event {
            Event::AttackHit {
                attacker,
                slot,
                multiplier,
                ..
            } => Some((attacker, slot, Some(multiplier))),
            Event::AttackBlocked { attacker, slot, .. } => Some((attacker, slot, None)),
            _ => None,
        })
        .collect()
}
