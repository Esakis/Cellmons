//! All activity starts/mutations hold involved user rows in UUID order before checking locks.
use crate::online::{Error, Result};
use sqlx::PgConnection;
use uuid::Uuid;
pub async fn ensure_idle(connection: &mut PgConnection, user: Uuid) -> Result<()> {
    let busy=sqlx::query_scalar!("SELECT EXISTS(SELECT 1 FROM duel_locks WHERE user_id=$1) OR EXISTS(SELECT 1 FROM activity_locks WHERE user_id=$1) AS \"busy!\"",user).fetch_one(connection).await?;
    if busy { Err(Error("BUSY")) } else { Ok(()) }
}
