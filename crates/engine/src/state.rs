//! Stan pojedynku (`docs/06-silnik-gry.md`, rozdz. 3).

use arrayvec::ArrayVec;

use crate::card::CardId;
use crate::end::Outcome;
use crate::rules::{MAX_DECK_SIZE, MAX_HAND_SIZE, MAX_ROUNDS, RulesConfig};
use crate::types::{Kind, SLOTS, Schools, Side, Stats};

/// Faza pojedynku.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Gracze wybierają ruchy na bieżącą rundę.
    AwaitingMoves,
    /// Pojedynek zakończony; stan już się nie zmienia.
    Finished(Outcome),
}

/// Stan jednego gracza.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerState {
    pub(crate) stats: Stats,
    pub(crate) schools: Schools,
    pub(crate) hp: i32,
    pub(crate) hp_start: i32,
    pub(crate) stamina: i32,
    pub(crate) focus: i32,
    pub(crate) attacks: [Kind; SLOTS],
    pub(crate) defenses: [Kind; SLOTS],
    pub(crate) hand: ArrayVec<CardId, MAX_HAND_SIZE>,
    pub(crate) deck: ArrayVec<CardId, MAX_DECK_SIZE>,
    pub(crate) discard: ArrayVec<CardId, MAX_DECK_SIZE>,
    pub(crate) used_interventions: ArrayVec<CardId, { MAX_ROUNDS as usize }>,
    pub(crate) combo: u8,
}

impl PlayerState {
    /// Statystyki efektywne (w F1 równe snapshotowi postaci).
    #[must_use]
    pub const fn stats(&self) -> &Stats {
        &self.stats
    }

    #[must_use]
    pub const fn schools(&self) -> &Schools {
        &self.schools
    }

    /// Punkty życia; mogą być ujemne (R-ATK-07).
    #[must_use]
    pub const fn hp(&self) -> i32 {
        self.hp
    }

    /// Ż w chwili startu (R-POJ-02), podstawa procentu życia (R-KON-03).
    #[must_use]
    pub const fn hp_start(&self) -> i32 {
        self.hp_start
    }

    #[must_use]
    pub const fn stamina(&self) -> i32 {
        self.stamina
    }

    #[must_use]
    pub const fn focus(&self) -> i32 {
        self.focus
    }

    /// Ataki z poprzedniej rundy, przed 1. rundą ustawienia domyślne (R-POJ-06).
    #[must_use]
    pub const fn attacks(&self) -> &[Kind; SLOTS] {
        &self.attacks
    }

    /// Obrony z poprzedniej rundy, przed 1. rundą ustawienia domyślne (R-POJ-06).
    #[must_use]
    pub const fn defenses(&self) -> &[Kind; SLOTS] {
        &self.defenses
    }

    /// Karty w ręce; indeksy jak w `CardAction`.
    #[must_use]
    pub fn hand(&self) -> &[CardId] {
        &self.hand
    }

    /// Pozostałe karty talii; wierzch na końcu.
    #[must_use]
    pub fn deck(&self) -> &[CardId] {
        &self.deck
    }

    /// Karty rzucone i odrzucone.
    #[must_use]
    pub fn discard(&self) -> &[CardId] {
        &self.discard
    }

    /// Interwencje rzucone w tym pojedynku (R-CZA-03).
    #[must_use]
    pub fn used_interventions(&self) -> &[CardId] {
        &self.used_interventions
    }

    /// Seria trafień w bieżącej rundzie (R-ATK-04).
    #[must_use]
    pub const fn combo(&self) -> u8 {
        self.combo
    }
}

/// Stan pojedynku.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuelState {
    pub(crate) rules: RulesConfig,
    pub(crate) round: u8,
    pub(crate) phase: Phase,
    pub(crate) initiative: Side,
    pub(crate) players: [PlayerState; 2],
}

impl DuelState {
    #[must_use]
    pub const fn rules(&self) -> &RulesConfig {
        &self.rules
    }

    /// Bieżąca runda, od 1; po zakończeniu pojedynku ostatnia rozegrana.
    #[must_use]
    pub const fn round(&self) -> u8 {
        self.round
    }

    #[must_use]
    pub const fn phase(&self) -> Phase {
        self.phase
    }

    #[must_use]
    pub const fn outcome(&self) -> Option<Outcome> {
        match self.phase {
            Phase::AwaitingMoves => None,
            Phase::Finished(outcome) => Some(outcome),
        }
    }

    #[must_use]
    pub const fn is_finished(&self) -> bool {
        matches!(self.phase, Phase::Finished(_))
    }

    /// Strona z inicjatywą (R-POJ-05).
    #[must_use]
    pub const fn initiative(&self) -> Side {
        self.initiative
    }

    /// Kolejność rozpatrywania czarów i ataków: najpierw strona z inicjatywą (R-POJ-05).
    #[must_use]
    pub const fn initiative_order(&self) -> [Side; 2] {
        [self.initiative, self.initiative.other()]
    }

    #[must_use]
    pub const fn player(&self, side: Side) -> &PlayerState {
        &self.players[side.index()]
    }

    pub(crate) fn player_mut(&mut self, side: Side) -> &mut PlayerState {
        &mut self.players[side.index()]
    }

    /// Strona i jej przeciwnik jednocześnie do zmiany.
    pub(crate) fn pair_mut(&mut self, side: Side) -> (&mut PlayerState, &mut PlayerState) {
        let [a, b] = &mut self.players;
        match side {
            Side::A => (a, b),
            Side::B => (b, a),
        }
    }
}
