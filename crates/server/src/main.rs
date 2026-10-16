//! Serwer gry (`docs/08-serwer.md`).

use anyhow::Context as _;
use server::{config::Config, db, gateway, telemetry};

fn main() -> anyhow::Result<()> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let command = match arguments.as_slice() {
        [] => "run",
        [arg] if arg == "migrate" => "migrate",
        [arg] if arg == "check-config" => "check-config",
        [arg] if arg == "maintenance" => "maintenance",
        [arg] if arg == "telemetry-report" => "telemetry-report",
        [arg] if arg == "reconcile" => "reconcile",
        [arg] if arg == "abuse-report" => "abuse-report",
        [arg] if arg == "privacy-maintenance" => "privacy-maintenance",
        [arg, ..] if arg == "drain" => "drain",
        [arg, ..] if arg == "admin" => "admin",
        [arg] if arg == "--help" || arg == "-h" => {
            println!(
                "server [migrate|check-config|reconcile|maintenance|telemetry-report|abuse-report|privacy-maintenance]\n  drain start|cancel|status|idle: audited deployment barrier\n  migrate: apply embedded database migrations\n  check-config: validate configuration/schema without starting workers\n  reconcile: read-only currency ledger check\n  maintenance: bounded cleanup, privacy, review signals and reconciliation\n  telemetry-report: read-only weekly aggregates, no guest tracking\n  admin role LOGIN ROLE REASON\n  admin reset-factor LOGIN confirm REASON\nOwner console actions are audited. No HTTP bootstrap exists."
            );
            return Ok(());
        }
        _ => anyhow::bail!("usage: server --help"),
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
            if command == "telemetry-report" {
                let pool = db::connect_lazy(&config)?;
                let report = server::operations::report::weekly(&pool).await?;
                println!("{}", serde_json::to_string_pretty(&report)?);
                pool.close().await;
                Ok(())
            } else if command == "maintenance" {
                anyhow::ensure!(
                    config.db_pool_size >= 2,
                    "maintenance requires DB_POOL_SIZE >= 2"
                );
                let pool = db::connect_lazy(&config)?;
                server::operations::maintenance(&pool).await?;
                pool.close().await;
                Ok(())
            } else if command == "check-config" {
                let pool = db::connect_lazy(&config)?;
                db::ping(&pool).await?;
                server::auth::AuthConfig::from_lookup(|key| std::env::var(key).ok())?;
                println!("Configuration and schema verified; no workers started.");
                pool.close().await;
                Ok(())
            } else if command == "drain" {
                let pool = db::connect_lazy(&config)?;
                server::operations::command(&pool, &arguments[1..]).await?;
                pool.close().await;
                Ok(())
            } else if command == "migrate" {
                let pool = db::migration_pool(&config)?;
                db::migrate(&pool).await?;
                pool.close().await;
                tracing::info!("database migrations applied");
                Ok(())
            } else if command == "reconcile" {
                let pool = db::connect_lazy(&config)?;
                let report = server::economy::reconcile(&pool)
                    .await
                    .map_err(|e| anyhow::anyhow!("reconciliation failed: {}", e.0))?;
                println!("{}", serde_json::to_string(&report)?);
                pool.close().await;
                anyhow::ensure!(report.is_ok(), "currency ledger mismatch");
                Ok(())
            } else if command == "privacy-maintenance" {
                let pool = db::connect_lazy(&config)?;
                let count = server::auth::privacy::cleanup(&pool)
                    .await
                    .map_err(|e| anyhow::anyhow!("account maintenance: {}", e.1))?;
                println!("Erased {count} accounts whose grace period expired.");
                pool.close().await;
                Ok(())
            } else if command == "admin" || command == "abuse-report" {
                let pool = db::connect_lazy(&config)?;
                if command == "admin" {
                    server::admin::console::run(&pool, &arguments[1..])
                        .await
                        .map_err(|e| anyhow::anyhow!("owner console: {}", e.0))?;
                    println!("Owner console action committed and audited.");
                } else {
                    let count = server::admin::signals::scan(&pool)
                        .await
                        .map_err(|e| anyhow::anyhow!("signals: {}", e.0))?;
                    println!("Updated {count} review signals; no sanctions applied.");
                }
                pool.close().await;
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
    let state = gateway::AppState::new(db.clone(), auth, server::online::Settings::from_env()?);
    let online = state.online.clone();
    // Optional private listener; never proxied by the public Caddy routes.
    let metrics = if let Ok(address) = std::env::var("OPS_LISTEN_ADDR") {
        let address: std::net::SocketAddr = address.parse().context("invalid OPS_LISTEN_ADDR")?;
        let listener = tokio::net::TcpListener::bind(address).await?;
        let router = axum::Router::new()
            .route("/metrics", axum::routing::get(telemetry::metrics::export))
            .with_state(state.clone());
        Some(tokio::spawn(
            async move { axum::serve(listener, router).await },
        ))
    } else {
        None
    };
    let app = gateway::router(state);

    let listener = tokio::net::TcpListener::bind(config.listen_addr)
        .await
        .with_context(|| format!("cannot listen on {}", config.listen_addr))?;
    tracing::info!(
        addr = %config.listen_addr,
        version = env!("CARGO_PKG_VERSION"),
        "server started"
    );

    axum::serve(
        gateway::realtime_listener(listener),
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        shutdown_signal().await;
        online.hub.shutdown().await;
    })
    .await
    .context("HTTP server failed")?;
    if let Some(metrics) = metrics {
        metrics.abort();
        let _ = metrics.await;
    }

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
