//! Brama HTTP: router i stan współdzielony przez obsługę żądań (`docs/08-serwer.md`, rozdz. 2).

mod health;

use axum::routing::get;
use axum::{Router, extract::DefaultBodyLimit, middleware};
use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub auth: crate::auth::Auth,
}

impl AppState {
    pub fn local(db: PgPool) -> Self {
        Self {
            db,
            auth: crate::auth::Auth::local(),
        }
    }
}

pub fn router(state: AppState) -> Router {
    let auth = crate::auth::routes()
        .layer(DefaultBodyLimit::max(8192))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::auth::security::guard,
        ))
        .layer(crate::auth::security::cors(&state.auth));
    Router::new()
        .merge(auth)
        .route("/healthz", get(health::healthz))
        .route("/readyz", get(health::readyz))
        .with_state(state)
}
