//! Pula połączeń z PostgreSQL (`docs/09-baza-danych.md`).

use std::time::Duration;

use anyhow::{Context as _, ensure};
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

use crate::config::Config;

/// Limit oczekiwania na połączenie z puli; przy niedostępnej bazie `/readyz` szybko zwraca 503.
const ACQUIRE_TIMEOUT: Duration = Duration::from_secs(3);

/// Górny limit sprawdzenia bazy, także gdy połączenie zawiśnie w trakcie zapytania.
const PING_TIMEOUT: Duration = Duration::from_secs(5);

pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations");

/// Tworzy pulę bez łączenia się z bazą. Serwer startuje także wtedy, gdy baza jest chwilowo
/// niedostępna, a jej stan pokazuje `/readyz`.
pub fn connect_lazy(config: &Config) -> anyhow::Result<PgPool> {
    pool(&config.database_url, config.db_pool_size)
}

pub fn migration_pool(config: &Config) -> anyhow::Result<PgPool> {
    pool(
        config
            .migration_database_url
            .as_deref()
            .unwrap_or(&config.database_url),
        1,
    )
}

fn pool(url: &str, size: u32) -> anyhow::Result<PgPool> {
    PgPoolOptions::new()
        .max_connections(size)
        .acquire_timeout(ACQUIRE_TIMEOUT)
        .connect_lazy(url)
        .context("invalid DATABASE_URL")
}

/// Explicit deployment command; normal server startup never executes DDL.
pub async fn migrate(pool: &PgPool) -> anyhow::Result<()> {
    MIGRATOR
        .run(pool)
        .await
        .context("database migration failed")
}

/// Sprawdza połączenie i zastosowanie najnowszej wymaganej migracji.
pub async fn ping(pool: &PgPool) -> anyhow::Result<()> {
    let required = MIGRATOR
        .iter()
        .last()
        .context("no embedded migrations")?
        .version;
    let migrated = tokio::time::timeout(PING_TIMEOUT, sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM _sqlx_migrations WHERE version = $1 AND success) AS "migrated!""#,
        required
    ).fetch_one(pool))
        .await
        .context("database did not respond in time")?
        .context("database query failed")?;
    ensure!(
        migrated,
        "database schema is not migrated to the required version"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn runtime_pool_never_uses_migration_credentials() {
        let mut config = Config {
            database_url: "postgres://game_app@127.0.0.1/game".into(),
            migration_database_url: Some("postgres://game_migrator@127.0.0.1/game".into()),
            db_pool_size: 2,
            listen_addr: "127.0.0.1:8090".parse().unwrap(),
            log_level: "info".into(),
            log_format: crate::config::LogFormat::Text,
        };
        assert_eq!(
            connect_lazy(&config)
                .unwrap()
                .connect_options()
                .get_username(),
            "game_app"
        );
        assert_eq!(
            migration_pool(&config)
                .unwrap()
                .connect_options()
                .get_username(),
            "game_migrator"
        );
        config.migration_database_url = None;
        assert_eq!(
            migration_pool(&config)
                .unwrap()
                .connect_options()
                .get_username(),
            "game_app"
        );
        config.migration_database_url = Some("invalid URL".into());
        assert!(connect_lazy(&config).is_ok());
        assert!(migration_pool(&config).is_err());
    }
}
