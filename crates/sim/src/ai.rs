//! Kryteria F4 dla algorytmów domyślnych oraz rzeczywistych profili treningowych i ich talii.

use std::io::{self, Write};
use std::time::Instant;

use engine::{
    Content, DuelSetup, DuelState, EventBuf, Outcome, Side, new_duel, resolve_round, view,
};
use engine_ai::{Bot, BotProfile, Level, Profiles, training_profiles};

struct Matchup<'a> {
    levels: [Level; 2],
    profiles: Option<[&'a BotProfile; 2]>,
    avatars: Vec<u16>,
}

impl<'a> Matchup<'a> {
    fn new(levels: [Level; 2], profiles: Option<&'a Profiles>) -> io::Result<Self> {
        let profiles = profiles
            .map(|profiles| {
                let find = |level| {
                    profiles
                        .bots
                        .iter()
                        .find(|profile| profile.level == level)
                        .ok_or_else(|| io::Error::other(format!("brak profilu {level:?}")))
                };
                Ok::<_, io::Error>([find(levels[0])?, find(levels[1])?])
            })
            .transpose()?;
        let mut avatars: Vec<u16> = match profiles {
            Some([a, b]) => a
                .avatar_pool
                .iter()
                .copied()
                .filter(|avatar| b.avatar_pool.contains(avatar))
                .collect(),
            None => engine_content::STARTERS
                .iter()
                .map(|starter| starter.avatar)
                .collect(),
        };
        avatars.sort_unstable();
        avatars.dedup();
        if avatars.is_empty() {
            return Err(io::Error::other(
                "brak wspólnych postaci do porównania profili",
            ));
        }
        Ok(Self {
            levels,
            profiles,
            avatars,
        })
    }

    fn duel(
        &self,
        game: u64,
        level: u8,
        seed: u64,
        content: &Content<'_>,
    ) -> io::Result<(DuelState, [Bot; 2], Side)> {
        let index = ((game / 2) % self.avatars.len() as u64) as usize;
        let avatar = self.avatars[index];
        let strong = if game.is_multiple_of(2) {
            Side::A
        } else {
            Side::B
        };
        let seeds =
            [0, 1].map(|offset| seed.wrapping_add(game.wrapping_mul(2)).wrapping_add(offset));
        let (mut players, mut bots) = match self.profiles {
            Some(profiles) => {
                let player = |profile: &BotProfile| {
                    let rotation = profile
                        .avatar_pool
                        .iter()
                        .position(|&id| id == avatar)
                        .ok_or_else(|| io::Error::other("postać spoza puli profilu"))?;
                    profile
                        .player(level, rotation)
                        .ok_or_else(|| io::Error::other("nie można zbudować talii profilu"))
                };
                (
                    [player(profiles[0])?, player(profiles[1])?],
                    [profiles[0].bot(seeds[0]), profiles[1].bot(seeds[1])],
                )
            }
            None => {
                let starter = engine_content::STARTERS
                    .iter()
                    .find(|starter| starter.avatar == avatar)
                    .ok_or_else(|| io::Error::other("brak zestawu startowego"))?;
                let avatar = engine_content::avatar(avatar)
                    .ok_or_else(|| io::Error::other("brak postaci"))?;
                let player = engine::PlayerSetup {
                    avatar: avatar.snapshot(level),
                    deck: starter.deck.iter().copied().collect(),
                };
                (
                    [player.clone(), player],
                    [
                        Bot::new(self.levels[0], seeds[0]),
                        Bot::new(self.levels[1], seeds[1]),
                    ],
                )
            }
        };
        if strong == Side::B {
            players.swap(0, 1);
            bots.swap(0, 1);
        }
        let state = new_duel(
            &DuelSetup {
                rules: engine_content::rules(),
                seed: crate::matrix::seed(index, index, seed.wrapping_add(game)),
                players,
            },
            content,
        )
        .map_err(io::Error::other)?;
        Ok((state, bots, strong))
    }
}

