use super::{
    Settings,
    hub::{Close, Hub, Outbound, Output},
    repo,
};
use crate::{
    auth::{self, Session},
    gateway::AppState,
};
use axum::{
    extract::{
        Query, State, WebSocketUpgrade,
        ws::{CloseFrame, Message, WebSocket},
    },
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse as _, Response},
};
use futures_util::SinkExt as _;
use protocol::{
    Message as _,
    game::v1::{self as pb, client_message::Body as Client, server_message::Body},
};
use serde::Deserialize;
use sqlx::PgPool;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio::{
    sync::{OwnedSemaphorePermit, Semaphore, mpsc, watch},
    time::{Duration, Instant},
};
use uuid::Uuid;

type PendingReply =
    std::pin::Pin<Box<dyn std::future::Future<Output = (u32, super::Result<Body>)> + Send>>;
type PendingValidation = std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send>>;

pub struct Online {
    pub hub: Hub,
    slots: Arc<Semaphore>,
    ips: Arc<Mutex<HashMap<String, usize>>>,
    pub settings: Settings,
}
struct Permit {
    _slot: OwnedSemaphorePermit,
    ips: Arc<Mutex<HashMap<String, usize>>>,
    ip: String,
}
impl Drop for Permit {
    fn drop(&mut self) {
        let mut ips = self.ips.lock().expect("IP counters");
        if let Some(count) = ips.get_mut(&self.ip) {
            *count -= 1;
            if *count == 0 {
                ips.remove(&self.ip);
            }
        }
    }
}
impl Online {
    pub fn connections(&self) -> usize {
        self.settings
            .max_connections
            .saturating_sub(self.slots.available_permits())
    }
    pub fn new(db: PgPool, settings: Settings) -> Self {
        Self {
            hub: Hub::new(db, settings.clone()),
            slots: Arc::new(Semaphore::new(settings.max_connections)),
            ips: Default::default(),
            settings,
        }
    }
    fn acquire(&self, ip: String) -> std::result::Result<Permit, StatusCode> {
        let slot = self
            .slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        let mut ips = self.ips.lock().expect("IP counters");
        let count = ips.entry(ip.clone()).or_default();
        if *count >= self.settings.per_ip {
            if *count == 0 {
                ips.remove(&ip);
            }
            return Err(StatusCode::TOO_MANY_REQUESTS);
        }
        *count += 1;
        Ok(Permit {
            _slot: slot,
            ips: self.ips.clone(),
            ip,
        })
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Parameters {
    ticket: Option<String>,
}

pub async fn upgrade(
    State(state): State<AppState>,
    peer: auth::security::Peer,
    headers: HeaderMap,
    Query(parameters): Query<Parameters>,
    upgrade: WebSocketUpgrade,
) -> Response {
    let origin = match headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) {
        Some(origin) => origin,
        None => return StatusCode::FORBIDDEN.into_response(),
    };
    let ip = auth::security::peer_key(&state.auth, peer.0, &headers);
    let permit = match state.online.acquire(ip) {
        Ok(permit) => permit,
        Err(status) => return status.into_response(),
    };
    let session = if let Some(ticket) = &parameters.ticket {
        state.auth.consume_ws_session(&state, ticket, origin).await
    } else {
        if origin != state.auth.config().public_origin
            || headers.contains_key(header::AUTHORIZATION)
        {
            return StatusCode::FORBIDDEN.into_response();
        }
        auth::security::session(&state, &headers).await
    };
    let session = match session {
        Ok(session) => session,
        Err(error) => return error.into_response(),
    };
    upgrade
        .read_buffer_size(16 * 1024)
        .write_buffer_size(4096)
        .max_message_size(16384)
        .max_frame_size(16384)
        .max_write_buffer_size(512 * 1024)
        .on_upgrade(move |socket| connection(socket, state, session, permit))
        .into_response()
}

async fn send(socket: &mut WebSocket, seq: &mut u32, body: Body, reply: u32) -> bool {
    send_encoded(socket, seq, Output::encode(body), reply).await
}
fn envelope(seq: u32, reply: u32, body: &[u8]) -> Vec<u8> {
    let mut bytes = pb::ServerMessage {
        seq,
        reply_to: reply,
        body: None,
    }
    .encode_to_vec();
    bytes.extend_from_slice(body);
    bytes
}
async fn send_encoded(
    socket: &mut WebSocket,
    seq: &mut u32,
    body: axum::body::Bytes,
    reply: u32,
) -> bool {
    if !queue_encoded(socket, seq, body, reply).await {
        return false;
    }
    tokio::time::timeout(Duration::from_secs(3), socket.flush())
        .await
        .is_ok_and(|r| r.is_ok())
}
async fn queue_encoded(
    socket: &mut WebSocket,
    seq: &mut u32,
    body: axum::body::Bytes,
    reply: u32,
) -> bool {
    let Some(next) = seq.checked_add(1) else {
        return false;
    };
    *seq = next;
    let bytes = envelope(*seq, reply, &body);
    if bytes.len() > 256 * 1024 {
        return false;
    }
    tokio::time::timeout(
        Duration::from_secs(3),
        socket.feed(Message::Binary(bytes.into())),
    )
    .await
    .is_ok_and(|result| result.is_ok())
}
async fn close(socket: &mut WebSocket, code: u16) {
    let _ = tokio::time::timeout(
        Duration::from_secs(1),
        socket.send(Message::Close(Some(CloseFrame {
            code,
            reason: "".into(),
        }))),
    )
    .await;
}
async fn first(socket: &mut WebSocket, deadline: Instant) -> Option<pb::ClientMessage> {
    loop {
        match tokio::time::timeout_at(deadline, socket.recv())
            .await
            .ok()??
            .ok()?
        {
            Message::Binary(bytes) => return pb::ClientMessage::decode(bytes).ok(),
            Message::Ping(value) => {
                socket.send(Message::Pong(value)).await.ok()?;
            }
            Message::Pong(_) => {}
            _ => return None,
        }
    }
}
async fn connection(mut socket: WebSocket, state: AppState, session: Session, _permit: Permit) {
    let settings = &state.online.settings;
    let mut server_seq = 0;
    let Some(first) = first(&mut socket, Instant::now() + settings.hello_timeout).await else {
        close(&mut socket, 1002).await;
        return;
    };
    let mut last_seq = first.seq;
    let resume = match first.body {
        Some(Client::Hello(hello))
            if first.seq > 0
                && hello.protocol_major == 1
                && hello.protocol_minor == 0
                && hello.content_version == engine_content::CONTENT_VERSION
                && ["pl", "en"].contains(&hello.locale.as_str())
                && hello.build.len() <= 128
                && ((session.kind == "web" && hello.client_kind == "web")
                    || (session.kind == "app"
                        && hello.client_kind == "android"
                        && auth::config::version(&hello.native_version)
                            .is_some_and(|v| v >= state.auth.config().native_version))) =>
        {
            None
        }
        Some(Client::Resume(resume))
            if first.seq > 0 && auth::crypto::unhex(&resume.resume_token).is_some() =>
        {
            Some(resume.resume_token)
        }
        _ => {
            send(
                &mut socket,
                &mut server_seq,
                super::error("VERSION_MISMATCH"),
                first.seq,
            )
            .await;
            close(&mut socket, 1002).await;
            return;
        }
    };
    let loaded =
        match tokio::time::timeout(settings.hello_timeout, repo::load(&state.db, session.user))
            .await
        {
            Ok(Ok(loaded)) => loaded,
            _ => {
                send(
                    &mut socket,
                    &mut server_seq,
                    super::error("INTERNAL"),
                    first.seq,
                )
                .await;
                close(&mut socket, 1011).await;
                return;
            }
        };
    let (tx, mut rx) = mpsc::channel(256);
    let (stop, mut cancelled) = watch::channel(None);
    let id = Uuid::now_v7();
    let attached = state
        .online
        .hub
        .attach(
            session.clone(),
            id,
            loaded,
            Output {
                data: tx,
                close: stop,
            },
            resume,
        )
        .await;
    let welcome = match attached {
        Ok(welcome) => welcome,
        Err(error) => {
            send(
                &mut socket,
                &mut server_seq,
                super::error(error.0),
                first.seq,
            )
            .await;
            close(&mut socket, 1008).await;
            return;
        }
    };
    if !send(
        &mut socket,
        &mut server_seq,
        Body::Welcome(Box::new(welcome)),
        first.seq,
    )
    .await
    {
        state.online.hub.detach(session.user, id);
        return;
    }
    let mut heartbeat = tokio::time::interval(settings.heartbeat);
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    heartbeat.tick().await;
    let mut last_frame = Instant::now();
    let mut bad = 0;
    let mut tokens = 60000u64;
    let mut refilled = Instant::now();
    let flush = tokio::time::sleep(Duration::from_secs(3600));
    tokio::pin!(flush);
    let mut buffered = false;
    let mut pending: Option<PendingReply> = None;
    let mut validation: Option<PendingValidation> = None;
    loop {
        tokio::select! {
            response = async { pending.as_mut().expect("pending reply").await }, if pending.is_some() => {
                pending=None;
                let (reply,result)=response;
                let body=result.unwrap_or_else(|error|super::error(error.0));
                if !send(&mut socket,&mut server_seq,body,reply).await {break;}
                buffered=false;
            }
            valid = async { validation.as_mut().expect("pending validation").await }, if validation.is_some() => {
                validation=None;
                if !valid {
                    send(&mut socket,&mut server_seq,super::error("UNAUTHENTICATED"),0).await;
                    close(&mut socket,1008).await;
                    break;
                }
                if !tokio::time::timeout(Duration::from_secs(3),socket.send(Message::Ping(Vec::new().into()))).await.is_ok_and(|result|result.is_ok()){break;}
                buffered=false;
            }
            _=flush.as_mut(), if buffered=>{
                if !tokio::time::timeout(Duration::from_secs(3),socket.flush()).await.is_ok_and(|r|r.is_ok()){break;}
                buffered=false;
            }
            _ = cancelled.changed() => {
                let reason = *cancelled.borrow_and_update();
                match reason {
                    Some(Close::Replaced) => {
                        send(&mut socket, &mut server_seq, Body::SessionReplaced(pb::SessionReplaced {}), 0).await;
                    }
                    Some(Close::Unauthorized) => {
                        send(&mut socket, &mut server_seq, super::error("UNAUTHENTICATED"), 0).await;
                    }
                    _ => {}
                }
                close(&mut socket, match reason {Some(Close::Replaced)=>4001,Some(Close::Slow)=>1013,Some(Close::Shutdown)=>1012,_=>1008}).await;
                break;
            }
            output = rx.recv() => {
                let Some((body, reply)) = output else { break; };
                let sent=match body {
                    Outbound::Body(body)=>{let sent=send(&mut socket,&mut server_seq,body,reply).await;buffered=false;sent},
                    Outbound::Encoded(body)=>{
                        let sent=queue_encoded(&mut socket,&mut server_seq,body,reply).await;
                        if sent && !buffered {buffered=true;flush.as_mut().reset(Instant::now()+Duration::from_millis(5));}
                        sent
                    },
                };
                if !sent {break;}
            }
            _ = heartbeat.tick() => {
                if last_frame.elapsed() >= settings.idle {
                    close(&mut socket, 1001).await;
                    break;
                }
                if validation.is_none() {
                    let db=state.db.clone();let user=session.user;let hash=session.hash.clone();
                    validation=Some(Box::pin(async move {
                        tokio::time::timeout(Duration::from_secs(3),repo::session_valid(&db,user,&hash)).await.is_ok_and(|result|result.unwrap_or(false))
                    }));
                }
            }
            message = socket.recv(), if pending.is_none() => {
                let Some(Ok(message)) = message else { break; };
                last_frame = Instant::now();
                let elapsed = refilled.elapsed().as_millis().min(2000) as u64;
                tokens = (tokens + elapsed * 30).min(60000);
                refilled = Instant::now();
                if tokens < 1000 {
                    send(&mut socket, &mut server_seq, super::error("RATE_LIMITED"), 0).await;
                    close(&mut socket, 1008).await;
                    break;
                }
                tokens -= 1000;
                let bytes = match message {
                    Message::Binary(bytes) => bytes,
                    Message::Ping(value) => {
                        if socket.send(Message::Pong(value)).await.is_err() { break; }
                        continue;
                    }
                    Message::Pong(_) => continue,
                    Message::Close(_) => break,
                    _ => {
                        bad += 1;
                        if bad >= 3 { close(&mut socket, 1002).await; break; }
                        continue;
                    }
                };
                let request = match pb::ClientMessage::decode(bytes) {
                    Ok(request) if request.seq > last_seq => request,
                    _ => {
                        bad += 1;
                        send(&mut socket, &mut server_seq, super::error("INVALID_MESSAGE"), 0).await;
                        if bad >= 3 { close(&mut socket, 1002).await; break; }
                        continue;
                    }
                };
                last_seq = request.seq;
                let hub=state.online.hub.clone();let user=session.user;
                pending=Some(Box::pin(async move {
                    let result=if let Some(body)=request.body { hub.apply(user,id,body).await } else {Ok(super::ok())};
                    (request.seq,result)
                }));
            }
        }
    }
    state.online.hub.detach(session.user, id);
}

#[cfg(test)]
mod broadcast_tests {
    use super::*;
    #[test]
    fn shared_body_preserves_each_receivers_sequence_and_correlation() {
        let body = Body::ChatMessage(pb::ChatMessage {
            text: "Zażółć <script>".into(),
            ..Default::default()
        });
        let encoded = Output::encode(body.clone());
        for seq in [1, 127, 128, 65536, u32::MAX] {
            for reply in [0, 31, u32::MAX] {
                let decoded =
                    pb::ServerMessage::decode(envelope(seq, reply, &encoded).as_slice()).unwrap();
                assert_eq!(decoded.seq, seq);
                assert_eq!(decoded.reply_to, reply);
                assert_eq!(decoded.body, Some(body.clone()));
            }
        }
    }
}
