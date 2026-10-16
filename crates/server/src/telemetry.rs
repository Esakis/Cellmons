//! Logi serwera (`docs/15-infrastruktura-i-wdrozenie.md`, rozdz. 8).

use anyhow::Context as _;
use tracing_subscriber::EnvFilter;

use crate::config::{Config, LogFormat};
pub mod metrics;

pub fn init(config: &Config) -> anyhow::Result<()> {
    let filter = EnvFilter::try_new(&config.log_level)
        .with_context(|| format!("invalid LOG_LEVEL: {}", config.log_level))?;
    let subscriber = tracing_subscriber::fmt().with_env_filter(filter);
    let result = match config.log_format {
        LogFormat::Json => subscriber.json().try_init(),
        LogFormat::Text => subscriber.try_init(),
    };
    result.map_err(|err| anyhow::anyhow!(err).context("cannot initialize logging"))
}
