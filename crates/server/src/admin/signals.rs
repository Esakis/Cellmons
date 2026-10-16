//! Heuristics open review signals, never impose sanctions. Reruns replace evidence, not add scores.
use crate::online::{Error, Result};
use sqlx::PgPool;
use uuid::Uuid;
#[derive(sqlx::FromRow)]
struct Signal {
    user_id: Uuid,
    kind: String,
    score: i32,
    details: String,
}
pub async fn scan(db: &PgPool) -> Result<usize> {
    // Salted IP hashes remain in SQL. Evidence exposes counts, never network addresses/hashes.
    let rows: Vec<Signal> = sqlx::query_as(include_str!("signals.sql"))
        .fetch_all(db)
        .await?;
    if rows.len() > 100_000 {
        return Err(Error("SIGNAL_CAPACITY"));
    }
    let mut tx = db.begin().await?;
    for row in &rows {
        sqlx::query("INSERT INTO abuse_signals(id,user_id,kind,day,score,details) VALUES ($1,$2,$3,(now() AT TIME ZONE 'UTC')::date,$4,$5) ON CONFLICT(user_id,kind,day) DO UPDATE SET score=excluded.score,details=excluded.details,updated_at=now(),reviewed=CASE WHEN abuse_signals.score=excluded.score AND abuse_signals.details=excluded.details THEN abuse_signals.reviewed ELSE false END")
            .bind(Uuid::now_v7()).bind(row.user_id).bind(&row.kind).bind(row.score).bind(row.details.as_bytes()).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(rows.len())
}
