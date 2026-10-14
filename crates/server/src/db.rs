//! Pula połączeń z PostgreSQL (`docs/09-baza-danych.md`).

use std::time::Duration;

use anyhow::Context as _;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

use crate::config::Config;

/// Limit oczekiwania na połączenie z puli; przy niedostępnej bazie `/readyz` szybko zwraca 503.
const ACQUIRE_TIMEOUT: Duration = Duration::from_secs(3);

/// Górny limit sprawdzenia bazy, także gdy połączenie zawiśnie w trakcie zapytania.
const PING_TIMEOUT: Duration = Duration::from_secs(5);

/// Tworzy pulę bez łączenia się z bazą. Serwer startuje także wtedy, gdy baza jest chwilowo
/// niedostępna, a jej stan pokazuje `/readyz`.
pub fn connect_lazy(config: &Config) -> anyhow::Result<PgPool> {
    PgPoolOptions::new()
        .max_connections(config.db_pool_size)
        .acquire_timeout(ACQUIRE_TIMEOUT)
        .connect_lazy(&config.database_url)
        .context("invalid DATABASE_URL")
}

/// Sprawdza, czy baza odpowiada na zapytania.
pub async fn ping(pool: &PgPool) -> anyhow::Result<()> {
    tokio::time::timeout(PING_TIMEOUT, sqlx::query("SELECT 1").execute(pool))
        .await
        .context("database did not respond in time")?
        .context("database query failed")?;
    Ok(())
}
