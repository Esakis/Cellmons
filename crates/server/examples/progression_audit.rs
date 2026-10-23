//! Reproducible pacing model for the released account/bronze/silver/gold progression.
//! This models qualifying online fights and successful casts, not measured human play time.
use anyhow::{Context as _, bail, ensure};
use serde::Serialize;

#[derive(Clone, Copy)]
struct Options {
    samples: u32,
    win: u32,
    seconds: u32,
    daily_minutes: u32,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            samples: 100,
            win: 5000,
            seconds: 180,
            daily_minutes: 30,
        }
    }
}
fn random(seed: &mut u64) -> u32 {
    *seed = seed.wrapping_add(0x9e3779b97f4a7c15);
    let mut n = *seed;
    n = (n ^ (n >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    n = (n ^ (n >> 27)).wrapping_mul(0x94d049bb133111eb);
    ((n ^ (n >> 31)) >> 32) as u32
}
fn career(o: Options, seed: &mut u64) -> [u32; 5] {
    // First completed training plus the introduction grant 250 total account XP.
    let (mut account, mut character, mut tier, mut wins) = (250_i64, 0_i32, 0_i16, 0_u32);
    let mut milestones = [1, 0, 0, 0, 0];
    for games in 1..=100000 {
        let ready = server::advancement::level_for_xp(character, tier) == 20;
        let won = random(seed) % 10000 < o.win;
        account += if won { 90 } else { 60 };
        character += if won { 120 } else { 80 };
        if tier == 0
            && milestones[1] == 0
            && server::advancement::level_for_xp(character, tier) == 20
        {
            milestones[1] = games + 1;
        }
        if tier < 2
            && ready
            && won
            && server::journey::level(account) >= if tier == 0 { 12 } else { 18 }
        {
            wins += 1;
            if wins == 3 {
                tier += 1;
                milestones[tier as usize + 1] = games + 1;
                character = 0;
                wins = 0;
            }
        }
        if tier == 2 && server::advancement::level_for_xp(character, tier) == 20 {
            milestones[4] = games + 1;
            return milestones;
        }
    }
    panic!("Scenario failed to reach gold level 20")
}
#[derive(Serialize)]
struct Milestone {
    name: &'static str,
    p10_matches: u32,
    median_matches: u32,
    p90_matches: u32,
    median_hours: f64,
    median_days: f64,
}
fn main() -> anyhow::Result<()> {
    let mut o = Options::default();
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    ensure!(
        args.len().is_multiple_of(2),
        "Expected --option value pairs"
    );
    for pair in args.as_chunks::<2>().0 {
        let n = pair[1]
            .parse::<u32>()
            .context("Expected unsigned integer")?;
        match pair[0].as_str() {
            "--samples" => o.samples = n,
            "--win-bps" => o.win = n,
            "--seconds" => o.seconds = n,
            "--daily-minutes" => o.daily_minutes = n,
            other => bail!("Unknown option {other}; the live XP policy is fixed at 80/120"),
        }
    }
    ensure!(
        (1..=1000).contains(&o.samples)
            && (1000..=10000).contains(&o.win)
            && (1..=3600).contains(&o.seconds)
            && (1..=1440).contains(&o.daily_minutes),
        "Invalid assumptions"
    );
    let mut columns: [Vec<u32>; 5] = std::array::from_fn(|_| Vec::new());
    for sample in 0..o.samples {
        for (column, value) in columns
            .iter_mut()
            .zip(career(o, &mut (u64::from(sample) ^ 0x93e72c4ab187f59d)))
        {
            column.push(value);
        }
    }
    let milestones: Vec<_> = [
        "arena",
        "bronze_level_20",
        "silver",
        "gold",
        "gold_level_20",
    ]
    .into_iter()
    .zip(columns)
    .map(|(name, mut values)| {
        values.sort_unstable();
        let rank =
            |percent: usize| values[(values.len() * percent).div_ceil(100).saturating_sub(1)];
        let hours = f64::from(rank(50)) * f64::from(o.seconds) / 3600.0;
        Milestone {
            name,
            p10_matches: rank(10),
            median_matches: rank(50),
            p90_matches: rank(90),
            median_hours: (hours * 10.0).round() / 10.0,
            median_days: (hours * 60.0 / f64::from(o.daily_minutes) * 10.0).round() / 10.0,
        }
    })
    .collect();
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "samples":o.samples,"win_basis_points":o.win,"assumed_seconds_per_match":o.seconds,"assumed_daily_minutes":o.daily_minutes,"milestones":milestones,
            "note":"Model, not telemetry. Verified account; every online fight qualifies and uses a successful spell. Includes one introductory training; excludes queue and menu time. Trials require three wins. No shades or third incarnation."
        }))?
    );
    Ok(())
}
#[cfg(test)]
mod tests {
    #[test]
    fn all_wins_respect_account_gates_and_three_wins_after_level_twenty() {
        assert_eq!(
            super::career(
                super::Options {
                    win: 10000,
                    ..Default::default()
                },
                &mut 1
            ),
            [1, 51, 54, 134, 234]
        );
    }
}
