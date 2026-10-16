//! Reproducible pacing audit using settlement's live XP policy; no database or network.
use anyhow::{Context as _, bail, ensure};
use serde::Serialize;
use server::duel::result::{level_for_xp, xp_award_using};
use server::rules::{Experience, RULES};

#[derive(Clone, Copy)]
struct Options {
    samples: u32,
    win: u32,
    draw: u32,
    seconds: u32,
    daily_minutes: u32,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            samples: 100,
            win: 5000,
            draw: 0,
            seconds: 90,
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
fn cycle(
    policy: &Experience,
    tier: i16,
    opponent_tier: i16,
    o: Options,
    seed: &mut u64,
) -> (u32, u32) {
    let mut xp = 0;
    let mut at_twenty = 0;
    for matches in 1..=2_000_000 {
        let level = level_for_xp(xp) as u32;
        let roll = random(seed) % 10000;
        let gained = xp_award_using(
            policy,
            level,
            tier,
            level,
            opponent_tier,
            roll < o.win,
            roll >= o.win && roll < o.win + o.draw,
        );
        xp = xp
            .saturating_add(gained as i32)
            .min(RULES.progression.reincarnation_xp);
        if at_twenty == 0 && xp >= RULES.progression.level_xp[19] {
            at_twenty = matches;
        }
        if xp >= RULES.progression.reincarnation_xp {
            return (at_twenty, matches);
        }
    }
    panic!("Pacing scenario did not reach reincarnation within two million matches")
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
fn milestone(name: &'static str, mut values: Vec<u32>, o: Options) -> Milestone {
    values.sort_unstable();
    let count = values.len();
    let rank = |percent: usize| values[(count * percent).div_ceil(100).saturating_sub(1)];
    let median = rank(50);
    let hours = f64::from(median) * f64::from(o.seconds) / 3600.0;
    Milestone {
        name,
        p10_matches: rank(10),
        median_matches: median,
        p90_matches: rank(90),
        median_hours: (hours * 10.0).round() / 10.0,
        median_days: (hours * 60.0 / f64::from(o.daily_minutes) * 10.0).round() / 10.0,
    }
}
fn career(policy: &Experience, o: Options, shades_face_base: bool) -> Vec<Milestone> {
    let mut columns: [Vec<u32>; 5] = std::array::from_fn(|_| Vec::new());
    for sample in 0..o.samples {
        let mut seed = u64::from(sample) ^ 0x93e72c4ab187f59d;
        let (twenty, first) = cycle(policy, 0, 0, o, &mut seed);
        let mut total = first;
        columns[0].push(twenty);
        columns[1].push(first);
        for tier in 1..=3 {
            total += cycle(policy, tier, tier, o, &mut seed).1;
            if tier == 2 {
                columns[2].push(total);
            }
        }
        columns[3].push(total);
        // Donors: one cycle for Adept, two for Master, three for Ascent.
        for donor in 1..=3 {
            for tier in 0..donor {
                total += cycle(policy, tier, tier, o, &mut seed).1;
            }
        }
        // Every shade cycle is valued as Ascent; its donor tier does not change that valuation.
        for _ in 0..3 {
            total += cycle(
                policy,
                3,
                if shades_face_base { 0 } else { 3 },
                o,
                &mut seed,
            )
            .1;
        }
        columns[4].push(total);
    }
    [
        "first_level_20",
        "first_reincarnation",
        "ascent_unlocked",
        "shade_created",
        "shade_ascent_including_all_donors",
    ]
    .into_iter()
    .zip(columns)
    .map(|(name, values)| milestone(name, values, o))
    .collect()
}
#[derive(Serialize)]
struct Report {
    content_version: String,
    xp_base: u32,
    samples: u32,
    win_basis_points: u32,
    draw_basis_points: u32,
    assumed_seconds_per_match: u32,
    assumed_daily_minutes: u32,
    verified_account: bool,
    equal_effective_tier: Vec<Milestone>,
    shade_facing_base_tier: Vec<Milestone>,
    daily_arena_ducats_max: u32,
    note: &'static str,
}
fn main() -> anyhow::Result<()> {
    let mut o = Options::default();
    let mut policy = RULES.xp.clone();
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    ensure!(args.len() % 2 == 0, "Expected --option value pairs");
    for pair in args.as_chunks::<2>().0 {
        let n = pair[1]
            .parse::<u32>()
            .context("Expected an unsigned integer")?;
        match pair[0].as_str() {
            "--samples" => o.samples = n,
            "--win-bps" => o.win = n,
            "--draw-bps" => o.draw = n,
            "--seconds" => o.seconds = n,
            "--daily-minutes" => o.daily_minutes = n,
            "--base" => policy.base = n,
            other => bail!("Unknown option {other}"),
        }
    }
    ensure!(
        (1..=1000).contains(&o.samples)
            && o.win <= 10000
            && o.draw <= 10000
            && o.win + o.draw <= 10000
            && o.win + o.draw >= 1000,
        "Invalid samples or win/draw rates; minimum rewarded outcome rate is 10%"
    );
    ensure!(
        (1..=3600).contains(&o.seconds)
            && (1..=1440).contains(&o.daily_minutes)
            && (1..=100000).contains(&policy.base),
        "Invalid time or XP assumptions"
    );
    let report = Report {
        content_version: engine_content::CONTENT_VERSION
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
        xp_base: policy.base,
        samples: o.samples,
        win_basis_points: o.win,
        draw_basis_points: o.draw,
        assumed_seconds_per_match: o.seconds,
        assumed_daily_minutes: o.daily_minutes,
        verified_account: true,
        equal_effective_tier: career(&policy, o, false),
        shade_facing_base_tier: career(&policy, o, true),
        daily_arena_ducats_max: RULES.economy.arena_first_win + RULES.economy.arena_win_daily_cap,
        note: "This simulates progression and sacrifices, not combat or real human decision time. Opponents have the player's current level; normal characters face equal tiers. Match duration, daily play and win/draw rates are explicit assumptions; no beta telemetry is claimed.",
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_wins_and_draws_use_the_live_thresholds_and_experience_formula() {
        let mut seed = 1;
        let o = Options {
            win: 10000,
            ..Options::default()
        };
        assert_eq!(cycle(&RULES.xp, 0, 0, o, &mut seed), (500, 650));
        let draws = Options {
            win: 0,
            draw: 10000,
            ..o
        };
        assert_eq!(cycle(&RULES.xp, 0, 0, draws, &mut seed), (1000, 1300));
        let report = career(&RULES.xp, Options { samples: 1, ..o }, false);
        assert_eq!(report[2].median_matches, 1950);
        assert_eq!(report[3].median_matches, 2600);
        assert_eq!(report[4].median_matches, 8450);
    }
}
