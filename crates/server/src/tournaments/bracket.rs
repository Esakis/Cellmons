//! R-TUR-04/05/08: deterministic private-seed pairing, series, clan totals and byes.
use crate::online::{Error, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Team {
    pub id: Uuid,
    pub users: Vec<Uuid>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Lane {
    pub users: [Uuid; 2],
    pub points: [u16; 2],
    pub fought: u8,
    pub active: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pair {
    pub teams: [Option<Uuid>; 2],
    pub lanes: Vec<Lane>,
    pub resolved: bool,
    pub winner: Option<Uuid>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Round {
    pub pairs: Vec<Pair>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fight {
    pub round: u8,
    pub pair: u16,
    pub lane: u8,
    pub game: u8,
    pub users: [Uuid; 2],
}
#[derive(Clone, Copy, Debug)]
pub enum Score {
    Draw,
    First,
    Second,
    /// Both missed the grace period: no duel, XP or rating; both leave the tournament.
    BothAbsent,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Bracket {
    pub teams: Vec<Team>,
    pub rounds: Vec<Round>,
    pub max_games: u8,
    pub departed: BTreeSet<Uuid>,
    pub finished: bool,
    pub champion: Option<Uuid>,
    seed: [u8; 32],
    counter: u64,
}
impl Bracket {
    pub fn new(teams: Vec<Team>, max_games: u8, seed: [u8; 32]) -> Result<Self> {
        let size = teams.first().map_or(0, |t| t.users.len());
        let users: BTreeSet<_> = teams.iter().flat_map(|t| t.users.iter()).collect();
        let ids: BTreeSet<_> = teams.iter().map(|t| t.id).collect();
        if !(2..=64).contains(&teams.len())
            || !(1..=16).contains(&size)
            || !(1..=9).contains(&max_games)
            || teams.iter().any(|t| t.users.len() != size)
            || teams.len() * size > 256
            || users.len() != teams.len() * size
            || ids.len() != teams.len()
        {
            return Err(Error("INVALID_MESSAGE"));
        }
        let mut bracket = Self {
            teams,
            rounds: vec![],
            max_games,
            departed: BTreeSet::new(),
            finished: false,
            champion: None,
            seed,
            counter: 0,
        };
        let entrants = bracket.teams.iter().map(|t| t.id).collect();
        bracket.next_round(entrants);
        Ok(bracket)
    }
    // Rejection sampling avoids modulo bias. This stream never leaves server storage.
    fn random(&mut self, upper: usize) -> usize {
        let upper = upper as u64;
        let threshold = upper.wrapping_neg() % upper;
        loop {
            let mut hash = Sha256::new();
            hash.update(b"spellgambit-tournament-v1");
            hash.update(self.seed);
            hash.update(self.counter.to_le_bytes());
            self.counter = self.counter.checked_add(1).expect("bounded bracket RNG");
            let bytes = hash.finalize();
            let value = u64::from_le_bytes(bytes[..8].try_into().expect("digest"));
            if value >= threshold {
                return (value % upper) as usize;
            }
        }
    }
    fn shuffle<T>(&mut self, values: &mut [T]) {
        for i in (1..values.len()).rev() {
            let j = self.random(i + 1);
            values.swap(i, j);
        }
    }
    fn next_round(&mut self, mut entrants: Vec<Uuid>) {
        entrants.retain(|id| {
            self.teams
                .iter()
                .find(|t| t.id == *id)
                .expect("team")
                .users
                .iter()
                .any(|u| !self.departed.contains(u))
        });
        if entrants.len() <= 1 {
            self.finished = true;
            self.champion = entrants.first().copied();
            return;
        }
        self.shuffle(&mut entrants);
        let byes = entrants.len().next_power_of_two() - entrants.len();
        let mut pairings: Vec<_> = entrants[..byes].iter().map(|t| [Some(*t), None]).collect();
        pairings.extend(
            entrants[byes..]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|p| [Some(p[0]), Some(p[1])]),
        );
        self.shuffle(&mut pairings);
        let mut pairs = Vec::new();
        for teams in pairings {
            let mut lanes = vec![];
            if let [Some(a), Some(b)] = teams {
                let mut first = self
                    .teams
                    .iter()
                    .find(|t| t.id == a)
                    .expect("team")
                    .users
                    .clone();
                let mut second = self
                    .teams
                    .iter()
                    .find(|t| t.id == b)
                    .expect("team")
                    .users
                    .clone();
                self.shuffle(&mut first);
                self.shuffle(&mut second);
                lanes = first
                    .into_iter()
                    .zip(second)
                    .map(|(a, b)| Lane {
                        users: [a, b],
                        points: [0, 0],
                        fought: 0,
                        active: false,
                    })
                    .collect();
            }
            pairs.push(Pair {
                teams,
                resolved: lanes.is_empty(),
                winner: if lanes.is_empty() {
                    teams[0].or(teams[1])
                } else {
                    None
                },
                lanes,
            });
        }
        self.rounds.push(Round { pairs });
    }
    pub fn withdraw(&mut self, user: Uuid) -> Result<()> {
        if !self.teams.iter().any(|t| t.users.contains(&user)) {
            return Err(Error("FORBIDDEN"));
        }
        self.departed.insert(user);
        Ok(())
    }
    /// Reserve one batch. A clan cannot advance while an already reserved fight is pending.
    pub fn plan(&mut self) -> Vec<Fight> {
        if self.finished {
            return vec![];
        }
        let round = (self.rounds.len() - 1) as u8;
        let mut fights = vec![];
        for (pair_index, pair) in self
            .rounds
            .last_mut()
            .expect("round")
            .pairs
            .iter_mut()
            .enumerate()
        {
            if pair.resolved || pair.lanes.iter().any(|l| l.active) {
                continue;
            }
            for (lane_index, lane) in pair.lanes.iter_mut().enumerate() {
                if lane.fought < self.max_games {
                    lane.active = true;
                    fights.push(Fight {
                        round,
                        pair: pair_index as u16,
                        lane: lane_index as u8,
                        game: lane.fought,
                        users: lane.users,
                    });
                }
            }
        }
        fights
    }
    pub fn score(&mut self, fight: Fight, score: Score) -> Result<()> {
        if self.finished || usize::from(fight.round) + 1 != self.rounds.len() {
            return Err(Error("CONFLICT"));
        }
        let pair = self
            .rounds
            .last_mut()
            .and_then(|r| r.pairs.get_mut(usize::from(fight.pair)))
            .ok_or(Error("CONFLICT"))?;
        let lane = pair
            .lanes
            .get_mut(usize::from(fight.lane))
            .ok_or(Error("CONFLICT"))?;
        if pair.resolved || !lane.active || lane.fought != fight.game || lane.users != fight.users {
            return Err(Error("CONFLICT"));
        }
        lane.active = false;
        lane.fought += 1;
        match score {
            Score::Draw => {
                lane.points[0] += 1;
                lane.points[1] += 1;
            }
            Score::First => lane.points[0] += 2,
            Score::Second => lane.points[1] += 2,
            Score::BothAbsent => {
                self.departed.extend(lane.users);
                lane.fought = self.max_games;
            }
        }
        self.resolve();
        Ok(())
    }
    fn resolve(&mut self) {
        let count = self.rounds.last().expect("round").pairs.len();
        for index in 0..count {
            let pair = &self.rounds.last().expect("round").pairs[index];
            if pair.resolved || pair.lanes.iter().any(|l| l.active) {
                continue;
            }
            let points = pair.lanes.iter().fold([0_u32; 2], |mut p, l| {
                p[0] += u32::from(l.points[0]);
                p[1] += u32::from(l.points[1]);
                p
            });
            let remaining: u32 = pair
                .lanes
                .iter()
                .map(|l| u32::from(self.max_games - l.fought) * 2)
                .sum();
            if remaining != 0 && points[0].abs_diff(points[1]) <= remaining {
                continue;
            }
            let eligible = pair.teams.map(|id| {
                id.is_some_and(|id| {
                    self.teams
                        .iter()
                        .find(|t| t.id == id)
                        .expect("team")
                        .users
                        .iter()
                        .any(|u| !self.departed.contains(u))
                })
            });
            let winner = match eligible {
                [false, false] => None,
                [true, false] => pair.teams[0],
                [false, true] => pair.teams[1],
                [true, true] if points[0] > points[1] => pair.teams[0],
                [true, true] if points[1] > points[0] => pair.teams[1],
                [true, true] => {
                    let teams = pair.teams;
                    teams[self.random(2)]
                }
            };
            let pair = &mut self.rounds.last_mut().expect("round").pairs[index];
            pair.winner = winner;
            pair.resolved = true;
        }
        let round = self.rounds.last().expect("round");
        if round.pairs.iter().all(|p| p.resolved) {
            let winners = round.pairs.iter().filter_map(|p| p.winner).collect();
            self.next_round(winners);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn teams(n: usize, size: usize) -> Vec<Team> {
        (0..n)
            .map(|i| Team {
                id: Uuid::from_u128(i as u128 + 1),
                users: (0..size)
                    .map(|j| Uuid::from_u128(1000 + (i * size + j) as u128))
                    .collect(),
            })
            .collect()
    }
    #[test]
    fn random_byes_and_reproducible_complete_brackets() {
        for n in 2..=64 {
            let mut b = Bracket::new(teams(n, 1), 3, [n as u8; 32]).unwrap();
            assert_eq!(
                b.rounds[0]
                    .pairs
                    .iter()
                    .filter(|p| p.lanes.is_empty())
                    .count(),
                n.next_power_of_two() - n
            );
            let mut restored: Bracket =
                serde_json::from_slice(&serde_json::to_vec(&b).unwrap()).unwrap();
            let mut games = 0;
            while !b.finished {
                let batch = b.plan();
                assert!(!batch.is_empty());
                assert_eq!(batch, restored.plan());
                for fight in batch {
                    b.score(fight, Score::First).unwrap();
                    restored.score(fight, Score::First).unwrap();
                    games += 1;
                }
            }
            assert_eq!(games, 2 * (n - 1));
            assert!(b.champion.is_some());
            assert_eq!(b.champion, restored.champion);
        }
    }
    #[test]
    fn clan_waits_for_reserved_games_and_advances_losing_members() {
        let original = teams(2, 4);
        let mut b = Bracket::new(original.clone(), 3, [7; 32]).unwrap();
        for _ in 0..2 {
            let batch = b.plan();
            assert_eq!(batch.len(), 4);
            for (i, fight) in batch.into_iter().enumerate() {
                b.score(fight, if i == 0 { Score::Second } else { Score::First })
                    .unwrap();
                assert!(!b.finished);
            }
        }
        let batch = b.plan();
        assert_eq!(batch.len(), 4);
        for (i, fight) in batch.into_iter().enumerate() {
            b.score(fight, if i == 0 { Score::Second } else { Score::First })
                .unwrap();
        }
        assert!(b.finished);
        let champion = b.champion.unwrap();
        assert_eq!(
            b.teams
                .iter()
                .find(|t| t.id == champion)
                .unwrap()
                .users
                .len(),
            4
        );
    }
    #[test]
    fn absent_both_gives_no_champion_and_duplicate_results_are_rejected() {
        let mut b = Bracket::new(teams(2, 1), 3, [0; 32]).unwrap();
        let fight = b.plan()[0];
        b.score(fight, Score::BothAbsent).unwrap();
        assert!(b.finished);
        assert_eq!(b.champion, None);
        assert!(b.score(fight, Score::Draw).is_err());
    }
    #[test]
    fn draws_play_entire_series_and_private_tiebreak_varies() {
        let mut winners = BTreeSet::new();
        for seed in 0..64 {
            let mut b = Bracket::new(teams(2, 1), 3, [seed; 32]).unwrap();
            for _ in 0..3 {
                let fight = b.plan()[0];
                b.score(fight, Score::Draw).unwrap();
            }
            assert!(b.finished);
            winners.insert(b.champion);
        }
        assert_eq!(winners.len(), 2);
    }
    #[test]
    fn departed_bye_team_cannot_win_when_both_other_players_are_absent() {
        let mut b = Bracket::new(teams(3, 1), 3, [4; 32]).unwrap();
        let bye = b.rounds[0]
            .pairs
            .iter()
            .find(|p| p.lanes.is_empty())
            .unwrap()
            .winner
            .unwrap();
        let user = b.teams.iter().find(|t| t.id == bye).unwrap().users[0];
        b.withdraw(user).unwrap();
        let fight = b.plan()[0];
        b.score(fight, Score::BothAbsent).unwrap();
        assert!(b.finished);
        assert_eq!(b.champion, None);
    }
}
