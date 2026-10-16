//! Pliki dla klienta i `content_version` (`docs/03-tresc-gry.md`, rozdz. 8.4).

use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::Path;

use serde::Serialize;

use crate::compile::Compiled;
use crate::i18n::Lang;
use crate::model::{AvatarSrc, CardSrc, SchoolSrc, StarterSrc};

/// Canonical rules and source definitions; also the input of content_version.
#[derive(Serialize)]
struct ClientContent<'a> {
    rules: &'a toml::Table,
    schools: &'a [SchoolSrc],
    avatars: Vec<&'a AvatarSrc>,
    cards: Vec<&'a CardSrc>,
    starters: Vec<&'a StarterSrc>,
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
    // The worker already holds executable effects and growth curves in WASM. The UI only
    // needs initial profiles and numeric Fluent parameters, not a second copy of the engine.
    // Hash the complete canonical source above, before producing this presentation payload.
    let mut payload = serde_json::to_value(client_content(compiled)).expect("serializable content");
    for avatar in payload["avatars"].as_array_mut().expect("avatar list") {
        let fields = avatar.as_object_mut().expect("avatar");
        fields.remove("growth");
        fields.remove("dev_points");
    }
    // Paths remain a separate catalogue: they cannot become freely collectible templates.
    let paths: Vec<_> = payload["avatars"]
        .as_array()
        .expect("avatar list")
        .iter()
        .filter(|a| a.get("path").is_some())
        .cloned()
        .collect();
    payload["avatars"]
        .as_array_mut()
        .expect("avatar list")
        .retain(|a| a.get("path").is_none());
    payload
        .as_object_mut()
        .expect("content")
        .insert("paths".into(), paths.into());
    for card in payload["cards"].as_array_mut().expect("card list") {
        let fields = card.as_object_mut().expect("card");
        let mut params = serde_json::Map::new();
        for key in ["focus", "stamina"] {
            params.insert(key.into(), fields["cost"][key].clone());
        }
        if let Some(round) = fields.get("condition").and_then(|c| c.get("round")) {
            params.insert("round".into(), round.clone());
        }
        if let Some(level) = fields.get("coupling").and_then(|c| c.get("level")) {
            params.insert("level".into(), level.clone());
        }
        for (index, effect) in fields["effects"]
            .as_array()
            .expect("effects")
            .iter()
            .enumerate()
        {
            for (key, value) in effect.as_object().expect("effect") {
                if value.is_number() {
                    let suffix = if index == 0 {
                        String::new()
                    } else {
                        format!("_{}", index + 1)
                    };
                    params.insert(format!("{key}{suffix}"), value.clone());
                }
            }
        }
        fields.remove("effects");
        fields.remove("condition");
        fields.insert("params".into(), params.into());
    }
    payload
        .as_object_mut()
        .expect("content")
        .insert("version".into(), hex(&version(compiled)).into());
    let mut json = serde_json::to_string_pretty(&payload).expect("serializable content");
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
