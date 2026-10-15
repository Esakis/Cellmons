//! Skrót stanu pojedynku (`docs/06-silnik-gry.md`, rozdz. 7).

use super::{idle, play};
use crate::hash::state_hash;
use crate::testkit::{Scenario, classic_rules, content, determinism_digest, stats};
use crate::types::{Side, Stats};

const A: Stats = stats(6, 1, 3, 2, 100, 50);
const B: Stats = stats(4, 2, 7, 1, 100, 45);

fn scenario() -> Scenario {
    Scenario::new(A, B)
        .hand(Side::A, &[1, 2])
        .deck(Side::A, &[3, 4])
        .deck(Side::B, &[5])
}

#[test]
fn equal_states_have_equal_hashes() {
    let state = scenario().build();
    assert_eq!(state_hash(&state), state_hash(&state.clone()));
    assert_eq!(state_hash(&state), state_hash(&scenario().build()));
}

#[test]
fn every_part_of_the_state_changes_the_hash() {
    let base = scenario().build();
    let hash = state_hash(&base);
    let changed = [
        scenario().initiative(Side::B).build(),
        scenario().deck(Side::A, &[4, 3]).build(),
        scenario().hand(Side::B, &[5]).build(),
        scenario()
            .rules({
                let mut rules = classic_rules();
                rules.duel.focus_cap = Some(10);
                rules
            })
            .build(),
    ];
    for state in &changed {
        assert_ne!(state_hash(state), hash);
    }

    let mut hp = base.clone();
    hp.player_mut(Side::B).hp -= 1;
    assert_ne!(state_hash(&hp), hash);

    let content = content(&[]);
    let mut next_round = base.clone();
    let moves = idle(&next_round);
    play(&mut next_round, &content, moves);
    assert_ne!(state_hash(&next_round), hash);
}

/// Zmiana tej wartości oznacza zmianę kodowania stanu albo zasad, a więc nowe skróty we wszystkich
/// zapisanych pojedynkach (`docs/06-silnik-gry.md`, rozdz. 9).
#[test]
fn state_hash_is_pinned() {
    assert_eq!(state_hash(&scenario().build()), 0xd488_f359_c9cf_2338);
}

#[test]
fn random_duels_are_deterministic() {
    assert_eq!(determinism_digest(0..50), determinism_digest(0..50));
    assert_ne!(determinism_digest(0..50), determinism_digest(50..100));
}
