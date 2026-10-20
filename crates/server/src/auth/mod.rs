pub mod config;
pub(crate) mod crypto;
mod handlers;
mod limits;
pub mod mail;
pub mod privacy;
pub(crate) mod security;
mod tickets;

use axum::{
    Json, Router,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::Semaphore;
use uuid::Uuid;

use crate::gateway::AppState;
pub use config::AuthConfig;
pub const LEGAL_VERSION: &str = "2026-10-16";

#[derive(Clone)]
pub struct Auth(pub(crate) Arc<Inner>);

pub(crate) struct Inner {
    config: AuthConfig,
    mail: mail::Mailer,
    limits: limits::Limits,
    hashing: Arc<Semaphore>,
    mailing: Arc<Semaphore>,
    exports: Arc<Semaphore>,
    http: reqwest::Client,
}

#[derive(Debug)]
pub struct Error(pub StatusCode, pub &'static str, pub u64);
impl Error {
    fn bad() -> Self {
        Self(StatusCode::BAD_REQUEST, "INVALID_INPUT", 0)
    }
    fn unauthorized() -> Self {
        Self(StatusCode::UNAUTHORIZED, "UNAUTHENTICATED", 0)
    }
    fn forbidden() -> Self {
        Self(StatusCode::FORBIDDEN, "FORBIDDEN", 0)
    }
    fn busy() -> Self {
        Self(StatusCode::SERVICE_UNAVAILABLE, "TRY_AGAIN", 1)
    }
    fn limited(seconds: u64) -> Self {
        Self(StatusCode::TOO_MANY_REQUESTS, "RATE_LIMITED", seconds)
    }
}
impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let mut response = (self.0, Json(serde_json::json!({"error":self.1}))).into_response();
        if self.2 > 0 {
            response.headers_mut().insert(
                header::RETRY_AFTER,
                self.2.to_string().parse().expect("integer"),
            );
        }
        response
    }
}
impl From<sqlx::Error> for Error {
    fn from(error: sqlx::Error) -> Self {
        // Do not log bound values, queries, addresses, passwords, cookies, or tokens.
        tracing::error!(code = ?error.as_database_error().and_then(|e| e.code()), "auth database operation failed");
        Self::busy()
    }
}
type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: Uuid,
    pub login: String,
    pub email: String,
    pub email_verified: bool,
    pub start_school: String,
    pub role: String,
    #[serde(default)]
    pub busy: bool,
    pub avatar: Option<Avatar>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Avatar {
    pub id: Uuid,
    pub template_id: i32,
    pub path_id: Option<i32>,
    pub tier: i16,
    pub level: i16,
    pub xp: i32,
}

#[derive(Clone)]
pub(crate) struct Session {
    pub hash: Vec<u8>,
    pub user: Uuid,
    pub kind: String,
}

impl Auth {
    pub(crate) fn config(&self) -> &AuthConfig {
        &self.0.config
    }
    pub fn new(config: AuthConfig, mail: mail::Mailer) -> Self {
        Self(Arc::new(Inner {
            config,
            mail,
            limits: Default::default(),
            hashing: Arc::new(Semaphore::new(4)),
            mailing: Arc::new(Semaphore::new(4)),
            exports: Arc::new(Semaphore::new(2)),
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(8))
                .build()
                .expect("HTTP client"),
        }))
    }
    pub fn from_config(config: AuthConfig) -> anyhow::Result<Self> {
        let mail = mail::Mailer::new(&config.mail)?;
        Ok(Self::new(config, mail))
    }
    pub fn local() -> Self {
        Self::from_config(AuthConfig::local()).expect("local auth service")
    }

    async fn hash_password(&self, password: String) -> Result<String> {
        let permit = self
            .0
            .hashing
            .clone()
            .try_acquire_owned()
            .map_err(|_| Error::busy())?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            crypto::password_hash(&password)
        })
        .await
        .map_err(|_| Error::busy())?
        .map_err(|_| Error::busy())
    }

    pub(crate) async fn check_password(
        &self,
        password: String,
        stored: Option<String>,
    ) -> Result<bool> {
        let permit = self
            .0
            .hashing
            .clone()
            .try_acquire_owned()
            .map_err(|_| Error::busy())?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            crypto::password_matches(&password, stored.as_deref())
        })
        .await
        .map_err(|_| Error::busy())
    }

    pub(crate) fn limit(
        &self,
        scope: &str,
        identity: &str,
        count: usize,
        seconds: u64,
    ) -> Result<usize> {
        self.0
            .limits
            .take(
                self.limit_key(scope, identity),
                count,
                std::time::Duration::from_secs(seconds),
            )
            .map_err(Error::limited)
    }
    fn limit_key(&self, scope: &str, identity: &str) -> Vec<u8> {
        crypto::sign(
            &self.0.config.ticket_key,
            format!("{scope}:{identity}").as_bytes(),
        )
    }
}

