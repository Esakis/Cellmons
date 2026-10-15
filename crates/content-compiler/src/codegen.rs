//! Generowanie `crates/engine-content/src/generated.rs`: treść jako kod Rust, bez parsowania
//! w czasie działania (`docs/06-silnik-gry.md`, rozdz. 10).

use std::fmt::Write as _;
use std::io::Write as _;
use std::process::{Command, Stdio};

use engine::{
    AttackFilter, CardKind, Condition, Duration, Effect, HandAction, Kind, Pool, RulesConfig,
    School, Schools, Stat, StatPair, Stats, Target, Trigger,
};

use crate::compile::{Compiled, CompiledAvatar, Growth};
use crate::output::{hex, version};

/// Ścieżka wygenerowanego pliku względem katalogu głównego repozytorium.
pub const GENERATED: &str = "crates/engine-content/src/generated.rs";

/// Kod pliku `generated.rs`, sformatowany przez `rustfmt`.
pub fn generate(compiled: &Compiled) -> Result<String, String> {
    format(&raw(compiled))
}

fn raw(compiled: &Compiled) -> String {
    let mut out = String::new();
    let version = version(compiled);
    let _ = writeln!(
        out,
        "// Plik wygenerowany przez `cargo run -p content-compiler -- build` z katalogu `content/`.\n\
         // Nie edytuj go ręcznie: zmiany wprowadzaj w plikach treści.\n\
         // content_version: {}\n",
        hex(&version)
    );
    // Część typów występuje tylko przy niektórych kartach, więc import może być nieużywany.
    out.push_str(
        "#![allow(unused_imports)]

",
    );
    out.push_str(
        "use engine::{AttackFilter, CardDef, CardId, CardKind, Condition, Cost, Coupling, \
         CounterResolution, DeckRules, DefaultSettings, DuelRules, Duration, Effect, HandAction, \
         Keywords, Kind, Pool, RulesConfig, School, Schools, Stat, StatPair, Stats, Target, Trigger};\n\n\
         use crate::{AvatarDef, Growth, StarterDef, effects};\n\n",
    );
    let _ = writeln!(
        out,
        "/// Wersja treści (`content_version`).\npub const CONTENT_VERSION: [u8; 32] = {version:?};\n"
    );

    out.push_str("/// Zasady z `content/rules.toml`.\npub(crate) fn rules() -> RulesConfig {\n");
    out.push_str(&rules(&compiled.rules));
    out.push_str("}\n\n");

    out.push_str("/// Karty rosnąco według identyfikatora.\npub(crate) fn cards() -> Vec<CardDef> {\nvec![\n");
    for card in &compiled.cards {
        let def = &card.def;
        let _ = writeln!(
            out,
            "CardDef {{ id: {}, requires: {}, cost: Cost {{ focus: {}, stamina: {} }}, kind: {}, \
             keywords: Keywords {{ intervention: {}, counter: {}, coupling: {}, random: {} }}, \
             condition: {}, effects: effects(&[{}]) }},",
            def.id,
            schools(&def.requires),
            def.cost.focus,
            def.cost.stamina,
            kind(def.kind),
            def.keywords.intervention,
            def.keywords.counter,
            def.keywords
                .coupling
                .map_or("None".to_owned(), |coupling| format!(
                    "Some(Coupling {{ school: {}, level: {} }})",
                    school(coupling.school),
                    coupling.level
                )),
            def.keywords.random,
            def.condition
                .map_or("None".to_owned(), |c| format!("Some({})", condition(c))),
            def.effects
                .iter()
                .map(effect)
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    out.push_str("]\n}\n\n");

    out.push_str("/// Klucze kart rosnąco według identyfikatora.\npub static CARD_KEYS: &[(CardId, &str)] = &[\n");
    for card in &compiled.cards {
        let _ = writeln!(out, "({}, {:?}),", card.def.id, card.src.key);
    }
    out.push_str("];\n\n");

    out.push_str(
        "/// Postacie rosnąco według identyfikatora.\npub static AVATARS: &[AvatarDef] = &[\n",
    );
    for avatar in &compiled.avatars {
        out.push_str(&avatar_def(avatar));
    }
    out.push_str("];\n\n");

    out.push_str(
        "/// Zestawy startowe w kolejności szkół.\npub static STARTERS: &[StarterDef] = &[\n",
    );
    for starter in &compiled.starters {
        let _ = writeln!(
            out,
            "StarterDef {{ school: {}, avatar: {}, cards: &{:?}, deck: &{:?} }},",
            school(starter.school),
            starter.avatar,
            starter.cards,
            starter.deck
        );
    }
    out.push_str("];\n");
    out
}

fn avatar_def(avatar: &CompiledAvatar) -> String {
    let growth: Vec<String> = avatar.growth.iter().map(growth).collect();
    format!(
        "AvatarDef {{ id: {}, key: {:?}, starter_for: {}, base: {}, schools: {}, growth: &[{}], dev_points: &{:?} }},\n",
        avatar.src.id,
        avatar.src.key,
        avatar
            .src
            .starter_for
            .map_or("None".to_owned(), |key| format!(
                "Some({})",
                school(crate::compile::school(key))
            )),
        stats(&avatar.base),
        schools(&avatar.schools),
        growth.join(", "),
        avatar.dev_points
    )
}

fn growth(step: &Growth) -> String {
    format!(
        "Growth {{ level: {}, delta: {}, schools: {} }}",
        step.level,
        stats(&step.delta),
        step.schools.map_or("None".to_owned(), |levels| format!(
            "Some({})",
            schools(&levels)
        ))
    )
}

fn rules(rules: &RulesConfig) -> String {
    let duel = &rules.duel;
    let deck = &rules.deck;
    // Wyczerpujące rozpakowanie: nowe pole zasad bez obsługi tutaj to błąd kompilacji.
    let engine::DuelRules {
        rounds,
        attacks_per_round,
        focus_per_round,
        focus_cap,
        change_cost,
        hand_size,
        combo_multipliers,
        block_min_stamina,
        default_settings,
        counter_resolution,
        move_timeout_after_opponent_s,
        round_timeout_s,
        max_consecutive_timeouts,
        reconnect_grace_s,
    } = duel;
    let engine::DeckRules {
        size,
        max_copies,
        fill_missing_with_random,
        saved_presets_max,
    } = deck;
    let default_settings = match default_settings {
        engine::DefaultSettings::HighestStat => "DefaultSettings::HighestStat",
    };
    let counter_resolution = match counter_resolution {
        engine::CounterResolution::InitiativeFirst => "CounterResolution::InitiativeFirst",
    };
    format!(
        "RulesConfig {{ duel: DuelRules {{ rounds: {rounds}, attacks_per_round: {attacks_per_round}, \
         focus_per_round: {focus_per_round}, focus_cap: {focus_cap:?}, change_cost: {change_cost}, \
         hand_size: {hand_size}, combo_multipliers: {combo_multipliers:?}, \
         block_min_stamina: {block_min_stamina}, default_settings: {default_settings}, \
         counter_resolution: {counter_resolution}, \
         move_timeout_after_opponent_s: {move_timeout_after_opponent_s}, \
         round_timeout_s: {round_timeout_s}, max_consecutive_timeouts: {max_consecutive_timeouts}, \
         reconnect_grace_s: {reconnect_grace_s} }}, deck: DeckRules {{ size: {size}, \
         max_copies: {max_copies}, fill_missing_with_random: {fill_missing_with_random}, \
         saved_presets_max: {saved_presets_max} }} }}\n"
    )
}

fn stats(stats: &Stats) -> String {
    format!(
        "Stats {{ phys_atk: {}, phys_def: {}, mag_atk: {}, mag_def: {}, hp_max: {}, stamina_max: {} }}",
        stats.phys_atk,
        stats.phys_def,
        stats.mag_atk,
        stats.mag_def,
        stats.hp_max,
        stats.stamina_max
    )
}

fn schools(schools: &Schools) -> String {
    format!("Schools({:?})", schools.0)
}

const fn school(school: School) -> &'static str {
    match school {
        School::Combat => "School::Combat",
        School::Divine => "School::Divine",
        School::Element => "School::Element",
        School::Arcane => "School::Arcane",
        School::Necro => "School::Necro",
        School::Tech => "School::Tech",
    }
}

fn kind(kind: CardKind) -> String {
    match kind {
        CardKind::Instant => "CardKind::Instant".to_owned(),
        CardKind::Enchantment(trigger) => format!(
            "CardKind::Enchantment({})",
            match trigger {
                Trigger::OnRound => "Trigger::OnRound",
                Trigger::OnHit => "Trigger::OnHit",
                Trigger::OnBlocked => "Trigger::OnBlocked",
                Trigger::OnDamageTaken => "Trigger::OnDamageTaken",
                Trigger::OnRoundEnd => "Trigger::OnRoundEnd",
            }
        ),
    }
}

fn condition(condition: Condition) -> String {
    match condition {
        Condition::RoundAtLeast(round) => format!("Condition::RoundAtLeast({round})"),
        Condition::HpPercentAhead => "Condition::HpPercentAhead".to_owned(),
        Condition::OpponentExhausted => "Condition::OpponentExhausted".to_owned(),
    }
}

const fn target(target: Target) -> &'static str {
    match target {
        Target::Caster => "Target::Caster",
        Target::Opponent => "Target::Opponent",
        Target::Both => "Target::Both",
    }
}

