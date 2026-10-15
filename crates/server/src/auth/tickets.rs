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
    let mut tickets = state.auth.0.tickets.lock().expect("ticket lock");
    tickets.retain(|_, expiry| *expiry > now());
    if tickets.len() >= 10000 {
        return Err(Error::busy());
    }
    tickets.insert(nonce, expires);
    Ok(Json(
        serde_json::json!({"ticket":format!("{body}.{}", crypto::hex(&signature)), "expires_in":30}),
    ))
}

impl Auth {
    /// F6.3 calls this during an Android WebSocket upgrade. Never log the ticket or URL.
    /// Only tickets issued by this running instance are accepted; restart invalidates them.
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
        {
            let mut tickets = self.0.tickets.lock().expect("ticket lock");
            tickets.retain(|_, expiry| *expiry > now());
            if tickets.remove(*nonce) != Some(expiry) {
                return Err(Error::unauthorized());
            }
        }
        security::session_hash(state, &hash, "app").await
    }
}
