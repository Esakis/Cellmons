//! Serwer gry (`docs/08-serwer.md`).

use anyhow::Context as _;
use server::{config::Config, db, gateway, telemetry};

fn main() -> anyhow::Result<()> {
    let migrate = match std::env::args().skip(1).collect::<Vec<_>>().as_slice() {
        [] => false,
        [arg] if arg == "migrate" => true,
        [arg] if arg == "--help" || arg == "-h" => {
            println!("server [migrate]\n  migrate: apply embedded database migrations, then exit");
            return Ok(());
        }
        _ => anyhow::bail!("usage: server [migrate]"),
    };
    // `.env` wczytujemy przed startem wątków Tokio, bo dotenvy zmienia zmienne środowiskowe procesu.
    load_dotenv()?;
    let config = Config::from_env()?;
    telemetry::init(&config)?;

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("cannot start the Tokio runtime")?
        .block_on(async {
            if migrate {
                let pool = db::migration_pool(&config)?;
                db::migrate(&pool).await?;
                pool.close().await;
                tracing::info!("database migrations applied");
                Ok(())
            } else {
                run(config).await
            }
        })
}

async fn run(config: Config) -> anyhow::Result<()> {
    let db = db::connect_lazy(&config)?;
    let auth_config = server::auth::AuthConfig::from_lookup(|key| {
        std::env::var(key).ok().or_else(|| {
            (key == "PUBLIC_ORIGIN" && config.listen_addr.ip().is_loopback())
                .then(|| "http://localhost:5173".into())
        })
    })?;
    let auth = server::auth::Auth::from_config(auth_config)?;
    let app = gateway::router(gateway::AppState {
        db: db.clone(),
        auth,
    });

    let listener = tokio::net::TcpListener::bind(config.listen_addr)
        .await
        .with_context(|| format!("cannot listen on {}", config.listen_addr))?;
    tracing::info!(
        addr = %config.listen_addr,
        version = env!("CARGO_PKG_VERSION"),
        "server started"
    );

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
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
