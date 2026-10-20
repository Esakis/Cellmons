//! Persistent account progression and server-authoritative location access.
use crate::{
    gateway::AppState,
    online::{Error, Result},
};
use axum::{Json, extract::State, http::HeaderMap};
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, PgPool};
use std::collections::BTreeMap;
use uuid::Uuid;

const LEVEL_XP: [i64; 30] = [
    0, 100, 250, 450, 700, 1000, 1400, 1850, 2400, 3050, 3800, 4700, 5700, 6800, 8000, 9250, 10600,
    12000, 14000, 16000, 18000, 20000, 22250, 24500, 27000, 29750, 32500, 35500, 38750, 42000,
];
const LOCATIONS: [(&str, u8); 11] = [
    ("square", 1),
    ("training", 1),
    ("library", 1),
    ("arena", 3),
    ("portal", 4),
    ("expedition", 4),
    ("shop", 6),
    ("clan", 10),
    ("tower", 10),
    ("castle", 20),
    ("bazaar", 22),
];
pub fn level(xp: i64) -> u8 {
    LEVEL_XP
        .partition_point(|threshold| *threshold <= xp)
        .max(1) as u8
}
#[derive(Serialize)]
pub struct Snapshot {
    pub level: u8,
    pub xp: i64,
    pub next_xp: Option<i64>,
    pub tutorial_step: i16,
    pub recruits: usize,
    pub locations: BTreeMap<&'static str, u8>,
}
pub async fn load(connection: &mut PgConnection, user: Uuid) -> Result<Snapshot> {
    let (xp, step): (i64, i16) = sqlx::query_as(
        "SELECT account_xp,tutorial_step FROM users WHERE id=$1 AND status='active'",
    )
    .bind(user)
    .fetch_optional(&mut *connection)
    .await?
    .ok_or(Error("UNAUTHENTICATED"))?;
    let level = level(xp);
    Ok(Snapshot {
        level,
        xp,
        next_xp: LEVEL_XP.get(usize::from(level)).copied(),
        tutorial_step: step,
        recruits: crate::recruitment::pending(connection, user, level).await?,
        locations: LOCATIONS.into_iter().collect(),
    })
}
pub async fn require(db: &PgPool, user: Uuid, location: &str) -> Result<()> {
    let name = if location.starts_with("clan:") {
        "clan"
    } else {
        location
    };
    let required = LOCATIONS
        .iter()
        .find(|(key, _)| *key == name)
        .map_or(1, |(_, n)| *n);
    if required == 1 {
        return Ok(());
    }
    let (xp, managed): (i64, bool) = sqlx::query_as("SELECT account_xp,EXISTS(SELECT 1 FROM bot_accounts WHERE user_id=$1) FROM users WHERE id=$1 AND status='active'")
        .bind(user).fetch_optional(db).await?.ok_or(Error("UNAUTHENTICATED"))?;
    if !managed && level(xp) < required {
        return Err(Error("LEVEL_REQUIRED"));
    }
    Ok(())
}
/// Called inside the unique match settlement transaction. Retries cannot grant XP twice.
pub async fn award(
    connection: &mut PgConnection,
    user: Uuid,
    training: bool,
    won: bool,
    rounds: usize,
) -> Result<()> {
    if rounds == 0 || (!training && rounds < 2) {
        return Ok(());
    }
    if training {
        sqlx::query("UPDATE users SET account_xp=account_xp+250,tutorial_step=3 WHERE id=$1 AND tutorial_step<3 AND NOT EXISTS(SELECT 1 FROM bot_accounts WHERE user_id=$1)")
            .bind(user).execute(connection).await?;
    } else {
        sqlx::query("UPDATE users SET account_xp=account_xp+$2 WHERE id=$1 AND NOT EXISTS(SELECT 1 FROM bot_accounts WHERE user_id=$1)")
            .bind(user).bind(if won { 90_i64 } else { 60_i64 }).execute(connection).await?;
    }
    Ok(())
}
pub async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> std::result::Result<Json<Snapshot>, crate::game::HttpError> {
    let session = crate::auth::security::session(&state, &headers)
        .await
        .map_err(crate::game::HttpError)?;
    let mut connection = state
        .db
        .acquire()
        .await
        .map_err(|e| crate::game::response(e.into()))?;
    load(&mut connection, session.user)
        .await
        .map(Json)
        .map_err(crate::game::response)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Advance {
    step: i16,
}
pub async fn post(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<Advance>,
) -> std::result::Result<Json<Snapshot>, crate::game::HttpError> {
    let session = crate::auth::security::session(&state, &headers)
        .await
        .map_err(crate::game::HttpError)?;
    if !(0..=1).contains(&input.step) {
        return Err(crate::game::response(Error("INVALID_MESSAGE")));
    }
    sqlx::query("UPDATE users SET tutorial_step=tutorial_step+1 WHERE id=$1 AND tutorial_step=$2")
        .bind(session.user)
        .bind(input.step)
        .execute(&state.db)
        .await
        .map_err(|e| crate::game::response(e.into()))?;
    let mut connection = state
        .db
        .acquire()
        .await
        .map_err(|e| crate::game::response(e.into()))?;
    load(&mut connection, session.user)
        .await
        .map(Json)
        .map_err(crate::game::response)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn levels_include_every_boundary_and_cap() {
        assert!(LEVEL_XP.windows(2).all(|p| p[0] < p[1]));
        for (index, xp) in LEVEL_XP.iter().enumerate() {
            assert_eq!(level(*xp), (index + 1) as u8);
            if index > 0 {
                assert_eq!(level(xp - 1), index as u8);
            }
        }
        assert_eq!(level(i64::MAX), 30);
    }
}
