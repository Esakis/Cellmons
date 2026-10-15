//! Server parameters share the versioned content file with the engine.
use serde::Deserialize;
use std::sync::LazyLock;

#[derive(Deserialize)]
pub struct Rules {
    pub rating: Rating,
    pub xp: Experience,
    pub training: Training,
    pub unlock: Unlock,
    pub economy: Economy,
    pub junkyard: Junkyard,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rating {
    pub k: u32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Experience {
    pub base: u32,
    pub min: u32,
    pub max: u32,
    pub tier_mult_pct: [u32; 4],
    pub tier_add: [u32; 4],
    pub unverified_cap: i32,
    pub cap: i32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Training {
    pub bot_rotation_min: u32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Unlock {
    pub arena_chance_ppm: u32,
    pub newbie_wins: Vec<u32>,
    pub rarity_weights: [u32; 3],
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Economy {
    pub initial: u32,
    pub arena_first_win: u32,
    pub arena_win: u32,
    pub arena_win_daily_cap: u32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Junkyard {
    pub rarity_points: [u32; 3],
    pub reward_threshold: u32,
    pub fallback_ducats: u32,
}
pub static RULES: LazyLock<Rules> = LazyLock::new(|| {
    let rules: Rules = toml::from_str(include_str!("../../../content/rules.toml"))
        .expect("validated server rules");
    assert!(rules.rating.k > 0 && rules.rating.k <= 1000);
    assert!(
        rules.xp.base > 0
            && rules.xp.min > 0
            && rules.xp.min <= rules.xp.max
            && rules.xp.max <= 100_000
    );
    assert!(
        rules.xp.tier_mult_pct.iter().all(|v| *v > 0 && *v <= 1000)
            && rules.xp.tier_add.iter().all(|v| *v <= 100)
    );
    assert!(rules.xp.unverified_cap >= 0 && rules.xp.cap >= rules.xp.unverified_cap);
    assert!((1..=60).contains(&rules.training.bot_rotation_min));
    assert!(
        rules.unlock.arena_chance_ppm <= 1_000_000
            && rules.unlock.rarity_weights.iter().sum::<u32>() == 100
    );
    assert!(
        [
            rules.economy.initial,
            rules.economy.arena_first_win,
            rules.economy.arena_win,
            rules.economy.arena_win_daily_cap
        ]
        .iter()
        .all(|v| *v <= 1_000_000)
    );
    assert!(
        (1..=1_000_000).contains(&rules.junkyard.reward_threshold)
            && rules.junkyard.rarity_points.iter().all(|v| *v <= 10_000)
            && rules.junkyard.fallback_ducats <= 1_000_000
    );
    rules
});

#[cfg(test)]
mod tests {
    #[test]
    fn compiled_rules_are_valid() {
        std::sync::LazyLock::force(&super::RULES);
    }
}
