//! Walidacja treści i przekład na typy silnika (`docs/03-tresc-gry.md`, rozdz. 8.3).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use engine::{
    AttackFilter, CardDef, CardId, CardKind, Condition, Cost, Coupling, Duration, Effect, Effects,
    HandAction, Keywords, Kind, MAX_EFFECTS, MAX_POOL, MAX_SCHOOL_LEVEL, MAX_STAT, Pool,
    RulesConfig, School, Schools, Stat, StatPair, Stats, Target, Trigger,
};

use crate::Diagnostic;
use crate::i18n::Messages;
use crate::load::{self, Located, Source};
use crate::model::{
    AvatarRarity, AvatarSrc, CardKindSrc, CardSrc, ConditionSrc, DurationSrc, EffectSrc,
    HandActionSrc, KeywordSrc, KindSrc, PoolSrc, SchoolKey, SchoolLevels, SchoolSrc, StarterSrc,
    StatPairSrc, StatSrc, TargetSrc, TriggerSrc,
};

/// Najwyższy poziom postaci (R-ENT-04).
pub const MAX_LEVEL: u8 = 20;

/// Górne granice kosztów karty; większe wartości to niemal na pewno pomyłka.
const MAX_FOCUS_COST: u16 = 20;
const MAX_STAMINA_COST: u16 = 200;

/// Najwyższy mnożnik otrzymywanych obrażeń w procentach.
const MAX_DAMAGE_PERCENT: u16 = 1_000;

/// Karta po walidacji.
#[derive(Debug, Clone)]
pub struct CompiledCard {
    pub file: PathBuf,
    pub src: CardSrc,
    pub def: CardDef,
}

/// Przyrosty jednego poziomu postaci.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Growth {
    pub level: u8,
    pub delta: Stats,
    /// Poziomy szkół od tego poziomu postaci, jeśli się zmieniają.
    pub schools: Option<Schools>,
}

/// Postać po walidacji.
#[derive(Debug, Clone)]
pub struct CompiledAvatar {
    pub src: AvatarSrc,
    pub base: Stats,
    pub schools: Schools,
    /// Poziomy 2–20 po kolei.
    pub growth: Vec<Growth>,
    /// Punkty rozwoju: (poziom, liczba).
    pub dev_points: Vec<(u8, u8)>,
}

/// Zestaw startowy po walidacji.
#[derive(Debug, Clone)]
pub struct CompiledStarter {
    pub src: StarterSrc,
    pub school: School,
    pub avatar: u16,
    /// Kolekcja: (karta, liczba kopii), rosnąco według karty.
    pub cards: Vec<(CardId, u8)>,
    /// Talia startowa, karty rosnąco.
    pub deck: Vec<CardId>,
}

/// Treść po walidacji, gotowa do generowania kodu i plików klienta.
#[derive(Debug, Clone)]
pub struct Compiled {
    pub rules: RulesConfig,
    pub rules_raw: toml::Table,
    /// Szkoły w kolejności [`SchoolKey::ALL`].
    pub schools: Vec<SchoolSrc>,
    /// Karty rosnąco według `id`.
    pub cards: Vec<CompiledCard>,
    /// Postacie rosnąco według `id`.
    pub avatars: Vec<CompiledAvatar>,
    /// Zestawy startowe w kolejności szkół.
    pub starters: Vec<CompiledStarter>,
    pub texts: Vec<Messages>,
}

