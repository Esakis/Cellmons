//! Password + TOTP step-up, bound to the original session, its owner, the peer and factor revision.
use super::{Config, crypto};
use crate::{
    auth::{
        self,
        security::{self, Peer},
    },
    gateway::AppState,
    online::{Error, Result},
};
use axum::{Json, extract::State, http::HeaderMap};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::PgConnection;
use uuid::Uuid;

#[derive(Clone)]
pub struct Identity {
    pub(super) session: auth::Session,
    pub(super) config: Config,
    pub(super) ip_hash: Vec<u8>,
    pub(super) login: String,
}
impl Identity {
    pub fn user(&self) -> Uuid {
        self.session.user
    }
}
pub async fn identity(
    state: &AppState,
    headers: &HeaderMap,
    peer: Option<axum::extract::ConnectInfo<std::net::SocketAddr>>,
) -> std::result::Result<Identity, crate::game::HttpError> {
    let config = state
        .auth
        .config()
        .admin
        .clone()
        .ok_or_else(|| crate::game::response(Error("FORBIDDEN")))?;
    let ip = security::peer_key(&state.auth, peer, headers)
        .parse::<std::net::IpAddr>()
        .map_err(|_| crate::game::response(Error("FORBIDDEN")))?
        .to_canonical();
    if !config.allows(ip) {
        return Err(crate::game::response(Error("FORBIDDEN")));
    }
    let session = security::session(state, headers)
        .await
        .map_err(crate::game::HttpError)?;
    let login:String=sqlx::query_scalar("SELECT login::text FROM users WHERE id=$1 AND status='active' AND email_verified_at IS NOT NULL AND role IN ('admin','moderator')").bind(session.user).fetch_optional(&state.db).await.map_err(|e|crate::game::response(e.into()))?.ok_or_else(||crate::game::response(Error("FORBIDDEN")))?;
    let ip_hash = auth::crypto::sign(
        &config.key,
        format!("spellgambit-admin-peer-v1:{ip}").as_bytes(),
    );
    Ok(Identity {
        session,
        config,
        ip_hash,
        login,
    })
}
async fn current(connection: &mut PgConnection, id: &Identity) -> Result<String> {
    sqlx::query_scalar("SELECT u.role FROM users u JOIN sessions s ON s.user_id=u.id WHERE u.id=$1 AND u.status='active' AND u.email_verified_at IS NOT NULL AND u.role IN ('admin','moderator') AND s.id_hash=$2 AND s.kind=$3 AND s.expires_at>clock_timestamp()")
        .bind(id.user()).bind(&id.session.hash).bind(&id.session.kind).fetch_optional(connection).await?.ok_or(Error("FORBIDDEN"))
}
pub(super) async fn require(connection: &mut PgConnection, id: &Identity) -> Result<String> {
    let role = current(connection, id).await?;
    let authorized:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM admin_sessions a JOIN admin_totp t ON t.user_id=a.user_id WHERE a.session_hash=$1 AND a.user_id=$2 AND a.ip_hash=$3 AND a.expires_at>clock_timestamp() AND a.totp_revision=t.revision AND t.secret_cipher IS NOT NULL)")
        .bind(&id.session.hash).bind(id.user()).bind(&id.ip_hash).fetch_one(connection).await?;
    if authorized {
        Ok(role)
    } else {
        Err(Error("STEP_UP_REQUIRED"))
    }
}
#[derive(Serialize)]
pub struct Gate {
    pub role: String,
    pub enrolled: bool,
    pub authorized: bool,
    pub expires_ms: Option<i64>,
}
pub async fn gate(state: &AppState, id: &Identity) -> Result<Gate> {
    let mut connection = state.db.acquire().await?;
    let role = current(&mut connection, id).await?;
    let enrolled: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM admin_totp WHERE user_id=$1 AND secret_cipher IS NOT NULL)",
    )
    .bind(id.user())
    .fetch_one(&mut *connection)
    .await?;
    let expires_ms:Option<i64>=sqlx::query_scalar("SELECT (extract(epoch FROM a.expires_at)*1000)::bigint FROM admin_sessions a JOIN admin_totp t ON t.user_id=a.user_id WHERE a.session_hash=$1 AND a.user_id=$2 AND a.ip_hash=$3 AND a.expires_at>clock_timestamp() AND a.totp_revision=t.revision AND t.secret_cipher IS NOT NULL")
        .bind(&id.session.hash).bind(id.user()).bind(&id.ip_hash).fetch_optional(&mut *connection).await?;
    Ok(Gate {
        role,
        enrolled,
        authorized: expires_ms.is_some(),
        expires_ms,
    })
}
pub(super) async fn audit(
    connection: &mut PgConnection,
    id: &Identity,
    action: &str,
    target: Option<Uuid>,
    request: Option<Uuid>,
    reason: &str,
    details: serde_json::Value,
) -> Result<()> {
    sqlx::query("INSERT INTO admin_audit(id,actor,action,target,request_id,reason,details) VALUES ($1,$2,$3,$4,$5,$6,$7)")
        .bind(Uuid::now_v7()).bind(id.user()).bind(action).bind(target).bind(request).bind(reason).bind(serde_json::to_vec(&details).map_err(|_|Error("INTERNAL"))?).execute(connection).await?;
    Ok(())
}
#[derive(Serialize)]
pub struct Setup {
    pub uri: String,
    pub secret: String,
    pub expires_ms: i64,
}
async fn password(state: &AppState, id: &Identity, value: String) -> Result<String> {
    if value.is_empty() || value.len() > 512 {
        return Err(Error("INVALID_CREDENTIALS"));
    }
    let stored: Option<String> = sqlx::query_scalar("SELECT password_hash FROM users WHERE id=$1")
        .bind(id.user())
        .fetch_optional(&state.db)
        .await?;
    let valid = state
        .auth
        .check_password(value, stored.clone())
        .await
        .map_err(|_| Error("UNAVAILABLE"))?;
    if !valid {
        return Err(Error("INVALID_CREDENTIALS"));
    }
    stored.ok_or(Error("FORBIDDEN"))
}
async fn lock_identity(
    connection: &mut PgConnection,
    id: &Identity,
    hash: Option<&str>,
) -> Result<()> {
    let stored: String =
        sqlx::query_scalar("SELECT password_hash FROM users WHERE id=$1 FOR UPDATE")
            .bind(id.user())
            .fetch_one(&mut *connection)
            .await?;
    if hash.is_some_and(|hash| hash != stored) {
        return Err(Error("INVALID_CREDENTIALS"));
    }
    current(connection, id).await?;
    Ok(())
}
pub async fn begin_setup(state: &AppState, id: &Identity, value: String) -> Result<Setup> {
    let stored = password(state, id, value).await?;
    let mut tx = state.db.begin().await?;
    lock_identity(&mut tx, id, Some(&stored)).await?;
    sqlx::query("INSERT INTO admin_totp(user_id) VALUES ($1) ON CONFLICT DO NOTHING")
        .bind(id.user())
        .execute(&mut *tx)
        .await?;
    let (active,pending,until):(Option<Vec<u8>>,Option<Vec<u8>>,i64)=sqlx::query_as("SELECT secret_cipher,pending_cipher,pending_until_ms FROM admin_totp WHERE user_id=$1 FOR UPDATE").bind(id.user()).fetch_one(&mut *tx).await?;
    if active.is_some() {
        return Err(Error("TOTP_ALREADY_ENABLED"));
    }
    let now = crate::online::now_ms();
    let (secret, expires_ms) = if let Some(pending) = pending.filter(|_| until > now) {
        (crypto::open(&id.config, id.user(), &pending)?, until)
    } else {
        let mut secret = vec![0; 20];
        getrandom::fill(&mut secret).map_err(|_| Error("INTERNAL"))?;
        let sealed = crypto::seal(&id.config, id.user(), &secret)?;
        let expires = now + 600000;
        sqlx::query("UPDATE admin_totp SET pending_cipher=$2,pending_until_ms=$3 WHERE user_id=$1")
            .bind(id.user())
            .bind(sealed)
            .bind(expires)
            .execute(&mut *tx)
            .await?;
        audit(
            &mut tx,
            id,
            "totp_setup",
            None,
            None,
            "",
            serde_json::json!({"expires_ms":expires}),
        )
        .await?;
        (secret, expires)
    };
    let totp = crypto::totp(secret, &id.login)?;
    let setup = Setup {
        uri: totp.to_url().map_err(|_| Error("INTERNAL"))?,
        secret: totp.secret().to_base32(),
        expires_ms,
    };
    tx.commit().await?;
    Ok(setup)
}
fn factor_fingerprint(id: &Identity, confirm: bool) -> Vec<u8> {
    let mut hash = Sha256::new();
    hash.update(b"spellgambit-admin-factor-request-v1");
    hash.update(&id.session.hash);
    hash.update(&id.ip_hash);
    hash.update([u8::from(confirm)]);
    hash.finalize().to_vec()
}
pub async fn complete_factor(
    state: &AppState,
    id: &Identity,
    request: Uuid,
    confirm: bool,
    value: Option<String>,
    code: String,
) -> Result<()> {
    let stored = match value {
        Some(value) => Some(password(state, id, value).await?),
        None if confirm => None,
        None => return Err(Error("INVALID_CREDENTIALS")),
    };
    let mut tx = state.db.begin().await?;
    lock_identity(&mut tx, id, stored.as_deref()).await?;
    let fingerprint = factor_fingerprint(id, confirm);
    if let Some(old) = sqlx::query_scalar::<_, Vec<u8>>(
        "SELECT fingerprint FROM admin_requests WHERE actor=$1 AND request_id=$2",
    )
    .bind(id.user())
    .bind(request)
    .fetch_optional(&mut *tx)
    .await?
    {
        if old != fingerprint {
            return Err(Error("CONFLICT"));
        }
        require(&mut tx, id).await?;
        return Ok(());
    }
    let (active,pending,until,last,revision):(Option<Vec<u8>>,Option<Vec<u8>>,i64,i64,i32)=sqlx::query_as("SELECT secret_cipher,pending_cipher,pending_until_ms,last_step,revision FROM admin_totp WHERE user_id=$1 FOR UPDATE").bind(id.user()).fetch_optional(&mut *tx).await?.ok_or(Error("TOTP_SETUP_REQUIRED"))?;
    let cipher = if confirm {
        if active.is_some() || until <= crate::online::now_ms() {
            return Err(Error("TOTP_SETUP_REQUIRED"));
        }
        pending.ok_or(Error("TOTP_SETUP_REQUIRED"))?
    } else {
        active.ok_or(Error("TOTP_SETUP_REQUIRED"))?
    };
    if code.len() != 6 || !code.bytes().all(|b| b.is_ascii_digit()) {
        return Err(Error("INVALID_OTP"));
    }
    let totp = crypto::totp(crypto::open(&id.config, id.user(), &cipher)?, &id.login)?;
    let step = totp
        .check(&code, (crate::online::now_ms().max(0) as u64) / 1000)
        .and_then(|step| i64::try_from(step).ok())
        .filter(|step| *step > last)
        .ok_or(Error("INVALID_OTP"))?;
    let revision = if confirm {
        revision.checked_add(1).ok_or(Error("CONFLICT"))?
    } else {
        revision
    };
    sqlx::query("UPDATE admin_totp SET secret_cipher=$2,pending_cipher=NULL,pending_until_ms=0,last_step=$3,revision=$4 WHERE user_id=$1").bind(id.user()).bind(cipher).bind(step).bind(revision).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO admin_sessions(session_hash,user_id,totp_revision,ip_hash,expires_at) VALUES ($1,$2,$3,$4,now()+interval '15 minutes') ON CONFLICT(session_hash) DO UPDATE SET totp_revision=excluded.totp_revision,ip_hash=excluded.ip_hash,expires_at=excluded.expires_at")
        .bind(&id.session.hash).bind(id.user()).bind(revision).bind(&id.ip_hash).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO admin_requests(actor,request_id,fingerprint) VALUES ($1,$2,$3)")
        .bind(id.user())
        .bind(request)
        .bind(fingerprint)
        .execute(&mut *tx)
        .await?;
    audit(
        &mut tx,
        id,
        if confirm { "totp_confirm" } else { "step_up" },
        None,
        Some(request),
        "",
        serde_json::json!({"revision":revision}),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}
#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum Mutation {
    Setup {
        password: String,
    },
    Confirm {
        request: Uuid,
        code: String,
    },
    Unlock {
        request: Uuid,
        password: String,
        code: String,
    },
    Lock,
}
#[derive(Serialize)]
pub struct Reply {
    pub gate: Gate,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub setup: Option<Setup>,
}
pub async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
    peer: Peer,
) -> std::result::Result<Json<Gate>, crate::game::HttpError> {
    let id = identity(&state, &headers, peer.0).await?;
    state
        .auth
        .limit("admin-gate", &id.user().to_string(), 60, 60)
        .map_err(crate::game::HttpError)?;
    Ok(Json(
        gate(&state, &id).await.map_err(crate::game::response)?,
    ))
}
pub async fn post(
    State(state): State<AppState>,
    headers: HeaderMap,
    peer: Peer,
    mutation: std::result::Result<Json<Mutation>, axum::extract::rejection::JsonRejection>,
) -> std::result::Result<Json<Reply>, crate::game::HttpError> {
    let id = identity(&state, &headers, peer.0).await?;
    state
        .auth
        .limit("admin-factor", &id.user().to_string(), 6, 300)
        .map_err(crate::game::HttpError)?;
    let Json(mutation) = mutation.map_err(|_| crate::game::response(Error("INVALID_MESSAGE")))?;
    let setup = match mutation {
        Mutation::Setup { password } => Some(
            begin_setup(&state, &id, password)
                .await
                .map_err(crate::game::response)?,
        ),
        Mutation::Confirm { request, code } => {
            complete_factor(&state, &id, request, true, None, code)
                .await
                .map_err(crate::game::response)?;
            None
        }
        Mutation::Unlock {
            request,
            password,
            code,
        } => {
            complete_factor(&state, &id, request, false, Some(password), code)
                .await
                .map_err(crate::game::response)?;
            None
        }
        Mutation::Lock => {
            sqlx::query("DELETE FROM admin_sessions WHERE session_hash=$1 AND user_id=$2")
                .bind(&id.session.hash)
                .bind(id.user())
                .execute(&state.db)
                .await
                .map_err(|e| crate::game::response(e.into()))?;
            None
        }
    };
    Ok(Json(Reply {
        gate: gate(&state, &id).await.map_err(crate::game::response)?,
        setup,
    }))
}
