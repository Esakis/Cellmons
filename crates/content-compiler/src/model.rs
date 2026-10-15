//! Format plików `content/` (`docs/03-tresc-gry.md`, rozdz. 8). Typy odpowiadają plikom TOML jeden
//! do jednego; nieznany klucz jest błędem, żeby literówka nie przeszła niezauważona. Te same typy
//! trafiają do `content.json` dla klienta.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Szkoła Esencji (R-ENT-02).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SchoolKey {
    Combat,
    Divine,
    Element,
    Arcane,
    Necro,
    Tech,
}

impl SchoolKey {
    pub const ALL: [SchoolKey; 6] = [
        SchoolKey::Combat,
        SchoolKey::Divine,
        SchoolKey::Element,
        SchoolKey::Arcane,
        SchoolKey::Necro,
        SchoolKey::Tech,
    ];

    /// Nazwa jak w plikach treści.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            SchoolKey::Combat => "combat",
            SchoolKey::Divine => "divine",
            SchoolKey::Element => "element",
            SchoolKey::Arcane => "arcane",
            SchoolKey::Necro => "necro",
            SchoolKey::Tech => "tech",
        }
    }
}

/// Poziomy szkół, np. `{ combat = 1, arcane = 2 }`.
pub type SchoolLevels = BTreeMap<SchoolKey, u8>;

/// `schools.toml`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SchoolsFile {
    pub school: Vec<SchoolSrc>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SchoolSrc {
    pub key: SchoolKey,
    /// Kolor przewodni szkoły w motywie jasnym i ciemnym, `#rrggbb`.
    pub color: String,
    pub color_dark: String,
}

/// `avatars/*.toml`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AvatarsFile {
    pub avatar: Vec<AvatarSrc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AvatarRarity {
    Common,
    Uncommon,
    Rare,
    Junkyard,
    Shade,
}

/// Statystyki bazowe postaci na poziomie 1 (R-ENT-06).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BaseStats {
    pub phys_atk: i32,
    pub phys_def: i32,
    pub mag_atk: i32,
    pub mag_def: i32,
    pub hp: i32,
    pub stamina: i32,
}

/// Przyrosty na jednym poziomie: `d = [AF, OF, AM, OM, W, Ż]`, opcjonalnie nowe poziomy szkół.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GrowthSrc {
    pub level: u8,
    pub d: [i32; 6],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schools: Option<SchoolLevels>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AvatarSrc {
    pub id: u16,
    pub key: String,
    /// Szablon referencyjny z `docs/dane/postacie.csv`; tylko dla projektanta.
    #[serde(rename = "ref", default, skip_serializing)]
    pub reference: Option<String>,
    pub rarity: AvatarRarity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub starter_for: Option<SchoolKey>,
    pub base: BaseStats,
    pub schools: SchoolLevels,
    /// Punkty rozwoju (PR) przyznawane na poziomach, np. `{ 5 = 1, 10 = 2 }`.
    pub dev_points: BTreeMap<String, u8>,
    /// Przyrosty na poziomach 2–20.
    pub growth: Vec<GrowthSrc>,
}