/// Sprawdza całą treść; zwraca wszystkie błędy naraz.
pub fn compile(source: &Source) -> Result<Compiled, Vec<Diagnostic>> {
    let mut errors = Vec::new();
    let mut expected = BTreeMap::new();

    let schools = check_schools(&source.schools, &mut errors, &mut expected);
    let mut cards: Vec<CompiledCard> = check_unique(
        &source.cards,
        |card| (card.id, card.key.as_str()),
        "card.",
        &mut errors,
    )
    .into_iter()
    .filter_map(|card| compile_card(card, &source.rules, &mut errors, &mut expected))
    .collect();
    cards.sort_by_key(|card| card.def.id);
    let mut avatars: Vec<CompiledAvatar> = check_unique(
        &source.avatars,
        |avatar| (avatar.id, avatar.key.as_str()),
        "avatar.",
        &mut errors,
    )
    .into_iter()
    .filter_map(|avatar| compile_avatar(avatar, &mut errors, &mut expected))
    .collect();
    avatars.sort_by_key(|avatar| avatar.src.id);
    // Odwołania rozwiązuje się po wszystkich elementach z plików, także błędnych, żeby błąd
    // karty albo postaci nie dawał dodatkowo mylącego „nieznana karta”.
    let starters = compile_starters(source, &mut errors);
    check_texts(&source.texts, &expected, &mut errors);

    if errors.is_empty() {
        Ok(Compiled {
            rules: source.rules.clone(),
            rules_raw: source.rules_raw.clone(),
            schools,
            cards,
            avatars,
            starters,
            texts: source.texts.clone(),
        })
    } else {
        Err(errors)
    }
}

/// Teksty, które muszą istnieć w każdym języku: identyfikator → (plik elementu, klucz, dozwolone
/// zmienne).
type Expected = BTreeMap<String, (PathBuf, String, BTreeSet<String>)>;

fn expect(expected: &mut Expected, file: &Path, key: &str, suffix: &str, vars: &BTreeSet<String>) {
    let (kind, name) = key.split_once('.').unwrap_or(("", key));
    expected.insert(
        format!("{kind}-{name}-{suffix}"),
        (file.to_path_buf(), key.to_owned(), vars.clone()),
    );
}

fn check_schools(
    schools: &[SchoolSrc],
    errors: &mut Vec<Diagnostic>,
    expected: &mut Expected,
) -> Vec<SchoolSrc> {
    let file = Path::new(load::SCHOOLS);
    let mut ordered = Vec::new();
    for key in SchoolKey::ALL {
        let found: Vec<&SchoolSrc> = schools.iter().filter(|school| school.key == key).collect();
        match found.as_slice() {
            [school] => {
                for color in [&school.color, &school.color_dark] {
                    if !is_color(color) {
                        errors.push(Diagnostic::new(
                            file,
                            Some(key.name()),
                            format!("kolor „{color}” nie ma postaci #rrggbb"),
                        ));
                    }
                }
                expect(
                    expected,
                    file,
                    &format!("school.{}", key.name()),
                    "name",
                    &BTreeSet::new(),
                );
                ordered.push((*school).clone());
            }
            [] => errors.push(Diagnostic::new(file, Some(key.name()), "brak opisu szkoły")),
            _ => errors.push(Diagnostic::new(
                file,
                Some(key.name()),
                "szkoła opisana więcej niż raz",
            )),
        }
    }
    ordered
}

fn is_color(text: &str) -> bool {
    text.len() == 7 && text.starts_with('#') && text[1..].chars().all(|c| c.is_ascii_hexdigit())
}

/// Klucz z prefiksem i nazwą z małych liter, cyfr i podkreśleń.
fn is_key(key: &str, prefix: &str) -> bool {
    key.strip_prefix(prefix).is_some_and(|name| {
        !name.is_empty()
            && name.starts_with(|c: char| c.is_ascii_lowercase())
            && name
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    })
}

