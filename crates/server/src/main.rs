//! Serwer gry (`docs/08-serwer.md`).

mod config;
mod db;
mod gateway;
mod telemetry;

use anyhow::Context as _;

use crate::config::Config;

fn main() -> anyhow::Result<()> {
    // `.env` wczytujemy przed startem wątków Tokio, bo dotenvy zmienia zmienne środowiskowe procesu.
    load_dotenv()?;
    let config = Config::from_env()?;
    telemetry::init(&config)?;

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("cannot start the Tokio runtime")?
        .block_on(run(config))
}

async fn run(config: Config) -> anyhow::Result<()> {
    let db = db::connect_lazy(&config)?;
    let app = gateway::router(gateway::AppState { db: db.clone() });

    let listener = tokio::net::TcpListener::bind(config.listen_addr)
        .await
        .with_context(|| format!("cannot listen on {}", config.listen_addr))?;
    tracing::info!(
        addr = %config.listen_addr,
        version = env!("CARGO_PKG_VERSION"),
        "server started"
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("HTTP server failed")?;

    db.close().await;
    tracing::info!("server stopped");
    Ok(())
}

/// Lokalnie zmienne mogą pochodzić z pliku `.env` (wzór: `.env.example`). Zmienne ustawione
/// w środowisku mają pierwszeństwo, a w produkcji pliku nie ma (`docs/08-serwer.md`, rozdz. 13).
fn load_dotenv() -> anyhow::Result<()> {
    match dotenvy::dotenv() {
        Ok(_) => Ok(()),
        Err(err) if err.not_found() => Ok(()),
        Err(err) => Err(err).context("invalid .env file"),
    }
}

/// Kończy pracę po Ctrl+C albo SIGTERM. Wygaszanie trwających pojedynków przed zamknięciem
/// (`docs/08-serwer.md`, rozdz. 9) dochodzi razem z pojedynkami.
async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(err) = tokio::signal::ctrl_c().await {
            tracing::error!(%err, "cannot listen for Ctrl+C");
            std::future::pending::<()>().await;
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(err) => {
                tracing::error!(%err, "cannot listen for SIGTERM");
                std::future::pending::<()>().await;
            }
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {}
        () = terminate => {}
    }
    tracing::info!("shutdown signal received");
}
