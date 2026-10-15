//! Walidacja treści (`docs/03-tresc-gry.md`, rozdz. 8.3): poprawny zestaw plików przechodzi,
//! a każdy celowo zepsuty plik daje czytelny błąd z plikiem i kluczem.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use content_compiler::output::{content_json, version};
use content_compiler::{build, codegen};

const RULES: &str = include_str!("../../../content/rules.toml");

const SCHOOLS: &str = r##"
[[school]]
key = "combat"
color = "#b03a2e"
color_dark = "#e06656"

[[school]]
key = "divine"
color = "#c9a227"
color_dark = "#e8c95a"

[[school]]
key = "element"
color = "#2e7d32"
color_dark = "#66bb6a"

[[school]]
key = "arcane"
color = "#3949ab"
color_dark = "#7986cb"

[[school]]
key = "necro"
color = "#6a1b9a"
color_dark = "#ab47bc"

[[school]]
key = "tech"
color = "#00838f"
color_dark = "#4dd0e1"
"##;

const CARDS: &str = r#"
[[card]]
id = 1
key = "card.edge"
set = "core"
rarity = "bronze"
requires = { combat = 1 }
cost = { focus = 2, stamina = 4 }
kind = "instant"
effects = [{ family = "stat_mod", target = "self", stat = "phys_atk", amount = 2 }]

[[card]]
id = 2
key = "card.prayer"
set = "core"
rarity = "bronze"
cost = { focus = 2, stamina = 8 }
kind = "enchantment"
trigger = "on_round"
effects = [
  { family = "heal", target = "self", amount = 2 },
  { family = "stamina", target = "self", amount = 2 },
]

[[card]]
id = 3
key = "card.dodge"
set = "core"
rarity = "bronze"
cost = { focus = 2, stamina = 4 }
kind = "instant"
effects = [{ family = "force_miss", target = "opponent", kind = "phys", first_only = true }]

[[card]]
id = 4
key = "card.late_win"
set = "core"
rarity = "gold"
requires = { combat = 2 }
cost = { focus = 4, stamina = 12 }
kind = "instant"
condition = { type = "round_at_least", round = 4 }
effects = [{ family = "damage", target = "opponent", amount = 10 }]

[[card]]
id = 5
key = "card.counter"
set = "core"
rarity = "silver"
cost = { focus = 3, stamina = 6 }
kind = "instant"
keywords = ["counter"]
effects = []
"#;

const STARTERS: &str = r#"
[[starter]]
school = "combat"
avatar = "avatar.runner"
cards = { "card.edge" = 4, "card.prayer" = 4, "card.dodge" = 4, "card.late_win" = 4, "card.counter" = 4 }
deck = { "card.edge" = 4, "card.prayer" = 4, "card.dodge" = 4, "card.late_win" = 4, "card.counter" = 4 }
"#;

const PL: &str = r#"
school-combat-name = Walka
school-divine-name = Kapłaństwo
school-element-name = Elementalizm
school-arcane-name = Magia
school-necro-name = Nekromancja
school-tech-name = Technologia
avatar-runner-name = Wędrowiec
card-edge-name = Klinga
card-edge-text = +{ $amount } AF do końca pojedynku.
card-prayer-name = Modlitwa
card-prayer-text = Co rundę +{ $amount } Ż i +{ $amount_2 } W.
card-dodge-name = Unik
card-dodge-text = Pierwszy atak fizyczny przeciwnika chybia.
card-late_win-name = Późny cios
card-late_win-text = Od rundy { $round }: { $amount } obrażeń.
card-counter-name = Kontra
card-counter-text = Anuluje czar przeciwnika.
"#;

const EN: &str = r#"
school-combat-name = Combat
school-divine-name = Divinity
school-element-name = Elementalism
school-arcane-name = Arcana
school-necro-name = Necromancy
school-tech-name = Technology
avatar-runner-name = Wanderer
card-edge-name = Edge
card-edge-text = +{ $amount } physical attack until the end of the duel.
card-prayer-name = Prayer
card-prayer-text = Each round +{ $amount } HP and +{ $amount_2 } stamina.
card-dodge-name = Dodge
card-dodge-text = The opponent's first physical attack misses.
card-late_win-name = Late Strike
card-late_win-text = From round { $round }: { $amount } damage.
card-counter-name = Counter
card-counter-text = Cancels the opponent's spell.
"#;

