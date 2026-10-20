use super::seed;
use crate::{
    auth::{Session, crypto},
    online::{
        hub::{Close, Hub, Outbound, Output},
        repo,
    },
};
use anyhow::{Context as _, ensure};
use protocol::{
    Message as _,
    game::v1::{self as pb, client_message::Body as Client, server_message::Body},
};
use sqlx::{Connection as _, PgConnection, PgPool};
use std::{collections::HashSet, sync::Arc, time::Duration};
use tokio::{
    sync::{mpsc, watch},
    task::{JoinHandle, JoinSet},
    time::Instant,
};
use uuid::Uuid;

#[derive(Clone)]
pub struct Settings {
    pub autonomous_duels: bool,
    pub action_interval: Duration,
    pub health_interval: Duration,
    pub accept_delay: Duration,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            autonomous_duels: true,
            action_interval: Duration::from_secs(120),
            health_interval: Duration::from_secs(30),
            accept_delay: Duration::from_secs(2),
        }
    }
}
impl Settings {
    pub fn from_env() -> anyhow::Result<Option<Self>> {
        match std::env::var("LOBBY_BOTS_ENABLED").as_deref() {
            Err(std::env::VarError::NotPresent) | Ok("") | Ok("false") | Ok("0") => Ok(None),
            Ok("true") | Ok("1") => Ok(Some(Self::default())),
            _ => anyhow::bail!("LOBBY_BOTS_ENABLED must be true or false"),
        }
    }
}

pub struct Population {
    stop: watch::Sender<bool>,
    task: JoinHandle<()>,
}
impl Population {
    /// Retry leadership after rollout or a database outage without preventing HTTP startup.
    pub fn spawn(db: PgPool, hub: Hub, settings: Settings) -> Self {
        let (stop, mut signal) = watch::channel(false);
        let task = tokio::spawn(async move {
            loop {
                if *signal.borrow() {
                    break;
                }
                let started = tokio::select! {
                    _ = signal.changed() => break,
                    result = Self::start(db.clone(), hub.clone(), settings.clone()) => result,
                };
                match started {
                    Ok(Some(mut active)) => {
                        tokio::select! {
                            _ = signal.changed() => {
                                active.shutdown().await;
                                break;
                            },
                            _ = &mut active.task => {},
                        }
                    }
                    Ok(None) => {}
                    Err(error) => tracing::warn!(%error, "managed account controller will retry"),
                }
                tokio::select! {
                    _ = signal.changed() => break,
                    _ = tokio::time::sleep(Duration::from_secs(10)) => {},
                }
            }
        });
        Self { stop, task }
    }
    /// A dedicated PostgreSQL connection owns the singleton lock, never a pooled idle session.
    pub async fn start(db: PgPool, hub: Hub, settings: Settings) -> anyhow::Result<Option<Self>> {
        ensure!(
            !settings.action_interval.is_zero() && !settings.health_interval.is_zero(),
            "invalid bot timing"
        );
        let mut lock = PgConnection::connect_with(&db.connect_options()).await?;
        if !sqlx::query_scalar::<_, bool>("SELECT pg_try_advisory_lock(736401228)")
            .fetch_one(&mut lock)
            .await?
        {
            tracing::info!("managed accounts already have an active controller");
            return Ok(None);
        }
        let report = seed(&db).await?;
        let rows = sqlx::query_as::<_, (Uuid, String)>(
            "SELECT user_id,roster_key FROM bot_accounts ORDER BY roster_key",
        )
        .fetch_all(&db)
        .await?;
        let ids = Arc::new(rows.iter().map(|r| r.0.to_string()).collect::<HashSet<_>>());
        let (stop, receiver) = watch::channel(false);
        let control = stop.clone();
        let task = tokio::spawn(async move {
            let mut workers = JoinSet::new();
            for (index, (user, key)) in rows.into_iter().enumerate() {
                workers.spawn(account_loop(
                    db.clone(),
                    hub.clone(),
                    user,
                    key,
                    index,
                    ids.clone(),
                    settings.clone(),
                    receiver.clone(),
                ));
            }
            tracing::info!(
                created = report.created,
                existing = report.existing,
                "managed accounts started"
            );
            let mut stopped = receiver;
            let mut heartbeat = tokio::time::interval(Duration::from_secs(10));
            loop {
                tokio::select! {
                    _ = stopped.changed() => break,
                    result = workers.join_next(), if !workers.is_empty() => {
                        if let Some(Err(error)) = result {
                            tracing::error!(%error, "managed account worker stopped unexpectedly");
                        }
                        if workers.is_empty() { break; }
                    },
                    _ = heartbeat.tick() => {
                        if !tokio::time::timeout(Duration::from_secs(3), lock.ping()).await
                            .is_ok_and(|result| result.is_ok()) {
                            tracing::error!("managed account controller lost its database lock connection");
                            break;
                        }
                    }
                }
            }
            let _ = control.send(true);
            while workers.join_next().await.is_some() {}
            let _ = lock.close().await;
        });
        Ok(Some(Self { stop, task }))
    }
    pub async fn shutdown(mut self) {
        let _ = self.stop.send(true);
        let _ = (&mut self.task).await;
    }
}
impl Drop for Population {
    fn drop(&mut self) {
        let _ = self.stop.send(true);
    }
}

