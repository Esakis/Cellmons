//! Tablice treści gry (karty, postacie, zestawy startowe, zasady) generowane przez
//! `content-compiler` z katalogu `content/` (`docs/03-tresc-gry.md`, rozdz. 8).
//!
//! Dane są w `generated.rs`; ten plik zawiera typy i funkcje dostępu.

use std::sync::LazyLock;

use engine::{
    AvatarSnapshot, CardDef, CardId, Content, Effect, Effects, RulesConfig, School, Schools, Stats,
};

#[rustfmt::skip]
mod generated;

pub use generated::{AVATARS, CARD_KEYS, CARD_META, CONTENT_VERSION, PATHS, STARTERS};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardRarity {
    Bronze,
    Silver,
    Gold,
}
#[derive(Debug, Clone, Copy)]
pub struct CardMeta {
    pub id: CardId,
    pub rarity: CardRarity,
    pub in_boosters: bool,
    pub set: &'static str,
}

/// Najwyższy poziom postaci (R-ENT-04).
pub const MAX_LEVEL: u8 = 20;

/// Przyrosty jednego poziomu postaci.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Growth {
    pub level: u8,
    pub delta: Stats,
    /// Poziomy szkół od tego poziomu postaci, jeśli się zmieniają.
    pub schools: Option<Schools>,
}

/// Szablon postaci (`content/avatars`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AvatarRarity {
    Common,
    Uncommon,
    Rare,
    Junkyard,
    Shade,
}

/// Szablon postaci (`content/avatars`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AvatarDef {
    pub id: u16,
    pub key: &'static str,
    pub rarity: AvatarRarity,
    /// Szkoła, dla której postać jest startowa.
    pub starter_for: Option<School>,
    /// Statystyki na poziomie 1.
    pub base: Stats,
    pub schools: Schools,
    /// Przyrosty na poziomach 2–20 po kolei.
    pub growth: &'static [Growth],
    /// Punkty rozwoju: (poziom, liczba).
    pub dev_points: &'static [(u8, u8)],
}

#[derive(Debug, Clone, Copy)]
pub struct PathDef {
    pub base_avatar: u16,
    pub variant: u8,
    pub avatar: AvatarDef,
    pub specializations: [engine::Specialization; 2],
    pub gifts: [CardId; 2],
}
#[must_use]
pub fn specialization_key(value: engine::Specialization) -> &'static str {
    match value {
        engine::Specialization::Essence => "essence",
        engine::Specialization::Concentration => "concentration",
        engine::Specialization::School(s) => match s {
            School::Combat => "combat",
            School::Divine => "divine",
            School::Element => "element",
            School::Arcane => "arcane",
            School::Necro => "necro",
            School::Tech => "tech",
        },
    }
}

impl AvatarDef {
    /// Statystyki i szkoły na poziomie `level` (1–20), bez wydanych punktów rozwoju.
    #[must_use]
    pub fn at_level(&self, level: u8) -> (Stats, Schools) {
        let mut stats = self.base;
        let mut schools = self.schools;
        for step in self.growth.iter().take_while(|step| step.level <= level) {
            stats.phys_atk += step.delta.phys_atk;
            stats.phys_def += step.delta.phys_def;
            stats.mag_atk += step.delta.mag_atk;
            stats.mag_def += step.delta.mag_def;
            stats.hp_max += step.delta.hp_max;
            stats.stamina_max += step.delta.stamina_max;
            if let Some(changed) = step.schools {
                schools = changed;
            }
        }
        (stats, schools)
    }

    /// Snapshot postaci na poziomie `level` do pojedynku (R-POJ-01).
    #[must_use]
    pub fn snapshot(&self, level: u8) -> AvatarSnapshot {
        let (stats, schools) = self.at_level(level);
        AvatarSnapshot {
            stats,
            schools,
            specialization: None,
            gift: None,
        }
    }
}

/// Zestaw startowy szkoły (`content/starter_decks.toml`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StarterDef {
    pub school: School,
    /// Identyfikator postaci startowej.
    pub avatar: u16,
    /// Karty dla nowego konta: (karta, liczba kopii).
    pub cards: &'static [(CardId, u8)],
    /// Talia ustawiona na start.
    pub deck: &'static [CardId],
}

fn effects(list: &[Effect]) -> Effects {
    list.iter().copied().collect()
}

static CARDS: LazyLock<Vec<CardDef>> = LazyLock::new(generated::cards);

/// Treść gry dla silnika.
#[must_use]
pub fn content() -> Content<'static> {
    Content::new(CONTENT_VERSION, &CARDS).expect("content-compiler sprawdził treść")
}

/// Zasady z `content/rules.toml`.
#[must_use]
pub fn rules() -> RulesConfig {
    generated::rules()
}

/// Postać o identyfikatorze `id`.
#[must_use]
pub fn avatar(id: u16) -> Option<&'static AvatarDef> {
    AVATARS
        .iter()
        .find(|avatar| avatar.id == id)
        .or_else(|| path(id).map(|p| &p.avatar))
}

#[must_use]
pub fn path(id: u16) -> Option<&'static PathDef> {
    PATHS.iter().find(|path| path.avatar.id == id)
}

/// Klucz karty, np. `card.bronze_edge`.
#[must_use]
pub fn card_key(id: CardId) -> Option<&'static str> {
    CARD_KEYS
        .binary_search_by_key(&id, |(card, _)| *card)
        .ok()
        .map(|index| CARD_KEYS[index].1)
}

