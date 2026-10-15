//! Zdarzenia pojedynku: jedyny materiał dla animacji klienta i zapisu powtórek do wyświetlenia
//! (`docs/06-silnik-gry.md`, rozdz. 6.5).

use crate::card::CardId;
use crate::effect::{AttackFilter, Resource};
use crate::end::Outcome;
use crate::moves::Move;
use crate::types::{Kind, School, Side, Stat};

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
    /// Czar anulowany przeciwzaklęciem: koszty pobrane, efektu nie ma (R-CZA-05).
    SpellCountered {
        side: Side,
        card: CardId,
        focus: i32,
        stamina: i32,
    },
    /// Karta losowa zastąpiona wylosowaną kartą `drawn` (R-CZA-07).
    RandomSpell {
        side: Side,
        card: CardId,
        drawn: CardId,
    },
    /// Warunek karty niespełniony: koszty pobrane, efektu nie ma (R-CZA-08, Z-CND).
    SpellFailed { side: Side, card: CardId },
    /// Czar przepadł bez kosztów, bo przy rozpatrzeniu nie dało się go opłacić (Z-SPW).
    SpellFizzled { side: Side, card: CardId },
    /// Odrzucenie karty bez kosztów (R-RUN-04, Z-DSC).
    CardDiscarded { side: Side, card: CardId },
    /// Zmiana statystyki efektywnej przez efekt karty `source`.
    StatChanged {
        side: Side,
        stat: Stat,
        old: i32,
        new: i32,
        source: CardId,
    },
    /// Zmiana W albo K przez efekt karty `source`.
    ResourceChanged {
        side: Side,
        resource: Resource,
        old: i32,
        new: i32,
        source: CardId,
    },
    /// Leczenie o `amount` (faktyczny przyrost Ż) przez efekt karty `source`.
    Healed {
        side: Side,
        amount: i32,
        source: CardId,
    },
    /// Obrażenia od czaru (po mnożniku) z karty `source`.
    Damaged {
        side: Side,
        amount: i32,
        source: CardId,
    },
    /// Zamiana ataków z obronami strony w tej rundzie.
    ChoicesSwapped { side: Side, source: CardId },
    /// Wymuszony wynik ataków strony `attacker` w tej rundzie: trafienie albo chybienie.
    AttackOverride {
        attacker: Side,
        hit: bool,
        filter: AttackFilter,
        source: CardId,
    },
    /// Nowy mnożnik otrzymywanych obrażeń strony, w procentach.
    DamageTakenChanged {
        side: Side,
        percent: u32,
        source: CardId,
    },
    /// Zmiana poziomu szkoły.
    SchoolChanged {
        side: Side,
        school: School,
        old: u8,
        new: u8,
        source: CardId,
    },
    /// Nowe aktywne umagicznienie strony (R-CZA-02).
    EnchantmentAdded { side: Side, card: CardId },
    /// Umagicznienie strony rozproszone przez kartę `source`.
    EnchantmentRemoved {
        side: Side,
        card: CardId,
        source: CardId,
    },
    /// Karta odrzucona z ręki przez efekt karty `source`.
    ForcedDiscard {
        side: Side,
        card: CardId,
        source: CardId,
    },
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
    /// Wymuszone chybienie (`force_miss`): bez obrażeń i bez kosztu, seria się zeruje.
    AttackMissed {
        attacker: Side,
        slot: u8,
        kind: Kind,
    },
    /// Dobranie karty (R-RUN-14 albo efekt karty); kartę widzi tylko właściciel.
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
