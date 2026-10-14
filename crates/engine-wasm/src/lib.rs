//! Funkcje silnika eksportowane do przeglądarki (`docs/06-silnik-gry.md`, rozdz. 13).

use wasm_bindgen::prelude::wasm_bindgen;

/// Wersja silnika skompilowanego do WebAssembly.
#[wasm_bindgen]
pub fn engine_version() -> String {
    engine::VERSION.to_owned()
}
