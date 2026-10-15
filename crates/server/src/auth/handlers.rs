use std::time::Duration;

use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse as _, Response},
};
use serde::Deserialize;
use sqlx::{PgConnection, Postgres, Transaction};
use uuid::Uuid;

use super::{Error, Result, crypto, mail::Mail, profile, security};
use crate::gateway::AppState;

#[derive(Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Client {
    #[default]
    Web,
    Android,
}
impl Client {
    fn android(&self) -> bool {
        matches!(self, Self::Android)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Register {
    login: String,
    email: String,
    password: String,
    school: String,
    #[serde(default)]
    client: Client,
    captcha_token: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Login {
    identity: String,
    password: String,
    #[serde(default)]
    client: Client,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Email {
    email: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Token {
    token: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reset {
    token: String,
    password: String,
}

fn email(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.len() > 254 || !trimmed.is_ascii() {
        return None;
    }
    let (local, domain) = trimmed.split_once('@')?;
    if local.is_empty()
        || local.len() > 64
        || domain.is_empty()
        || domain.contains('@')
        || trimmed
            .bytes()
            .any(|b| b.is_ascii_whitespace() || b.is_ascii_control())
    {
        return None;
    }
    let _: lettre::Address = trimmed.parse().ok()?;
    Some(trimmed.to_lowercase())
}

fn login_allowed(login: &str) -> bool {
    (3..=20).contains(&login.len())
        && login
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

pub async fn options(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(
        serde_json::json!({"captcha_site_key":state.auth.0.config.captcha.as_ref().map(|(site,_)| site)}),
    )
}

async fn captcha(state: &AppState, token: Option<&str>, origin: Option<&str>) -> Result<()> {
    let Some((_, secret)) = &state.auth.0.config.captcha else {
        return Ok(());
    };
    let token = token
        .filter(|token| !token.is_empty() && token.len() <= 2048)
        .ok_or(Error(StatusCode::BAD_REQUEST, "CAPTCHA_REQUIRED", 0))?;
    #[derive(Deserialize)]
    struct Verification {
        success: bool,
        hostname: Option<String>,
        action: Option<String>,
    }
    let reply = state
        .auth
        .0
        .http
        .post("https://challenges.cloudflare.com/turnstile/v0/siteverify")
        .form(&[("secret", secret.as_str()), ("response", token)])
        .send()
        .await
        .map_err(|_| Error::busy())?
        .error_for_status()
        .map_err(|_| Error::busy())?
        .json::<Verification>()
        .await
        .map_err(|_| Error::busy())?;
    let host = reqwest::Url::parse(origin.unwrap_or(&state.auth.0.config.public_origin))
        .expect("validated origin");
    if !reply.success
        || reply.hostname.as_deref() != host.host_str()
        || reply.action.as_deref() != Some("register")
    {
        return Err(Error(StatusCode::BAD_REQUEST, "CAPTCHA_REQUIRED", 0));
    }
    Ok(())
}

async fn message(
    state: &AppState,
    connection: &mut PgConnection,
    user: Uuid,
    email: &str,
    kind: &str,
) -> Result<()> {
    let raw = crypto::token().map_err(|_| Error::busy())?;
    let hash = crypto::hash(&raw);
    sqlx::query!(
        "DELETE FROM email_tokens WHERE user_id=$1 AND kind=$2",
        user,
        kind
    )
    .execute(&mut *connection)
    .await?;
    let ttl = if kind == "verify" {
        86400_i32
    } else {
        1800_i32
    };
    sqlx::query!("INSERT INTO email_tokens(token_hash,user_id,kind,expires_at) VALUES ($1,$2,$3,now()+$4 * interval '1 second')", hash, user, kind, ttl as f64).execute(&mut *connection).await?;
    let action = if kind == "verify" { "verify" } else { "reset" };
    let link = format!(
        "{}/#/account?action={action}&token={raw}",
        state.auth.0.config.public_origin
    );
    let subject = if kind == "verify" {
        "Spellgambit: verify your email / potwierdz e-mail"
    } else {
        "Spellgambit: password reset / reset hasla"
    };
    let body = format!(
        "{link}\n\nThis link expires in {} minutes. If you did not request it, ignore this message.\nLink wygasa za {} minut. Jesli nie wyslano zadania, zignoruj wiadomosc.",
        ttl / 60,
        ttl / 60
    );
    state
        .auth
        .0
        .mail
        .send(Mail {
            to: email.into(),
            subject: subject.into(),
            body,
        })
        .await
        .map_err(|_| {
            tracing::error!("auth email delivery failed");
            Error::busy()
        })
}

pub async fn register(
    State(state): State<AppState>,
    peer: security::Peer,
    headers: HeaderMap,
    Json(input): Json<Register>,
) -> Result<impl axum::response::IntoResponse> {
    security::client(&state.auth, &headers, input.client.android())?;
    let ip = security::peer_key(&state.auth, peer.0, &headers);
    state.auth.limit("register", &ip, 5, 3600)?;
    if !login_allowed(&input.login) || !crypto::password_allowed(&input.password) {
        return Err(Error::bad());
    }
    let email = email(&input.email).ok_or_else(Error::bad)?;
    let schools = ["combat", "divine", "element", "arcane", "necro", "tech"];
    let index = schools
        .iter()
        .position(|school| *school == input.school)
        .ok_or_else(Error::bad)?;
    let starter = engine_content::STARTERS
        .iter()
        .find(|starter| starter.school == engine::SCHOOLS[index])
        .expect("compiled starters");
    captcha(
        &state,
        input.captcha_token.as_deref(),
        headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()),
    )
    .await?;
    let password = state.auth.hash_password(input.password).await?;
    let user = Uuid::now_v7();
    let avatar = Uuid::now_v7();
    let mut tx = state.db.begin().await?;
    let inserted = sqlx::query!("INSERT INTO users(id,login,email,password_hash,start_school) VALUES ($1,$2,$3,$4,$5) ON CONFLICT DO NOTHING RETURNING id", user, input.login, email, password, input.school).fetch_optional(&mut *tx).await?;
    if inserted.is_none() {
        return Err(Error(StatusCode::CONFLICT, "IDENTITY_UNAVAILABLE", 0));
    }
    let template = i32::from(starter.avatar);
    sqlx::query!(
        "INSERT INTO unlocked_templates(user_id,template_id,source) VALUES ($1,$2,'start')",
        user,
        template
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        "INSERT INTO avatars(id,user_id,slot,template_id,is_active) VALUES ($1,$2,0,$3,true)",
        avatar,
        user,
        template
    )
    .execute(&mut *tx)
    .await?;
    for (card, qty) in starter.cards {
        let card = *card as i16;
        let qty = i32::from(*qty);
        sqlx::query!(
            "INSERT INTO collection(user_id,card_id,qty) VALUES ($1,$2,$3)",
            user,
            card,
            qty
        )
        .execute(&mut *tx)
        .await?;
    }
    let deck: Vec<i16> = starter.deck.iter().map(|card| *card as i16).collect();
    sqlx::query!(
        "INSERT INTO decks(avatar_id,cards) VALUES ($1,$2)",
        avatar,
        &deck
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!("INSERT INTO ratings(user_id) VALUES ($1)", user)
        .execute(&mut *tx)
        .await?;
    crate::economy::grant_initial(&mut tx, user)
        .await
        .map_err(|_| Error::busy())?;
    message(&state, &mut tx, user, &email, "verify").await?;
    tx.commit().await?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({"registered":true})),
    ))
}

async fn lock_user(tx: &mut Transaction<'_, Postgres>, user: Uuid) -> Result<()> {
    sqlx::query!(
        "SELECT id FROM users WHERE id=$1 AND status='active' FOR UPDATE",
        user
    )
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(Error::unauthorized)?;
    Ok(())
}

async fn lock_email_token(
    tx: &mut Transaction<'_, Postgres>,
    hash: &[u8],
    kind: &str,
) -> Result<Uuid> {
    // Every token operation locks the account before its tokens, including resend/reset.
    let row = sqlx::query!("SELECT user_id FROM email_tokens WHERE token_hash=$1 AND kind=$2 AND used_at IS NULL AND expires_at>now()", hash, kind).fetch_optional(&mut **tx).await?.ok_or(Error(StatusCode::BAD_REQUEST,"INVALID_TOKEN",0))?;
    lock_user(tx, row.user_id)
        .await
        .map_err(|_| Error(StatusCode::BAD_REQUEST, "INVALID_TOKEN", 0))?;
    let row = sqlx::query!("SELECT user_id FROM email_tokens WHERE token_hash=$1 AND kind=$2 AND used_at IS NULL AND expires_at>now() FOR UPDATE", hash, kind).fetch_optional(&mut **tx).await?.ok_or(Error(StatusCode::BAD_REQUEST,"INVALID_TOKEN",0))?;
    Ok(row.user_id)
}

async fn insert_session(
    tx: &mut Transaction<'_, Postgres>,
    state: &AppState,
    user: Uuid,
    raw: &str,
    android: bool,
    headers: &HeaderMap,
    ip: &str,
) -> Result<Option<(Uuid, Vec<u8>)>> {
    let mut replaced = None;
    if !android && let Some(old) = security::cookie_token(&state.auth, headers) {
        let old = crypto::hash(&old);
        if let Some(row) = sqlx::query!(
            "DELETE FROM sessions WHERE id_hash=$1 AND kind='web' RETURNING user_id",
            old
        )
        .fetch_optional(&mut **tx)
        .await?
        {
            replaced = Some((row.user_id, old));
        }
    }
    sqlx::query!(
        "DELETE FROM sessions WHERE user_id=$1 AND expires_at<=now()",
        user
    )
    .execute(&mut **tx)
    .await?;
    let count = sqlx::query_scalar!(
        "SELECT count(*) AS \"count!\" FROM sessions WHERE user_id=$1",
        user
    )
    .fetch_one(&mut **tx)
    .await?;
    if count >= 20 {
        return Err(Error(StatusCode::TOO_MANY_REQUESTS, "SESSION_LIMIT", 0));
    }
    let hash = crypto::hash(raw);
    let kind = if android { "app" } else { "web" };
    let agent = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .chars()
        .take(512)
        .collect::<String>();
    let device = if android { "Android" } else { "Web" };
    let ip_hash = state.auth.limit_key("session-ip", ip);
    sqlx::query!("INSERT INTO sessions(id_hash,kind,user_id,expires_at,user_agent,device_label,rotated_at,ip_hash) VALUES ($1,$2,$3,now()+interval '30 days',$4,$5,now(),$6)", hash, kind, user, agent, device, ip_hash).execute(&mut **tx).await?;
    Ok(replaced)
}

pub async fn login(
    State(state): State<AppState>,
    peer: security::Peer,
    headers: HeaderMap,
    Json(input): Json<Login>,
) -> Result<Response> {
    security::client(&state.auth, &headers, input.client.android())?;
    if input.identity.len() > 254 || input.password.len() > 512 {
        return Err(Error::unauthorized());
    }
    let ip = security::peer_key(&state.auth, peer.0, &headers);
    state.auth.limit("login-ip", &ip, 20, 900)?;
    let identity = input.identity.trim().to_lowercase();
    let row = sqlx::query!(
        "SELECT id, password_hash, status FROM users WHERE login=$1::citext OR email=$1::citext",
        identity
    )
    .fetch_optional(&state.db)
    .await?;
    let key = row
        .as_ref()
        .map(|row| row.id.to_string())
        .unwrap_or(identity);
    let attempt = state.auth.limit("login-account", &key, 5, 900)?;
    let valid = state
        .auth
        .check_password(
            input.password,
            row.as_ref().map(|row| row.password_hash.clone()),
        )
        .await?;
    if !valid || row.as_ref().is_none_or(|row| row.status != "active") {
        tokio::time::sleep(Duration::from_millis(100 << (attempt - 1).min(3))).await;
        return Err(Error::unauthorized());
    }
    let row = row.expect("verified existing account");
    let mut tx = state.db.begin().await?;
    let current = sqlx::query!(
        "SELECT password_hash, status FROM users WHERE id=$1 FOR UPDATE",
        row.id
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(Error::unauthorized)?;
    if current.password_hash != row.password_hash || current.status != "active" {
        return Err(Error::unauthorized());
    }
    let raw = crypto::token().map_err(|_| Error::busy())?;
    let replaced = insert_session(
        &mut tx,
        &state,
        row.id,
        &raw,
        input.client.android(),
        &headers,
        &ip,
    )
    .await?;
    sqlx::query!("UPDATE users SET last_login_at=now() WHERE id=$1", row.id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    if let Some((user, hash)) = replaced {
        state.online.hub.revoke(user, Some(hash)).await;
    }
    state
        .auth
        .0
        .limits
        .clear(&state.auth.limit_key("login-account", &key));
    let profile = profile(&state.db, row.id).await?;
    let mut response = if input.client.android() {
        Json(serde_json::json!({"profile":profile,"token":raw,"expires_in":2592000}))
            .into_response()
    } else {
        Json(serde_json::json!({"profile":profile})).into_response()
    };
    if !input.client.android() {
        response.headers_mut().insert(
            header::SET_COOKIE,
            security::session_cookie(&state.auth, &raw)
                .parse()
                .expect("generated cookie"),
        );
    }
    Ok(response)
}

pub async fn me(State(state): State<AppState>, headers: HeaderMap) -> Result<Json<super::Profile>> {
    let session = security::session(&state, &headers).await?;
    Ok(Json(profile(&state.db, session.user).await?))
}

pub async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Result<Response> {
    let (raw, kind) = security::credential(&state.auth, &headers)?;
    let hash = crypto::hash(&raw);
    let removed = sqlx::query!(
        "DELETE FROM sessions WHERE id_hash=$1 AND kind=$2 RETURNING user_id",
        hash,
        kind
    )
    .fetch_optional(&state.db)
    .await?;
    if let Some(removed) = removed {
        state.online.hub.revoke(removed.user_id, Some(hash)).await;
    }
    let mut response = StatusCode::NO_CONTENT.into_response();
    if kind == "web" {
        response.headers_mut().insert(
            header::SET_COOKIE,
            security::session_cookie(&state.auth, "").parse().unwrap(),
        );
    }
    Ok(response)
}

pub async fn verify_email(
    State(state): State<AppState>,
    peer: security::Peer,
    headers: HeaderMap,
    Json(input): Json<Token>,
) -> Result<Json<serde_json::Value>> {
    let ip = security::peer_key(&state.auth, peer.0, &headers);
    state.auth.limit("verify", &ip, 20, 900)?;
    if crypto::unhex(&input.token).is_none() {
        return Err(Error::bad());
    }
    let hash = crypto::hash(&input.token);
    let mut tx = state.db.begin().await?;
    let user_id = lock_email_token(&mut tx, &hash, "verify").await?;
    let user = sqlx::query!("UPDATE users SET email_verified_at=COALESCE(email_verified_at,now()) WHERE id=$1 AND status='active' RETURNING id", user_id).fetch_optional(&mut *tx).await?;
    if user.is_none() {
        return Err(Error(StatusCode::BAD_REQUEST, "INVALID_TOKEN", 0));
    }
    sqlx::query!(
        "UPDATE email_tokens SET used_at=now() WHERE token_hash=$1",
        hash
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(serde_json::json!({"verified":true})))
}

async fn request_email(
    state: AppState,
    peer: security::Peer,
    headers: HeaderMap,
    input: Email,
    kind: &'static str,
) -> Result<Json<serde_json::Value>> {
    let ip = security::peer_key(&state.auth, peer.0, &headers);
    state.auth.limit("email-ip", &ip, 10, 3600)?;
    let email = email(&input.email).ok_or_else(Error::bad)?;
    state.auth.limit("email-account", &email, 3, 3600)?;
    // Return before account lookup, so SMTP/DB latency cannot disclose account existence.
    // Bound background work; a failed delivery rolls back the token and may be retried.
    if let Ok(permit) = state.auth.0.mailing.clone().try_acquire_owned() {
        tokio::spawn(async move {
            let _permit = permit;
            let result: Result<()> = async {
                let mut tx = state.db.begin().await?;
                let user = sqlx::query!("SELECT id, email_verified_at IS NOT NULL AS \"verified!\" FROM users WHERE email=$1::citext AND status='active' FOR UPDATE", email).fetch_optional(&mut *tx).await?;
                if let Some(user) = user && (kind == "reset" || !user.verified) {
                    message(&state, &mut tx, user.id, &email, kind).await?;
                    tx.commit().await?;
                }
                Ok(())
            }.await;
            if result.is_err() {
                tracing::warn!("auth email request could not be completed");
            }
        });
    }
    Ok(Json(serde_json::json!({"accepted":true})))
}
pub async fn resend(
    State(state): State<AppState>,
    peer: security::Peer,
    headers: HeaderMap,
    Json(input): Json<Email>,
) -> Result<Json<serde_json::Value>> {
    request_email(state, peer, headers, input, "verify").await
}
pub async fn reset_request(
    State(state): State<AppState>,
    peer: security::Peer,
    headers: HeaderMap,
    Json(input): Json<Email>,
) -> Result<Json<serde_json::Value>> {
    request_email(state, peer, headers, input, "reset").await
}

pub async fn reset_confirm(
    State(state): State<AppState>,
    peer: security::Peer,
    headers: HeaderMap,
    Json(input): Json<Reset>,
) -> Result<Json<serde_json::Value>> {
    let ip = security::peer_key(&state.auth, peer.0, &headers);
    state.auth.limit("reset-confirm", &ip, 10, 900)?;
    if crypto::unhex(&input.token).is_none() || !crypto::password_allowed(&input.password) {
        return Err(Error::bad());
    }
    let hash = crypto::hash(&input.token);
    let password = state.auth.hash_password(input.password).await?;
    let mut tx = state.db.begin().await?;
    let user_id = lock_email_token(&mut tx, &hash, "reset").await?;
    let user = sqlx::query!(
        "UPDATE users SET password_hash=$2 WHERE id=$1 AND status='active' RETURNING id",
        user_id,
        password
    )
    .fetch_optional(&mut *tx)
    .await?;
    if user.is_none() {
        return Err(Error(StatusCode::BAD_REQUEST, "INVALID_TOKEN", 0));
    }
    sqlx::query!("DELETE FROM sessions WHERE user_id=$1", user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query!(
        "UPDATE email_tokens SET used_at=now() WHERE user_id=$1 AND used_at IS NULL",
        user_id
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    state.online.hub.revoke(user_id, None).await;
    Ok(Json(serde_json::json!({"reset":true})))
}

pub async fn rotate(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>> {
    let current = security::session(&state, &headers).await?;
    if current.kind != "app" {
        return Err(Error::forbidden());
    }
    state
        .auth
        .limit("rotate", &crypto::hex(&current.hash), 5, 60)?;
    let mut tx = state.db.begin().await?;
    lock_user(&mut tx, current.user).await?;
    let row = sqlx::query!("SELECT id_hash FROM sessions WHERE id_hash=$1 AND kind='app' AND expires_at>now() FOR UPDATE", current.hash).fetch_optional(&mut *tx).await?;
    if row.is_none() {
        return Err(Error::unauthorized());
    }
    let raw = crypto::token().map_err(|_| Error::busy())?;
    let next = crypto::hash(&raw);
    sqlx::query!("UPDATE sessions SET id_hash=$2, rotated_at=now(), last_seen_at=now(), expires_at=now()+interval '30 days' WHERE id_hash=$1", current.hash, next).execute(&mut *tx).await?;
    tx.commit().await?;
    state
        .online
        .hub
        .revoke(current.user, Some(current.hash))
        .await;
    Ok(Json(serde_json::json!({"token":raw,"expires_in":2592000})))
}

pub async fn sessions(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>> {
    let current = security::session(&state, &headers).await?;
    let rows = sqlx::query!("SELECT id_hash, kind, device_label, created_at::text AS \"created_at!\", last_seen_at::text AS \"last_seen_at!\", expires_at::text AS \"expires_at!\" FROM sessions WHERE user_id=$1 AND expires_at>now() ORDER BY created_at DESC", current.user).fetch_all(&state.db).await?;
    Ok(Json(
        serde_json::json!({"sessions":rows.iter().map(|row| serde_json::json!({"id":crypto::hex(&row.id_hash),"kind":row.kind,"device":row.device_label,"created_at":row.created_at,"last_seen_at":row.last_seen_at,"expires_at":row.expires_at,"current":row.id_hash==current.hash})).collect::<Vec<_>>()}),
    ))
}

pub async fn revoke(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode> {
    let current = security::session(&state, &headers).await?;
    let hash = crypto::unhex(&id).ok_or_else(Error::bad)?;
    sqlx::query!(
        "DELETE FROM sessions WHERE id_hash=$1 AND user_id=$2",
        hash,
        current.user
    )
    .execute(&state.db)
    .await?;
    state.online.hub.revoke(current.user, Some(hash)).await;
    Ok(StatusCode::NO_CONTENT)
}
