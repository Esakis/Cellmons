//! Online transport and bounded actors for presence, rooms and challenges (F6.3–F7.1).
mod duels;
pub mod hub;
mod repo;
mod room;
pub mod ws;

use protocol::game::v1::{self as pb, server_message::Body};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug)]
pub struct Settings {
    pub duels: crate::duel::Settings,
    /// Disable only in isolated tests of the F7.1 reservation handoff.
    pub run_duels: bool,
    pub max_connections: usize,
    pub per_ip: usize,
    pub heartbeat: Duration,
    pub idle: Duration,
    pub hello_timeout: Duration,
    pub resume_ttl: Duration,
    pub challenge_ttl: Duration,
    pub reservation_ttl: Duration,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            duels: crate::duel::Settings::default(),
            run_duels: true,
            max_connections: 5000,
            per_ip: 20,
            heartbeat: Duration::from_secs(15),
            idle: Duration::from_secs(30),
            hello_timeout: Duration::from_secs(10),
            resume_ttl: Duration::from_secs(60),
            challenge_ttl: Duration::from_secs(30),
            reservation_ttl: Duration::from_secs(30),
        }
    }
}
impl Settings {
    pub fn from_env() -> anyhow::Result<Self> {
        let mut value = Self::default();
        if let Some(limit) = std::env::var("WS_CONNECTIONS_PER_IP")
            .ok()
            .filter(|s| !s.is_empty())
        {
            value.per_ip = limit.parse()?;
        }
        if let Some(limit) = std::env::var("WS_MAX_CONNECTIONS")
            .ok()
            .filter(|s| !s.is_empty())
        {
            value.max_connections = limit.parse()?;
        }
        anyhow::ensure!(
            value.per_ip > 0
                && value.per_ip <= 10000
                && value.max_connections > 0
                && value.max_connections <= 50000,
            "invalid WebSocket connection limits"
        );
        Ok(value)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Error(pub &'static str);
pub type Result<T> = std::result::Result<T, Error>;
impl From<sqlx::Error> for Error {
    fn from(error: sqlx::Error) -> Self {
        if error
            .as_database_error()
            .is_some_and(|e| e.is_unique_violation())
        {
            return Self("CONFLICT");
        }
        tracing::error!(code=?error.as_database_error().and_then(|e| e.code()), "online database operation failed");
        Self("INTERNAL")
    }
}
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}
pub fn ok() -> Body {
    Body::Ok(pb::NetOk {})
}
pub fn error(code: &str) -> Body {
    Body::Error(pb::NetError { code: code.into() })
}
pub(crate) fn text(value: &str, max: usize) -> Result<String> {
    let value = value.trim();
    if value.is_empty()
        || value.chars().count() > max
        || value.chars().any(|c| c.is_control() && c != '\n')
    {
        return Err(Error("INVALID_MESSAGE"));
    }
    Ok(value.to_owned())
}
