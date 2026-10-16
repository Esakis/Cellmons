//! Account rights use a valid own credential even when gameplay is banned.
mod erasure;
mod export;
pub use erasure::cleanup;
pub use export::export;

use super::{Error, Result, Session, crypto, security};
use crate::gateway::AppState;
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse as _, Response},
};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Login {
    identity: String,
    password: String,
    #[serde(default)]
    client: super::handlers::Client,
}
pub async fn login(
    State(state): State<AppState>,
    peer: security::Peer,
    headers: HeaderMap,
    Json(input): Json<Login>,
) -> Result<Response> {
    let android = input.client.android();
    security::client(&state.auth, &headers, android)?;
    let ip = security::peer_key(&state.auth, peer.0, &headers);
    state.auth.limit("login-ip", &ip, 20, 900)?;
    if input.identity.len() > 254 || input.password.len() > 512 {
        return Err(Error::unauthorized());
    }
    let identity = input.identity.trim().to_lowercase();
    let row: Option<(Uuid,String)>=sqlx::query_as("SELECT id,password_hash FROM users WHERE (login=$1::citext OR email=$1::citext) AND status IN ('active','banned')")
        .bind(&identity).fetch_optional(&state.db).await?;
    let key = row
        .as_ref()
        .map(|row| row.0.to_string())
        .unwrap_or(identity);
    let attempt = state.auth.limit("login-account", &key, 5, 900)?;
    let valid = state
        .auth
        .check_password(input.password, row.as_ref().map(|row| row.1.clone()))
        .await?;
    let Some((user, password)) = row.filter(|_| valid) else {
        tokio::time::sleep(std::time::Duration::from_millis(
            100 << (attempt - 1).min(3),
        ))
        .await;
        return Err(Error::unauthorized());
    };
    let mut tx = state.db.begin().await?;
    let fresh: Option<String> = sqlx::query_scalar(
        "SELECT password_hash FROM users WHERE id=$1 AND status IN ('active','banned') FOR UPDATE",
    )
    .bind(user)
    .fetch_optional(&mut *tx)
    .await?;
    if fresh.as_deref() != Some(&password) {
        return Err(Error::unauthorized());
    }
    let raw = crypto::token().map_err(|_| Error::busy())?;
    let replaced =
        super::handlers::insert_session(&mut tx, &state, user, &raw, android, &headers, &ip)
            .await?;
    sqlx::query("UPDATE sessions SET purpose='privacy',expires_at=now()+interval '15 minutes' WHERE id_hash=$1")
        .bind(crypto::hash(&raw)).execute(&mut *tx).await?;
    tx.commit().await?;
    if let Some((user, hash)) = replaced {
        state.online.hub.revoke(user, Some(hash)).await;
    }
    state
        .auth
        .0
        .limits
        .clear(&state.auth.limit_key("login-account", &key));
    let mut response = Json(if android {
        serde_json::json!({"token":raw})
    } else {
        serde_json::json!({"authenticated":true})
    })
    .into_response();
    if !android {
        response.headers_mut().insert(
            header::SET_COOKIE,
            security::session_cookie(&state.auth, &raw).parse().unwrap(),
        );
    }
    Ok(response)
}

