//! Konfiguracja serwera ze zmiennych środowiskowych (`docs/08-serwer.md`, rozdz. 13).

use std::net::SocketAddr;
use std::num::NonZeroUsize;

use anyhow::{Context as _, bail};
use tracing::level_filters::LevelFilter;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    /// Ustrukturyzowane logi dla produkcji (`docs/04-architektura.md`, rozdz. 9).
    Json,
    /// Czytelny tekst do pracy lokalnej.
    Text,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    /// Optional DDL credentials. Only the explicit `server migrate` command uses these.
    pub migration_database_url: Option<String>,
    pub db_pool_size: u32,
    pub listen_addr: SocketAddr,
    pub log_level: String,
    pub log_format: LogFormat,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    /// Pusta wartość zmiennej oznacza to samo co brak zmiennej.
    fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> anyhow::Result<Self> {
        let var = |key: &str| lookup(key).filter(|value| !value.is_empty());

        let database_url = var("DATABASE_URL").context("missing required variable DATABASE_URL")?;
        let migration_database_url = var("MIGRATION_DATABASE_URL");

        let db_pool_size = match var("DB_POOL_SIZE") {
            Some(value) => {
                let size: u32 = value
                    .parse()
                    .with_context(|| format!("invalid DB_POOL_SIZE: {value}"))?;
                if size == 0 {
                    bail!("DB_POOL_SIZE must be greater than 0");
                }
                size
            }
            None => default_pool_size(),
        };

        let listen_addr = var("LISTEN_ADDR").unwrap_or_else(|| "0.0.0.0:8080".to_owned());
        let listen_addr = listen_addr
            .parse()
            .with_context(|| format!("invalid LISTEN_ADDR: {listen_addr}"))?;

        let log_level = var("LOG_LEVEL").unwrap_or_else(|| "info".to_owned());
        check_log_level(&log_level)?;

        let log_format = match var("LOG_FORMAT").as_deref() {
            None | Some("json") => LogFormat::Json,
            Some("text") => LogFormat::Text,
            Some(other) => bail!("invalid LOG_FORMAT: {other} (expected json or text)"),
        };

        Ok(Self {
            database_url,
            migration_database_url,
            db_pool_size,
            listen_addr,
            log_level,
            log_format,
        })
    }
}

/// Element bez `=` i `[` musi być poziomem logów. `EnvFilter` uznałby inne słowo (np. literówkę
/// `warning` albo ` warn` ze spacją po przecinku) za nazwę modułu i bez żadnego błędu wyłączyłby
/// wszystkie pozostałe logi.
fn check_log_level(value: &str) -> anyhow::Result<()> {
    for directive in value.split(',').filter(|directive| !directive.is_empty()) {
        if !directive.contains(['=', '[']) && directive.parse::<LevelFilter>().is_err() {
            bail!(
                "invalid LOG_LEVEL: {directive:?} is not a log level \
                 (use error, warn, info, debug, trace or off, and target=level for a module)"
            );
        }
    }
    Ok(())
}

/// Domyślnie dwa połączenia na rdzeń procesora.
fn default_pool_size() -> u32 {
    let cores = std::thread::available_parallelism().map_or(1, NonZeroUsize::get);
    u32::try_from(cores.saturating_mul(2)).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn config(pairs: &[(&str, &str)]) -> anyhow::Result<Config> {
        let vars: HashMap<String, String> = pairs
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect();
        Config::from_lookup(|key| vars.get(key).cloned())
    }

    const DB: (&str, &str) = ("DATABASE_URL", "postgres://game:game@127.0.0.1:5433/game");

    #[test]
    fn defaults_apply_when_only_database_url_is_set() {
        let config = config(&[DB]).expect("valid config");
        assert_eq!(config.listen_addr, "0.0.0.0:8080".parse().expect("address"));
        assert_eq!(config.log_level, "info");
        assert_eq!(config.log_format, LogFormat::Json);
        assert!(config.db_pool_size >= 2);
        assert!(config.migration_database_url.is_none());
    }

    #[test]
    fn reads_all_variables() {
        let config = config(&[
            DB,
            ("DB_POOL_SIZE", "7"),
            ("LISTEN_ADDR", "127.0.0.1:8090"),
            ("LOG_LEVEL", "debug,sqlx=warn"),
            ("LOG_FORMAT", "text"),
            (
                "MIGRATION_DATABASE_URL",
                "postgres://migrator@127.0.0.1:5433/game",
            ),
        ])
        .expect("valid config");
        assert_eq!(config.database_url, DB.1);
        assert_eq!(config.db_pool_size, 7);
        assert_eq!(
            config.listen_addr,
            "127.0.0.1:8090".parse().expect("address")
        );
        assert_eq!(config.log_level, "debug,sqlx=warn");
        assert_eq!(config.log_format, LogFormat::Text);
        assert_eq!(
            config.migration_database_url.as_deref(),
            Some("postgres://migrator@127.0.0.1:5433/game")
        );
    }

    #[test]
    fn database_url_is_required() {
        assert!(config(&[]).is_err());
        assert!(config(&[("DATABASE_URL", "")]).is_err());
        assert!(
            config(&[DB, ("MIGRATION_DATABASE_URL", "")])
                .unwrap()
                .migration_database_url
                .is_none()
        );
    }

    #[test]
    fn invalid_values_are_rejected() {
        assert!(config(&[DB, ("DB_POOL_SIZE", "0")]).is_err());
        assert!(config(&[DB, ("DB_POOL_SIZE", "many")]).is_err());
        assert!(config(&[DB, ("LISTEN_ADDR", "localhost")]).is_err());
        assert!(config(&[DB, ("LOG_FORMAT", "xml")]).is_err());
    }

    #[test]
    fn log_level_typos_are_rejected() {
        for typo in ["warning", "inf", "info,warning", "info, warn"] {
            assert!(config(&[DB, ("LOG_LEVEL", typo)]).is_err(), "{typo}");
        }
        for valid in [
            "WARN",
            "off",
            "info,",
            "info,sqlx=warn",
            "server[duel]=debug",
        ] {
            assert!(config(&[DB, ("LOG_LEVEL", valid)]).is_ok(), "{valid}");
        }
    }
}
