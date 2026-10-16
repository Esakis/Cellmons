//! L2 holdout audit after a seeded joint search of legal decks and development points.
//! This is a reproducible search, not a claim that a deck or point allocation is globally optimal.
use engine::{
    AvatarSnapshot, CardId, DuelSetup, EventBuf, Outcome, PlayerSetup, Side, new_duel,
    resolve_round, view,
};
use engine_ai::{Bot, Level};
use engine_content::{AVATARS, AvatarDef, AvatarRarity};
use std::{fs, io::Write as _};

fn random(seed: &mut u64) -> u64 {
    *seed = seed.wrapping_add(0x9e3779b97f4a7c15);
    let mut value = *seed;
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
    value ^ (value >> 31)
}
#[derive(Clone)]
struct Build {
    points: [u8; 6],
    deck: Vec<CardId>,
}
fn available(def: &AvatarDef, level: u8) -> u8 {
    def.dev_points
        .iter()
        .filter(|(at, _)| *at <= level)
        .map(|(_, n)| *n)
        .sum()
}
fn snapshot(def: &AvatarDef, level: u8, points: [u8; 6]) -> AvatarSnapshot {
    let mut avatar = def.snapshot(level);
    avatar.stats.phys_atk += i32::from(points[0]);
    avatar.stats.phys_def += i32::from(points[1]);
    avatar.stats.mag_atk += i32::from(points[2]);
    avatar.stats.mag_def += i32::from(points[3]);
    avatar.stats.hp_max += 7 * i32::from(points[4]);
    avatar.stats.stamina_max += 7 * i32::from(points[5]);
    avatar
}
fn player(def: &AvatarDef, level: u8, build: &Build) -> PlayerSetup {
    assert_eq!(build.points.iter().sum::<u8>(), available(def, level));
    assert_eq!(build.deck.len(), 20);
    let avatar = snapshot(def, level, build.points);
    assert!(build.deck.iter().all(|id| {
        engine_content::content()
            .get(*id)
            .is_some_and(|c| avatar.schools.meets(&c.requires))
            && build.deck.iter().filter(|c| *c == id).count() <= 4
    }));
    PlayerSetup {
        avatar,
        deck: build.deck.iter().copied().collect(),
    }
}
fn pool(def: &AvatarDef, level: u8) -> Vec<CardId> {
    engine_content::content()
        .cards()
        .iter()
        .filter(|c| def.snapshot(level).schools.meets(&c.requires))
        .map(|c| c.id)
        .collect()
}
fn random_build(def: &AvatarDef, level: u8, rng: &mut u64) -> Build {
    let mut pool = pool(def, level);
    let mut deck = Vec::new();
    assert!(pool.len() >= 5);
    for i in (1..pool.len()).rev() {
        let j = random(rng) as usize % (i + 1);
        pool.swap(i, j);
    }
    for _ in 0..4 {
        for id in &pool {
            if deck.len() < 20 {
                deck.push(*id);
            }
        }
    }
    let mut points = [0; 6];
    for _ in 0..available(def, level) {
        points[random(rng) as usize % 6] += 1;
    }
    Build { points, deck }
}
fn game(players: [PlayerSetup; 2], seed: u64) -> u16 {
    let mut rng = seed;
    let mut bytes = [0; 32];
    for part in bytes.chunks_mut(8) {
        part.copy_from_slice(&random(&mut rng).to_le_bytes());
    }
    let content = engine_content::content();
    let mut state = new_duel(
        &DuelSetup {
            rules: engine_content::rules(),
            seed: bytes,
            players,
        },
        &content,
    )
    .expect("legal search player");
    let mut bots = [
        Bot::new(Level::L2, random(&mut rng)),
        Bot::new(Level::L2, random(&mut rng)),
    ];
    let mut events = EventBuf::new();
    while !state.is_finished() {
        let moves = Side::BOTH.map(|s| bots[s.index()].choose(&view(&state, s), &content));
        events.clear();
        resolve_round(&mut state, &content, &moves, &mut events).expect("legal bot move");
        for side in Side::BOTH {
            bots[side.index()].observe(&view(&state, side));
        }
    }
    match state.outcome().expect("finished") {
        Outcome::Win {
            winner: Side::A, ..
        } => 2,
        Outcome::Draw { .. } => 1,
        _ => 0,
    }
}
fn fitness(own: PlayerSetup, opponents: &[PlayerSetup]) -> u32 {
    opponents
        .iter()
        .enumerate()
        .map(|(i, opponent)| {
            let seed = 0x18473a10 + (i as u64) * 47;
            u32::from(game([own.clone(), opponent.clone()], seed)) + 2
                - u32::from(game([opponent.clone(), own.clone()], seed))
        })
        .sum()
}
fn optimize(def: &AvatarDef, level: u8, opponents: &[PlayerSetup], iterations: usize) -> Build {
    let mut rng = 0x6e251c4b ^ (u64::from(def.id) << 16) ^ u64::from(level);
    let legal = pool(def, level);
    let mut best = random_build(def, level, &mut rng);
    let mut score = fitness(player(def, level, &best), opponents);
    for iteration in 0..iterations {
        let mut trial = if iteration % 8 == 0 {
            random_build(def, level, &mut rng)
        } else {
            best.clone()
        };
        if available(def, level) > 0 && iteration % 2 == 0 {
            let occupied: Vec<_> = (0..6).filter(|i| trial.points[*i] > 0).collect();
            let from = occupied[random(&mut rng) as usize % occupied.len()];
            let to = random(&mut rng) as usize % 6;
            trial.points[from] -= 1;
            trial.points[to] += 1;
        }
        let slot = random(&mut rng) as usize % 20;
        let id = legal[random(&mut rng) as usize % legal.len()];
        if trial.deck.iter().filter(|c| **c == id).count() < 4 {
            trial.deck[slot] = id;
        }
        let value = fitness(player(def, level, &trial), opponents);
        if value > score {
            best = trial;
            score = value;
        }
    }
    best
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let games = args.first().map_or(Ok(100), |n| n.parse::<u64>())?;
    let iterations = args.get(1).map_or(Ok(24), |n| n.parse::<usize>())?;
    assert!(args.len() <= 2 && (2..=10000).contains(&games) && (1..=1000).contains(&iterations));
    let defs: Vec<_> = AVATARS
        .iter()
        .filter(|a| a.rarity != AvatarRarity::Shade)
        .collect();
    fs::create_dir_all("target")?;
    let mut csv = fs::File::create("target/roster-optimized.csv")?;
    let mut builds = fs::File::create("target/roster-optimized-builds.csv")?;
    writeln!(csv, "level,avatar,opponent,wins,draws,losses")?;
    writeln!(builds, "level,avatar,points,deck")?;
    println!(
        "L2, {games} holdout games per pair, {iterations} seeded search trials, legal innate-school cards and all earned PR; no paths/specs/gifts. Training seeds are disjoint from holdout seeds."
    );
    for level in [1, 10, 20] {
        // Twelve fixed training opponents span six schools; search never sees the evaluation matches.
        let mut rng = 0x5ca6719 + u64::from(level);
        let opponents: Vec<_> = (0..12)
            .map(|i| {
                let def = defs[(i * 7) % defs.len()];
                player(def, level, &random_build(def, level, &mut rng))
            })
            .collect();
        let players: Vec<_> = defs
            .iter()
            .map(|def| {
                let build = optimize(def, level, &opponents, iterations);
                writeln!(
                    builds,
                    "{level},{},\"{:?}\",\"{:?}\"",
                    def.key, build.points, build.deck
                )
                .expect("build report");
                player(def, level, &build)
            })
            .collect();
        let mut totals = vec![[0u64; 3]; defs.len()];
        for a in 0..defs.len() {
            for b in a + 1..defs.len() {
                let mut result = [0u64; 3];
                for n in 0..games {
                    let swapped = n % 2 == 1;
                    let pair = if swapped { [b, a] } else { [a, b] };
                    let value = game(
                        pair.map(|i| players[i].clone()),
                        0x9bd7e531_00000000
                            ^ ((level as u64) << 48)
                            ^ ((a as u64) << 32)
                            ^ ((b as u64) << 24)
                            ^ n,
                    );
                    let value = if swapped { 2 - value } else { value };
                    result[usize::from(2 - value)] += 1;
                }
                writeln!(
                    csv,
                    "{level},{},{},{},{},{}",
                    defs[a].key, defs[b].key, result[0], result[1], result[2]
                )?;
                for i in 0..3 {
                    totals[a][i] += result[i];
                    totals[b][2 - i] += result[i];
                }
            }
        }
        println!("Level {level}:");
        for (def, row) in defs.iter().zip(totals) {
            let n = row.iter().sum::<u64>();
            let score = (2 * row[0] + row[1]) * 1000 / (2 * n);
            println!(
                "{}: {}.{}% ({} / {} / {}){}",
                def.key,
                score / 10,
                score % 10,
                row[0],
                row[1],
                row[2],
                if !(400..=600).contains(&score) {
                    " REVIEW"
                } else {
                    ""
                }
            );
        }
    }
    Ok(())
}