/// Zostawia elementy o poprawnych i niepowtórzonych identyfikatorach i kluczach.
fn check_unique<'a, T>(
    items: &'a [Located<T>],
    id_key: impl Fn(&T) -> (u16, &str),
    prefix: &str,
    errors: &mut Vec<Diagnostic>,
) -> Vec<&'a Located<T>> {
    let mut ids = BTreeMap::new();
    let mut keys = BTreeMap::new();
    for located in items {
        let (id, key) = id_key(&located.item);
        *ids.entry(id).or_insert(0) += 1;
        *keys.entry(key).or_insert(0) += 1;
    }
    let mut kept = Vec::new();
    for located in items {
        let (id, key) = id_key(&located.item);
        let mut ok = true;
        if !is_key(key, prefix) {
            errors.push(Diagnostic::new(
                &located.file,
                Some(key),
                format!("klucz musi mieć postać {prefix}nazwa (małe litery, cyfry, _)"),
            ));
            ok = false;
        }
        if id == 0 {
            errors.push(Diagnostic::new(
                &located.file,
                Some(key),
                "identyfikator musi być większy od 0",
            ));
            ok = false;
        }
        if ids[&id] > 1 {
            errors.push(Diagnostic::new(
                &located.file,
                Some(key),
                format!("identyfikator {id} się powtarza"),
            ));
            ok = false;
        }
        if keys[key] > 1 {
            errors.push(Diagnostic::new(
                &located.file,
                Some(key),
                "klucz się powtarza",
            ));
            ok = false;
        }
        if ok {
            kept.push(located);
        }
    }
    kept
}

pub const fn school(key: SchoolKey) -> School {
    match key {
        SchoolKey::Combat => School::Combat,
        SchoolKey::Divine => School::Divine,
        SchoolKey::Element => School::Element,
        SchoolKey::Arcane => School::Arcane,
        SchoolKey::Necro => School::Necro,
        SchoolKey::Tech => School::Tech,
    }
}

/// Poziomy szkół 1–4; 0 jest błędem, bo szkołę bez poziomu się pomija.
fn schools(levels: &SchoolLevels, what: &str, report: &mut impl FnMut(String)) -> Schools {
    let mut result = Schools::NONE;
    for (&key, &level) in levels {
        if level == 0 || level > MAX_SCHOOL_LEVEL {
            report(format!(
                "{what}: poziom szkoły {} musi być w zakresie 1–{MAX_SCHOOL_LEVEL}",
                key.name()
            ));
        }
        result = result.with(school(key), level);
    }
    result
}

/// Zmienne dostępne w tekście karty: koszty, poziom sprzężenia, runda z warunku i liczbowe
/// parametry efektów; parametry drugiego i dalszych efektów mają przyrostek `_2`, `_3`…
#[must_use]
pub fn card_vars(card: &CardSrc) -> BTreeSet<String> {
    let mut vars: BTreeSet<String> = ["focus", "stamina"].map(ToOwned::to_owned).into();
    if card.coupling.is_some() {
        vars.insert("level".to_owned());
    }
    if let Some(ConditionSrc::RoundAtLeast { .. }) = card.condition {
        vars.insert("round".to_owned());
    }
    for (index, effect) in card.effects.iter().enumerate() {
        let suffix = if index == 0 {
            String::new()
        } else {
            format!("_{}", index + 1)
        };
        for name in effect_params(effect) {
            vars.insert(format!("{name}{suffix}"));
        }
    }
    vars
}

/// Nazwy liczbowych parametrów efektu, dostępnych w tekście.
#[must_use]
pub fn effect_params(effect: &EffectSrc) -> Vec<&'static str> {
    match effect {
        EffectSrc::StatMod { .. }
        | EffectSrc::Heal { .. }
        | EffectSrc::Damage { .. }
        | EffectSrc::Stamina { .. }
        | EffectSrc::Focus { .. }
        | EffectSrc::Drain { .. }
        | EffectSrc::SchoolMod { .. } => vec!["amount"],
        EffectSrc::StatSet { .. } => vec!["value"],
        EffectSrc::DamageTakenMult { .. } => vec!["percent"],
        EffectSrc::Hand { .. } => vec!["count"],
        EffectSrc::ForceHit { slot, .. }
        | EffectSrc::ForceMiss { slot, .. }
        | EffectSrc::Unblockable { slot, .. } => {
            if slot.is_some() {
                vec!["slot"]
            } else {
                vec![]
            }
        }
        EffectSrc::ConditionalWin {
            condition: ConditionSrc::RoundAtLeast { .. },
        } => vec!["round"],
        EffectSrc::SwapStats { .. }
        | EffectSrc::SwapChoices { .. }
        | EffectSrc::Dispel { .. }
        | EffectSrc::ConditionalWin { .. } => vec![],
    }
}

