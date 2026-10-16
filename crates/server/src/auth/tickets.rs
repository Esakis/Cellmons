use axum::{Json, extract::State, http::HeaderMap};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

use super::{Auth, Error, Result, crypto, security};
use crate::gateway::AppState;

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("UTC clock")
        .as_secs()
}

pub(super) async fn issue(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>> {
    let session = security::session(&state, &headers).await?;
    if session.kind != "app" {
        return Err(Error::forbidden());
    }
    state
        .auth
        .limit("ticket", &crypto::hex(&session.hash), 30, 60)?;
    let expires = now() + 30;
    let nonce = crypto::token().map_err(|_| Error::busy())?;
    let body = format!("{}.{}.{}", crypto::hex(&session.hash), expires, nonce);
    let signature = crypto::sign(&state.auth.0.config.ticket_key, body.as_bytes());
    let mut tx = state.db.begin().await?;
    // Match rotation's parent-before-child lock order; do not hold ticket rows while
    // waiting for a rotating session's FK lock.
    let live: Option<Vec<u8>> = sqlx::query_scalar("SELECT id_hash FROM sessions WHERE id_hash=$1 AND user_id=$2 AND purpose='game' AND expires_at>clock_timestamp() FOR SHARE")
        .bind(&session.hash).bind(session.user).fetch_optional(&mut *tx).await?;
    if live.is_none() {
        return Err(Error::unauthorized());
    }
    // A bounded shared issuance pool survives a blue/green cutover. Only the nonce hash is stored.
    sqlx::query("SELECT pg_advisory_xact_lock(731592805)")
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM ws_tickets WHERE expires_at<=clock_timestamp()")
        .execute(&mut *tx)
        .await?;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM ws_tickets")
        .fetch_one(&mut *tx)
        .await?;
    if count >= 10000 {
        return Err(Error::busy());
    }
    sqlx::query("INSERT INTO ws_tickets(nonce_hash,session_hash,expires_at) VALUES ($1,$2,to_timestamp($3))")
        .bind(crypto::hash(&nonce)).bind(&session.hash).bind(expires as f64).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(
        serde_json::json!({"ticket":format!("{body}.{}", crypto::hex(&signature)), "expires_in":30}),
    ))
}

impl Auth {
    /// F6.3 calls this during an Android WebSocket upgrade. Never log the ticket or URL.
    /// The database consumes each issued nonce once, including across release instances.
    pub async fn consume_ws_ticket(
        &self,
        state: &AppState,
        ticket: &str,
        origin: &str,
    ) -> Result<Uuid> {
        Ok(self.consume_ws_session(state, ticket, origin).await?.user)
    }
    pub(crate) async fn consume_ws_session(
        &self,
        state: &AppState,
        ticket: &str,
        origin: &str,
    ) -> Result<super::Session> {
        if !self
            .0
            .config
            .app_origins
            .iter()
            .any(|allowed| allowed == origin)
            || ticket.len() > 250
        {
            return Err(Error::unauthorized());
        }
        let fields: Vec<_> = ticket.split('.').collect();
        let [hash, expiry, nonce, signature] = fields.as_slice() else {
            return Err(Error::unauthorized());
        };
        let expiry: u64 = expiry.parse().map_err(|_| Error::unauthorized())?;
        let hash = crypto::unhex(hash).ok_or_else(Error::unauthorized)?;
        let signature = crypto::unhex(signature).ok_or_else(Error::unauthorized)?;
        let body = ticket.rsplit_once('.').ok_or_else(Error::unauthorized)?.0;
        if expiry <= now()
            || expiry > now() + 30
            || !crypto::verify(&self.0.config.ticket_key, body.as_bytes(), &signature)
        {
            return Err(Error::unauthorized());
        }
        if crypto::unhex(nonce).is_none() {
            return Err(Error::unauthorized());
        }
        let consumed = sqlx::query("DELETE FROM ws_tickets WHERE nonce_hash=$1 AND session_hash=$2 AND expires_at>clock_timestamp()")
            .bind(crypto::hash(nonce)).bind(&hash).execute(&state.db).await?;
        if consumed.rows_affected() != 1 {
            return Err(Error::unauthorized());
        }
        security::session_hash(state, &hash, "app").await
    }
}
