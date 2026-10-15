//! Isolated real backend for Playwright. Never reads the application's .env or DATABASE_URL.
use anyhow::Context as _;
use axum::{
    Json, Router,
    extract::Query,
    routing::{get, post},
};
use server::{
    auth::{Auth, AuthConfig, mail::Mailer},
    gateway::AppState,
    online::Settings,
};
use sqlx::{
    Connection as _, PgConnection,
    postgres::{PgConnectOptions, PgPoolOptions},
};
use std::{
    net::{IpAddr, SocketAddr},
    str::FromStr,
    sync::{Arc, Mutex},
};
use tokio::sync::watch;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let url = std::env::var("E2E_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://game:game@127.0.0.1:5433/game".into());
    let options = PgConnectOptions::from_str(&url)?;
    anyhow::ensure!(
        options.get_host() == "localhost"
            || options
                .get_host()
                .parse::<IpAddr>()
                .is_ok_and(|ip| ip.is_loopback()),
        "E2E database must be local"
    );
    let mut admin = PgConnection::connect_with(&options)
        .await
        .context("Playwright requires local PostgreSQL (E2E_DATABASE_URL)")?;
    let database = format!("game_e2e_{}", uuid::Uuid::now_v7().simple());
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "CREATE DATABASE \"{database}\""
    )))
    .execute(&mut admin)
    .await?;
    let marker = std::path::Path::new("target/e2e-backend-running");
    tokio::fs::write(marker, database.as_bytes()).await?;
    let result = run(options.database(&database)).await;
    // The name is generated above; no existing or user-supplied database can be dropped.
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "DROP DATABASE \"{database}\" WITH (FORCE)"
    )))
    .execute(&mut admin)
    .await?;
    tokio::fs::remove_file(marker).await?;
    result
}
async fn run(options: PgConnectOptions) -> anyhow::Result<()> {
    let db = PgPoolOptions::new()
        .max_connections(16)
        .connect_with(options)
        .await?;
    let result = async {
        server::db::migrate(&db).await?;
        let mut config = AuthConfig::local();
        config.public_origin = "http://localhost:4173".into();
        let messages = Arc::new(Mutex::new(Vec::<server::auth::mail::Mail>::new()));
        let auth = Auth::new(config, Mailer::Capture(messages.clone()));
        let state = AppState::new(db.clone(), auth, Settings::default());
        let (shutdown, mut receiver) = watch::channel(false);
        let app = server::gateway::router(state.clone()).merge(Router::new().route(
            "/__shutdown",
            post(move || {
                let shutdown = shutdown.clone();
                async move {
                    let _ = shutdown.send(true);
                }
            }),
        ));
        let mailbox_db = db.clone();
        // Test-only mailbox: this example binds loopback and always uses a disposable database.
        let app = app.merge(Router::new().route(
            "/__mail",
            get(
                move |Query(query): Query<std::collections::HashMap<String, String>>| {
                    let messages = messages.clone();
                    let db = mailbox_db.clone();
                    async move {
                        let to = query
                            .get("to")
                            .map(|s| s.to_ascii_lowercase())
                            .unwrap_or_default();
                        // Delivery is captured before commit; wait for the mail transaction's user lock.
                        sqlx::query("SELECT id FROM users WHERE email=$1::citext FOR UPDATE")
                            .bind(&to)
                            .execute(&db)
                            .await
                            .map_err(|_| axum::http::StatusCode::SERVICE_UNAVAILABLE)?;
                        Ok::<_, axum::http::StatusCode>(Json(
                            messages
                                .lock()
                                .expect("captured mailbox")
                                .iter()
                                .filter(|m| m.to.eq_ignore_ascii_case(&to))
                                .cloned()
                                .collect::<Vec<_>>(),
                        ))
                    }
                },
            ),
        ));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:18092").await?;
        println!("Playwright backend ready on 127.0.0.1:18092 (isolated database, captured mail)");
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(async move {
            tokio::select! { _ = receiver.changed() => {}, _ = tokio::signal::ctrl_c() => {} }
            state.online.hub.shutdown().await;
        })
        .await?;
        anyhow::Ok(())
    }
    .await;
    db.close().await;
    result
}
