//! Strumienie losowości, losowanie z przedziału i tasowanie (`docs/06-silnik-gry.md`, rozdz. 7).

use std::num::NonZeroU32;

use rand_chacha::ChaCha8Rng;
use rand_chacha::rand_core::Rng;

use crate::rng::{Stream, below, shuffle, stream};

fn bound(value: u32) -> NonZeroU32 {
    NonZeroU32::new(value).expect("dodatnia granica")
}

fn first_values(mut rng: ChaCha8Rng) -> [u32; 4] {
    [
        rng.next_u32(),
        rng.next_u32(),
        rng.next_u32(),
        rng.next_u32(),
    ]
}

#[test]
fn streams_are_deterministic_and_independent() {
    let seed = [42; 32];
    let deck_a = first_values(stream(&seed, Stream::DeckA));
    assert_eq!(deck_a, first_values(stream(&seed, Stream::DeckA)));
    assert_ne!(deck_a, first_values(stream(&seed, Stream::DeckB)));
    assert_ne!(deck_a, first_values(stream(&seed, Stream::Initiative)));
    assert_ne!(deck_a, first_values(stream(&[43; 32], Stream::DeckA)));
}

/// Zmiana tej wartości oznacza inny przebieg wszystkich pojedynków, więc wymaga nowej głównej
/// wersji silnika (`docs/06-silnik-gry.md`, rozdz. 9).
#[test]
fn stream_derivation_is_pinned() {
    assert_eq!(
        stream(&[0; 32], Stream::DeckA).next_u64(),
        0x91cf_4066_105c_8fb5
    );
}

#[test]
fn below_stays_in_range_and_reaches_every_value() {
    let mut rng = stream(&[1; 32], Stream::DeckA);
    for n in 1..=12 {
        let mut seen = [false; 12];
        for _ in 0..500 {
            let value = below(&mut rng, bound(n));
            assert!(value < n);
            seen[value as usize] = true;
        }
        assert!(seen[..n as usize].iter().all(|hit| *hit), "n = {n}");
    }
}

#[test]
fn below_is_uniform() {
    let mut rng = stream(&[2; 32], Stream::DeckB);
    let mut counts = [0u32; 3];
    for _ in 0..30_000 {
        counts[below(&mut rng, bound(3)) as usize] += 1;
    }
    assert!(
        counts.iter().all(|count| (9_500..=10_500).contains(count)),
        "{counts:?}"
    );
}

#[test]
fn shuffle_permutes_items() {
    let mut rng = stream(&[3; 32], Stream::Initiative);
    let original: Vec<u16> = (0..40).collect();
    let mut items = original.clone();
    shuffle(&mut rng, &mut items);
    assert_ne!(items, original);
    items.sort_unstable();
    assert_eq!(items, original);

    let mut empty: [u16; 0] = [];
    shuffle(&mut rng, &mut empty);
    let mut single = [5u16];
    shuffle(&mut rng, &mut single);
    assert_eq!(single, [5]);
}
