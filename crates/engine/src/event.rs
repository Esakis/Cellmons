//! Zdarzenia pojedynku: jedyny materiał dla animacji klienta i zapisu powtórek do wyświetlenia
//! (`docs/06-silnik-gry.md`, rozdz. 6.5).

use crate::card::CardId;
use crate::end::Outcome;
use crate::moves::Move;
use crate::types::{Kind, Side};

/// Zdarzenie pojedynku.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// Początek rundy; K obu stron (A, B) po przyroście (R-RUN-01).
    RoundStarted { round: u8, focus: [i32; 2] },
    /// Ujawnienie ruchów i K pobrana za zmiany ustawień (R-RUN-10), dla stron A i B.
    MovesRevealed {
        moves: [Move; 2],
        change_cost: [i32; 2],
    },
    /// Rzucenie czaru z pobraniem kosztów (R-RUN-12).
    SpellCast {
        side: Side,
        card: CardId,
        focus: i32,
        stamina: i32,
    },
    /// Czar przepadł bez kosztów, bo przy rozpatrzeniu nie dało się go opłacić (Z-SPW).
    SpellFizzled { side: Side, card: CardId },
    /// Odrzucenie karty bez kosztów (R-RUN-04, Z-DSC).
    CardDiscarded { side: Side, card: CardId },
    /// Trafienie (R-ATK-04); `stamina_cost` to faktyczny ubytek W atakującego.
    AttackHit {
        attacker: Side,
        slot: u8,
        kind: Kind,
        multiplier: i32,
        damage: i32,
        stamina_cost: i32,
    },
    /// Blok (R-ATK-02, R-ATK-05).
    AttackBlocked {
        attacker: Side,
        slot: u8,
        kind: Kind,
    },
    /// Dobranie karty (R-RUN-14); kartę widzi tylko właściciel.
    CardDrawn { side: Side, card: CardId },
    /// Koniec pojedynku.
    DuelEnded { outcome: Outcome },
}

/// Bufor zdarzeń używany wielokrotnie (`clear`), żeby rozstrzyganie rund nie alokowało pamięci.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventBuf {
    events: Vec<Event>,
}

impl EventBuf {
    #[must_use]
    pub fn new() -> Self {
        Self {
            events: Vec::with_capacity(32),
        }
    }

    pub fn clear(&mut self) {
        self.events.clear();
    }

    #[must_use]
    pub fn as_slice(&self) -> &[Event] {
        &self.events
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.events.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub(crate) fn push(&mut self, event: Event) {
        self.events.push(event);
    }

    pub(crate) fn truncate(&mut self, len: usize) {
        self.events.truncate(len);
    }
}

impl Default for EventBuf {
    fn default() -> Self {
        Self::new()
    }
}