fn room_for(index: usize, cycle: usize) -> &'static str {
    if index % 10 < 7 {
        return "arena";
    }
    ["arena", "training", "tower", "shop", "bazaar"][(index + cycle) % 5]
}

async fn enabled(db: &PgPool, user: Uuid) -> anyhow::Result<bool> {
    Ok(sqlx::query_scalar::<_, bool>(
        "SELECT b.enabled AND u.status='active' FROM bot_accounts b JOIN users u ON u.id=b.user_id WHERE b.user_id=$1"
    ).bind(user).fetch_optional(db).await?.unwrap_or(false))
}

#[allow(clippy::too_many_arguments)]
async fn account_loop(
    db: PgPool,
    hub: Hub,
    user: Uuid,
    key: String,
    index: usize,
    ids: Arc<HashSet<String>>,
    settings: Settings,
    mut stop: watch::Receiver<bool>,
) {
    while !*stop.borrow() {
        if enabled(&db, user).await.unwrap_or(false) {
            // Random hash with no bearer-token preimage retained by the application.
            let hash = match crypto::token() {
                Ok(token) => crypto::hash(&token),
                Err(_) => break,
            };
            let connection = Uuid::now_v7();
            let result = connected(
                &db, &hub, user, connection, &hash, index, &ids, &settings, &mut stop,
            )
            .await;
            hub.detach(user, connection);
            let _ = sqlx::query("DELETE FROM sessions WHERE id_hash=$1 AND user_id=$2")
                .bind(&hash)
                .bind(user)
                .execute(&db)
                .await;
            if let Err(error) = result {
                tracing::warn!(account=%key, %error, "managed account reconnecting");
            }
        }
        if *stop.borrow() {
            break;
        }
        tokio::select! {
            _ = stop.changed() => break,
            _ = tokio::time::sleep(settings.health_interval) => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn connected(
    db: &PgPool,
    hub: &Hub,
    user: Uuid,
    connection: Uuid,
    hash: &[u8],
    index: usize,
    ids: &HashSet<String>,
    settings: &Settings,
    stop: &mut watch::Receiver<bool>,
) -> anyhow::Result<()> {
    sqlx::query("INSERT INTO sessions(id_hash,kind,user_id,expires_at,device_label) VALUES($1,'web',$2,now()+interval '2 hours','managed-controller')")
        .bind(hash).bind(user).execute(db).await?;
    let loaded = repo::load(db, user).await.map_err(net)?;
    let (data, mut events) = mpsc::channel(256);
    let (close, mut closed) = watch::channel::<Option<Close>>(None);
    hub.attach(
        Session {
            hash: hash.to_vec(),
            user,
            kind: "web".into(),
        },
        connection,
        loaded,
        Output { data, close },
        None,
    )
    .await
    .map_err(net)?;
    call(
        hub,
        user,
        connection,
        Client::RoomJoin(pb::RoomJoin {
            room: room_for(index, 0).into(),
        }),
    )
    .await?;
    let mut timer = tokio::time::interval(Duration::from_millis(250));
    timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut health = Instant::now() + settings.health_interval;
    let mut renew = Instant::now() + Duration::from_secs(1800);
    let mut action =
        Instant::now() + settings.action_interval + Duration::from_secs((index % 11) as u64 * 7);
    let mut offer: Option<(String, Instant)> = None;
    let mut cycle = 0;
    loop {
        if *stop.borrow() {
            return Ok(());
        }
        tokio::select! {
            _ = stop.changed() => return Ok(()),
            _ = closed.changed() => return Ok(()),
            event = events.recv() => {
                let Some((event, _)) = event else { return Ok(()); };
                let body = match event {
                    Outbound::Body(body) => body,
                    Outbound::Encoded(bytes) => pb::ServerMessage::decode(bytes)?.body.context("missing event")?,
                };
                match body {
                    Body::ChallengeReceived(challenge) if challenge.to.as_ref().is_some_and(|p| p.id == user.to_string()) => {
                        if offer.is_none() {
                            offer = Some((challenge.challenge_id, Instant::now() + settings.accept_delay));
                        }
                    }
                    Body::ChallengeUpdate(update) if offer.as_ref().is_some_and(|(id,_)| *id == update.challenge_id) => {
                        offer = None;
                    }
                    // Social/chat/duel events are drained, but no synthetic conversations are sent.
                    _ => {}
                }
            },
            _ = timer.tick() => {
                let now = Instant::now();
                if now >= health {
                    if !enabled(db, user).await? { return Ok(()); }
                    health = now + settings.health_interval;
                }
                if now >= renew {
                    let changed = sqlx::query("UPDATE sessions SET expires_at=now()+interval '2 hours',last_seen_at=now() WHERE id_hash=$1 AND user_id=$2")
                        .bind(hash).bind(user).execute(db).await?.rows_affected();
                    ensure!(changed == 1, "controller session revoked");
                    renew = now + Duration::from_secs(1800);
                }
                if offer.as_ref().is_some_and(|(_, at)| *at <= now) {
                    let (id, _) = offer.take().expect("due offer");
                    if enabled(db, user).await? {
                        let _ = call(hub, user, connection, Client::ChallengeRespond(pb::ChallengeRespond { challenge_id: id, accept: true })).await;
                    }
                    action = now + settings.action_interval;
                }
                if now >= action {
                    action = now + settings.action_interval + Duration::from_secs((index % 7) as u64 * 9);
                    if offer.is_some() || hub.is_locked(user).await.map_err(net)? { continue; }
                    cycle += 1;
                    let room = room_for(index, cycle);
                    let reply = call(hub, user, connection, Client::RoomJoin(pb::RoomJoin { room: room.into() })).await;
                    if settings.autonomous_duels && index.is_multiple_of(8) && room == "arena"
                        && let Ok(Body::RoomSnapshot(snapshot)) = reply {
                        let targets: Vec<_> = snapshot.players.iter().filter(|p|
                            p.id != user.to_string() && ids.contains(&p.id) && !p.busy && !p.afk
                        ).collect();
                        if !targets.is_empty() {
                            let target = targets[(cycle + index) % targets.len()].login.clone();
                            let _ = call(hub, user, connection, Client::ChallengeSend(pb::ChallengeSend { target })).await;
                        }
                    }
                }
            }
        }
    }
}
async fn call(hub: &Hub, user: Uuid, connection: Uuid, body: Client) -> anyhow::Result<Body> {
    hub.apply(user, connection, body).await.map_err(net)
}
fn net(error: crate::online::Error) -> anyhow::Error {
    anyhow::anyhow!("lobby: {}", error.0)
}
