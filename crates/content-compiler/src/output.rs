//! Pliki dla klienta i `content_version` (`docs/03-tresc-gry.md`, rozdz. 8.4).

use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::Path;

use serde::Serialize;

use crate::compile::Compiled;
use crate::i18n::Lang;
use crate::model::{AvatarSrc, CardSrc, SchoolSrc, StarterSrc};

/// Treść dla klienta: te same pola co w plikach TOML, bez pól tylko dla projektanta (`ref`).
#[derive(Serialize)]
struct ClientContent<'a> {
    rules: &'a toml::Table,
    schools: &'a [SchoolSrc],
    avatars: Vec<&'a AvatarSrc>,
    cards: Vec<&'a CardSrc>,
    starters: Vec<&'a StarterSrc>,
}

#[derive(Serialize)]
struct Versioned<'a> {
    version: String,
    #[serde(flatten)]
    content: &'a ClientContent<'a>,
}

fn client_content(compiled: &Compiled) -> ClientContent<'_> {
    ClientContent {
        rules: &compiled.rules_raw,
        schools: &compiled.schools,
        avatars: compiled.avatars.iter().map(|avatar| &avatar.src).collect(),
        cards: compiled.cards.iter().map(|card| &card.src).collect(),
        starters: compiled
            .starters
            .iter()
            .map(|starter| &starter.src)
            .collect(),
    }
}

/// `content_version`: BLAKE3 treści dla klienta i tekstów wszystkich języków. Każda zmiana treści,
/// także samego tekstu, daje nową wersję.
#[must_use]
pub fn version(compiled: &Compiled) -> [u8; 32] {
    let json = serde_json::to_vec(&client_content(compiled)).expect("treść da się zapisać w JSON");
    let mut hasher = blake3::Hasher::new_derive_key("Spellgambit content_version 2026-10-15");
    hasher.update(&(json.len() as u64).to_le_bytes());
    hasher.update(&json);
    for messages in &compiled.texts {
        hasher.update(&(messages.source.len() as u64).to_le_bytes());
        hasher.update(messages.source.as_bytes());
    }
    *hasher.finalize().as_bytes()
}

#[must_use]
pub fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut text, byte| {
            let _ = write!(text, "{byte:02x}");
            text
        })
}

/// `content.json` z wersją treści.
#[must_use]
pub fn content_json(compiled: &Compiled) -> String {
    let content = client_content(compiled);
    let versioned = Versioned {
        version: hex(&version(compiled)),
        content: &content,
    };
    let mut json = serde_json::to_string_pretty(&versioned).expect("treść da się zapisać w JSON");
    json.push('\n');
    json
}

/// Zapisuje pliki dla klienta: `content.json` i `i18n.<język>.ftl`.
pub fn write_client_files(compiled: &Compiled, dir: &Path) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    fs::write(dir.join("content.json"), content_json(compiled))?;
    for (lang, messages) in Lang::ALL.iter().zip(&compiled.texts) {
        fs::write(
            dir.join(format!("i18n.{}.ftl", lang.code())),
            &messages.source,
        )?;
    }
    Ok(())
}