fn avatars() -> String {
    let mut text = String::from(
        "[[avatar]]\nid = 1\nkey = \"avatar.runner\"\nrarity = \"common\"\nstarter_for = \"combat\"\n\
         base = { phys_atk = 4, phys_def = 3, mag_atk = 3, mag_def = 0, hp = 30, stamina = 30 }\n\
         schools = { combat = 1 }\ndev_points = { 5 = 1, 10 = 2, 15 = 1, 20 = 2 }\ngrowth = [\n",
    );
    for level in 2..=20 {
        let schools = if level == 5 {
            ", schools = { combat = 2 }"
        } else {
            ""
        };
        text.push_str(&format!(
            "  {{ level = {level}, d = [1, 0, 0, 0, 3, 4]{schools} }},\n"
        ));
    }
    text.push_str("]\n");
    text
}

/// Katalog z poprawną treścią, do zepsucia w teście.
struct Fixture {
    dir: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "content-compiler-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&dir);
        let files = [
            ("rules.toml", RULES.to_owned()),
            ("schools.toml", SCHOOLS.to_owned()),
            ("avatars/basic.toml", avatars()),
            ("cards/core/combat.toml", CARDS.to_owned()),
            ("starter_decks.toml", STARTERS.to_owned()),
            ("i18n/pl.ftl", PL.to_owned()),
            ("i18n/en.ftl", EN.to_owned()),
        ];
        for (file, text) in files {
            let path = dir.join(file);
            fs::create_dir_all(path.parent().expect("plik w katalogu")).expect("katalog testowy");
            fs::write(path, text).expect("plik testowy");
        }
        Self { dir }
    }

    /// Zamienia w pliku `file` tekst `from` na `to`.
    fn replace(&self, file: &str, from: &str, to: &str) -> &Self {
        let path = self.dir.join(file);
        let text = fs::read_to_string(&path).expect("plik testowy");
        assert!(text.contains(from), "w {file} nie ma „{from}”");
        fs::write(path, text.replacen(from, to, 1)).expect("plik testowy");
        self
    }

    fn path(&self) -> &Path {
        &self.dir
    }

    /// Błędy kompilacji jako tekst, jeden w wierszu.
    fn errors(&self) -> String {
        match build(self.path()) {
            Ok(_) => String::new(),
            Err(errors) => errors.iter().map(|error| format!("{error}\n")).collect(),
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn valid_content_compiles_to_engine_types_code_and_client_json() {
    let fixture = Fixture::new();
    let compiled = build(fixture.path()).expect("poprawna treść");
    assert_eq!(compiled.cards.len(), 5);
    assert_eq!(compiled.starters[0].deck.len(), 20);
    let code = codegen::generate(&compiled).expect("rustfmt działa");
    assert!(code.contains("pub const CONTENT_VERSION"));
    assert!(code.contains("Effect::ForceMiss"));
    let json = content_json(&compiled);
    assert!(json.contains("\"key\": \"card.edge\""));
    assert!(!json.contains("\"ref\""));
}

#[test]
fn content_version_changes_with_any_change_including_texts() {
    let fixture = Fixture::new();
    let first = version(&build(fixture.path()).expect("poprawna treść"));
    assert_eq!(
        first,
        version(&build(fixture.path()).expect("poprawna treść"))
    );
    fixture.replace("i18n/en.ftl", "Edge", "Blade");
    let second = version(&build(fixture.path()).expect("poprawna treść"));
    assert_ne!(first, second);
}

/// Zepsuty plik daje błąd zawierający `expected`.
fn rejects(file: &str, from: &str, to: &str, expected: &str) {
    let fixture = Fixture::new();
    fixture.replace(file, from, to);
    let errors = fixture.errors();
    assert!(
        errors.contains(expected),
        "oczekiwano „{expected}”, a jest:\n{errors}"
    );
}

#[test]
fn syntax_errors_point_to_the_line() {
    rejects(
        "cards/core/combat.toml",
        "rarity = \"bronze\"",
        "rarity = \"brass\"",
        "cards/core/combat.toml: wiersz 6:",
    );
    rejects(
        "cards/core/combat.toml",
        "set = \"core\"",
        "sett = \"core\"",
        "unknown field `sett`",
    );
    rejects("rules.toml", "rounds = 5", "rounds = 50", "rules.toml: ");
    rejects(
        "i18n/pl.ftl",
        "card-edge-name = Klinga",
        "card-edge-name = { Klinga",
        "i18n/pl.ftl: wiersz",
    );
}

#[test]
fn ids_keys_and_references_are_checked() {
    rejects(
        "cards/core/combat.toml",
        "id = 2",
        "id = 1",
        "[card.edge]: identyfikator 1 się powtarza",
    );
    rejects(
        "cards/core/combat.toml",
        "key = \"card.prayer\"",
        "key = \"prayer\"",
        "[prayer]: klucz musi mieć postać card.nazwa",
    );
    rejects(
        "starter_decks.toml",
        "avatar = \"avatar.runner\"",
        "avatar = \"avatar.nobody\"",
        "nieznana postać avatar.nobody",
    );
    rejects(
        "starter_decks.toml",
        "deck = { \"card.edge\"",
        "deck = { \"card.edgy\"",
        "talia: nieznana karta card.edgy",
    );
}

#[test]
fn card_rules_are_checked() {
    rejects(
        "cards/core/combat.toml",
        "requires = { combat = 1 }",
        "requires = { combat = 5 }",
        "[card.edge]: wymagania: poziom szkoły combat",
    );
    rejects(
        "cards/core/combat.toml",
        "trigger = \"on_round\"\n",
        "",
        "[card.prayer]: umagicznienie musi mieć wyzwalacz",
    );
    rejects(
        "cards/core/combat.toml",
        "stat = \"phys_atk\", amount = 2 }",
        "stat = \"phys_atk\", amount = 2, duration = \"while_active\" }",
        "[card.edge]: efekt 1: czas „while_active” działa tylko w umagicznieniach",
    );
    rejects(
        "cards/core/combat.toml",
        "round = 4",
        "round = 9",
        "runda w warunku musi być w zakresie 1–5",
    );
    rejects(
        "cards/core/combat.toml",
        "keywords = [\"counter\"]\n",
        "",
        "[card.counter]: karta musi mieć co najmniej jeden efekt",
    );
}

#[test]
fn avatars_and_starters_are_checked() {
    rejects(
        "avatars/basic.toml",
        "  { level = 7, d = [1, 0, 0, 0, 3, 4] },\n",
        "",
        "tabela rozwoju musi mieć po kolei poziomy 2–20",
    );
    rejects(
        "avatars/basic.toml",
        "20 = 2",
        "20 = 3",
        "punkty rozwoju nie zgadzają się z harmonogramem",
    );
    rejects(
        "schools.toml",
        "#b03a2e",
        "red",
        "kolor „red” nie ma postaci #rrggbb",
    );
    rejects(
        "starter_decks.toml",
        "deck = { \"card.edge\" = 4",
        "deck = { \"card.edge\" = 3",
        "talia ma 19 kart zamiast 20",
    );
}

#[test]
fn texts_must_exist_in_both_languages_and_use_known_variables() {
    rejects(
        "i18n/en.ftl",
        "card-dodge-name = Dodge\n",
        "",
        "i18n/en.ftl [card.dodge]: brak tekstu card-dodge-name",
    );
    rejects(
        "i18n/pl.ftl",
        "{ $amount_2 } W",
        "{ $percent } W",
        "tekst card-prayer-text używa nieznanej zmiennej $percent",
    );
    rejects(
        "i18n/pl.ftl",
        "school-tech-name = Technologia\n",
        "school-tech-name = Technologia\nextra-text = x\n",
        "tekst extra-text nie należy do żadnego elementu treści",
    );
}
