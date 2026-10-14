//! Silnik reguł gry: czysty, deterministyczny, bez wejścia/wyjścia (`docs/06-silnik-gry.md`).
//!
//! Funkcje reguł mają w komentarzach identyfikatory `R-…` z `docs/02-zasady-gry.md`, a nazwy testów
//! zaczynają się od identyfikatora reguły (np. `r_atk_02_…`). Test `tests/rule_coverage.rs`
//! sprawdza, że każda reguła z zakresu silnika ma test.

mod card;
mod end;
mod event;
mod hash;
mod moves;
mod replay;
mod rng;
mod round;
mod rules;
mod setup;
mod state;
mod types;

#[cfg(any(test, feature = "testkit"))]
pub mod testkit;

#[cfg(test)]
mod tests;

pub use card::{CardDef, CardId, Content, ContentError, Cost, Keywords};
pub use end::{Outcome, Reason, Termination, end_by};
pub use event::{Event, EventBuf};
pub use hash::state_hash;
pub use moves::{
    CardAction, Move, MoveCost, MoveError, Preview, default_move, preview, validate_move,
};
pub use replay::{ENGINE_VERSION, ReplayError, ReplayRecord, Version, replay};
pub use round::{DuelError, PlayerTotals, RoundSummary, resolve_round};
pub use rules::{
    CounterResolution, DeckRules, DefaultSettings, DuelRules, MAX_DECK_SIZE, MAX_HAND_SIZE,
    MAX_ROUNDS, RulesConfig, RulesError,
};
pub use setup::{AvatarSnapshot, DuelSetup, PlayerSetup, SetupError, new_duel};
pub use state::{DuelState, Phase, PlayerState};
pub use types::{Kind, MAX_POOL, MAX_SCHOOL_LEVEL, MAX_STAT, SLOTS, School, Schools, Side, Stats};

/// Wersja silnika, zapisywana w powtórkach (`docs/06-silnik-gry.md`, rozdz. 9).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
