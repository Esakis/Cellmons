//! Stan procesu dla Caddy i monitoringu (`docs/07-protokol-sieciowy.md`, rozdz. 8;
//! `docs/15-infrastruktura-i-wdrozenie.md`, rozdz. 5).

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::Serialize;

use super::AppState;
use crate::db;

#[derive(Debug, Serialize)]
pub struct Health {
    status: &'static str,
    version: &'static str,
}

/// `/healthz`: proces działa. Nie sprawdza zależności, więc chwilowa awaria bazy nie powoduje
/// restartu serwera.
pub async fn healthz() -> Json<Health> {
    Json(Health {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}

#[derive(Debug, Serialize)]
pub struct Readiness {
    status: &'static str,
    database: &'static str,
}

/// `/readyz`: serwer może przyjmować ruch. 503, gdy baza nie odpowiada; Caddy kieruje wtedy
/// nowe połączenia do drugiego slotu.
pub async fn readyz(State(state): State<AppState>) -> (StatusCode, Json<Readiness>) {
    match db::ping(&state.db).await {
        Ok(()) => (
            StatusCode::OK,
            Json(Readiness {
                status: "ok",
                database: "ok",
            }),
        ),
        Err(err) => {
            tracing::warn!(error = format!("{err:#}"), "readiness check failed");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(Readiness {
                    status: "unavailable",
                    database: "unavailable",
                }),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt as _;
    use sqlx::postgres::PgPoolOptions;
    use tower::ServiceExt as _;

    use crate::gateway::{AppState, router};

    /// Pula wskazująca port, na którym nie ma bazy: każde połączenie się nie uda.
    fn state_without_database() -> AppState {
        let db = PgPoolOptions::new()
            .acquire_timeout(Duration::from_millis(500))
            .connect_lazy("postgres://game:game@127.0.0.1:1/game")
            .expect("valid database URL");
        AppState { db }
    }

    async fn get(path: &str) -> (StatusCode, serde_json::Value) {
        let request = Request::get(path).body(Body::empty()).expect("request");
        let response = router(state_without_database())
            .oneshot(request)
            .await
            .expect("response");
        let status = response.status();
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        (status, serde_json::from_slice(&body).expect("JSON body"))
    }

    #[tokio::test]
    async fn healthz_reports_ok_without_database() {
        let (status, body) = get("/healthz").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "ok");
        assert_eq!(body["version"], env!("CARGO_PKG_VERSION"));
    }

    #[tokio::test]
    async fn readyz_reports_unavailable_database() {
        let (status, body) = get("/readyz").await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["status"], "unavailable");
        assert_eq!(body["database"], "unavailable");
    }
}