async fn session(state: &AppState, headers: &HeaderMap) -> Result<Session> {
    let (raw, kind) = security::credential(&state.auth, headers)?;
    let hash = crypto::hash(&raw);
    let user=sqlx::query_scalar::<_,Uuid>("SELECT s.user_id FROM sessions s JOIN users u ON u.id=s.user_id WHERE s.id_hash=$1 AND s.kind=$2 AND s.expires_at>now() AND u.status IN ('active','banned')")
        .bind(&hash).bind(kind).fetch_optional(&state.db).await?.ok_or_else(Error::unauthorized)?;
    Ok(Session {
        hash,
        user,
        kind: kind.into(),
    })
}
pub async fn summary(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>> {
    let current = session(&state, &headers).await?;
    state
        .auth
        .limit("privacy-summary", &current.user.to_string(), 30, 60)?;
    let row: (String,String,bool,bool)=sqlx::query_as("SELECT login::text,status,EXISTS(SELECT 1 FROM duel_locks WHERE user_id=$1) OR EXISTS(SELECT 1 FROM activity_locks WHERE user_id=$1),EXISTS(SELECT 1 FROM clan_members m WHERE m.user_id=$1 AND m.role='master' AND (SELECT count(*) FROM clan_members WHERE clan_id=m.clan_id)>1) FROM users WHERE id=$1")
        .bind(current.user).fetch_one(&state.db).await?;
    Ok(Json(
        serde_json::json!({"id":current.user,"login":row.0,"status":row.1,"busy":row.2,"clan_master":row.3,"grace_days":30,"session":crypto::hex(&current.hash)}),
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Delete {
    password: String,
    confirm_login: String,
}
pub async fn request_deletion(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<Delete>,
) -> Result<Response> {
    let current = session(&state, &headers).await?;
    state
        .auth
        .limit("privacy-delete", &current.user.to_string(), 5, 900)?;
    if input.password.len() > 512 || input.confirm_login.len() > 20 {
        return Err(Error::bad());
    }
    let (login, password): (String, String) =
        sqlx::query_as("SELECT login::text,password_hash FROM users WHERE id=$1")
            .bind(current.user)
            .fetch_one(&state.db)
            .await?;
    if !login.eq_ignore_ascii_case(input.confirm_login.trim())
        || !state
            .auth
            .check_password(input.password, Some(password.clone()))
            .await?
    {
        return Err(Error(StatusCode::UNAUTHORIZED, "INVALID_CREDENTIALS", 0));
    }
    let mut tx = state.db.begin().await?;
    let fresh: (String, String, String) =
        sqlx::query_as("SELECT password_hash,status,role FROM users WHERE id=$1 FOR UPDATE")
            .bind(current.user)
            .fetch_one(&mut *tx)
            .await?;
    if fresh.0 != password || !matches!(fresh.1.as_str(), "active" | "banned") {
        return Err(Error::unauthorized());
    }
    let live_session:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sessions WHERE id_hash=$1 AND user_id=$2 AND kind=$3 AND expires_at>now())").bind(&current.hash).bind(current.user).bind(&current.kind).fetch_one(&mut *tx).await?;
    if !live_session {
        return Err(Error::unauthorized());
    }
    if matches!(fresh.2.as_str(), "admin" | "moderator") {
        return Err(Error(StatusCode::CONFLICT, "PRIVACY_ADMIN", 0));
    }
    let busy:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM duel_locks WHERE user_id=$1) OR EXISTS(SELECT 1 FROM activity_locks WHERE user_id=$1)").bind(current.user).fetch_one(&mut *tx).await?;
    if busy {
        return Err(Error(StatusCode::CONFLICT, "BUSY", 0));
    }
    let member: Option<(Uuid, String)> =
        sqlx::query_as("SELECT clan_id,role FROM clan_members WHERE user_id=$1")
            .bind(current.user)
            .fetch_optional(&mut *tx)
            .await?;
    let mut affected = vec![current.user];
    if let Some((clan, _)) = member {
        sqlx::query("SELECT id FROM clans WHERE id=$1 FOR UPDATE")
            .bind(clan)
            .fetch_one(&mut *tx)
            .await?;
        let fresh_role: Option<String> =
            sqlx::query_scalar("SELECT role FROM clan_members WHERE clan_id=$1 AND user_id=$2")
                .bind(clan)
                .bind(current.user)
                .fetch_optional(&mut *tx)
                .await?;
        if let Some(role) = fresh_role {
            let members: Vec<Uuid> =
                sqlx::query_scalar("SELECT user_id FROM clan_members WHERE clan_id=$1")
                    .bind(clan)
                    .fetch_all(&mut *tx)
                    .await?;
            if role == "master" && members.len() > 1 {
                return Err(Error(StatusCode::CONFLICT, "CLAN_MASTER", 0));
            }
            affected.extend(members);
            if role == "master" {
                sqlx::query("UPDATE clans SET status='disbanded' WHERE id=$1")
                    .bind(clan)
                    .execute(&mut *tx)
                    .await?;
                sqlx::query("DELETE FROM clan_applications WHERE clan_id=$1")
                    .bind(clan)
                    .execute(&mut *tx)
                    .await?;
            }
            sqlx::query("DELETE FROM clan_members WHERE user_id=$1")
                .bind(current.user)
                .execute(&mut *tx)
                .await?;
            sqlx::query("UPDATE clans SET revision=revision+1 WHERE id=$1")
                .bind(clan)
                .execute(&mut *tx)
                .await?;
            sqlx::query("INSERT INTO clan_audit(id,clan_id,actor_id,action,details) VALUES($1,$2,$3,'leave','{\"reason\":\"account_deletion\"}'::jsonb)").bind(Uuid::now_v7()).bind(clan).bind(current.user).execute(&mut *tx).await?;
        }
    }
    // This operation remains available during maintenance and cannot create a new activity.
    sqlx::query("UPDATE users SET deletion_previous_status=status,status='pending_deletion',deletion_requested_at=now() WHERE id=$1").bind(current.user).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM sessions WHERE user_id=$1")
        .bind(current.user)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM email_tokens WHERE user_id=$1")
        .bind(current.user)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM admin_totp WHERE user_id=$1")
        .bind(current.user)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM clan_applications WHERE user_id=$1")
        .bind(current.user)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO privacy_events(id,user_id,event) VALUES($1,$2,'requested')")
        .bind(Uuid::now_v7())
        .bind(current.user)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    state.online.hub.revoke(current.user, None).await;
    state.online.hub.refresh_clans(affected).await;
    let mut response = StatusCode::NO_CONTENT.into_response();
    if current.kind == "web" {
        response.headers_mut().insert(
            header::SET_COOKIE,
            security::session_cookie(&state.auth, "").parse().unwrap(),
        );
    }
    Ok(response)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Restore {
    identity: String,
    password: String,
}
pub async fn restore(
    State(state): State<AppState>,
    peer: security::Peer,
    headers: HeaderMap,
    Json(input): Json<Restore>,
) -> Result<StatusCode> {
    let key = security::peer_key(&state.auth, peer.0, &headers);
    state.auth.limit("privacy-identity", &key, 5, 900)?;
    if input.identity.len() > 254 || input.password.len() > 512 {
        return Err(Error::unauthorized());
    }
    let row:Option<(Uuid,String)>=sqlx::query_as("SELECT id,password_hash FROM users WHERE (login=$1::citext OR email=$1::citext) AND status='pending_deletion' AND deletion_requested_at>now()-interval '30 days'").bind(input.identity.trim().to_lowercase()).fetch_optional(&state.db).await?;
    let valid = state
        .auth
        .check_password(input.password, row.as_ref().map(|r| r.1.clone()))
        .await?;
    let Some((user, password)) = row.filter(|_| valid) else {
        return Err(Error::unauthorized());
    };
    let mut tx = state.db.begin().await?;
    let fresh:Option<String>=sqlx::query_scalar("SELECT password_hash FROM users WHERE id=$1 AND status='pending_deletion' AND deletion_requested_at>now()-interval '30 days' FOR UPDATE").bind(user).fetch_optional(&mut *tx).await?;
    if fresh.as_deref() != Some(&password) {
        return Err(Error::unauthorized());
    }
    sqlx::query("UPDATE users SET status=CASE WHEN deletion_previous_status='banned' AND (banned_until IS NULL OR banned_until>now()) THEN 'banned' ELSE 'active' END,deletion_requested_at=NULL,deletion_previous_status=NULL WHERE id=$1").bind(user).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO privacy_events(id,user_id,event) VALUES($1,$2,'restored')")
        .bind(Uuid::now_v7())
        .bind(user)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Resolve a lost deletion acknowledgement without restoring access or requiring an app install.
pub async fn deletion_status(
    State(state): State<AppState>,
    peer: security::Peer,
    headers: HeaderMap,
    Json(input): Json<Restore>,
) -> Result<Json<serde_json::Value>> {
    let key = security::peer_key(&state.auth, peer.0, &headers);
    state.auth.limit("privacy-identity", &key, 5, 900)?;
    if input.identity.len() > 254 || input.password.len() > 512 {
        return Err(Error::unauthorized());
    }
    let row:Option<(Uuid,String)>=sqlx::query_as("SELECT id,password_hash FROM users WHERE (login=$1::citext OR email=$1::citext) AND status<>'deleted'").bind(input.identity.trim().to_lowercase()).fetch_optional(&state.db).await?;
    let valid = state
        .auth
        .check_password(input.password, row.as_ref().map(|r| r.1.clone()))
        .await?;
    let Some((user, password)) = row.filter(|_| valid) else {
        return Err(Error::unauthorized());
    };
    let value:Option<String>=sqlx::query_scalar("SELECT json_build_object('status',status,'pending',status='pending_deletion','erase_after',deletion_requested_at+interval '30 days')::text FROM users WHERE id=$1 AND password_hash=$2").bind(user).bind(password).fetch_optional(&state.db).await?;
    Ok(Json(
        serde_json::from_str(&value.ok_or_else(Error::unauthorized)?).map_err(|_| Error::busy())?,
    ))
}