#[cfg(test)]
mod tests {
    use super::{AVATARS, AvatarRarity, MAX_LEVEL, PATHS, STARTERS, avatar, content, rules};

    #[test]
    fn generated_content_is_accepted_by_the_engine() {
        let content = content();
        assert_eq!(content.cards().len(), super::CARD_KEYS.len());
        assert!(rules().validate().is_ok());
        for starter in STARTERS {
            assert!(avatar(starter.avatar).is_some());
            assert_eq!(starter.deck.len(), usize::from(rules().deck.size));
            assert!(starter.deck.iter().all(|card| content.get(*card).is_some()));
        }
    }

    #[test]
    fn avatar_curves_stay_in_the_reference_ranges() {
        // Przyrosty z poziomu 1 na 20 (docs/03-tresc-gry.md, rozdz. 3.2).
        for avatar in AVATARS {
            let (start, _) = avatar.at_level(1);
            let (end, schools) = avatar.at_level(MAX_LEVEL);
            assert_eq!(start, avatar.base);
            let combat = (end.phys_atk + end.phys_def + end.mag_atk + end.mag_def)
                - (start.phys_atk + start.phys_def + start.mag_atk + start.mag_def);
            if avatar.rarity == AvatarRarity::Shade {
                assert_eq!(combat, 0, "{}", avatar.key);
                assert_eq!(end.hp_max - start.hp_max, 76);
                assert_eq!(end.stamina_max - start.stamina_max, 76);
                assert_eq!(
                    avatar
                        .dev_points
                        .iter()
                        .map(|(_, n)| u16::from(*n))
                        .sum::<u16>(),
                    21
                );
                let perfection = PATHS
                    .iter()
                    .find(|p| p.base_avatar == avatar.id && p.variant == 1)
                    .unwrap();
                let (_, final_schools) = perfection.avatar.at_level(MAX_LEVEL);
                assert_eq!(final_schools.0.iter().filter(|n| **n != 0).count(), 1);
                assert_eq!(final_schools.0.iter().copied().max(), Some(4));
                let madness = PATHS
                    .iter()
                    .find(|p| p.base_avatar == avatar.id && p.variant == 0)
                    .unwrap();
                let (_, mut final_schools) = madness.avatar.at_level(MAX_LEVEL);
                final_schools.0.sort_unstable();
                assert_eq!(final_schools.0, [0, 0, 0, 2, 2, 3]);
                continue;
            }
            assert!(
                (14..=20).contains(&combat),
                "{}: walka +{combat}",
                avatar.key
            );
            let stamina = end.stamina_max - start.stamina_max;
            assert!((55..=90).contains(&stamina), "{}: W +{stamina}", avatar.key);
            let hp = end.hp_max - start.hp_max;
            assert!((55..=95).contains(&hp), "{}: Ż +{hp}", avatar.key);
            // Final schools usually 3+2 or 3+3; the reference Masakrator has a single school.
            let mut levels = schools.0;
            levels.sort_unstable();
            assert!(
                levels[5] == 3 && (levels[4] == 0 || levels[4] >= 2),
                "{}: szkoły {levels:?}",
                avatar.key
            );
            let points: u8 = avatar.dev_points.iter().map(|(_, count)| count).sum();
            assert_eq!(
                points,
                if avatar.rarity == AvatarRarity::Junkyard {
                    10
                } else {
                    6
                },
                "{}",
                avatar.key
            );
        }
    }
    #[test]
    fn each_template_has_two_separate_paths_and_all_gifts_are_active_from_start() {
        assert_eq!(PATHS.len(), AVATARS.len() * 2);
        let content = content();
        for path in PATHS {
            let base = AVATARS.iter().find(|a| a.id == path.base_avatar).unwrap();
            for level in [1, MAX_LEVEL] {
                let (a, _) = base.at_level(level);
                let (b, _) = path.avatar.at_level(level);
                assert_eq!(
                    a.phys_atk + a.phys_def + a.mag_atk + a.mag_def,
                    b.phys_atk + b.phys_def + b.mag_atk + b.mag_def,
                    "{}: combat budget",
                    path.avatar.key
                );
                assert_eq!(
                    a.hp_max + a.stamina_max,
                    b.hp_max + b.stamina_max,
                    "{}: resource budget",
                    path.avatar.key
                );
            }
            assert_eq!(
                base.dev_points, path.avatar.dev_points,
                "{}: PR",
                path.avatar.key
            );
            assert!(AVATARS.iter().any(|a| a.id == path.base_avatar));
            assert!(!AVATARS.iter().any(|a| a.id == path.avatar.id));
            for gift in path.gifts {
                let mut own = path.avatar.snapshot(1);
                own.gift = Some(gift);
                own.specialization = Some(path.specializations[0]);
                let opponent = AVATARS[0].snapshot(1);
                let state = engine::new_duel(
                    &engine::DuelSetup {
                        rules: rules(),
                        seed: [43; 32],
                        players: [
                            engine::PlayerSetup {
                                avatar: own,
                                deck: Default::default(),
                            },
                            engine::PlayerSetup {
                                avatar: opponent,
                                deck: Default::default(),
                            },
                        ],
                    },
                    &content,
                )
                .unwrap();
                assert!(
                    engine::view(&state, engine::Side::A)
                        .enchantments
                        .contains(&(engine::Side::A, gift))
                );
            }
        }
    }
}
