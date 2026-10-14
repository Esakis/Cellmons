//! Strumienie losowości pojedynku (`docs/06-silnik-gry.md`, rozdz. 7; ADR-007).
//!
//! Każdy strumień to ChaCha8 z ziarnem wyprowadzonym z ziarna pojedynku przez BLAKE3 w trybie
//! wyprowadzania klucza. Losowanie z przedziału i tasowanie są zaimplementowane tutaj, a nie wzięte
//! z biblioteki `rand`, żeby jej aktualizacja nie zmieniała przebiegu zapisanych pojedynków.

use std::num::NonZeroU32;

use rand_chacha::ChaCha8Rng;
use rand_chacha::rand_core::{Rng, SeedableRng};

use crate::types::Side;

/// Kontekst BLAKE3 strumieni. Jego zmiana zmienia przebieg wszystkich pojedynków.
const STREAM_CONTEXT: &str = "Spellgambit engine 2026-10-14 duel RNG stream";

/// Strumienie losowości pojedynku.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stream {
    /// Uzupełnienie (R-TAL-05) i tasowanie (R-POJ-03) talii gracza A.
    DeckA,
    /// Uzupełnienie i tasowanie talii gracza B.
    DeckB,
    /// Losowanie inicjatywy (R-POJ-05).
    Initiative,
}

impl Stream {
    pub(crate) const fn deck(side: Side) -> Stream {
        match side {
            Side::A => Stream::DeckA,
            Side::B => Stream::DeckB,
        }
    }

    const fn label(self) -> &'static [u8] {
        match self {
            Stream::DeckA => b"deck/A",
            Stream::DeckB => b"deck/B",
            Stream::Initiative => b"initiative",
        }
    }
}

/// Generator strumienia `stream` dla ziarna pojedynku.
pub(crate) fn stream(seed: &[u8; 32], stream: Stream) -> ChaCha8Rng {
    let mut hasher = blake3::Hasher::new_derive_key(STREAM_CONTEXT);
    hasher.update(seed);
    hasher.update(stream.label());
    ChaCha8Rng::from_seed(*hasher.finalize().as_bytes())
}

/// Liczba z przedziału `0..n` z rozkładu równomiernego (metoda Lemire'a, bez obciążenia).
pub(crate) fn below(rng: &mut impl Rng, n: NonZeroU32) -> u32 {
    let n = n.get();
    // 2^32 mod n: wyniki z młodszą połową iloczynu poniżej progu są odrzucane.
    let threshold = n.wrapping_neg() % n;
    loop {
        let product = u64::from(rng.next_u32()) * u64::from(n);
        if product as u32 >= threshold {
            return (product >> 32) as u32;
        }
    }
}

/// Tasowanie Fishera–Yatesa (wariant Durstenfelda).
pub(crate) fn shuffle<T>(rng: &mut impl Rng, items: &mut [T]) {
    for last in (1..items.len()).rev() {
        // Talie mają najwyżej kilkadziesiąt kart, więc przedział mieści się w u32.
        let Some(bound) = u32::try_from(last + 1).ok().and_then(NonZeroU32::new) else {
            return;
        };
        items.swap(last, below(rng, bound) as usize);
    }
}