fn compile_card(
    located: &Located<CardSrc>,
    rules: &RulesConfig,
    errors: &mut Vec<Diagnostic>,
    expected: &mut Expected,
) -> Option<CompiledCard> {
    let card = &located.item;
    let before = errors.len();
    let mut report =
        |message: String| errors.push(Diagnostic::new(&located.file, Some(&card.key), message));

    if !is_key(&format!("set.{}", card.set), "set.") {
        report(format!(
            "nazwa zestawu „{}” może mieć tylko małe litery, cyfry i _",
            card.set
        ));
    }
    let requires = schools(&card.requires, "wymagania", &mut report);
    if card.cost.focus > MAX_FOCUS_COST || card.cost.stamina > MAX_STAMINA_COST {
        report(format!(
            "koszt poza zakresem (K ≤ {MAX_FOCUS_COST}, W ≤ {MAX_STAMINA_COST})"
        ));
    }
    let kind = match (card.kind, card.trigger) {
        (CardKindSrc::Instant, None) => CardKind::Instant,
        (CardKindSrc::Enchantment, Some(trigger)) => CardKind::Enchantment(match trigger {
            TriggerSrc::OnRound => Trigger::OnRound,
            TriggerSrc::OnHit => Trigger::OnHit,
            TriggerSrc::OnBlocked => Trigger::OnBlocked,
            TriggerSrc::OnDamageTaken => Trigger::OnDamageTaken,
            TriggerSrc::OnRoundEnd => Trigger::OnRoundEnd,
        }),
        (CardKindSrc::Instant, Some(_)) => {
            report("wyzwalacz (trigger) mają tylko umagicznienia".to_owned());
            CardKind::Instant
        }
        (CardKindSrc::Enchantment, None) => {
            report("umagicznienie musi mieć wyzwalacz (trigger)".to_owned());
            CardKind::Instant
        }
    };
    let unique: BTreeSet<KeywordSrc> = card.keywords.iter().copied().collect();
    if unique.len() != card.keywords.len() {
        report("słowo kluczowe się powtarza".to_owned());
    }
    let coupling = card.coupling.map(|coupling| {
        if coupling.level == 0 || coupling.level > MAX_SCHOOL_LEVEL {
            report(format!(
                "poziom sprzężenia musi być w zakresie 1–{MAX_SCHOOL_LEVEL}"
            ));
        }
        Coupling {
            school: school(coupling.school),
            level: coupling.level,
        }
    });
    let keywords = Keywords {
        intervention: unique.contains(&KeywordSrc::Intervention),
        counter: unique.contains(&KeywordSrc::Counter),
        coupling,
        random: unique.contains(&KeywordSrc::Random),
    };
    if keywords.random && !card.effects.is_empty() {
        report("karta losowa działa jak wylosowana karta, więc nie ma własnych efektów".to_owned());
    }
    if card.effects.is_empty() && !keywords.counter && !keywords.random {
        report("karta musi mieć co najmniej jeden efekt".to_owned());
    }
    if card.effects.len() > MAX_EFFECTS {
        report(format!("najwyżej {MAX_EFFECTS} efekty na kartę"));
    }
    let condition = card
        .condition
        .map(|condition| self::condition(condition, rules, &mut report));
    let enchantment = matches!(kind, CardKind::Enchantment(_));
    let mut effects = Effects::new();
    for (index, effect) in card.effects.iter().enumerate() {
        let converted = self::effect(effect, enchantment, rules, &mut |message| {
            report(format!("efekt {}: {message}", index + 1));
        });
        let _ = effects.try_push(converted);
    }

    let vars = card_vars(card);
    expect(expected, &located.file, &card.key, "name", &BTreeSet::new());
    expect(expected, &located.file, &card.key, "text", &vars);

    (errors.len() == before).then(|| CompiledCard {
        file: located.file.clone(),
        src: card.clone(),
        def: CardDef {
            id: card.id,
            requires,
            cost: Cost {
                focus: card.cost.focus,
                stamina: card.cost.stamina,
            },
            kind,
            keywords,
            condition,
            effects,
        },
    })
}

