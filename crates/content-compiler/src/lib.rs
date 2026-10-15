//! Kompilator treści: walidacja plików `content/`, generowanie `engine-content` i plików dla klienta
//! (`docs/03-tresc-gry.md`, rozdz. 8).

pub mod codegen;
pub mod compile;
pub mod curves;
pub mod i18n;
pub mod load;
pub mod model;
pub mod output;

use std::fmt;
use std::path::{Path, PathBuf};

pub use compile::{Compiled, compile};
pub use load::{Source, load};

/// Błąd treści z miejscem, którego dotyczy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// Plik względem katalogu treści.
    pub file: PathBuf,
    /// Klucz elementu (np. `card.bronze_edge`), jeśli błąd dotyczy jednego elementu.
    pub key: Option<String>,
    pub message: String,
}

impl Diagnostic {
    pub fn new(file: impl AsRef<Path>, key: Option<&str>, message: impl Into<String>) -> Self {
        Self {
            file: file.as_ref().to_path_buf(),
            key: key.map(ToOwned::to_owned),
            message: message.into(),
        }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Ukośniki zamiast `\`, żeby komunikaty były takie same w każdym systemie.
        let file = self.file.to_string_lossy().replace('\\', "/");
        match &self.key {
            Some(key) => write!(f, "{file} [{key}]: {}", self.message),
            None => write!(f, "{file}: {}", self.message),
        }
    }
}

/// Wczytuje i kompiluje treść z katalogu `dir`; zwraca wszystkie znalezione błędy naraz.
pub fn build(dir: &Path) -> Result<Compiled, Vec<Diagnostic>> {
    let source = load(dir)?;
    compile(&source)
}
