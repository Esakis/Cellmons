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

pub use generated::{AVATARS, CARD_KEYS, CONTENT_VERSION, STARTERS};

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
pub struct AvatarDef {
    pub id: u16,
    pub key: &'static str,
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
    AVATARS.iter().find(|avatar| avatar.id == id)
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
    use super::{AVATARS, MAX_LEVEL, STARTERS, avatar, content, rules};

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
            assert!(
                (14..=20).contains(&combat),
                "{}: walka +{combat}",
                avatar.key
            );
            let stamina = end.stamina_max - start.stamina_max;
            assert!((55..=90).contains(&stamina), "{}: W +{stamina}", avatar.key);
            let hp = end.hp_max - start.hp_max;
            assert!((55..=95).contains(&hp), "{}: Ż +{hp}", avatar.key);
            // Na poziomie 20 dwie szkoły: 3 + 2 albo 3 + 3.
            let mut levels = schools.0;
            levels.sort_unstable();
            assert!(
                levels[5] == 3 && levels[4] >= 2,
                "{}: szkoły {levels:?}",
                avatar.key
            );
            let points: u8 = avatar.dev_points.iter().map(|(_, count)| count).sum();
            assert_eq!(points, 6, "{}", avatar.key);
        }
    }
}
