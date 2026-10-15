//! Durable inputs, never an engine memory dump and never sent to a client.
use super::{Error, Result, Settings};
use engine::{AvatarSnapshot, DuelSetup, EventBuf, PlayerSetup, Side, Specialization, Stats};
use protocol::{Message as _, game::v1 as pb};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize)]
pub struct Seat {
    pub user: Option<Uuid>,
    pub avatar: Option<Uuid>,
    pub profile: Vec<u8>,
    pub stats: [i32; 6],
    pub schools: [u8; 6],
    pub specialization: u8,
    pub gift: Option<u16>,
    pub deck: Vec<u16>,
    pub bot: Option<String>,
}
impl Seat {
    pub fn player(
        user: Uuid,
        avatar: Uuid,
        setup: &PlayerSetup,
        profile: &pb::PublicPlayer,
    ) -> Self {
        Self::from_setup(Some(user), Some(avatar), setup, profile, None)
    }
    pub fn from_setup(
        user: Option<Uuid>,
        avatar: Option<Uuid>,
        setup: &PlayerSetup,
        profile: &pb::PublicPlayer,
        bot: Option<String>,
    ) -> Self {
        Self {
            user,
            avatar,
            profile: profile.encode_to_vec(),
            stats: engine::Stat::ALL.map(|stat| setup.avatar.stats.get(stat)),
            schools: setup.avatar.schools.0,
            specialization: match setup.avatar.specialization {
                None => 0,
                Some(Specialization::School(school)) => {
                    engine::SCHOOLS
                        .iter()
                        .position(|s| *s == school)
                        .expect("school") as u8
                        + 1
                }
                Some(Specialization::Essence) => 7,
                Some(Specialization::Concentration) => 8,
            },
            gift: setup.avatar.gift,
            deck: setup.deck.to_vec(),
            bot,
        }
    }
    pub fn setup(&self) -> Result<PlayerSetup> {
        let [phys_atk, phys_def, mag_atk, mag_def, hp_max, stamina_max] = self.stats;
        let specialization = match self.specialization {
            0 => None,
            1..=6 => Some(Specialization::School(
                engine::SCHOOLS[usize::from(self.specialization - 1)],
            )),
            7 => Some(Specialization::Essence),
            8 => Some(Specialization::Concentration),
            _ => return Err(Error("INVALID_JOURNAL")),
        };
        let mut deck = arrayvec::ArrayVec::new();
        deck.try_extend_from_slice(&self.deck)
            .map_err(|_| Error("INVALID_JOURNAL"))?;
        Ok(PlayerSetup {
            avatar: AvatarSnapshot {
                stats: Stats {
                    phys_atk,
                    phys_def,
                    mag_atk,
                    mag_def,
                    hp_max,
                    stamina_max,
                },
                schools: engine::Schools(self.schools),
                specialization,
                gift: self.gift,
            },
            deck,
        })
    }
    pub fn public(&self) -> Result<pb::PublicPlayer> {
        pb::PublicPlayer::decode(self.profile.as_slice()).map_err(|_| Error("INVALID_JOURNAL"))
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ExternalEnd {
    pub loser: Option<u8>,
    pub cause: u8,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Record {
    pub id: Uuid,
    pub kind: String,
    pub engine_version: String,
    pub content_version: [u8; 32],
    pub seed: [u8; 32],
    pub seats: [Seat; 2],
    pub moves: Vec<[Vec<u8>; 2]>,
    pub pending: [Option<Vec<u8>>; 2],
    pub timeouts: [u8; 2],
    pub connected: [bool; 2],
    pub grace_until: [Option<i64>; 2],
    pub deadline_ms: i64,
    pub ended: Option<ExternalEnd>,
    pub revision: u32,
}
impl Record {
    pub fn new(
        id: Uuid,
        kind: &str,
        seats: [Seat; 2],
        seed: [u8; 32],
        settings: &Settings,
    ) -> Result<Self> {
        let value = Self {
            id,
            kind: kind.into(),
            engine_version: engine::VERSION.into(),
            content_version: engine_content::CONTENT_VERSION,
            seed,
            seats,
            moves: Vec::new(),
            pending: [None, None],
            timeouts: [0, 0],
            connected: [true, true],
            grace_until: [None, None],
            deadline_ms: crate::online::now_ms() + settings.round_ms(),
            ended: None,
            revision: 0,
        };
        value.rebuild()?;
        Ok(value)
    }
    pub fn bytes(&self) -> Result<Vec<u8>> {
        serde_json::to_vec(self).map_err(|_| Error("INTERNAL"))
    }
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > 1048576 {
            return Err(Error("INVALID_JOURNAL"));
        }
        let value: Self = serde_json::from_slice(bytes).map_err(|_| Error("INVALID_JOURNAL"))?;
        if value.engine_version != engine::VERSION
            || value.content_version != engine_content::CONTENT_VERSION
            || value.moves.len() > usize::from(engine::MAX_ROUNDS)
        {
            return Err(Error("CONTENT_MISMATCH"));
        }
        Ok(value)
    }
    pub fn setup(&self) -> Result<DuelSetup> {
        Ok(DuelSetup {
            rules: engine_content::rules(),
            seed: self.seed,
            players: [self.seats[0].setup()?, self.seats[1].setup()?],
        })
    }
    pub fn rebuild(&self) -> Result<(engine::DuelState, EventBuf)> {
        let content = engine_content::content();
        let mut state =
            engine::new_duel(&self.setup()?, &content).map_err(|_| Error("INVALID_JOURNAL"))?;
        let mut events = EventBuf::new();
        for pair in &self.moves {
            let moves = [decode_move(&pair[0])?, decode_move(&pair[1])?];
            events.clear();
            engine::resolve_round(&mut state, &content, &moves, &mut events)
                .map_err(|_| Error("INVALID_JOURNAL"))?;
        }
        if let Some(end) = &self.ended {
            let cause = match end.cause {
                0 => engine::Termination::Forfeit,
                1 => engine::Termination::Disconnect,
                2 => engine::Termination::Timeout,
                _ => return Err(Error("INVALID_JOURNAL")),
            };
            match end.loser {
                Some(0) => engine::end_by(&mut state, Side::A, cause, &mut events),
                Some(1) => engine::end_by(&mut state, Side::B, cause, &mut events),
                None => engine::end_draw(&mut state, cause, &mut events),
                _ => return Err(Error("INVALID_JOURNAL")),
            }
            .map_err(|_| Error("INVALID_JOURNAL"))?;
        }
        Ok((state, events))
    }
    pub fn side(&self, user: Uuid) -> Result<Side> {
        self.seats
            .iter()
            .position(|s| s.user == Some(user))
            .map(|i| if i == 0 { Side::A } else { Side::B })
            .ok_or(Error("FORBIDDEN"))
    }
}
pub fn decode_move(bytes: &[u8]) -> Result<engine::Move> {
    protocol::duel::decode_move(&pb::DuelMove::decode(bytes).map_err(|_| Error("INVALID_JOURNAL"))?)
        .map_err(|_| Error("INVALID_JOURNAL"))
}
