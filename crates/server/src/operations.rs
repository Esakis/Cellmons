//! Owner console operations. No public HTTP mutation or bootstrap credential.
use anyhow::{Context as _, ensure};
use serde::Serialize;
use sqlx::PgPool;
pub mod report;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Status {
    pub draining: bool,
    pub duels: i64,
    pub tournaments: i64,
    pub expeditions: i64,
    pub trades: i64,
}
impl Status {
    pub fn idle(&self) -> bool {
        self.duels == 0 && self.tournaments == 0 && self.expeditions == 0 && self.trades == 0
    }
}
pub async fn status(db: &PgPool) -> anyhow::Result<Status> {
    tokio::time::timeout(std::time::Duration::from_secs(5), sqlx::query_as(
        "SELECT deployment_draining AS draining,(SELECT count(*) FROM duel_journal WHERE status='active') AS duels,(SELECT count(*) FROM tournaments WHERE status IN ('registering','running')) AS tournaments,(SELECT count(*) FROM expeditions WHERE status='active') AS expeditions,(SELECT count(*) FROM trades WHERE status IN ('invited','offering')) AS trades FROM server_controls WHERE singleton"
    ).fetch_one(db)).await.context("operations status timeout")?.context("operations status failed")
}
pub async fn drain(db: &PgPool, enabled: bool) -> anyhow::Result<()> {
    let mut tx = db.begin().await?;
    sqlx::query("SET LOCAL lock_timeout='5s'")
        .execute(&mut *tx)
        .await?;
    // Activity starts hold a SHARE row lock until their whole transaction commits.
    sqlx::query(
        "UPDATE server_controls SET deployment_draining=$1,revision=revision+1 WHERE singleton",
    )
    .bind(enabled)
    .execute(&mut *tx)
    .await?;
    sqlx::query("INSERT INTO admin_audit(id,action,reason,details) VALUES ($1,'deployment_drain','Owner deployment console',$2)")
        .bind(uuid::Uuid::now_v7()).bind(serde_json::to_vec(&serde_json::json!({"enabled":enabled}))?).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}
pub async fn command(db: &PgPool, arguments: &[String]) -> anyhow::Result<()> {
    match arguments
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["start"] => drain(db, true).await?,
        ["cancel"] => drain(db, false).await?,
        ["status"] => {}
        ["idle"] => {
            let status = status(db).await?;
            ensure!(
                status.draining && status.idle(),
                "deployment still has live activities"
            );
        }
        _ => anyhow::bail!("server drain start|cancel|status|idle"),
    }
    println!("{}", serde_json::to_string(&status(db).await?)?);
    Ok(())
}

/// Idempotent bounded cleanup, reconciliation and review signals; no automatic sanctions.
pub async fn maintenance(db: &PgPool) -> anyhow::Result<()> {
    let mut guard = db.begin().await?;
    let locked: bool = sqlx::query_scalar("SELECT pg_try_advisory_xact_lock(731592806)")
        .fetch_one(&mut *guard)
        .await?;
    ensure!(locked, "maintenance is already running");
    sqlx::query("INSERT INTO operation_jobs(name,started_at) VALUES ('maintenance',now()) ON CONFLICT(name) DO UPDATE SET started_at=now(),finished_at=NULL,success=false,summary='{}'").execute(db).await?;
    let result = tokio::time::timeout(std::time::Duration::from_secs(600), async {
        let sessions = sqlx::query("DELETE FROM sessions WHERE id_hash IN (SELECT id_hash FROM sessions WHERE expires_at<now() ORDER BY expires_at LIMIT 1000 FOR UPDATE SKIP LOCKED)").execute(db).await?.rows_affected();
        let tokens = sqlx::query("DELETE FROM email_tokens WHERE token_hash IN (SELECT token_hash FROM email_tokens WHERE expires_at<now() ORDER BY expires_at LIMIT 1000 FOR UPDATE SKIP LOCKED)").execute(db).await?.rows_affected();
        sqlx::query("DELETE FROM admin_sessions WHERE expires_at<now()").execute(db).await?;
        sqlx::query("DELETE FROM ws_tickets WHERE expires_at<now()").execute(db).await?;
        crate::online::repo::retention(db).await.map_err(|e| anyhow::anyhow!(e.0))?;
        let erased = crate::auth::privacy::cleanup(db).await.map_err(|e| anyhow::anyhow!(e.1))?;
        let signals = crate::admin::signals::scan(db).await.map_err(|e| anyhow::anyhow!(e.0))?;
        let ledger = crate::economy::reconcile(db).await.map_err(|e| anyhow::anyhow!(e.0))?;
        ensure!(ledger.is_ok(), "currency ledger mismatch");
        Ok::<_, anyhow::Error>(serde_json::json!({"expired_sessions":sessions,"expired_email_tokens":tokens,"erased_accounts":erased,"review_signals":signals,"ledger_mismatches":0}))
    }).await.context("maintenance exceeded ten minutes").and_then(|r| r);
    let summary = result.as_ref().cloned().unwrap_or_else(|_| serde_json::json!({"error":"Maintenance failed; inspect operator logs and reconciliation."}));
    sqlx::query("UPDATE operation_jobs SET finished_at=now(),success=$1,last_success_at=CASE WHEN $1 THEN now() ELSE last_success_at END,summary=$2::jsonb WHERE name='maintenance'")
        .bind(result.is_ok()).bind(summary.to_string()).execute(db).await?;
    guard.rollback().await?;
    println!("{summary}");
    result.map(|_| ())
}
