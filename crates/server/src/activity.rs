//! All activity starts/mutations hold involved user rows in UUID order before checking locks.
use crate::online::{Error, Result};
use sqlx::PgConnection;
use uuid::Uuid;
pub async fn ensure_idle(connection: &mut PgConnection, user: Uuid) -> Result<()> {
    ensure_available(connection).await?;
    let busy=sqlx::query_scalar!("SELECT EXISTS(SELECT 1 FROM duel_locks WHERE user_id=$1) OR EXISTS(SELECT 1 FROM activity_locks WHERE user_id=$1) AS \"busy!\"",user).fetch_one(connection).await?;
    if busy { Err(Error("BUSY")) } else { Ok(()) }
}
pub async fn ensure_available(connection: &mut PgConnection) -> Result<()> {
    let maintenance: bool = sqlx::query_scalar(
        "SELECT maintenance OR deployment_draining FROM server_controls WHERE singleton FOR SHARE",
    )
    .fetch_one(connection)
    .await?;
    if maintenance {
        Err(Error("MAINTENANCE"))
    } else {
        Ok(())
    }
}
