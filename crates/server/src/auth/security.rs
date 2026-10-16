use axum::{
    extract::{ConnectInfo, Request, State},
    http::{HeaderMap, Method, StatusCode, header},
    middleware::Next,
    response::{IntoResponse as _, Response},
};
use cookie::{Cookie, SameSite};
use std::net::{IpAddr, SocketAddr};
use tower_http::cors::CorsLayer;

use super::{Auth, Error, Result, Session, config, crypto};
use crate::gateway::AppState;

pub struct Peer(pub Option<ConnectInfo<SocketAddr>>);
impl<S: Send + Sync> axum::extract::FromRequestParts<S> for Peer {
    type Rejection = std::convert::Infallible;
    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _: &S,
    ) -> std::result::Result<Self, Self::Rejection> {
        Ok(Self(
            parts.extensions.get::<ConnectInfo<SocketAddr>>().cloned(),
        ))
    }
}

pub fn cors(auth: &Auth) -> CorsLayer {
    CorsLayer::new()
        .expose_headers([header::RETRY_AFTER])
        .allow_origin(
            auth.0
                .config
                .app_origins
                .iter()
                .map(|value| value.parse().expect("validated origin"))
                .collect::<Vec<axum::http::HeaderValue>>(),
        )
        .allow_methods([Method::GET, Method::POST, Method::DELETE])
        .allow_headers([
            header::CONTENT_TYPE,
            header::AUTHORIZATION,
            "x-requested-with".parse().unwrap(),
            "x-native-version".parse().unwrap(),
        ])
}

pub async fn guard(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let auth = &state.auth;
    let headers = request.headers();
    let origin = headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok());
    let app =
        origin.is_some_and(|origin| auth.0.config.app_origins.iter().any(|app| app == origin));
    let mut response = if origin.is_some_and(|origin| origin != auth.0.config.public_origin && !app)
    {
        Error::forbidden().into_response()
    } else if request.method() != Method::OPTIONS && app && !native_version(auth, headers) {
        Error(StatusCode::BAD_REQUEST, "VERSION_MISMATCH", 0).into_response()
    } else if request.method() != Method::GET
        && request.method() != Method::OPTIONS
        && headers
            .get("x-requested-with")
            .and_then(|v| v.to_str().ok())
            != Some("app")
    {
        Error::forbidden().into_response()
    } else {
        next.run(request).await
    };
    if response.status().is_client_error()
        && !response
            .headers()
            .get(header::CONTENT_TYPE)
            .is_some_and(|value| value.as_bytes().starts_with(b"application/json"))
    {
        let status = response.status();
        response = Error(
            status,
            if status == StatusCode::PAYLOAD_TOO_LARGE {
                "BODY_TOO_LARGE"
            } else {
                "INVALID_INPUT"
            },
            0,
        )
        .into_response();
    }
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    response
        .headers_mut()
        .insert("x-content-type-options", "nosniff".parse().unwrap());
    response
}

fn native_version(auth: &Auth, headers: &HeaderMap) -> bool {
    headers
        .get("x-native-version")
        .and_then(|v| v.to_str().ok())
        .and_then(config::version)
        .is_some_and(|version| version >= auth.0.config.native_version)
}

pub fn client(auth: &Auth, headers: &HeaderMap, android: bool) -> Result<()> {
    let origin = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok());
    if android {
        if !native_version(auth, headers) {
            return Err(Error(StatusCode::BAD_REQUEST, "VERSION_MISMATCH", 0));
        }
        if origin.is_some_and(|origin| !auth.0.config.app_origins.iter().any(|app| app == origin)) {
            return Err(Error::forbidden());
        }
    } else if origin.is_some_and(|origin| auth.0.config.app_origins.iter().any(|app| app == origin))
    {
        return Err(Error::forbidden());
    }
    Ok(())
}

pub fn cookie_token(auth: &Auth, headers: &HeaderMap) -> Option<String> {
    let mut found = None;
    for header in headers.get_all(header::COOKIE) {
        let value = header.to_str().ok()?;
        for parsed in Cookie::split_parse(value) {
            let parsed = parsed.ok()?;
            if parsed.name() == auth.0.config.cookie_name() {
                if found.is_some() {
                    return None;
                }
                crypto::unhex(parsed.value())?;
                found = Some(parsed.value().to_owned());
            }
        }
    }
    found
}

pub fn credential(auth: &Auth, headers: &HeaderMap) -> Result<(String, &'static str)> {
    if let Some(header) = headers.get(header::AUTHORIZATION) {
        client(auth, headers, true)?;
        let token = header
            .to_str()
            .ok()
            .and_then(|value| value.strip_prefix("Bearer "))
            .filter(|value| crypto::unhex(value).is_some())
            .ok_or_else(Error::unauthorized)?;
        return Ok((token.into(), "app"));
    }
    client(auth, headers, false)?;
    cookie_token(auth, headers)
        .map(|token| (token, "web"))
        .ok_or_else(Error::unauthorized)
}

pub async fn session(state: &AppState, headers: &HeaderMap) -> Result<Session> {
    let (token, kind) = credential(&state.auth, headers)?;
    session_hash(state, &crypto::hash(&token), kind).await
}

pub(crate) async fn session_hash(state: &AppState, hash: &[u8], kind: &str) -> Result<Session> {
    let owner:Option<uuid::Uuid>=sqlx::query_scalar("SELECT user_id FROM sessions WHERE id_hash=$1 AND kind=$2 AND expires_at>clock_timestamp()")
        .bind(hash).bind(kind).fetch_optional(&state.db).await?;
    if let Some(user) = owner {
        crate::admin::moderation::expire_ban(&state.db, Some(user), None)
            .await
            .map_err(|_| Error::busy())?;
    }
    let row = sqlx::query!("UPDATE sessions AS s SET last_seen_at=now(), expires_at=now()+interval '30 days' FROM users AS u WHERE s.id_hash=$1 AND s.kind=$2 AND s.purpose='game' AND s.expires_at>now() AND u.id=s.user_id AND u.status='active' RETURNING s.id_hash, s.user_id, s.kind", hash, kind).fetch_optional(&state.db).await?.ok_or_else(Error::unauthorized)?;
    Ok(Session {
        hash: row.id_hash,
        user: row.user_id,
        kind: row.kind,
    })
}

pub fn session_cookie(auth: &Auth, token: &str) -> String {
    Cookie::build((auth.0.config.cookie_name().to_owned(), token.to_owned()))
        .path("/")
        .secure(auth.0.config.secure_cookie)
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(cookie::time::Duration::seconds(if token.is_empty() {
            0
        } else {
            auth.0.config.session_age().as_secs() as i64
        }))
        .build()
        .to_string()
}

pub fn peer_key(
    auth: &Auth,
    connect: Option<ConnectInfo<SocketAddr>>,
    headers: &HeaderMap,
) -> String {
    let Some(ConnectInfo(peer)) = connect else {
        return "unknown".into();
    };
    let mut ip = peer.ip();
    if auth.0.config.trusted_proxies.contains(&ip)
        && let Some(forwarded) = headers
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .filter(|v| v.len() <= 512)
    {
        let chain: Option<Vec<IpAddr>> = forwarded
            .split(',')
            .map(|part| part.trim().parse().ok())
            .collect();
        if let Some(chain) = chain.filter(|chain| chain.len() <= 8) {
            for next in chain.into_iter().rev() {
                ip = next;
                if !auth.0.config.trusted_proxies.contains(&ip) {
                    break;
                }
            }
        }
    }
    ip.to_string()
}
