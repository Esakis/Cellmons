//! Durable per-match actors, separate from lobby presence and transport.
pub mod actor;
pub mod journal;
pub mod manager;
pub mod record;
pub mod result;

pub use crate::online::{Error, Result};
use sha2::{Digest as _, Sha256};
use std::time::Duration;
pub fn state_digest(state: &engine::DuelState) -> [u8; 32] {
    Sha256::digest(engine::state_hash(state).to_le_bytes()).into()
}

#[derive(Clone, Debug)]
pub struct Settings {
    pub max_active: usize,
    pub round: Duration,
    pub after_opponent: Duration,
    pub reconnect: Duration,
    pub lease: Duration,
    pub tick: Duration,
}
impl Default for Settings {
    fn default() -> Self {
        let rules = engine_content::rules().duel;
        Self {
            max_active: 1024,
            round: Duration::from_secs(u64::from(rules.round_timeout_s)),
            after_opponent: Duration::from_secs(u64::from(rules.move_timeout_after_opponent_s)),
            reconnect: Duration::from_secs(u64::from(rules.reconnect_grace_s)),
            lease: Duration::from_secs(5),
            tick: Duration::from_millis(100),
        }
    }
}
fn ms(value: Duration) -> i64 {
    value.as_millis().min(i64::MAX as u128 / 2) as i64
}
impl Settings {
    pub fn round_ms(&self) -> i64 {
        ms(self.round)
    }
    pub fn opponent_ms(&self) -> i64 {
        ms(self.after_opponent)
    }
    pub fn reconnect_ms(&self) -> i64 {
        ms(self.reconnect)
    }
    pub fn lease_ms(&self) -> i64 {
        ms(self.lease)
    }
}
