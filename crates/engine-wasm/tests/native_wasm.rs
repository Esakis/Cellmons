//! Natywne = WASM (`docs/06-silnik-gry.md`, rozdz. 7): losowe pojedynki dają ten sam skrót stanu
//! po każdej rundzie natywnie i w WebAssembly. Ten sam test działa w obu środowiskach:
//! `cargo test -p engine-wasm` oraz `cargo test -p engine-wasm --target wasm32-unknown-unknown`.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

/// Liczba pojedynków z kryterium kroku F1.6 planu (`docs/16-plan-wdrozenia.md`).
const DUELS: u64 = 10_000;

/// Skrót przebiegu pojedynków o ziarnach `0..DUELS`. Zmienia się razem z zasadami, kodowaniem stanu
/// albo losowaniem w `engine::testkit`; zmiana zasad wymaga też nowej wersji silnika
/// (`docs/06-silnik-gry.md`, rozdz. 9).
const EXPECTED: u64 = 0x8a89_2414_79e4_672e;

#[test]
fn random_duels_give_the_same_hashes_natively_and_in_wasm() {
    assert_eq!(engine::testkit::determinism_digest(0..DUELS), EXPECTED);
}
