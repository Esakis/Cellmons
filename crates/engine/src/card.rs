//! Karty w pojedynku: koszty, wymagania szkół, słowa kluczowe i efekty
//! (`docs/06-silnik-gry.md`, rozdz. 6.1).

use std::fmt;

use crate::effect::{Effects, Trigger};
use crate::types::{School, Schools};

/// Identyfikator karty z treści gry (`content/cards`).
pub type CardId = u16;

/// Koszt rzucenia karty (R-ENT-07).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Cost {
    pub focus: u16,
    pub stamina: u16,
}

/// Słowa kluczowe karty (R-CZA-03…07).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Keywords {
    /// Interwencja: najwyżej jedno rzucenie tej karty w pojedynku (R-CZA-03).
    pub intervention: bool,
    /// Przeciwzaklęcie: anuluje czar przeciwnika z tej rundy (R-CZA-04, R-CZA-05).
    pub counter: bool,
    /// Sprzężenie ze szkołą (R-CZA-06).
    pub coupling: Option<Coupling>,
    /// Karta losowa: przy rzuceniu zastępuje ją losowa karta z puli (R-CZA-07).
    pub random: bool,
}

/// Sprzężenie ze szkołą `school` od poziomu `level` (R-CZA-06): przy poziomie postaci co najmniej
/// `level` karta kosztuje dodatkowo 1 K i 5 W za każdy poziom tej szkoły, a liczbowe wartości jej
/// efektów mnożą się przez ten poziom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Coupling {
    pub school: School,
    pub level: u8,
}

/// Warunek działania karty (R-CZA-08) albo wygranej warunkowej (R-KON-06), z punktu widzenia
/// strony, której karta działa.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Condition {
    /// Trwa runda co najmniej o tym numerze.
    RoundAtLeast(u8),
    /// Procent życia strony jest wyższy niż przeciwnika (porównanie jak w R-KON-03).
    HpPercentAhead,
    /// Przeciwnik nie ma W.
    OpponentExhausted,
}

/// Definicja karty potrzebna silnikowi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardDef {
    pub id: CardId,
    /// Wymagane poziomy szkół; 0 oznacza brak wymagania (R-ENT-07).
    pub requires: Schools,
    pub cost: Cost,
    pub kind: CardKind,
    pub keywords: Keywords,
    /// Warunek działania: gdy nie jest spełniony przy rozpatrzeniu, koszty są pobrane, a efektu
    /// nie ma (R-CZA-08, Z-CND).
    pub condition: Option<Condition>,
    /// Efekty wykonywane po kolei: przy rzuceniu czaru zwykłego (R-RUN-12) albo przy każdym
    /// wyzwoleniu umagicznienia (R-CZA-02).
    pub effects: Effects,
}

/// Typ karty (R-CZA-01, R-CZA-02).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardKind {
    /// Czar zwykły: efekty działają raz, przy rzuceniu.
    Instant,
    /// Umagicznienie: działa od rzucenia do końca pojedynku albo do rozproszenia. Efekty
    /// „póki działa” (`Duration::WhileActive`) działają ciągle, pozostałe przy każdym wyzwoleniu.
    Enchantment(Trigger),
}

/// Treść gry widziana przez silnik: wersja i karty posortowane ściśle rosnąco według `id`.
#[derive(Debug, Clone, Copy)]
pub struct Content<'a> {
    version: [u8; 32],
    cards: &'a [CardDef],
}

/// Niepoprawna treść gry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentError {
    /// Identyfikator powtórzony albo poza kolejnością rosnącą.
    UnsortedId(CardId),
    /// Wymagany poziom szkoły powyżej 4.
    InvalidRequirement(CardId),
}

impl fmt::Display for ContentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsortedId(id) => {
                write!(f, "karta {id}: identyfikatory muszą rosnąć bez powtórzeń")
            }
            Self::InvalidRequirement(id) => {
                write!(f, "karta {id}: wymagany poziom szkoły powyżej 4")
            }
        }
    }
}

impl std::error::Error for ContentError {}

impl<'a> Content<'a> {
    /// Sprawdza kolejność identyfikatorów i poziomy wymagań.
    pub fn new(version: [u8; 32], cards: &'a [CardDef]) -> Result<Self, ContentError> {
        if let Some(pair) = cards.windows(2).find(|pair| pair[0].id >= pair[1].id) {
            return Err(ContentError::UnsortedId(pair[1].id));
        }
        if let Some(card) = cards.iter().find(|card| !card.requires.is_valid()) {
            return Err(ContentError::InvalidRequirement(card.id));
        }
        Ok(Self { version, cards })
    }

    /// Wersja treści (`content_version`, `docs/03-tresc-gry.md`, rozdz. 8.4).
    #[must_use]
    pub const fn version(&self) -> &[u8; 32] {
        &self.version
    }

    /// Wszystkie karty gry (pula dla R-TAL-05).
    #[must_use]
    pub const fn cards(&self) -> &'a [CardDef] {
        self.cards
    }

    #[must_use]
    pub fn get(&self, id: CardId) -> Option<&'a CardDef> {
        let index = self.cards.binary_search_by_key(&id, |card| card.id).ok()?;
        self.cards.get(index)
    }
}