fn condition(
    condition: ConditionSrc,
    rules: &RulesConfig,
    report: &mut impl FnMut(String),
) -> Condition {
    match condition {
        ConditionSrc::RoundAtLeast { round } => {
            if round == 0 || round > rules.duel.rounds {
                report(format!(
                    "runda w warunku musi być w zakresie 1–{}",
                    rules.duel.rounds
                ));
            }
            Condition::RoundAtLeast(round)
        }
        ConditionSrc::HpPercentAhead => Condition::HpPercentAhead,
        ConditionSrc::OpponentExhausted => Condition::OpponentExhausted,
    }
}

const fn target(target: TargetSrc) -> Target {
    match target {
        TargetSrc::Caster => Target::Caster,
        TargetSrc::Opponent => Target::Opponent,
        TargetSrc::Both => Target::Both,
    }
}

const fn stat(stat: StatSrc) -> Stat {
    match stat {
        StatSrc::PhysAtk => Stat::PhysAtk,
        StatSrc::PhysDef => Stat::PhysDef,
        StatSrc::MagAtk => Stat::MagAtk,
        StatSrc::MagDef => Stat::MagDef,
        StatSrc::HpMax => Stat::HpMax,
        StatSrc::StaminaMax => Stat::StaminaMax,
    }
}

