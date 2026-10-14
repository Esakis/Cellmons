//! Parametry zasad z `content/rules.toml` (krok F1.1 planu).

use crate::rules::{RulesConfig, RulesError};
use crate::testkit::classic_rules;

const RULES_TOML: &str = include_str!("../../../../content/rules.toml");

fn parse(text: &str) -> Result<RulesConfig, toml::de::Error> {
    toml::from_str(text)
}

/// `content/rules.toml` z jedną linią zastąpioną przez `replacement`.
fn with_line(key: &str, replacement: &str) -> String {
    let mut found = false;
    let text = RULES_TOML
        .lines()
        .map(|line| {
            if line.starts_with(&format!("{key} =")) {
                found = true;
                replacement
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(found, "brak klucza {key} w content/rules.toml");
    text
}

fn invalid_key(key: &str, replacement: &str) -> &'static str {
    let rules = parse(&with_line(key, replacement)).expect("plik nadal jest poprawnym TOML");
    let RulesError { key, .. } = rules.validate().expect_err("wartość powinna być odrzucona");
    key
}

#[test]
fn content_rules_file_matches_classic_rules() {
    let rules = parse(RULES_TOML).expect("content/rules.toml jest poprawny");
    rules.validate().expect("parametry w zakresie");
    assert_eq!(rules, classic_rules());
}

#[test]
fn focus_cap_is_optional() {
    assert_eq!(classic_rules().duel.focus_cap, None);
    let rules = parse(&with_line("# focus_cap", "focus_cap = 12")).expect("poprawny TOML");
    assert_eq!(rules.duel.focus_cap, Some(12));
}

#[test]
fn sections_for_the_server_are_ignored() {
    let text = format!("{RULES_TOML}\n[rating]\ninitial = 1000\n");
    assert_eq!(parse(&text).expect("poprawny TOML"), classic_rules());
}

#[test]
fn unknown_and_missing_keys_are_rejected() {
    let typo = with_line("rounds", "rouns = 5");
    let error = parse(&typo).expect_err("literówka w kluczu").to_string();
    assert!(error.contains("rouns"), "{error}");

    let without_hand = with_line("hand_size", "");
    let error = parse(&without_hand).expect_err("brak klucza").to_string();
    assert!(error.contains("hand_size"), "{error}");
}

#[test]
fn wrong_types_and_variants_are_rejected() {
    assert!(parse(&with_line("rounds", "rounds = \"5\"")).is_err());
    assert!(
        parse(&with_line("rounds", "rounds = 300")).is_err(),
        "poza zakresem u8"
    );
    assert!(
        parse(&with_line(
            "combo_multipliers",
            "combo_multipliers = [1, 2]"
        ))
        .is_err()
    );
    assert!(
        parse(&with_line(
            "default_settings",
            "default_settings = \"random\""
        ))
        .is_err()
    );
}

#[test]
fn values_outside_engine_limits_are_rejected() {
    assert_eq!(invalid_key("rounds", "rounds = 0"), "duel.rounds");
    assert_eq!(invalid_key("rounds", "rounds = 10"), "duel.rounds");
    assert_eq!(
        invalid_key("attacks_per_round", "attacks_per_round = 4"),
        "duel.attacks_per_round"
    );
    assert_eq!(
        invalid_key("focus_per_round", "focus_per_round = -1"),
        "duel.focus_per_round"
    );
    assert_eq!(
        invalid_key("# focus_cap", "focus_cap = -1"),
        "duel.focus_cap"
    );
    assert_eq!(
        invalid_key("change_cost", "change_cost = -1"),
        "duel.change_cost"
    );
    assert_eq!(invalid_key("hand_size", "hand_size = 0"), "duel.hand_size");
    assert_eq!(invalid_key("hand_size", "hand_size = 9"), "duel.hand_size");
    assert_eq!(
        invalid_key("combo_multipliers", "combo_multipliers = [1, 0, 4]"),
        "duel.combo_multipliers"
    );
    assert_eq!(
        invalid_key("block_min_stamina", "block_min_stamina = -1"),
        "duel.block_min_stamina"
    );
    assert_eq!(
        invalid_key("round_timeout_s", "round_timeout_s = 0"),
        "duel.round_timeout_s"
    );
    assert_eq!(invalid_key("size", "size = 41"), "deck.size");
    assert_eq!(
        invalid_key("max_copies", "max_copies = 21"),
        "deck.max_copies"
    );
    assert_eq!(
        invalid_key("saved_presets_max", "saved_presets_max = 0"),
        "deck.saved_presets_max"
    );
}

#[test]
fn hand_cannot_be_larger_than_deck() {
    let rules = parse(&with_line("size", "size = 4")).expect("poprawny TOML");
    assert_eq!(
        rules.validate().map_err(|error| error.key),
        Err("duel.hand_size")
    );
}
