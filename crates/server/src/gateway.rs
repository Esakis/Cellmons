//! Brama HTTP: router i stan współdzielony przez obsługę żądań (`docs/08-serwer.md`, rozdz. 2).

mod health;

use axum::routing::get;
use axum::{Router, extract::DefaultBodyLimit, middleware};
use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub auth: crate::auth::Auth,
    pub online: std::sync::Arc<crate::online::ws::Online>,
}

impl AppState {
    pub fn local(db: PgPool) -> Self {
        Self::new(
            db,
            crate::auth::Auth::local(),
            crate::online::Settings::default(),
        )
    }
    pub fn new(db: PgPool, auth: crate::auth::Auth, settings: crate::online::Settings) -> Self {
        Self {
            online: std::sync::Arc::new(crate::online::ws::Online::new(db.clone(), settings)),
            db,
            auth,
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
        .route("/ws", get(crate::online::ws::upgrade))
        .with_state(state)
}
