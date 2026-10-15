//! Reproducible roster audit with varied legal decks, independent of the six starter decks.
use engine::{DuelSetup, EventBuf, Outcome, PlayerSetup, Side, new_duel, resolve_round, view};
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
fn player(def: &AvatarDef, level: u8, seed: &mut u64) -> PlayerSetup {
    let avatar = def.snapshot(level);
    let content = engine_content::content();
    let mut pool: Vec<_> = content
        .cards()
        .iter()
        .filter(|c| avatar.schools.meets(&c.requires))
        .map(|c| c.id)
        .collect();
    for i in (1..pool.len()).rev() {
        let j = (random(seed) % ((i + 1) as u64)) as usize;
        pool.swap(i, j);
    }
    let rules = engine_content::rules();
    let mut player = PlayerSetup {
        avatar,
        deck: Default::default(),
    };
    for _ in 0..rules.deck.max_copies {
        for id in &pool {
            if player.deck.len() == usize::from(rules.deck.size) {
                return player;
            }
            player.deck.push(*id);
        }
    }
    assert_eq!(
        player.deck.len(),
        usize::from(rules.deck.size),
        "{} has insufficient legal cards",
        def.key
    );
    player
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let games = std::env::args()
        .nth(1)
        .map_or(Ok(200), |s| s.parse::<u64>())?;
    assert!((1..=100_000).contains(&games));
    let avatars: Vec<_> = AVATARS
        .iter()
        .filter(|a| a.rarity != AvatarRarity::Shade)
        .collect();
    let content = engine_content::content();
    let mut events = EventBuf::new();
    fs::create_dir_all("target")?;
    let mut csv = fs::File::create("target/roster-balance.csv")?;
    writeln!(csv, "level,avatar,opponent,wins,draws,losses")?;
    println!(
        "Roster audit: {games} games per unordered pair, L1, levels 1/10/20, varied legal decks, alternating sides, no PR/path/gift bonuses."
    );
    for level in [1, 10, 20] {
        let mut totals = vec![[0u64; 3]; avatars.len()];
        for a in 0..avatars.len() {
            for b in a + 1..avatars.len() {
                let mut result = [0u64; 3];
                for game in 0..games {
                    let mut rng =
                        game ^ ((a as u64) << 32) ^ ((b as u64) << 40) ^ ((level as u64) << 48);
                    let swapped = game % 2 == 1;
                    let pair = if swapped { [b, a] } else { [a, b] };
                    let players = pair.map(|i| player(avatars[i], level, &mut rng));
                    let mut seed = [0; 32];
                    for part in seed.chunks_mut(8) {
                        part.copy_from_slice(&random(&mut rng).to_le_bytes());
                    }
                    let mut state = new_duel(
                        &DuelSetup {
                            rules: engine_content::rules(),
                            seed,
                            players,
                        },
                        &content,
                    )?;
                    let mut bots = [
                        Bot::new(Level::L1, random(&mut rng)),
                        Bot::new(Level::L1, random(&mut rng)),
                    ];
                    while !state.is_finished() {
                        let moves = Side::BOTH
                            .map(|side| bots[side.index()].choose(&view(&state, side), &content));
                        events.clear();
                        resolve_round(&mut state, &content, &moves, &mut events)?;
                        for side in Side::BOTH {
                            bots[side.index()].observe(&view(&state, side));
                        }
                    }
                    let mine = if swapped { Side::B } else { Side::A };
                    match state.outcome().expect("finished") {
                        Outcome::Win { winner, .. } if winner == mine => result[0] += 1,
                        Outcome::Win { .. } => result[2] += 1,
                        Outcome::Draw { .. } => result[1] += 1,
                    }
                }
                writeln!(
                    csv,
                    "{level},{},{},{},{},{}",
                    avatars[a].key, avatars[b].key, result[0], result[1], result[2]
                )?;
                for i in 0..3 {
                    totals[a][i] += result[i];
                    totals[b][2 - i] += result[i];
                }
            }
        }
        for (avatar, total) in avatars.iter().zip(totals) {
            let rate =
                (total[0] as f64 + total[1] as f64 / 2.0) / (total.iter().sum::<u64>() as f64);
            println!(
                "level {level:2} {:24} {:5.1}% (W/D/L {}/{}/{}){}",
                avatar.key,
                rate * 100.0,
                total[0],
                total[1],
                total[2],
                if !(0.35..=0.65).contains(&rate) {
                    " REVIEW"
                } else {
                    ""
                }
            );
        }
    }
    Ok(())
}