fn effect(
    effect: &EffectSrc,
    enchantment: bool,
    rules: &RulesConfig,
    report: &mut impl FnMut(String),
) -> Effect {
    let mut duration = |duration: DurationSrc| match duration {
        DurationSrc::Duel => Duration::Duel,
        DurationSrc::Round => Duration::Round,
        DurationSrc::WhileActive => {
            if !enchantment {
                report("czas „while_active” działa tylko w umagicznieniach".to_owned());
            }
            Duration::WhileActive
        }
    };
    match *effect {
        EffectSrc::StatMod {
            target: t,
            stat: s,
            amount,
            duration: d,
        } => {
            let duration = duration(d);
            if amount == 0 {
                report("zmiana o 0 nic nie robi".to_owned());
            }
            Effect::StatMod {
                target: target(t),
                stat: stat(s),
                amount,
                duration,
            }
        }
        EffectSrc::StatSet {
            target: t,
            stat: s,
            value,
        } => {
            if value < i16::try_from(stat(s).floor()).unwrap_or(0) {
                report(format!(
                    "wartość {value} poniżej najmniejszej dozwolonej dla tej statystyki"
                ));
            }
            Effect::StatSet {
                target: target(t),
                stat: stat(s),
                value,
            }
        }
        EffectSrc::SwapStats { target: t, pair } => Effect::SwapStats {
            target: target(t),
            pair: match pair {
                StatPairSrc::Attacks => StatPair::Attacks,
                StatPairSrc::Defenses => StatPair::Defenses,
                StatPairSrc::Both => StatPair::Both,
            },
        },
        EffectSrc::Heal { target: t, amount } | EffectSrc::Damage { target: t, amount } => {
            if amount == 0 {
                report("wartość 0 nic nie robi".to_owned());
            }
            if matches!(effect, EffectSrc::Heal { .. }) {
                Effect::Heal {
                    target: target(t),
                    amount,
                }
            } else {
                Effect::Damage {
                    target: target(t),
                    amount,
                }
            }
        }
        EffectSrc::Stamina { target: t, amount } | EffectSrc::Focus { target: t, amount } => {
            if amount == 0 {
                report("zmiana o 0 nic nie robi".to_owned());
            }
            if matches!(effect, EffectSrc::Stamina { .. }) {
                Effect::Stamina {
                    target: target(t),
                    amount,
                }
            } else {
                Effect::Focus {
                    target: target(t),
                    amount,
                }
            }
        }
        EffectSrc::Drain { pool, amount } => {
            if amount == 0 {
                report("wartość 0 nic nie robi".to_owned());
            }
            Effect::Drain {
                pool: match pool {
                    PoolSrc::Hp => Pool::Hp,
                    PoolSrc::Stamina => Pool::Stamina,
                    PoolSrc::Focus => Pool::Focus,
                },
                amount,
            }
        }
        EffectSrc::ForceHit {
            target: t,
            kind,
            slot,
            first_only,
        }
        | EffectSrc::ForceMiss {
            target: t,
            kind,
            slot,
            first_only,
        }
        | EffectSrc::Unblockable {
            target: t,
            kind,
            slot,
            first_only,
        } => {
            let slots = rules.duel.attacks_per_round;
            if slot.is_some_and(|slot| slot == 0 || slot > slots) {
                report(format!("slot musi być w zakresie 1–{slots}"));
            }
            let filter = AttackFilter {
                kind: kind.map(|kind| match kind {
                    KindSrc::Phys => Kind::Phys,
                    KindSrc::Mag => Kind::Mag,
                }),
                slot: slot.map(|slot| slot.saturating_sub(1)),
                first_only,
            };
            let target = target(t);
            match effect {
                EffectSrc::ForceHit { .. } => Effect::ForceHit { target, filter },
                EffectSrc::ForceMiss { .. } => Effect::ForceMiss { target, filter },
                _ => Effect::Unblockable { target, filter },
            }
        }
        EffectSrc::SwapChoices { target: t } => Effect::SwapChoices { target: target(t) },
        EffectSrc::DamageTakenMult {
            target: t,
            percent,
            duration: d,
        } => {
            let duration = duration(d);
            if percent > MAX_DAMAGE_PERCENT || percent == 100 {
                report(format!(
                    "mnożnik musi być różny od 100% i nie większy niż {MAX_DAMAGE_PERCENT}%"
                ));
            }
            Effect::DamageTakenMult {
                target: target(t),
                percent,
                duration,
            }
        }
        EffectSrc::SchoolMod {
            target: t,
            school: s,
            amount,
            duration: d,
        } => {
            let duration = duration(d);
            if amount == 0 {
                report("zmiana o 0 nic nie robi".to_owned());
            }
            Effect::SchoolMod {
                target: target(t),
                school: school(s),
                amount,
                duration,
            }
        }
        EffectSrc::Hand {
            target: t,
            action,
            count,
        } => {
            if count == 0 || count > rules.duel.hand_size {
                report(format!(
                    "liczba kart musi być w zakresie 1–{}",
                    rules.duel.hand_size
                ));
            }
            Effect::Hand {
                target: target(t),
                action: match action {
                    HandActionSrc::DiscardRandom => HandAction::DiscardRandom,
                    HandActionSrc::Draw => HandAction::Draw,
                },
                count,
            }
        }
        EffectSrc::Dispel { target: t } => Effect::Dispel { target: target(t) },
        EffectSrc::ConditionalWin { condition: c } => Effect::ConditionalWin {
            condition: condition(c, rules, report),
        },
    }
}

/// Harmonogram punktów rozwoju (R-ENT-04, `docs/03-tresc-gry.md`, rozdz. 3.2): (poziom, liczba).
#[must_use]
pub fn dev_point_schedule(rarity: AvatarRarity) -> Option<Vec<(u8, u8)>> {
    match rarity {
        AvatarRarity::Common | AvatarRarity::Uncommon | AvatarRarity::Rare => {
            Some(vec![(5, 1), (10, 2), (15, 1), (20, 2)])
        }
        AvatarRarity::Junkyard => {
            Some((2..=MAX_LEVEL).step_by(2).map(|level| (level, 1)).collect())
        }
        // Cienie rozwijają się tylko przez PR (R-CIE); harmonogram ustali faza z cieniami.
        AvatarRarity::Shade => None,
    }
}