const fn stat(stat: Stat) -> &'static str {
    match stat {
        Stat::PhysAtk => "Stat::PhysAtk",
        Stat::PhysDef => "Stat::PhysDef",
        Stat::MagAtk => "Stat::MagAtk",
        Stat::MagDef => "Stat::MagDef",
        Stat::HpMax => "Stat::HpMax",
        Stat::StaminaMax => "Stat::StaminaMax",
    }
}

const fn duration(duration: Duration) -> &'static str {
    match duration {
        Duration::Duel => "Duration::Duel",
        Duration::Round => "Duration::Round",
        Duration::WhileActive => "Duration::WhileActive",
    }
}

fn filter(filter: AttackFilter) -> String {
    let kind = match filter.kind {
        None => "None",
        Some(Kind::Phys) => "Some(Kind::Phys)",
        Some(Kind::Mag) => "Some(Kind::Mag)",
    };
    format!(
        "AttackFilter {{ kind: {kind}, slot: {:?}, first_only: {} }}",
        filter.slot, filter.first_only
    )
}

fn effect(effect: &Effect) -> String {
    match *effect {
        Effect::StatMod {
            target: t,
            stat: s,
            amount,
            duration: d,
        } => format!(
            "Effect::StatMod {{ target: {}, stat: {}, amount: {amount}, duration: {} }}",
            target(t),
            stat(s),
            duration(d)
        ),
        Effect::StatSet {
            target: t,
            stat: s,
            value,
        } => {
            format!(
                "Effect::StatSet {{ target: {}, stat: {}, value: {value} }}",
                target(t),
                stat(s)
            )
        }
        Effect::SwapStats { target: t, pair } => format!(
            "Effect::SwapStats {{ target: {}, pair: {} }}",
            target(t),
            match pair {
                StatPair::Attacks => "StatPair::Attacks",
                StatPair::Defenses => "StatPair::Defenses",
                StatPair::Both => "StatPair::Both",
            }
        ),
        Effect::Heal { target: t, amount } => {
            format!("Effect::Heal {{ target: {}, amount: {amount} }}", target(t))
        }
        Effect::Damage { target: t, amount } => format!(
            "Effect::Damage {{ target: {}, amount: {amount} }}",
            target(t)
        ),
        Effect::Stamina { target: t, amount } => format!(
            "Effect::Stamina {{ target: {}, amount: {amount} }}",
            target(t)
        ),
        Effect::Focus { target: t, amount } => format!(
            "Effect::Focus {{ target: {}, amount: {amount} }}",
            target(t)
        ),
        Effect::Drain { pool, amount } => format!(
            "Effect::Drain {{ pool: {}, amount: {amount} }}",
            match pool {
                Pool::Hp => "Pool::Hp",
                Pool::Stamina => "Pool::Stamina",
                Pool::Focus => "Pool::Focus",
            }
        ),
        Effect::ForceHit {
            target: t,
            filter: f,
        } => {
            format!(
                "Effect::ForceHit {{ target: {}, filter: {} }}",
                target(t),
                filter(f)
            )
        }
        Effect::ForceMiss {
            target: t,
            filter: f,
        } => {
            format!(
                "Effect::ForceMiss {{ target: {}, filter: {} }}",
                target(t),
                filter(f)
            )
        }
        Effect::Unblockable {
            target: t,
            filter: f,
        } => {
            format!(
                "Effect::Unblockable {{ target: {}, filter: {} }}",
                target(t),
                filter(f)
            )
        }
        Effect::SwapChoices { target: t } => {
            format!("Effect::SwapChoices {{ target: {} }}", target(t))
        }
        Effect::DamageTakenMult {
            target: t,
            percent,
            duration: d,
        } => format!(
            "Effect::DamageTakenMult {{ target: {}, percent: {percent}, duration: {} }}",
            target(t),
            duration(d)
        ),
        Effect::SchoolMod {
            target: t,
            school: s,
            amount,
            duration: d,
        } => format!(
            "Effect::SchoolMod {{ target: {}, school: {}, amount: {amount}, duration: {} }}",
            target(t),
            school(s),
            duration(d)
        ),
        Effect::Hand {
            target: t,
            action,
            count,
        } => format!(
            "Effect::Hand {{ target: {}, action: {}, count: {count} }}",
            target(t),
            match action {
                HandAction::DiscardRandom => "HandAction::DiscardRandom",
                HandAction::Draw => "HandAction::Draw",
            }
        ),
        Effect::Dispel { target: t } => format!("Effect::Dispel {{ target: {} }}", target(t)),
        Effect::ConditionalWin { condition: c } => {
            format!("Effect::ConditionalWin {{ condition: {} }}", condition(c))
        }
    }
}

/// Formatuje kod przez `rustfmt` (edycja 2024), żeby plik przechodził `cargo fmt --check`.
fn format(code: &str) -> Result<String, String> {
    let mut child = Command::new("rustfmt")
        .args(["--edition", "2024", "--emit", "stdout"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("nie da się uruchomić rustfmt: {error}"))?;
    child
        .stdin
        .take()
        .ok_or("brak wejścia rustfmt")?
        .write_all(code.as_bytes())
        .map_err(|error| format!("rustfmt: {error}"))?;
    let output = child
        .wait_with_output()
        .map_err(|error| format!("rustfmt: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "rustfmt: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    String::from_utf8(output.stdout).map_err(|error| format!("rustfmt: {error}"))
}
