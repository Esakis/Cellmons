//! Durable tournaments, with a pure bracket shared by the scheduler and simulations.
pub mod bracket;
mod repository;
mod rewards;
mod scheduler;
mod views;
pub use repository::{Mutation, Terms, cancel, create, join, leave, ready};
pub use rewards::{Reward, cancel_system, create_system};
pub use scheduler::{Manager, tick};
pub use views::{get, post};

use crate::online::{Error, Result};
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use uuid::Uuid;

fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    serde_json::to_vec(value).map_err(|_| Error("INTERNAL"))
}
fn decode<T: for<'de> Deserialize<'de>>(bytes: &[u8]) -> Result<T> {
    serde_json::from_slice(bytes).map_err(|_| Error("INVALID_JOURNAL"))
}
async fn lock_users(connection: &mut PgConnection, mut users: Vec<Uuid>) -> Result<()> {
    users.sort();
    users.dedup();
    for user in users {
        sqlx::query("SELECT id FROM users WHERE id=$1 FOR UPDATE")
            .bind(user)
            .fetch_one(&mut *connection)
            .await?;
    }
    Ok(())
}
#[derive(sqlx::FromRow)]
struct Tournament {
    engine_version: String,
    content_version: Vec<u8>,
    creator: Uuid,
    status: String,
    terms: Vec<u8>,
    seed: Vec<u8>,
    bracket: Option<Vec<u8>>,
    balance: i64,
    scheduler_instance: Option<Uuid>,
    scheduler_until_ms: i64,
    created_ms: i64,
    system_reward: Option<Vec<u8>>,
}
async fn locked(connection: &mut PgConnection, id: Uuid) -> Result<Tournament> {
    let row: Tournament = sqlx::query_as("SELECT id,creator,name,status,terms,seed,bracket,balance,revision,scheduler_instance,scheduler_until_ms,(extract(epoch FROM created_at)*1000)::bigint AS created_ms,system_reward,engine_version,content_version FROM tournaments WHERE id=$1 FOR UPDATE")
        .bind(id).fetch_optional(connection).await?.ok_or(Error("NOT_FOUND"))?;
    row.compatible()?;
    Ok(row)
}
impl Tournament {
    fn compatible(&self) -> Result<()> {
        if self.engine_version != engine::VERSION
            || self.content_version != engine_content::CONTENT_VERSION
        {
            Err(Error("CONTENT_MISMATCH"))
        } else {
            Ok(())
        }
    }
}
#[derive(sqlx::FromRow)]
struct Entry {
    user_id: Uuid,
    avatar_id: Uuid,
    team_id: Uuid,
    seat: Vec<u8>,
    fee: i64,
    status: String,
    ready_since_ms: Option<i64>,
    ready_until_ms: i64,
}
async fn entries(connection: &mut PgConnection, id: Uuid) -> Result<Vec<Entry>> {
    Ok(sqlx::query_as("SELECT user_id,avatar_id,team_id,seat,fee,status,ready_since_ms,ready_until_ms FROM tournament_entries WHERE tournament_id=$1 ORDER BY user_id")
        .bind(id).fetch_all(connection).await?)
}
