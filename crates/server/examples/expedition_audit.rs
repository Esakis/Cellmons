//! F11.3 acceptance: genuine L3 wins, using public opponent statistics to choose each stage.
use engine::{DuelSetup, EventBuf, Outcome, PlayerSetup, Side};
use engine_ai::{Bot, Level};
use protocol::game::v1 as pb;
use server::{
    duel::record::Seat,
    expeditions::{CONFIG, model::Plan},
};

fn human(base: u16, variant: u8) -> Seat {
    let path = engine_content::PATHS
        .iter()
        .find(|p| p.base_avatar == base && p.variant == variant)
        .unwrap();
    let mut profile = engine_ai::training_profiles()
        .bots
        .into_iter()
        .find(|p| p.id == "archmage")
        .unwrap();
    profile.avatar_pool = vec![path.avatar.id];
    let mut player = profile.player(20, 0).unwrap();
    player.avatar.specialization = Some(path.specializations[0]);
    player.avatar.gift = Some(path.gifts[0]);
    let points: u16 = path
        .avatar
        .dev_points
        .iter()
        .map(|(_, n)| u16::from(*n))
        .sum::<u16>()
        + server::rules::RULES.progression.reincarnation_bonus_dp[2] as u16;
    for i in 0..points {
        match i % 4 {
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
        login: "L3 acceptance".into(),
        template_id: u32::from(base),
        path_id: u32::from(path.avatar.id),
        tier: 3,
        level: 20,
        gift_id: u32::from(path.gifts[0]),
        specialization: engine_content::specialization_key(path.specializations[0]).into(),
        ..Default::default()
    };
    Seat::from_setup(None, None, &player, &public, None)
}
fn game(players: [PlayerSetup; 2], seed: [u8; 32], npc: &str) -> bool {
    let content = engine_content::content();
    let mut state = engine::new_duel(
        &DuelSetup {
            rules: engine_content::rules(),
            seed,
            players,
        },
        &content,
    )
    .unwrap();
    let profiles = engine_ai::training_profiles();
    let profile = profiles.bots.iter().find(|p| p.id == npc).unwrap();
    let value = u64::from_le_bytes(seed[..8].try_into().unwrap());
    let mut bots = [
        Bot::new(Level::L3, value ^ 0x9928),
        profile.bot(value ^ 0x6144),
    ];
    while !state.is_finished() {
        let moves = Side::BOTH.map(|s| bots[s.index()].choose(&engine::view(&state, s), &content));
        engine::resolve_round(&mut state, &content, &moves, &mut EventBuf::new()).unwrap();
        for side in Side::BOTH {
            bots[side.index()].observe(&engine::view(&state, side));
        }
    }
    matches!(
        state.outcome(),
        Some(Outcome::Win {
            winner: Side::A,
            ..
        })
    )
}
fn main() {
    for difficulty in ["walk", "expedition", "ordeal"] {
        let mut success = None;
        'build: for (base, variant) in [(14, 0), (16, 0), (1, 1), (22, 1)] {
            let player = human(base, variant);
            for seed in 0_u64..64 {
                let mut bytes = [0; 32];
                bytes[..8].copy_from_slice(&seed.to_le_bytes());
                let plan = Plan::new(player.clone(), difficulty, &CONFIG, &[], bytes).unwrap();
                let mut wins = 0;
                for stage in 0..plan.total_stages() as u8 {
                    let enemy = if stage as usize == plan.stages.len() {
                        &plan.boss
                    } else {
                        plan.stages[stage as usize]
                            .iter()
                            .min_by_key(|e| {
                                let s = e.seat.stats;
                                s[0] + s[1] + s[2] + s[3] + s[4] / 7 + s[5] / 7
                            })
                            .unwrap()
                    };
                    if !game(
                        [player.setup().unwrap(), enemy.seat.setup().unwrap()],
                        plan.duel_seed(stage),
                        enemy.seat.bot.as_deref().unwrap(),
                    ) {
                        break;
                    }
                    wins += 1;
                }
                if wins == plan.total_stages() {
                    success = Some((base, variant, seed, wins, player.clone()));
                    break 'build;
                }
            }
        }
        let (base, variant, seed, wins, player) = success.unwrap_or_else(|| {
            panic!("no genuine L3 completion for {difficulty} within the audit budget")
        });
        println!(
            "{difficulty}: completed {wins} stages, base={base}, variant={variant}, level=20, tier=3, seed={seed}, deck={:?}, gift={:?}, specialization={}",
            player.deck, player.gift, player.specialization
        );
    }
}