fn compile_avatar(
    located: &Located<AvatarSrc>,
    errors: &mut Vec<Diagnostic>,
    expected: &mut Expected,
) -> Option<CompiledAvatar> {
    let avatar = &located.item;
    let before = errors.len();
    let mut report =
        |message: String| errors.push(Diagnostic::new(&located.file, Some(&avatar.key), message));

    let b = avatar.base;
    let base = Stats {
        phys_atk: b.phys_atk,
        phys_def: b.phys_def,
        mag_atk: b.mag_atk,
        mag_def: b.mag_def,
        hp_max: b.hp,
        stamina_max: b.stamina,
    };
    if [b.phys_atk, b.phys_def, b.mag_atk, b.mag_def]
        .iter()
        .any(|value| !(0..=MAX_STAT).contains(value))
    {
        report(format!(
            "statystyki walki muszą być w zakresie 0–{MAX_STAT}"
        ));
    }
    if [b.hp, b.stamina]
        .iter()
        .any(|value| !(1..=MAX_POOL).contains(value))
    {
        report(format!("Ż i W muszą być w zakresie 1–{MAX_POOL}"));
    }
    if avatar.schools.is_empty() {
        report("postać musi znać co najmniej jedną szkołę".to_owned());
    }
    let start = schools(&avatar.schools, "szkoły", &mut report);

    let mut growth = Vec::new();
    let mut current = start;
    let levels: Vec<u8> = avatar.growth.iter().map(|step| step.level).collect();
    let wanted: Vec<u8> = (2..=MAX_LEVEL).collect();
    if levels != wanted {
        report(format!(
            "tabela rozwoju musi mieć po kolei poziomy 2–{MAX_LEVEL}"
        ));
    }
    for step in &avatar.growth {
        if step.d.iter().any(|delta| *delta < 0) {
            report(format!(
                "poziom {}: przyrosty nie mogą być ujemne",
                step.level
            ));
        }
        let new_schools = step.schools.as_ref().map(|levels| {
            let mut next = current;
            let changed = schools(levels, &format!("poziom {}", step.level), &mut report);
            for key in SchoolKey::ALL {
                let level = changed.level(school(key));
                if levels.contains_key(&key) {
                    if level < current.level(school(key)) {
                        report(format!(
                            "poziom {}: poziom szkoły {} nie może spaść",
                            step.level,
                            key.name()
                        ));
                    }
                    next = next.with(school(key), level);
                }
            }
            current = next;
            next
        });
        let [af, of, am, om, st, hp] = step.d;
        growth.push(Growth {
            level: step.level,
            delta: Stats {
                phys_atk: af,
                phys_def: of,
                mag_atk: am,
                mag_def: om,
                hp_max: hp,
                stamina_max: st,
            },
            schools: new_schools,
        });
    }

    let mut dev_points = Vec::new();
    for (level, &count) in &avatar.dev_points {
        match level.parse::<u8>() {
            Ok(level) if (2..=MAX_LEVEL).contains(&level) => dev_points.push((level, count)),
            _ => report(format!(
                "punkty rozwoju: „{level}” nie jest poziomem 2–{MAX_LEVEL}"
            )),
        }
    }
    dev_points.sort_unstable();
    if let Some(schedule) = dev_point_schedule(avatar.rarity)
        && dev_points != schedule
    {
        report(format!(
            "punkty rozwoju nie zgadzają się z harmonogramem {schedule:?} (poziom, liczba)"
        ));
    }

    expect(
        expected,
        &located.file,
        &avatar.key,
        "name",
        &BTreeSet::new(),
    );
    (errors.len() == before).then(|| CompiledAvatar {
        src: avatar.clone(),
        base,
        schools: start,
        growth,
        dev_points,
    })
}