pub fn report(games: u64, level: u8, seed: u64, out: &mut impl Write) -> io::Result<bool> {
    if games == 0 || !(1..=20).contains(&level) {
        return Err(io::Error::other(
            "liczba walk musi być dodatnia, a poziom w zakresie 1–20",
        ));
    }
    let content = engine_content::content();
    let profiles = training_profiles();
    let mut ok = true;
    for configured in [false, true] {
        for (stronger, weaker, threshold) in
            [(Level::L2, Level::L1, 60), (Level::L3, Level::L2, 55)]
        {
            let matchup = Matchup::new([stronger, weaker], configured.then_some(&profiles))?;
            let (mut wins, mut draws, mut losses) = (0u64, 0u64, 0u64);
            let mut timings = Vec::new();
            let mut events = EventBuf::new();
            for game in 0..games {
                let (mut state, mut bots, side) = matchup.duel(game, level, seed, &content)?;
                while !state.is_finished() {
                    let moves = Side::BOTH.map(|player| {
                        let seen = view(&state, player);
                        let started = Instant::now();
                        let mv = bots[player.index()].choose(&seen, &content);
                        if player == side {
                            timings.push(started.elapsed().as_nanos());
                        }
                        mv
                    });
                    events.clear();
                    resolve_round(&mut state, &content, &moves, &mut events)
                        .expect("poprawne ruchy botów");
                    for side in Side::BOTH {
                        bots[side.index()].observe(&view(&state, side));
                    }
                }
                match state.outcome().expect("koniec walki") {
                    Outcome::Win { winner, .. } if winner == side => wins += 1,
                    Outcome::Win { .. } => losses += 1,
                    Outcome::Draw { .. } => draws += 1,
                }
            }
            timings.sort_unstable();
            let ms = |percent: usize| {
                timings
                    .get((timings.len() * percent / 100).min(timings.len().saturating_sub(1)))
                    .copied()
                    .unwrap_or(0) as f64
                    / 1e6
            };
            let rate = wins as f64 / games.max(1) as f64;
            // Wilson dla zwycięstw (remisy nie liczą się jako zwycięstwa).
            let n = games.max(1) as f64;
            let center = (rate + 1.9208 / n) / (1.0 + 3.8416 / n);
            let margin =
                1.96 * (rate * (1.0 - rate) / n + 0.9604 / (n * n)).sqrt() / (1.0 + 3.8416 / n);
            let strength_ok = wins * 100 >= games * threshold;
            let time_ok = stronger != Level::L3 || cfg!(debug_assertions) || ms(95) <= 50.0;
            let setup = matchup.profiles.map_or_else(
                || "domyślne boty, talie startowe".to_owned(),
                |[a, b]| format!("profile {}–{}, talie profili", a.id, b.id),
            );
            writeln!(
                out,
                "{stronger:?} przeciw {weaker:?} ({setup}): poziom {level}, {games} walk od ziarna {seed}; wygrane {wins}, remisy {draws}, porażki {losses}."
            )?;
            writeln!(
                out,
                "  Wygrane {:.1}% (95% CI {:.1}–{:.1}%), próg {threshold}%: {}.",
                rate * 100.0,
                (center - margin) * 100.0,
                (center + margin) * 100.0,
                if strength_ok { "OK" } else { "BŁĄD" }
            )?;
            writeln!(
                out,
                "  Decyzja: mediana {:.3} ms, p95 {:.3} ms, max {:.3} ms; {}.",
                ms(50),
                ms(95),
                ms(100),
                if cfg!(debug_assertions) {
                    "pomiar debug (limit sprawdzany w release)"
                } else if time_ok {
                    "limit OK"
                } else {
                    "przekroczono 50 ms"
                }
            )?;
            out.flush()?;
            ok &= strength_ok && time_ok;
        }
    }
    Ok(ok)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configured_matches_use_profile_decks_on_identical_avatars_on_both_sides() {
        let profiles = training_profiles();
        let content = engine_content::content();
        let matchup = Matchup::new([Level::L2, Level::L1], Some(&profiles)).unwrap();
        for game in 0..12 {
            let (state, bots, strong) = matchup.duel(game, 1, 7, &content).unwrap();
            assert_eq!(
                state.player(Side::A).base_stats(),
                state.player(Side::B).base_stats()
            );
            for (index, side) in [strong, strong.other()].into_iter().enumerate() {
                let profile = matchup.profiles.unwrap()[index];
                assert_eq!(bots[side.index()].level(), profile.level);
                let avatar = matchup.avatars[(game / 2) as usize % matchup.avatars.len()];
                let rotation = profile
                    .avatar_pool
                    .iter()
                    .position(|&id| id == avatar)
                    .unwrap();
                let mut expected = profile.player(1, rotation).unwrap().deck.to_vec();
                let mut actual: Vec<_> = state
                    .player(side)
                    .hand()
                    .iter()
                    .chain(state.player(side).deck())
                    .copied()
                    .collect();
                actual.sort_unstable();
                expected.sort_unstable();
                assert_eq!(actual, expected);
            }
        }
    }

    #[test]
    fn missing_comparable_profiles_and_empty_series_are_errors() {
        let mut profiles = training_profiles();
        assert!(report(0, 1, 0, &mut Vec::new()).is_err());
        assert!(report(1, 21, 0, &mut Vec::new()).is_err());
        profiles.bots.retain(|profile| profile.level != Level::L2);
        assert!(Matchup::new([Level::L2, Level::L1], Some(&profiles)).is_err());
        let mut profiles = training_profiles();
        profiles.bots[2].avatar_pool = vec![1];
        profiles.bots[1].avatar_pool = vec![2];
        assert!(Matchup::new([Level::L2, Level::L1], Some(&profiles)).is_err());
    }
}
