//! Owner console only. This is deliberately not exposed as an HTTP bootstrap endpoint.
use crate::online::{Error, Result};
use sqlx::PgPool;
use uuid::Uuid;
pub async fn run(db: &PgPool, args: &[String]) -> Result<()> {
    let [command, login, value, reason] = args else {
        return Err(Error("USAGE_ADMIN"));
    };
    if reason.trim().chars().count() < 5 || reason.chars().count() > 2000 {
        return Err(Error("INVALID_MESSAGE"));
    }
    if !matches!(command.as_str(), "role" | "reset-factor") {
        return Err(Error("USAGE_ADMIN"));
    }
    let mut tx = db.begin().await?;
    // Serialize console role changes as well as staff rows; HTTP never edits administrators.
    sqlx::query("SELECT pg_advisory_xact_lock(70612545)")
        .execute(&mut *tx)
        .await?;
    let (user,role):(Uuid,String)=sqlx::query_as("SELECT id,role FROM users WHERE login=$1::citext AND status='active' AND email_verified_at IS NOT NULL FOR UPDATE").bind(login).fetch_optional(&mut *tx).await?.ok_or(Error("NOT_FOUND"))?;
    let action = if command == "role" {
        if !matches!(value.as_str(), "player" | "mentor" | "moderator" | "admin") {
            return Err(Error("INVALID_MESSAGE"));
        }
        if role == "admin" && value != "admin" {
            let count:i64=sqlx::query_scalar("SELECT count(*) FROM users WHERE status='active' AND email_verified_at IS NOT NULL AND role='admin'").fetch_one(&mut *tx).await?;
            if count <= 1 {
                return Err(Error("LAST_ADMIN"));
            }
        }
        sqlx::query("UPDATE users SET role=$2 WHERE id=$1")
            .bind(user)
            .bind(value)
            .execute(&mut *tx)
            .await?;
        "console_role"
    } else {
        if value != "confirm" || !matches!(role.as_str(), "admin" | "moderator") {
            return Err(Error("INVALID_MESSAGE"));
        }
        sqlx::query("DELETE FROM admin_totp WHERE user_id=$1")
            .bind(user)
            .execute(&mut *tx)
            .await?;
        "console_reset_factor"
    };
    sqlx::query("DELETE FROM admin_sessions WHERE user_id=$1")
        .bind(user)
        .execute(&mut *tx)
        .await?;
    let details = serde_json::to_vec(&serde_json::json!({"previous_role":role,"value":value}))
        .map_err(|_| Error("INTERNAL"))?;
    sqlx::query("INSERT INTO admin_audit(id,action,target,reason,details) VALUES ($1,$2,$3,$4,$5)")
        .bind(Uuid::now_v7())
        .bind(action)
        .bind(user)
        .bind(reason)
        .bind(details)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}
