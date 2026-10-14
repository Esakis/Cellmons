//! Brama HTTP: router i stan współdzielony przez obsługę żądań (`docs/08-serwer.md`, rozdz. 2).

mod health;

use axum::Router;
use axum::routing::get;
use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(health::healthz))
        .route("/readyz", get(health::readyz))
        .with_state(state)
}