/// `cards/<zestaw>/*.toml`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CardsFile {
    pub card: Vec<CardSrc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CardRarity {
    Bronze,
    Silver,
    Gold,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CardKindSrc {
    Instant,
    Enchantment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerSrc {
    OnRound,
    OnHit,
    OnBlocked,
    OnDamageTaken,
    OnRoundEnd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KeywordSrc {
    Intervention,
    Counter,
    Random,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CostSrc {
    pub focus: u16,
    pub stamina: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CouplingSrc {
    pub school: SchoolKey,
    pub level: u8,
}

/// Warunek karty (R-CZA-08) albo wygranej warunkowej (R-KON-06).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ConditionSrc {
    RoundAtLeast { round: u8 },
    HpPercentAhead,
    OpponentExhausted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetSrc {
    #[serde(rename = "self")]
    Caster,
    Opponent,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StatSrc {
    PhysAtk,
    PhysDef,
    MagAtk,
    MagDef,
    HpMax,
    StaminaMax,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DurationSrc {
    #[default]
    Duel,
    Round,
    WhileActive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PoolSrc {
    Hp,
    Stamina,
    Focus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StatPairSrc {
    Attacks,
    Defenses,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HandActionSrc {
    DiscardRandom,
    Draw,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KindSrc {
    Phys,
    Mag,
}

/// Rodzina efektu z parametrami (`docs/03-tresc-gry.md`, rozdz. 5.3). Sloty w `slot` liczy się
/// od 1, jak na ekranie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "family", rename_all = "snake_case", deny_unknown_fields)]
pub enum EffectSrc {
    StatMod {
        target: TargetSrc,
        stat: StatSrc,
        amount: i16,
        #[serde(default)]
        duration: DurationSrc,
    },
    StatSet {
        target: TargetSrc,
        stat: StatSrc,
        value: i16,
    },
    SwapStats {
        target: TargetSrc,
        pair: StatPairSrc,
    },
    Heal {
        target: TargetSrc,
        amount: u16,
    },
    Damage {
        target: TargetSrc,
        amount: u16,
    },
    Stamina {
        target: TargetSrc,
        amount: i16,
    },
    Focus {
        target: TargetSrc,
        amount: i16,
    },
    Drain {
        pool: PoolSrc,
        amount: u16,
    },
    ForceHit {
        target: TargetSrc,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        kind: Option<KindSrc>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        slot: Option<u8>,
        #[serde(default)]
        first_only: bool,
    },
    ForceMiss {
        target: TargetSrc,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        kind: Option<KindSrc>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        slot: Option<u8>,
        #[serde(default)]
        first_only: bool,
    },
    Unblockable {
        target: TargetSrc,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        kind: Option<KindSrc>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        slot: Option<u8>,
        #[serde(default)]
        first_only: bool,
    },
    SwapChoices {
        target: TargetSrc,
    },
    DamageTakenMult {
        target: TargetSrc,
        percent: u16,
        #[serde(default)]
        duration: DurationSrc,
    },
    SchoolMod {
        target: TargetSrc,
        school: SchoolKey,
        amount: i8,
        #[serde(default)]
        duration: DurationSrc,
    },
    Hand {
        target: TargetSrc,
        action: HandActionSrc,
        count: u8,
    },
    Dispel {
        target: TargetSrc,
    },
    ConditionalWin {
        condition: ConditionSrc,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CardSrc {
    pub id: u16,
    pub key: String,
    /// Karta referencyjna z `docs/dane/karty_klasyczne.csv`; tylko dla projektanta.
    #[serde(rename = "ref", default, skip_serializing)]
    pub reference: Option<String>,
    pub set: String,
    pub rarity: CardRarity,
    #[serde(default)]
    pub requires: SchoolLevels,
    pub cost: CostSrc,
    pub kind: CardKindSrc,
    /// Wyzwalacz umagicznienia; tylko przy `kind = "enchantment"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger: Option<TriggerSrc>,
    #[serde(default)]
    pub keywords: Vec<KeywordSrc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coupling: Option<CouplingSrc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<ConditionSrc>,
    pub effects: Vec<EffectSrc>,
    #[serde(default = "yes")]
    pub in_boosters: bool,
    #[serde(default)]
    pub tags: Vec<String>,
}

const fn yes() -> bool {
    true
}

/// `starter_decks.toml`: zestawy startowe (R-KOL, `docs/03-tresc-gry.md`, rozdz. 6).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StartersFile {
    pub starter: Vec<StarterSrc>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StarterSrc {
    pub school: SchoolKey,
    /// Postać startowa szkoły.
    pub avatar: String,
    /// Karty, które dostaje nowe konto: klucz karty → liczba kopii.
    pub cards: BTreeMap<String, u8>,
    /// Talia ustawiona na start, złożona z kart zestawu.
    pub deck: BTreeMap<String, u8>,
}