pub(crate) fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/auth/options", get(handlers::options))
        .route("/api/auth/register", post(handlers::register))
        .route("/api/auth/login", post(handlers::login))
        .route("/api/auth/logout", post(handlers::logout))
        .route("/api/auth/verify-email", post(handlers::verify_email))
        .route("/api/auth/verify-email/request", post(handlers::resend))
        .route(
            "/api/auth/password-reset/request",
            post(handlers::reset_request),
        )
        .route(
            "/api/auth/password-reset/confirm",
            post(handlers::reset_confirm),
        )
        .route("/api/auth/rotate", post(handlers::rotate))
        .route("/api/auth/ws-ticket", post(tickets::issue))
        .route("/api/auth/sessions", get(handlers::sessions))
        .route("/api/auth/sessions/{id}", delete(handlers::revoke))
        .route(
            "/api/me",
            get(handlers::me).delete(privacy::request_deletion),
        )
        .route("/api/me/privacy", get(privacy::summary))
        .route("/api/me/export", get(privacy::export))
        .route("/api/auth/cancel-deletion", post(privacy::restore))
        .route("/api/auth/deletion-status", post(privacy::deletion_status))
        .route("/api/auth/privacy-login", post(privacy::login))
        .route(
            "/api/admin",
            get(crate::admin::views::get).post(crate::admin::views::post),
        )
        .route("/api/reports", post(crate::admin::moderation::post))
        .route("/api/notices", get(crate::admin::moderation::notices))
        .route(
            "/api/bulletin",
            get(crate::admin::moderation::public_notices),
        )
        .route(
            "/api/admin/session",
            get(crate::admin::access::get).post(crate::admin::access::post),
        )
        .route("/api/game", get(crate::game::get).post(crate::game::post))
        .route(
            "/api/journey",
            get(crate::journey::get).post(crate::journey::post),
        )
        .route(
            "/api/clans",
            get(crate::clans::get).post(crate::clans::post),
        )
        .route("/api/economy", get(crate::economy::get))
        .route(
            "/api/expeditions",
            get(crate::expeditions::get).post(crate::expeditions::post),
        )
        .route(
            "/api/tournaments",
            get(crate::tournaments::get).post(crate::tournaments::post),
        )
        .route(
            "/api/store",
            get(crate::store::get).post(crate::store::post),
        )
}

async fn profile(pool: &PgPool, user: Uuid) -> Result<Profile> {
    let row = sqlx::query!("SELECT id, login::text AS \"login!\", email::text AS \"email!\", email_verified_at IS NOT NULL AS \"verified!\", start_school, role FROM users WHERE id=$1 AND status='active'", user).fetch_optional(pool).await?.ok_or_else(Error::unauthorized)?;
    let avatar = sqlx::query_as!(Avatar, "SELECT id, template_id, path_id, tier, level, xp FROM avatars WHERE user_id=$1 AND is_active AND deleted_at IS NULL", user).fetch_optional(pool).await?;
    Ok(Profile {
        id: row.id,
        login: row.login,
        email: row.email,
        email_verified: row.verified,
        start_school: row.start_school,
        role: row.role,
        busy: sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM duel_locks WHERE user_id=$1) OR EXISTS(SELECT 1 FROM activity_locks WHERE user_id=$1)").bind(user).fetch_one(pool).await?,
        avatar,
    })
}
