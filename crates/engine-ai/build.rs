//! Compile training profiles once, keeping the TOML parser out of the browser binary.
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Profiles {
    bots: Vec<Profile>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Profile {
    id: String,
    name_pl: String,
    name_en: String,
    level: String,
    aggression: u8,
    spell_bias: u8,
    epsilon_percent: u8,
    temperature_milli: u16,
    deck_archetype: String,
    avatar_pool: Vec<u16>,
}
fn main() {
    let source = std::fs::read_to_string("../../content/bots.toml").expect("bots.toml");
    let profiles: Profiles = toml::from_str(&source).expect("valid bot profiles");
    let mut code = String::from("Profiles { bots: vec![");
    for p in profiles.bots {
        assert!(["L0", "L1", "L2", "L3"].contains(&p.level.as_str()));
        let archetype = match p.deck_archetype.as_str() {
            "balanced" => "Balanced",
            "burst" => "Burst",
            "endurance" => "Endurance",
            "enchantment" => "Enchantment",
            _ => panic!("invalid bot archetype"),
        };
        code.push_str(&format!("BotProfile {{ id: {:?}.into(), name_pl: {:?}.into(), name_en: {:?}.into(), level: crate::Level::{}, aggression: {}, spell_bias: {}, epsilon_percent: {}, temperature_milli: {}, deck_archetype: Archetype::{}, avatar_pool: vec!{:?} }},", p.id, p.name_pl, p.name_en, p.level, p.aggression, p.spell_bias, p.epsilon_percent, p.temperature_milli, archetype, p.avatar_pool));
    }
    code.push_str("] }");
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR"));
    std::fs::write(out.join("profiles.rs"), code).expect("write compiled profiles");
    println!("cargo:rerun-if-changed=../../content/bots.toml");
}
