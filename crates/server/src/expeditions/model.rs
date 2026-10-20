//! Frozen expedition plans. Choices, duel inputs and reward randomness survive every retry.
use crate::{
    duel::record::Seat,
    online::{Error, Result},
};
use engine_content::AvatarRarity;
use protocol::game::v1 as pb;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Difficulty {
    pub id: String,
    pub stages: u8,
    pub bot: String,
    pub boss_bot: String,
    pub level_min: i8,
    pub level_max: i8,
    pub boss_level: i8,
    pub rarity: String,
    pub fixed_ducats: u32,
    pub random_ducats: u32,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub avatar_chance_bps: u32,
    pub card_chance_bps: u32,
    pub ducats_chance_bps: u32,
    pub avatar_pool: Vec<u16>,
    pub difficulties: Vec<Difficulty>,
}
impl Config {
    pub fn validate(&self) -> Result<()> {
        let profiles = engine_ai::training_profiles();
        let avatars: std::collections::BTreeSet<_> = self.avatar_pool.iter().collect();
        let ids: std::collections::BTreeSet<_> =
            self.difficulties.iter().map(|d| d.id.as_str()).collect();
        if u64::from(self.avatar_chance_bps)
            + u64::from(self.card_chance_bps)
            + u64::from(self.ducats_chance_bps)
            > 10000
            || self.avatar_pool.len() < 3
            || avatars.len() != self.avatar_pool.len()
            || self.avatar_pool.iter().any(|id| {
                !engine_content::avatar(*id).is_some_and(|a| {
                    a.playable
                        && matches!(
                            a.rarity,
                            AvatarRarity::Common | AvatarRarity::Uncommon | AvatarRarity::Rare
                        )
                })
            })
            || self.difficulties.len() != 3
            || ids != std::collections::BTreeSet::from(["walk", "expedition", "ordeal"])
            || self.difficulties.iter().any(|d| {
                !(1..=8).contains(&d.stages)
                    || d.level_min > d.level_max
                    || d.level_min < -10
                    || d.level_max > 10
                    || !(0..=10).contains(&d.boss_level)
                    || d.fixed_ducats > 1_000_000
                    || d.random_ducats > 1_000_000
                    || !matches!(d.rarity.as_str(), "common" | "uncommon" | "rare")
                    || !profiles.bots.iter().any(|p| p.id == d.bot)
                    || !profiles.bots.iter().any(|p| p.id == d.boss_bot)
            })
        {
            return Err(Error("CONTENT_MISMATCH"));
        }
        Ok(())
    }
    pub fn difficulty(&self, id: &str) -> Result<&Difficulty> {
        self.difficulties
            .iter()
            .find(|d| d.id == id)
            .ok_or(Error("INVALID_MESSAGE"))
    }
}
struct Random {
    seed: [u8; 32],
    counter: u64,
}
impl Random {
    fn number(&mut self, upper: u64) -> u64 {
        let threshold = upper.wrapping_neg() % upper;
        loop {
            let mut hash = Sha256::new();
            hash.update(b"spellgambit-expedition-v1");
            hash.update(self.seed);
            hash.update(self.counter.to_le_bytes());
            self.counter += 1;
            let digest = hash.finalize();
            let value = u64::from_le_bytes(digest[..8].try_into().expect("digest"));
            if value >= threshold {
                return value % upper;
            }
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Enemy {
    pub seat: Seat,
    pub template: u16,
    pub level: u8,
    pub boss: bool,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Plan {
    pub human: Seat,
    pub difficulty: Difficulty,
    pub stages: Vec<[Enemy; 3]>,
    pub boss: Enemy,
    pub avatar_pool: Vec<u16>,
    pub card_pool: Vec<u16>,
    reward_kind: u8,
    reward_selector: u64,
    seed: [u8; 32],
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Award {
    pub xp: u32,
    pub ducats: u32,
    pub template: Option<u16>,
    pub card: Option<u16>,
}
impl Plan {
    pub fn new(
        human: Seat,
        difficulty: &str,
        config: &Config,
        owned: &[i32],
        seed: [u8; 32],
    ) -> Result<Self> {
        config.validate()?;
        let difficulty = config.difficulty(difficulty)?.clone();
        let level = human.public()?.level as u8;
        let mut rng = Random { seed, counter: 0 };
        let mut stages = vec![];
        for _ in 0..difficulty.stages {
            let mut pool = config.avatar_pool.clone();
            for i in (1..pool.len()).rev() {
                let j = rng.number((i + 1) as u64) as usize;
                pool.swap(i, j);
            }
            let mut enemies = vec![];
            for template in &pool[..3] {
                let offset = i16::from(difficulty.level_min)
                    + rng.number(
                        (i16::from(difficulty.level_max) - i16::from(difficulty.level_min) + 1)
                            as u64,
                    ) as i16;
                let enemy_level = (i16::from(level) + offset).clamp(1, 20) as u8;
                enemies.push(enemy(*template, enemy_level, &difficulty.bot, false)?);
            }
            stages.push(enemies.try_into().map_err(|_| Error("INTERNAL"))?);
        }
        let boss_template =
            config.avatar_pool[rng.number(config.avatar_pool.len() as u64) as usize];
        let boss_level = (i16::from(level) + i16::from(difficulty.boss_level)).clamp(1, 20) as u8;
        let boss = enemy(boss_template, boss_level, &difficulty.boss_bot, true)?;
        let reward_kind = bonus_kind(config, rng.number(10000) as u32);
        let avatar_pool = engine_content::AVATARS
            .iter()
            .filter(|a| {
                a.playable
                    && !owned.contains(&i32::from(a.id))
                    && matches!(
                        (difficulty.rarity.as_str(), a.rarity),
                        ("common", AvatarRarity::Common)
                            | ("uncommon", AvatarRarity::Uncommon)
                            | ("rare", AvatarRarity::Rare)
                    )
            })
            .map(|a| a.id)
            .collect();
        let card_pool = engine_content::content()
            .cards()
            .iter()
            .map(|c| c.id)
            .collect();
        Ok(Self {
            human,
            difficulty,
            stages,
            boss,
            avatar_pool,
            card_pool,
            reward_kind,
            reward_selector: rng.number(u64::MAX),
            seed,
        })
    }
    pub fn total_stages(&self) -> u32 {
        u32::from(self.difficulty.stages) + 1
    }
    pub fn enemy(&self, stage: u8, choice: u8) -> Result<&Enemy> {
        if stage == self.difficulty.stages {
            if choice != 0 {
                return Err(Error("INVALID_MESSAGE"));
            }
            return Ok(&self.boss);
        }
        self.stages
            .get(usize::from(stage))
            .and_then(|s| s.get(usize::from(choice)))
            .ok_or(Error("INVALID_MESSAGE"))
    }
    pub fn reward(&self, owned: &[i32]) -> Award {
        let mut result = Award {
            ducats: self.difficulty.fixed_ducats,
            ..Default::default()
        };
        match self.reward_kind {
            1 => {
                let pool: Vec<_> = self
                    .avatar_pool
                    .iter()
                    .copied()
                    .filter(|id| {
                        engine_content::is_playable(*id) && !owned.contains(&i32::from(*id))
                    })
                    .collect();
                result.template = self.select(&pool);
            }
            2 => {
                result.card = self.select(&self.card_pool);
            }
            3 => result.ducats += self.difficulty.random_ducats,
            _ => {}
        }
        result
    }
    fn select(&self, pool: &[u16]) -> Option<u16> {
        if pool.is_empty() {
            return None;
        }
        let mut hash = Sha256::new();
        hash.update(b"spellgambit-expedition-reward-v1");
        hash.update(self.seed);
        hash.update(self.reward_selector.to_le_bytes());
        let mut random = Random {
            seed: hash.finalize().into(),
            counter: 0,
        };
        Some(pool[random.number(pool.len() as u64) as usize])
    }
    pub fn duel_seed(&self, stage: u8) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(b"spellgambit-expedition-duel-v1");
        hash.update(self.seed);
        hash.update([stage]);
        hash.finalize().into()
    }
}
fn bonus_kind(config: &Config, roll: u32) -> u8 {
    if roll < config.avatar_chance_bps {
        1
    } else if roll < config.avatar_chance_bps + config.card_chance_bps {
        2
    } else if roll < config.avatar_chance_bps + config.card_chance_bps + config.ducats_chance_bps {
        3
    } else {
        0
    }
}
fn enemy(template: u16, level: u8, bot: &str, boss: bool) -> Result<Enemy> {
    let mut profile = engine_ai::training_profiles()
        .bots
        .into_iter()
        .find(|p| p.id == bot)
        .ok_or(Error("CONTENT_MISMATCH"))?;
    profile.avatar_pool = vec![template];
    let mut player = profile.player(level, 0).ok_or(Error("CONTENT_MISMATCH"))?;
    let definition = engine_content::avatar(template).ok_or(Error("CONTENT_MISMATCH"))?;
    let points: u16 = definition
        .dev_points
        .iter()
        .filter(|(at, _)| *at <= level)
        .map(|(_, n)| u16::from(*n))
        .sum();
    for point in 0..points {
        match point % 4 {
            0 | 1 => {
                if player.avatar.stats.phys_atk >= player.avatar.stats.mag_atk {
                    player.avatar.stats.phys_atk += 1;
                } else {
                    player.avatar.stats.mag_atk += 1;
                }
            }
            2 => player.avatar.stats.hp_max += 7,
            _ => player.avatar.stats.stamina_max += 7,
        }
    }
    let public = pb::PublicPlayer {
        id: format!("expedition-ai:{template}:{level}:{}", u8::from(boss)),
        login: format!("[AI] {} {level}", profile.name_en),
        template_id: u32::from(template),
        level: u32::from(level),
        stats: engine::Stat::ALL
            .map(|s| player.avatar.stats.get(s))
            .to_vec(),
        schools: player.avatar.schools.0.map(u32::from).to_vec(),
        ..Default::default()
    };
    let mut seat = Seat::from_setup(None, None, &player, &public, Some(bot.into()));
    seat.bot_profile = Some(profile);
    Ok(Enemy {
        seat,
        template,
        level,
        boss,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn human() -> Seat {
        let player = engine_ai::training_profiles().bots[1]
            .player(10, 0)
            .unwrap();
        Seat::from_setup(
            None,
            None,
            &player,
            &pb::PublicPlayer {
                template_id: 1,
                level: 10,
                ..Default::default()
            },
            None,
        )
    }
    #[test]
    fn every_reward_roll_belongs_to_exactly_one_category_with_the_documented_probabilities() {
        let mut counts = [0; 4];
        for roll in 0..10000 {
            counts[usize::from(bonus_kind(&crate::expeditions::CONFIG, roll))] += 1;
        }
        assert_eq!(counts, [5600, 4000, 300, 100]);
    }
    #[test]
    fn plans_are_frozen_legal_distinct_and_boss_has_no_alternative() {
        for difficulty in ["walk", "expedition", "ordeal"] {
            let plan = Plan::new(
                human(),
                difficulty,
                &crate::expeditions::CONFIG,
                &[],
                [9; 32],
            )
            .unwrap();
            let again = Plan::new(
                human(),
                difficulty,
                &crate::expeditions::CONFIG,
                &[],
                [9; 32],
            )
            .unwrap();
            assert_eq!(
                serde_json::to_vec(&plan).unwrap(),
                serde_json::to_vec(&again).unwrap()
            );
            for stage in &plan.stages {
                let unique: std::collections::BTreeSet<_> =
                    stage.iter().map(|e| e.template).collect();
                assert_eq!(unique.len(), 3);
                for enemy in stage {
                    assert!(enemy.seat.bot_profile.is_some());
                    assert_eq!(enemy.seat.deck.len(), 20);
                    let setup = engine::DuelSetup {
                        rules: engine_content::rules(),
                        seed: [0; 32],
                        players: [plan.human.setup().unwrap(), enemy.seat.setup().unwrap()],
                    };
                    engine::new_duel(&setup, &engine_content::content()).unwrap();
                }
            }
            assert!(plan.enemy(plan.difficulty.stages, 1).is_err());
            assert!(plan.enemy(0, 3).is_err());
            assert_ne!(plan.duel_seed(0), plan.duel_seed(1));
        }
    }
    #[test]
    fn exclusive_rewards_never_choose_owned_junk_or_shade_templates() {
        for (avatar, card, currency) in [(10000, 0, 0), (0, 10000, 0), (0, 0, 10000), (0, 0, 0)] {
            let mut config = crate::expeditions::CONFIG.clone();
            config.avatar_chance_bps = avatar;
            config.card_chance_bps = card;
            config.ducats_chance_bps = currency;
            let plan = Plan::new(human(), "walk", &config, &[1], [18; 32]).unwrap();
            let award = plan.reward(&[1]);
            assert_eq!(award.template.is_some(), avatar != 0);
            assert_eq!(award.card.is_some(), card != 0);
            assert_eq!(award.ducats, if currency != 0 { 15 } else { 5 });
            if let Some(id) = award.template {
                assert_ne!(id, 1);
                assert_eq!(
                    engine_content::avatar(id).unwrap().rarity,
                    AvatarRarity::Common
                );
            }
            let all = engine_content::AVATARS
                .iter()
                .map(|a| i32::from(a.id))
                .collect::<Vec<_>>();
            assert!(plan.reward(&all).template.is_none());
        }
        let mut malformed = crate::expeditions::CONFIG.clone();
        malformed.avatar_chance_bps = u32::MAX;
        malformed.card_chance_bps = u32::MAX;
        assert!(malformed.validate().is_err());
    }
}
