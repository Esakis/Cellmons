//! Wczytywanie plików `content/` (`docs/03-tresc-gry.md`, rozdz. 8.1).

use std::fs;
use std::path::{Path, PathBuf};

use engine::RulesConfig;
use serde::de::DeserializeOwned;

use crate::Diagnostic;
use crate::i18n::{self, Lang, Messages};
use crate::model::{
    AvatarSrc, AvatarsFile, CardSrc, CardsFile, SchoolSrc, SchoolsFile, StarterSrc, StartersFile,
};

/// Element treści z plikiem, z którego pochodzi.
#[derive(Debug, Clone)]
pub struct Located<T> {
    /// Plik względem katalogu treści.
    pub file: PathBuf,
    pub item: T,
}

/// Cała treść wczytana z plików, przed walidacją.
#[derive(Debug, Clone)]
pub struct Source {
    pub rules: RulesConfig,
    /// `rules.toml` w całości, także sekcje dla serwera; trafia do klienta.
    pub rules_raw: toml::Table,
    pub schools: Vec<SchoolSrc>,
    pub avatars: Vec<Located<AvatarSrc>>,
    pub cards: Vec<Located<CardSrc>>,
    pub starters: Vec<StarterSrc>,
    /// Teksty w kolejności [`Lang::ALL`].
    pub texts: Vec<Messages>,
}

pub const RULES: &str = "rules.toml";
pub const SCHOOLS: &str = "schools.toml";
pub const AVATARS: &str = "avatars";
pub const CARDS: &str = "cards";
pub const STARTERS: &str = "starter_decks.toml";

/// Wczytuje treść z katalogu `dir`. Błędy składni zbiera ze wszystkich plików.
pub fn load(dir: &Path) -> Result<Source, Vec<Diagnostic>> {
    let mut errors = Vec::new();
    let rules =
        read_toml::<RulesConfig>(dir, Path::new(RULES), &mut errors).and_then(|rules| match rules
            .validate()
        {
            Ok(()) => Some(rules),
            Err(error) => {
                errors.push(Diagnostic::new(RULES, None, error.to_string()));
                None
            }
        });
    let rules_raw = read_toml::<toml::Table>(dir, Path::new(RULES), &mut Vec::new());
    let schools = read_toml::<SchoolsFile>(dir, Path::new(SCHOOLS), &mut errors);
    let starters = read_toml::<StartersFile>(dir, Path::new(STARTERS), &mut errors);

    let mut avatars = Vec::new();
    for file in toml_files(dir, Path::new(AVATARS), &mut errors) {
        if let Some(parsed) = read_toml::<AvatarsFile>(dir, &file, &mut errors) {
            avatars.extend(parsed.avatar.into_iter().map(|item| Located {
                file: file.clone(),
                item,
            }));
        }
    }
    let mut cards = Vec::new();
    for file in toml_files(dir, Path::new(CARDS), &mut errors) {
        if let Some(parsed) = read_toml::<CardsFile>(dir, &file, &mut errors) {
            cards.extend(parsed.card.into_iter().map(|item| Located {
                file: file.clone(),
                item,
            }));
        }
    }

    let mut texts = Vec::new();
    for lang in Lang::ALL {
        let file = lang.file();
        match fs::read_to_string(dir.join(&file)) {
            Ok(text) => match i18n::parse(&file, text) {
                Ok(messages) => texts.push(messages),
                Err(mut found) => errors.append(&mut found),
            },
            Err(error) => errors.push(Diagnostic::new(
                &file,
                None,
                format!("nie da się odczytać pliku: {error}"),
            )),
        }
    }

    match (rules, rules_raw, schools, starters) {
        (Some(rules), Some(rules_raw), Some(schools), Some(starters)) if errors.is_empty() => {
            Ok(Source {
                rules,
                rules_raw,
                schools: schools.school,
                avatars,
                cards,
                starters: starters.starter,
                texts,
            })
        }
        _ => Err(errors),
    }
}

fn read_toml<T: DeserializeOwned>(
    dir: &Path,
    file: &Path,
    errors: &mut Vec<Diagnostic>,
) -> Option<T> {
    let text = match fs::read_to_string(dir.join(file)) {
        Ok(text) => text,
        Err(error) => {
            errors.push(Diagnostic::new(
                file,
                None,
                format!("nie da się odczytać pliku: {error}"),
            ));
            return None;
        }
    };
    match toml::from_str(&text) {
        Ok(value) => Some(value),
        Err(error) => {
            let line = error
                .span()
                .map(|span| text[..span.start.min(text.len())].matches('\n').count() + 1);
            let message = error.message().trim_end().to_owned();
            let message = match line {
                Some(line) => format!("wiersz {line}: {message}"),
                None => message,
            };
            errors.push(Diagnostic::new(file, None, message));
            None
        }
    }
}

/// Pliki `*.toml` w katalogu `sub` i jego podkatalogach, posortowane według ścieżki.
fn toml_files(dir: &Path, sub: &Path, errors: &mut Vec<Diagnostic>) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![sub.to_path_buf()];
    while let Some(current) = pending.pop() {
        let entries = match fs::read_dir(dir.join(&current)) {
            Ok(entries) => entries,
            Err(error) => {
                errors.push(Diagnostic::new(
                    &current,
                    None,
                    format!("nie da się odczytać katalogu: {error}"),
                ));
                continue;
            }
        };
        for entry in entries.flatten() {
            let relative = current.join(entry.file_name());
            if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                pending.push(relative);
            } else if relative.extension().is_some_and(|ext| ext == "toml") {
                found.push(relative);
            }
        }
    }
    found.sort();
    found
}
