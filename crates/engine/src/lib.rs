//! Silnik reguł gry: czysty, deterministyczny, bez wejścia/wyjścia (`docs/06-silnik-gry.md`).
//!
//! Funkcje reguł mają w komentarzach identyfikatory `R-…` z `docs/02-zasady-gry.md`, a nazwy testów
//! zaczynają się od identyfikatora reguły (np. `r_atk_02_…`). Test `tests/rule_coverage.rs`
//! sprawdza, że każda reguła z zakresu silnika ma test.

mod card;
mod effect;
mod enchant;
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
mod view;

#[cfg(any(test, feature = "testkit"))]
pub mod testkit;

#[cfg(test)]
mod tests;

pub use card::{
    CardDef, CardId, CardKind, Condition, Content, ContentError, Cost, Coupling, Keywords,
};
pub use effect::{
    AttackFilter, Duration, Effect, Effects, HandAction, MAX_EFFECTS, Pool, Resource, StatPair,
    Target, Trigger,
};
pub use enchant::MAX_ENCHANTMENTS;
pub use end::{Outcome, Reason, Termination, end_by, end_draw};
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
pub use types::{
    Kind, MAX_POOL, MAX_SCHOOL_LEVEL, MAX_STAT, SCHOOLS, SLOTS, School, Schools, Side,
    Specialization, Stat, Stats,
};

pub use rng::below as uniform_below;
pub use view::{PlayerView, SideView, view};

/// Wersja silnika, zapisywana w powtórkach (`docs/06-silnik-gry.md`, rozdz. 9).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
