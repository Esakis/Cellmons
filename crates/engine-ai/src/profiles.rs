//! Profile treningowe są wspólne dla serwera i klienta WASM.

use engine::{CardKind, Effect, PlayerSetup};
use serde::{Deserialize, Serialize};

use crate::{Bot, Level};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Archetype {
    Balanced,
    Burst,
    Endurance,
    Enchantment,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BotProfile {
    pub id: String,
    pub name_pl: String,
    pub name_en: String,
    pub level: Level,
    pub aggression: u8,
    pub spell_bias: u8,
    pub epsilon_percent: u8,
    pub temperature_milli: u16,
    pub deck_archetype: Archetype,
    pub avatar_pool: Vec<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profiles {
    pub bots: Vec<BotProfile>,
}

impl Profiles {
    pub fn parse(text: &str) -> Result<Self, String> {
        let profiles: Self = toml::from_str(text).map_err(|error| error.to_string())?;
        if profiles.bots.is_empty() {
            return Err("bots.toml: brak profili".into());
        }
        for (index, profile) in profiles.bots.iter().enumerate() {
            if profile.id.is_empty()
                || profile.name_pl.is_empty()
                || profile.name_en.is_empty()
                || profiles.bots[..index]
                    .iter()
                    .any(|old| old.id == profile.id)
                || profile.aggression > 200
                || profile.spell_bias > 200
                || profile.epsilon_percent > 100
                || !(1..=1000).contains(&profile.temperature_milli)
                || profile.avatar_pool.is_empty()
                || profile
                    .avatar_pool
                    .iter()
                    .any(|&id| engine_content::avatar(id).is_none())
            {
                return Err(format!(
                    "bots.toml [{}]: niepoprawny lub powtórzony profil",
                    profile.id
                ));
            }
        }
        Ok(profiles)
    }
}

pub fn training_profiles() -> Profiles {
    include!(concat!(env!("OUT_DIR"), "/profiles.rs"))
}

impl BotProfile {
    pub fn bot(&self, seed: u64) -> Bot {
        let mut bot = Bot::new(self.level, seed);
        bot.aggression = i32::from(self.aggression);
        bot.spell_bias = i32::from(self.spell_bias);
        bot.epsilon = u32::from(self.epsilon_percent);
        bot.temperature = i32::from(self.temperature_milli);
        bot
    }

    /// Rotacja postaci (R-TRE-02) i talia archetypu zgodna ze szkołami i limitem kopii.
    pub fn player(&self, level: u8, rotation: usize) -> Option<PlayerSetup> {
        if !(1..=20).contains(&level) || self.avatar_pool.is_empty() {
            return None;
        }
        let avatar = engine_content::avatar(self.avatar_pool[rotation % self.avatar_pool.len()])?;
        let snapshot = avatar.snapshot(level);
        let content = engine_content::content();
        let mut cards: Vec<_> = content
            .cards()
            .iter()
            .filter(|card| snapshot.schools.meets(&card.requires))
            .collect();
        cards.sort_by_key(|card| {
            let affinity = match self.deck_archetype {
                Archetype::Balanced => 0,
                Archetype::Enchantment => i32::from(matches!(card.kind, CardKind::Enchantment(_))),
                Archetype::Burst => i32::from(card.effects.iter().any(|effect| {
                    matches!(
                        effect,
                        Effect::Damage { .. }
                            | Effect::ForceHit { .. }
                            | Effect::Unblockable { .. }
                    )
                })),
                Archetype::Endurance => i32::from(card.effects.iter().any(|effect| {
                    matches!(
                        effect,
                        Effect::Stamina { .. } | Effect::Heal { .. } | Effect::Drain { .. }
                    )
                })),
            };
            (-affinity, card.cost.focus, card.id)
        });
        let rules = engine_content::rules();
        let mut deck = engine::PlayerSetup {
            avatar: snapshot,
            deck: Default::default(),
        };
        for _ in 0..rules.deck.max_copies {
            for card in &cards {
                if deck.deck.len() == usize::from(rules.deck.size) {
                    return Some(deck);
                }
                deck.deck.push(card.id);
            }
        }
        (deck.deck.len() == usize::from(rules.deck.size)).then_some(deck)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_profiles_match_the_validated_source() {
        assert_eq!(
            training_profiles(),
            Profiles::parse(include_str!("../../../content/bots.toml")).unwrap()
        );
    }

    #[test]
    fn four_levels_have_valid_rotating_decks() {
        let profiles = training_profiles();
        assert_eq!(
            profiles.bots.iter().map(|p| p.level).collect::<Vec<_>>(),
            vec![Level::L0, Level::L1, Level::L2, Level::L3]
        );
        for profile in profiles.bots {
            for level in [1, 10, 20] {
                for rotation in 0..profile.avatar_pool.len() {
                    let player = profile.player(level, rotation).unwrap();
                    let content = engine_content::content();
                    assert!(player.deck.iter().all(|&id| {
                        player
                            .avatar
                            .schools
                            .meets(&content.get(id).unwrap().requires)
                    }));
                    let setup = engine::DuelSetup {
                        rules: engine_content::rules(),
                        seed: [0; 32],
                        players: [player.clone(), player],
                    };
                    assert!(engine::new_duel(&setup, &engine_content::content()).is_ok());
                }
            }
        }
    }

    #[test]
    fn rejects_invalid_profiles_and_unknown_fields() {
        let text = include_str!("../../../content/bots.toml");
        for changed in [
            text.replace("aggression = 100", "aggression = 201"),
            text.replace("epsilon_percent = 15", "epsilon_percent = 101"),
            text.replace("avatar_pool = [", "unknown = true\navatar_pool = ["),
            text.replace("temperature_milli = 300", "temperature_milli = 0"),
        ] {
            assert!(Profiles::parse(&changed).is_err());
        }
    }

    #[test]
    fn personality_changes_choices_and_archetypes_change_decks() {
        let profiles = training_profiles();
        let profile = &profiles.bots[2];
        let mut calm = profile.clone();
        let mut aggressive = profile.clone();
        calm.aggression = 0;
        aggressive.aggression = 200;
        let mut different = false;
        let content = engine_content::content();
        for rotation in 0..6 {
            let player = profile.player(10, rotation).unwrap();
            let state = engine::new_duel(
                &engine::DuelSetup {
                    rules: engine_content::rules(),
                    seed: [7; 32],
                    players: [player.clone(), player],
                },
                &content,
            )
            .unwrap();
            let seen = engine::view(&state, engine::Side::A);
            different |=
                calm.bot(7).choose(&seen, &content) != aggressive.bot(7).choose(&seen, &content);
        }
        assert!(different);
        let mut burst = profile.clone();
        let mut endurance = profile.clone();
        burst.deck_archetype = Archetype::Burst;
        endurance.deck_archetype = Archetype::Endurance;
        assert!(
            (0..6).any(|rotation| burst.player(1, rotation).unwrap().deck
                != endurance.player(1, rotation).unwrap().deck)
        );
    }
}