fn compile_starters(source: &Source, errors: &mut Vec<Diagnostic>) -> Vec<CompiledStarter> {
    let (starters, rules) = (&source.starters, &source.rules);
    let file = Path::new(load::STARTERS);
    let card_ids: BTreeMap<&str, CardId> = source
        .cards
        .iter()
        .map(|card| (card.item.key.as_str(), card.item.id))
        .collect();
    let mut compiled = Vec::new();
    for key in SchoolKey::ALL {
        let found: Vec<&StarterSrc> = starters
            .iter()
            .filter(|starter| starter.school == key)
            .collect();
        // Szkoła bez zestawu startowego jest dozwolona; pełną szóstkę sprawdzają testy treści.
        let starter = match found.as_slice() {
            [starter] => *starter,
            [] => continue,
            _ => {
                errors.push(Diagnostic::new(
                    file,
                    Some(key.name()),
                    "zestaw startowy szkoły opisany więcej niż raz",
                ));
                continue;
            }
        };
        let before = errors.len();
        let mut report =
            |message: String| errors.push(Diagnostic::new(file, Some(key.name()), message));
        let avatar = source
            .avatars
            .iter()
            .map(|avatar| &avatar.item)
            .find(|avatar| avatar.key == starter.avatar);
        match avatar {
            None => report(format!("nieznana postać {}", starter.avatar)),
            Some(avatar) if avatar.starter_for != Some(key) => {
                report(format!(
                    "postać {} nie ma starter_for = \"{}\"",
                    starter.avatar,
                    key.name()
                ));
            }
            Some(_) => {}
        }
        let mut resolve = |list: &BTreeMap<String, u8>, what: &str| -> Vec<(CardId, u8)> {
            let mut result = Vec::new();
            for (card, &count) in list {
                match card_ids.get(card.as_str()) {
                    Some(&id) if (1..=rules.deck.max_copies).contains(&count) => {
                        result.push((id, count))
                    }
                    Some(_) => report(format!(
                        "{what}: liczba kopii {card} musi być w zakresie 1–{}",
                        rules.deck.max_copies
                    )),
                    None => report(format!("{what}: nieznana karta {card}")),
                }
            }
            result.sort_unstable();
            result
        };
        let collection = resolve(&starter.cards, "karty");
        let deck = resolve(&starter.deck, "talia");
        let size: u32 = deck.iter().map(|(_, count)| u32::from(*count)).sum();
        if size != u32::from(rules.deck.size) {
            report(format!("talia ma {size} kart zamiast {}", rules.deck.size));
        }
        for (card, count) in &deck {
            let owned = collection
                .iter()
                .find(|(id, _)| id == card)
                .map_or(0, |(_, owned)| *owned);
            if *count > owned {
                report(format!("talia: karta {card} ma więcej kopii niż zestaw"));
            }
        }
        if errors.len() == before
            && let Some(avatar) = avatar
        {
            compiled.push(CompiledStarter {
                src: starter.clone(),
                school: school(key),
                avatar: avatar.id,
                cards: collection,
                deck: deck
                    .iter()
                    .flat_map(|&(id, count)| std::iter::repeat_n(id, usize::from(count)))
                    .collect(),
            });
        }
    }
    compiled
}

fn check_texts(texts: &[Messages], expected: &Expected, errors: &mut Vec<Diagnostic>) {
    for messages in texts {
        for (id, (file, key, allowed)) in expected {
            match messages.vars.get(id) {
                None => errors.push(Diagnostic::new(
                    &messages.file,
                    Some(key),
                    format!(
                        "brak tekstu {id} (element w {})",
                        file.to_string_lossy().replace('\\', "/")
                    ),
                )),
                Some(used) => {
                    for var in used.difference(allowed) {
                        errors.push(Diagnostic::new(
                            &messages.file,
                            Some(key),
                            format!("tekst {id} używa nieznanej zmiennej ${var}"),
                        ));
                    }
                }
            }
        }
        for id in messages
            .vars
            .keys()
            .filter(|id| !expected.contains_key(*id))
        {
            errors.push(Diagnostic::new(
                &messages.file,
                None,
                format!("tekst {id} nie należy do żadnego elementu treści"),
            ));
        }
    }
}
