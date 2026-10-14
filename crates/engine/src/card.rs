//! Karty w pojedynku: koszty, wymagania szkół i słowa kluczowe (`docs/06-silnik-gry.md`, rozdz. 6.1).
//!
//! Efekty kart dochodzą w fazie F2. W F1 rzucenie karty pobiera koszty i odkłada ją na stos
//! odrzuconych.

use std::fmt;

use crate::types::Schools;

/// Identyfikator karty z treści gry (`content/cards`).
pub type CardId = u16;

/// Koszt rzucenia karty (R-ENT-07).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Cost {
    pub focus: u16,
    pub stamina: u16,
}

/// Słowa kluczowe karty (R-CZA-03…07). Pozostałe słowa kluczowe dochodzą w fazie F2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Keywords {
    /// Interwencja: najwyżej jedno rzucenie tej karty w pojedynku (R-CZA-03).
    pub intervention: bool,
}

/// Definicja karty potrzebna silnikowi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CardDef {
    pub id: CardId,
    /// Wymagane poziomy szkół; 0 oznacza brak wymagania (R-ENT-07).
    pub requires: Schools,
    pub cost: Cost,
    pub keywords: Keywords,
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
