pub use super::repo::PreparedPlayer;
use super::{
    Error, Result, Settings, duels, now_ms, ok, repo,
    room::{Member, Room},
};
use crate::auth::{Session, crypto};
use crate::duel::{
    self,
    actor::{Action, Notice},
    manager::Manager,
    record::{Record, Seat},
};
use protocol::game::v1::{self as pb, client_message::Body as Client, server_message::Body};
use sqlx::PgPool;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::Arc,
    time::Duration,
};
use tokio::{
    sync::{mpsc, oneshot, watch},
    time::Instant,
};
use uuid::Uuid;

#[derive(Clone, Copy, Debug)]
pub enum Close {
    Replaced,
    Unauthorized,
    Slow,
    Shutdown,
}
#[derive(Clone)]
pub struct Output {
    pub data: mpsc::Sender<(Body, u32)>,
    pub close: watch::Sender<Option<Close>>,
}
impl Output {
    pub fn send(&self, body: Body, reply: u32) {
        if self.data.try_send((body, reply)).is_err() {
            let _ = self.close.send(Some(Close::Slow));
        }
    }
}
struct Connected {
    id: Uuid,
    output: Output,
}
struct Player {
    info: pb::PublicPlayer,
    session: Vec<u8>,
    connection: Option<Connected>,
    room: String,
    resume: Vec<u8>,
    expires: Instant,
    friends: Vec<pb::SocialEntry>,
    ignores: Vec<pb::SocialEntry>,
    filter: String,
    chat: VecDeque<Instant>,
    last_chat: Option<(String, Instant)>,
    challenges: VecDeque<Instant>,
    mail: VecDeque<Instant>,
}
impl Player {
    fn member(&self) -> Option<Member> {
        Some(Member {
            player: self.info.clone(),
            output: self.connection.as_ref()?.output.clone(),
            ignores: self.ignores.iter().map(|v| v.id.clone()).collect(),
            filter: self.filter.clone(),
        })
    }
}
struct Offer {
    id: Uuid,
    from: Uuid,
    to: Uuid,
    from_avatar: String,
    to_avatar: String,
    expires: Instant,
}
#[derive(Clone, Debug)]
pub struct PreparedDuel {
    pub id: Uuid,
    pub players: [PreparedPlayer; 2],
    pub seed: [u8; 32],
}
struct Reservation {
    duel: PreparedDuel,
    expires: Instant,
    claimed: bool,
}
struct Attach {
    session: Session,
    id: Uuid,
    loaded: repo::Loaded,
    output: Output,
    resume: Option<String>,
    reply: oneshot::Sender<Result<pb::Welcome>>,
}
enum Command {
    Attach(Box<Attach>),
    Detach(Uuid, Uuid),
    Apply(Uuid, Uuid, Client, oneshot::Sender<Result<Body>>),
    Revoke(Uuid, Option<Vec<u8>>),
    Take(Uuid, oneshot::Sender<Result<PreparedDuel>>),
    Release(Uuid),
    Locked(Uuid, oneshot::Sender<bool>),
    Game(
        Uuid,
        crate::game::Mutation,
        oneshot::Sender<Result<crate::game::Snapshot>>,
    ),
    Stop,
}
#[derive(Clone)]
pub struct Hub {
    tx: mpsc::Sender<Command>,
    settings: Arc<Settings>,
}
impl Hub {
    pub fn new(db: PgPool, settings: Settings) -> Self {
        let (tx, rx) = mpsc::channel(1024);
        let settings = Arc::new(settings);
        let (notices, events) = mpsc::channel(4096);
        let trades = crate::trade::Manager::new(db.clone(), notices.clone());
        let duels = Manager::new(
            db.clone(),
            settings.duels.clone(),
            notices,
            settings.run_duels,
        );
        tokio::spawn(
            State {
                db,
                settings: settings.clone(),
                players: HashMap::new(),
                rooms: HashMap::new(),
                offers: HashMap::new(),
                reservations: HashMap::new(),
                duels,
                trades,
            }
            .run(rx, events),
        );
        Self { tx, settings }
    }
    pub fn settings(&self) -> &Settings {
        &self.settings
    }
    pub(crate) async fn attach(
        &self,
        session: Session,
        id: Uuid,
        loaded: repo::Loaded,
        output: Output,
        resume: Option<String>,
    ) -> Result<pb::Welcome> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .try_send(Command::Attach(Box::new(Attach {
                session,
                id,
                loaded,
                output,
                resume,
                reply: tx,
            })))
            .map_err(|_| Error("BUSY"))?;
        tokio::time::timeout(Duration::from_secs(5), rx)
            .await
            .map_err(|_| Error("TIMEOUT"))?
            .map_err(|_| Error("INTERNAL"))?
    }
    pub async fn apply(&self, user: Uuid, connection: Uuid, body: Client) -> Result<Body> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .try_send(Command::Apply(user, connection, body, tx))
            .map_err(|_| Error("BUSY"))?;
        tokio::time::timeout(Duration::from_secs(5), rx)
            .await
            .map_err(|_| Error("TIMEOUT"))?
            .map_err(|_| Error("INTERNAL"))?
    }
    pub fn detach(&self, user: Uuid, id: Uuid) {
        let _ = self.tx.try_send(Command::Detach(user, id));
    }
    pub async fn revoke(&self, user: Uuid, session: Option<Vec<u8>>) {
        let _ = self.tx.send(Command::Revoke(user, session)).await;
    }
    /// F7.2 atomically claims a reservation; nobody can claim the same duel twice.
    pub async fn claim_duel(&self, id: Uuid) -> Result<PreparedDuel> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .try_send(Command::Take(id, tx))
            .map_err(|_| Error("BUSY"))?;
        rx.await.map_err(|_| Error("INTERNAL"))?
    }
    /// F8 mutation handlers must consult the lobby lock before changing an avatar or deck.
    pub async fn is_locked(&self, user: Uuid) -> Result<bool> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .try_send(Command::Locked(user, tx))
            .map_err(|_| Error("BUSY"))?;
        rx.await.map_err(|_| Error("INTERNAL"))
    }
    pub async fn finish_duel(&self, id: Uuid) {
        let _ = self.tx.send(Command::Release(id)).await;
    }
    pub async fn game_mutation(
        &self,
        user: Uuid,
        mutation: crate::game::Mutation,
    ) -> Result<crate::game::Snapshot> {
        let (reply, receiver) = oneshot::channel();
        self.tx
            .try_send(Command::Game(user, mutation, reply))
            .map_err(|_| Error("BUSY"))?;
        tokio::time::timeout(Duration::from_secs(5), receiver)
            .await
            .map_err(|_| Error("TIMEOUT"))?
            .map_err(|_| Error("UNAVAILABLE"))?
    }
    pub async fn shutdown(&self) {
        let _ = self.tx.send(Command::Stop).await;
    }
}
struct State {
    duels: Manager,
    trades: crate::trade::Manager,
    db: PgPool,
    settings: Arc<Settings>,
    players: HashMap<Uuid, Player>,
    rooms: HashMap<String, Room>,
    offers: HashMap<Uuid, Offer>,
    reservations: HashMap<Uuid, Reservation>,
}
impl State {
    async fn run(mut self, mut rx: mpsc::Receiver<Command>, mut notices: mpsc::Receiver<Notice>) {
        let mut tick = tokio::time::interval(Duration::from_millis(250));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut retention = tokio::time::interval(Duration::from_secs(3600));
        retention.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                command = rx.recv() => match command {
                    Some(Command::Attach(attach)) => {
                        let Attach { session, id, loaded, output, resume, reply } = *attach;
                        if reply.is_closed() { continue; }
                        let result = tokio::time::timeout(
                            Duration::from_secs(5), self.attach(session, id, loaded, output, resume),
                        ).await.unwrap_or(Err(Error("TIMEOUT")));
                        let _ = reply.send(result);
                    }
                    Some(Command::Detach(user, id)) => self.detach(user, id).await,
                    Some(Command::Apply(user, id, body, reply)) => {
                        if reply.is_closed() { continue; }
                        let result = tokio::time::timeout(
                            Duration::from_secs(5), self.apply(user, id, body),
                        ).await.unwrap_or(Err(Error("TIMEOUT")));
                        let _ = reply.send(result);
                    }
                    Some(Command::Revoke(user, hash)) => {
                        if let Some(player) = self.players.get(&user)
                            && hash.as_ref().is_none_or(|hash| *hash == player.session)
                        {
                            if let Some(connection) = &player.connection {
                                let _ = connection.output.close.send(Some(Close::Unauthorized));
                            }
                            self.remove(user).await;
                        }
                    }
                    Some(Command::Take(id, reply)) => {
                        self.take(id, reply);
                    }
                    Some(Command::Release(id)) => self.release(id, "finished").await,
                    Some(Command::Locked(user, reply)) => {
                        let _ = reply.send(self.is_busy(user) || self.offers.values().any(|o| o.from == user));
                    }
                    Some(Command::Game(user,mutation,reply))=>{
                        if reply.is_closed(){continue;}
                        let result=if self.is_busy(user) || self.offers.values().any(|offer|offer.from==user) {Err(Error("BUSY"))}
                        else {tokio::time::timeout(Duration::from_secs(3),crate::game::mutate(&self.db,user,mutation)).await.unwrap_or(Err(Error("TIMEOUT")))};
                        if result.is_ok() {
                            self.cancel_for(user,"cancelled");
                            if let Ok(Ok(mut info))=tokio::time::timeout(Duration::from_secs(1),repo::player(&self.db,user)).await
                                && let Some(player)=self.players.get_mut(&user) {
                                info.afk=player.info.afk;player.info=info;
                            }
                            self.update(user).await;
                        }
                        let _=reply.send(result);
                    },
                    Some(Command::Stop) | None => break,
                },
                _ = tick.tick() => self.expire().await,
                notice = notices.recv() => if let Some(notice) = notice { self.duel_notice(notice).await; },
                _ = retention.tick() => {
                    let db = self.db.clone();
                    tokio::spawn(async move {
                        let _ = repo::retention(&db).await;
                        match crate::economy::reconcile(&db).await {
                            Ok(report) if !report.is_ok()=>tracing::error!(transactions=report.unbalanced_transactions,wallets=report.incorrect_wallets,"currency reconciliation failed"),
                            Err(_)=>tracing::warn!("currency reconciliation unavailable"),_=>{}
                        }
                    });
                }
            }
        }
        self.trades.shutdown().await;
        self.duels.shutdown().await;
        for player in self.players.values() {
            if let Some(connection) = &player.connection {
                let _ = connection.output.close.send(Some(Close::Shutdown));
            }
        }
    }
    async fn duel_notice(&mut self, notice: Notice) {
        match notice {
            Notice::Send(user, body) => self.notify(user, body),
            Notice::ProfilesChanged(users) => {
                for user in users {
                    if let Ok(Ok(mut info)) =
                        tokio::time::timeout(Duration::from_secs(1), repo::player(&self.db, user))
                            .await
                        && let Some(player) = self.players.get_mut(&user)
                    {
                        info.afk = player.info.afk;
                        player.info = info;
                    }
                    self.update(user).await;
                }
            }
            Notice::Recovered(id, users) => {
                if !self.duels.contains(id) {
                    return;
                }
                for user in users {
                    if let Some(player) = self.players.get_mut(&user) {
                        player.info.busy = true;
                    }
                    if self.online(user) {
                        if let Some(id) = self.trades.for_user(user)
                            && let Ok(body) = self
                                .trades
                                .request(user, id, crate::trade::Action::Snapshot)
                                .await
                        {
                            self.notify(user, body);
                        }
                        self.duels.connection(user, true);
                    }
                    self.update(user).await;
                }
            }
            Notice::Finished(id, users) => {
                let active = sqlx::query_scalar!(
                    "SELECT EXISTS(SELECT 1 FROM duel_locks WHERE duel_id=$1) AS \"active!\"",
                    id
                )
                .fetch_one(&self.db)
                .await
                .unwrap_or(true);
                if !active {
                    self.release(id, "finished").await;
                }
                for user in users {
                    if let Ok(Ok(mut info)) =
                        tokio::time::timeout(Duration::from_secs(3), repo::player(&self.db, user))
                            .await
                        && let Some(player) = self.players.get_mut(&user)
                    {
                        info.afk = player.info.afk;
                        player.info = info;
                    }
                    self.update(user).await;
                }
            }
        }
    }
    fn take(&mut self, id: Uuid, reply: oneshot::Sender<Result<PreparedDuel>>) {
        if reply.is_closed() {
            return;
        }
        let result = self
            .reservations
            .get_mut(&id)
            .filter(|r| !r.claimed && r.expires > Instant::now())
            .map(|r| {
                r.claimed = true;
                r.duel.clone()
            })
            .ok_or(Error("NOT_FOUND"));
        let newly_claimed = result.is_ok();
        if reply.send(result).is_err()
            && newly_claimed
            && let Some(reservation) = self.reservations.get_mut(&id)
        {
            reservation.claimed = false;
        }
    }
    async fn attach(
        &mut self,
        session: Session,
        id: Uuid,
        loaded: repo::Loaded,
        output: Output,
        resume: Option<String>,
    ) -> Result<pb::Welcome> {
        if !repo::session_valid(&self.db, session.user, &session.hash).await? {
            return Err(Error("UNAUTHENTICATED"));
        }
        if let Some(resume) = &resume {
            let old = self
                .players
                .get(&session.user)
                .ok_or(Error("RESUME_EXPIRED"))?;
            if (old.connection.is_none() && old.expires <= Instant::now())
                || old.session != session.hash
                || old.resume != crypto::hash(resume)
            {
                return Err(Error("RESUME_EXPIRED"));
            }
        }
        let previous = self.players.remove(&session.user);
        let room = previous
            .as_ref()
            .map(|p| p.room.clone())
            .unwrap_or_default();
        let filter = previous
            .as_ref()
            .map(|p| p.filter.clone())
            .unwrap_or_else(|| "all".into());
        if let Some(previous) = &previous
            && let Some(connection) = &previous.connection
        {
            let _ = connection.output.close.send(Some(Close::Replaced));
        }
        self.leave_room(session.user, &room).await;
        self.cancel_for(session.user, "cancelled");
        let raw = crypto::token().map_err(|_| Error("INTERNAL"))?;
        let user = session.user;
        let mut info = loaded.player;
        info.busy |= self.is_busy(user);
        info.afk = previous.as_ref().is_some_and(|p| p.info.afk);
        self.players.insert(
            user,
            Player {
                info: info.clone(),
                session: session.hash,
                connection: Some(Connected { id, output }),
                room: String::new(),
                resume: crypto::hash(&raw),
                expires: Instant::now() + self.settings.resume_ttl,
                friends: loaded.friends,
                ignores: loaded.ignores,
                filter,
                chat: previous
                    .as_ref()
                    .map(|p| p.chat.clone())
                    .unwrap_or_default(),
                last_chat: previous.as_ref().and_then(|p| p.last_chat.clone()),
                challenges: previous
                    .as_ref()
                    .map(|p| p.challenges.clone())
                    .unwrap_or_default(),
                mail: previous.map(|p| p.mail).unwrap_or_default(),
            },
        );
        if !room.is_empty()
            && let Ok(snapshot) = self.join(user, &room).await
        {
            self.notify(user, Body::RoomSnapshot(snapshot));
        }
        self.duels.connection(user, true);
        self.publish_social(user);
        self.publish_presence_social(user);
        Ok(pb::Welcome {
            profile: Some(info),
            server_time_ms: now_ms(),
            content_version: engine_content::CONTENT_VERSION.to_vec(),
            resume_token: raw,
            features: vec![
                "rooms".into(),
                "chat".into(),
                "social".into(),
                "mail".into(),
                "challenges".into(),
            ],
        })
    }
    fn notify(&self, user: Uuid, body: Body) {
        if let Some(player) = self.players.get(&user)
            && let Some(connection) = &player.connection
        {
            connection.output.send(body, 0);
        }
    }
    fn online(&self, user: Uuid) -> bool {
        self.players
            .get(&user)
            .is_some_and(|p| p.connection.is_some())
    }
    fn publish_social(&self, user: Uuid) {
        if let Some(player) = self.players.get(&user) {
            let mut snapshot = pb::SocialSnapshot {
                friends: player.friends.clone(),
                ignores: player.ignores.clone(),
            };
            for row in snapshot.friends.iter_mut().chain(&mut snapshot.ignores) {
                row.online = Uuid::parse_str(&row.id).is_ok_and(|id| self.online(id));
            }
            self.notify(user, Body::SocialSnapshot(snapshot));
        }
    }
    fn publish_presence_social(&self, changed: Uuid) {
        for (user, player) in &self.players {
            if player.friends.iter().any(|f| f.id == changed.to_string()) {
                self.publish_social(*user);
            }
        }
    }
    async fn leave_room(&self, user: Uuid, room: &str) {
        if let Some(room) = self.rooms.get(room) {
            let _ = room.leave(user.to_string()).await;
        }
    }
    async fn detach(&mut self, user: Uuid, id: Uuid) {
        if self
            .players
            .get(&user)
            .and_then(|p| p.connection.as_ref())
            .is_none_or(|c| c.id != id)
        {
            return;
        }
        self.trades.cancel_for(user).await;
        self.duels.connection(user, false);
        let room = self.players[&user].room.clone();
        self.leave_room(user, &room).await;
        if let Some(player) = self.players.get_mut(&user) {
            player.connection = None;
            player.expires = Instant::now() + self.settings.resume_ttl;
        }
        self.cancel_for(user, "cancelled");
        self.publish_presence_social(user);
    }
    async fn remove(&mut self, user: Uuid) {
        if let Some(player) = self.players.remove(&user) {
            self.trades.cancel_for(user).await;
            self.duels.connection(user, false);
            self.leave_room(user, &player.room).await;
            self.cancel_for(user, "cancelled");
            self.publish_presence_social(user);
        }
    }
    async fn expire(&mut self) {
        let now = Instant::now();
        self.rooms.retain(|name, _| {
            !name.starts_with("clan:")
                || self
                    .players
                    .values()
                    .any(|p| p.connection.is_some() && p.room == *name)
        });
        let lost: Vec<_> = self
            .players
            .iter()
            .filter_map(|(u, p)| {
                p.connection
                    .as_ref()
                    .filter(|c| c.output.data.is_closed())
                    .map(|c| (*u, c.id))
            })
            .collect();
        for (u, id) in lost {
            self.detach(u, id).await;
        }
        self.players
            .retain(|_, p| p.connection.is_some() || p.expires > now || p.info.busy);
        let expired: Vec<_> = self
            .offers
            .iter()
            .filter(|(_, o)| o.expires <= now)
            .map(|(id, _)| *id)
            .collect();
        for id in expired {
            self.end_offer(id, "expired");
        }
        let orphaned: Vec<_> = self
            .reservations
            .iter()
            .filter(|(id, r)| r.claimed && self.settings.run_duels && !self.duels.contains(**id))
            .map(|(id, _)| *id)
            .collect();
        if !orphaned.is_empty() {
            let active = tokio::time::timeout(
                Duration::from_secs(1),
                sqlx::query_scalar!(
                    "SELECT DISTINCT duel_id FROM duel_locks WHERE duel_id=ANY($1)",
                    &orphaned
                )
                .fetch_all(&self.db),
            )
            .await;
            if let Ok(Ok(active)) = active {
                for id in orphaned {
                    if !active.contains(&id) {
                        self.release(id, "finished").await;
                    }
                }
            }
        }
        let expired: Vec<_> = self
            .reservations
            .iter()
            .filter(|(_, r)| !r.claimed && r.expires <= now)
            .map(|(id, _)| *id)
            .collect();
        for id in expired {
            self.release(id, "unavailable").await;
        }
    }
    fn social_body(&self, user: Uuid) -> Body {
        let player = &self.players[&user];
        let mut snapshot = pb::SocialSnapshot {
            friends: player.friends.clone(),
            ignores: player.ignores.clone(),
        };
        for row in snapshot.friends.iter_mut().chain(&mut snapshot.ignores) {
            row.online = Uuid::parse_str(&row.id).is_ok_and(|id| self.online(id));
        }
        Body::SocialSnapshot(snapshot)
    }
    async fn update(&self, user: Uuid) {
        if let Some(player) = self.players.get(&user)
            && let Some(member) = player.member()
            && let Some(room) = self.rooms.get(&player.room)
        {
            let _ = room.update(member).await;
        }
    }
    async fn room_access(&self, user: Uuid, name: &str) -> Result<Option<HashSet<String>>> {
        if [
            "arena", "training", "castle", "portal", "bazaar", "shop", "tower",
        ]
        .contains(&name)
        {
            return Ok(None);
        }
        if let Some(id) = name
            .strip_prefix("clan:")
            .and_then(|id| Uuid::parse_str(id).ok())
            && repo::clan(&self.db, user).await? == Some(id)
        {
            return Ok(Some(repo::clan_members(&self.db, id).await?));
        }
        Err(Error("FORBIDDEN"))
    }
    async fn join(&mut self, user: Uuid, name: &str) -> Result<pb::RoomSnapshot> {
        let allowed = self.room_access(user, name).await?;
        if self
            .players
            .iter()
            .filter(|(id, p)| **id != user && p.connection.is_some() && p.room == name)
            .count()
            >= 100
        {
            return Err(Error("ROOM_FULL"));
        }
        let player = &self.players[&user];
        let previous = player.room.clone();
        let member = player.member().ok_or(Error("UNAUTHENTICATED"))?;
        let room = self
            .rooms
            .entry(name.into())
            .or_insert_with(|| Room::new(name.into()))
            .clone();
        let snapshot = room.join(member, allowed).await?;
        if previous != name {
            self.trades.cancel_for(user).await;
            self.cancel_for(user, "cancelled");
            self.leave_room(user, &previous).await;
        }
        self.players.get_mut(&user).expect("connected player").room = name.into();
        Ok(snapshot)
    }
    async fn apply(&mut self, user: Uuid, id: Uuid, body: Client) -> Result<Body> {
        if self
            .players
            .get(&user)
            .and_then(|p| p.connection.as_ref())
            .is_none_or(|c| c.id != id)
        {
            return Err(Error("UNAUTHENTICATED"));
        }
        match body {
            Client::Ping(_) => Ok(Body::Pong(pb::Pong {
                server_time_ms: now_ms(),
            })),
            Client::RoomJoin(value) => Ok(Body::RoomSnapshot(self.join(user, &value.room).await?)),
            Client::RoomLeave(_) => {
                self.trades.cancel_for(user).await;
                self.cancel_for(user, "cancelled");
                let room = std::mem::take(&mut self.players.get_mut(&user).unwrap().room);
                self.leave_room(user, &room).await;
                Ok(ok())
            }
            Client::SetAfk(value) => {
                self.players.get_mut(&user).unwrap().info.afk = value.afk;
                self.update(user).await;
                Ok(ok())
            }
            Client::SetChallengeFilter(value) => {
                if !["all", "level_pm2", "none"].contains(&value.filter.as_str()) {
                    return Err(Error("INVALID_MESSAGE"));
                }
                repo::challenge_filter(&self.db, user, &value.filter).await?;
                self.players.get_mut(&user).unwrap().info.challenge_filter = value.filter;
                self.update(user).await;
                Ok(ok())
            }
            Client::SetChatFilter(value) => {
                if !["all", "clan_private", "private"].contains(&value.filter.as_str()) {
                    return Err(Error("INVALID_MESSAGE"));
                }
                self.players.get_mut(&user).unwrap().filter = value.filter;
                self.update(user).await;
                Ok(ok())
            }
            Client::WhoIs(value) => {
                let other = repo::target(&self.db, &value.player).await?;
                let mut info = repo::player(&self.db, other).await?;
                if let Some(player) = self.players.get(&other) {
                    info.afk = player.info.afk;
                    info.busy = player.info.busy;
                }
                Ok(Body::PlayerInfo(Box::new(pb::PlayerInfo {
                    player: Some(info),
                    online: self.online(other),
                })))
            }
            Client::FriendAdd(value) => self.social_change(user, &value.player, false, true).await,
            Client::FriendRemove(value) => {
                self.social_change(user, &value.player, false, false).await
            }
            Client::IgnoreAdd(value) => self.social_change(user, &value.player, true, true).await,
            Client::IgnoreRemove(value) => {
                self.social_change(user, &value.player, true, false).await
            }
            Client::SocialList(_) => Ok(self.social_body(user)),
            Client::ChatSend(value) => self.chat(user, value).await,
            Client::MailSend(value) => {
                let player = self.players.get_mut(&user).unwrap();
                limit(&mut player.mail, 5, Duration::from_secs(60))?;
                let text = super::text(&value.text, 1000)?;
                let other = repo::target(&self.db, &value.to).await?;
                if other == user {
                    return Err(Error("INVALID_MESSAGE"));
                }
                let (ignored, invited) = repo::relationship(&self.db, user, other).await?;
                if ignored || (!self.players[&user].info.verified && !invited) {
                    return Err(Error("FORBIDDEN"));
                }
                repo::send_mail(&self.db, user, other, &text).await?;
                Ok(ok())
            }
            Client::MailList(value) => Ok(Body::MailSnapshot(
                repo::mail_list(&self.db, user, &value).await?,
            )),
            Client::MailRead(value) => {
                let id = Uuid::parse_str(&value.id).map_err(|_| Error("INVALID_MESSAGE"))?;
                repo::mail_change(&self.db, user, id, false).await?;
                Ok(ok())
            }
            Client::MailDelete(value) => {
                let id = Uuid::parse_str(&value.id).map_err(|_| Error("INVALID_MESSAGE"))?;
                repo::mail_change(&self.db, user, id, true).await?;
                Ok(ok())
            }
            Client::ChallengeSend(value) => self.challenge(user, &value.target).await,
            Client::ChallengeRespond(value) => self.respond(user, value).await,
            Client::ChallengeCancel(value) => {
                let id =
                    Uuid::parse_str(&value.challenge_id).map_err(|_| Error("INVALID_MESSAGE"))?;
                let offer = self.offers.get(&id).ok_or(Error("NOT_FOUND"))?;
                if offer.from != user {
                    return Err(Error("FORBIDDEN"));
                }
                self.end_offer(id, "cancelled");
                Ok(ok())
            }
            Client::Hello(_) | Client::Resume(_) => Err(Error("CONFLICT")),
            Client::DuelSnapshot(value) => {
                let id = duel_id(&value.duel_id)?;
                match self.duels.request(user, id, Action::Snapshot).await {
                    Err(Error("NOT_FOUND" | "UNAVAILABLE")) => {
                        duels::finished_snapshot(&self.db, user, id).await
                    }
                    response => response,
                }
            }
            Client::DuelSubmitMove(value) => {
                self.duels
                    .request(
                        user,
                        duel_id(&value.duel_id)?,
                        Action::Submit {
                            round: value.round,
                            mv: value.r#move.ok_or(Error("INVALID_MESSAGE"))?,
                        },
                    )
                    .await
            }
            Client::DuelPreviewMove(value) => {
                self.duels
                    .request(
                        user,
                        duel_id(&value.duel_id)?,
                        Action::Preview {
                            round: value.round,
                            mv: value.r#move.ok_or(Error("INVALID_MESSAGE"))?,
                        },
                    )
                    .await
            }
            Client::DuelForfeit(value) => {
                self.duels
                    .request(user, duel_id(&value.duel_id)?, Action::Forfeit)
                    .await
            }
            Client::TrainingChallengeBot(value) => self.train(user, &value.bot_id).await,
            Client::TrainingBots(_) => Ok(Body::TrainingBots(pb::TrainingBots {
                bots: duels::bots().into_iter().map(|b| b.0).collect(),
            })),
            Client::RankingGet(value) => duels::ranking(&self.db, value).await,
            Client::MatchHistoryGet(value) => duels::history(&self.db, user, value).await,
            Client::TradeInvite(value) => self.trade_invite(user, &value.target).await,
            Client::TradeRespond(value) => {
                self.trades
                    .request(
                        user,
                        duel_id(&value.trade_id)?,
                        crate::trade::Action::Respond(value.accept),
                    )
                    .await
            }
            Client::TradeUpdate(value) => {
                self.trades
                    .request(
                        user,
                        duel_id(&value.trade_id)?,
                        crate::trade::Action::Update(
                            value.version,
                            value.offer.ok_or(Error("INVALID_MESSAGE"))?,
                        ),
                    )
                    .await
            }
            Client::TradeConfirm(value) => {
                self.trades
                    .request(
                        user,
                        duel_id(&value.trade_id)?,
                        crate::trade::Action::Confirm(value.version),
                    )
                    .await
            }
            Client::TradeCancel(value) => {
                self.trades
                    .request(
                        user,
                        duel_id(&value.trade_id)?,
                        crate::trade::Action::Cancel,
                    )
                    .await
            }
            Client::TradeSnapshot(value) => {
                self.trades
                    .request(
                        user,
                        duel_id(&value.trade_id)?,
                        crate::trade::Action::Snapshot,
                    )
                    .await
            }
            Client::MatchReplay(value) => duels::replay(&self.db, duel_id(&value.match_id)?).await,
        }
    }
    async fn trade_invite(&mut self, user: Uuid, name: &str) -> Result<Body> {
        limit(
            &mut self.players.get_mut(&user).unwrap().challenges,
            10,
            Duration::from_secs(60),
        )?;
        let other = repo::target(&self.db, name).await?;
        if other == user || !self.online(other) {
            return Err(Error("NOT_FOUND"));
        }
        if self.players[&user].room != "bazaar" || self.players[&other].room != "bazaar" {
            return Err(Error("FORBIDDEN"));
        }
        if self.is_busy(user)
            || self.is_busy(other)
            || self.players[&other].info.afk
            || self
                .offers
                .values()
                .any(|o| o.from == user || o.from == other)
        {
            return Err(Error("BUSY"));
        }
        if repo::relationship(&self.db, user, other).await?.0 {
            return Err(Error("FORBIDDEN"));
        }
        let logins = [
            self.players[&user].info.login.clone(),
            self.players[&other].info.login.clone(),
        ];
        let body = self.trades.invite([user, other], logins).await?;
        self.cancel_for(user, "cancelled");
        self.cancel_for(other, "cancelled");
        Ok(body)
    }
    async fn train(&mut self, user: Uuid, bot_id: &str) -> Result<Body> {
        if !self.duels.has_capacity() {
            return Err(Error("BUSY"));
        }
        if self.players[&user].room != "training" {
            return Err(Error("FORBIDDEN"));
        }
        if self.is_busy(user) {
            return Err(Error("BUSY"));
        }
        limit(
            &mut self.players.get_mut(&user).unwrap().challenges,
            10,
            Duration::from_secs(60),
        )?;
        let (bot, setup, profile) = duels::bots()
            .into_iter()
            .find(|b| b.0.id == bot_id)
            .ok_or(Error("NOT_FOUND"))?;
        let mut tx = self.db.begin().await?;
        sqlx::query!(
            "SELECT id FROM users WHERE id=$1 AND status='active' FOR UPDATE",
            user
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(Error("UNAUTHENTICATED"))?;
        let own = repo::prepare_player(&mut tx, user, &self.players[&user].info.avatar_id).await?;
        let mut seed = [0; 32];
        getrandom::fill(&mut seed).map_err(|_| Error("INTERNAL"))?;
        let record = Record::new(
            Uuid::now_v7(),
            "training",
            [
                Seat::player(user, own.avatar, &own.setup, &self.players[&user].info),
                Seat::from_setup(None, None, &setup, &bot, Some(profile)),
            ],
            seed,
            &self.settings.duels,
        )?;
        let id = record.id;
        duel::journal::create(
            &mut tx,
            &record,
            self.duels.instance(),
            self.settings.duels.lease_ms(),
        )
        .await?;
        tx.commit().await?;
        self.cancel_for(user, "cancelled");
        self.players.get_mut(&user).unwrap().info.busy = true;
        self.duels.start(record);
        self.update(user).await;
        self.duels.request(user, id, Action::Snapshot).await
    }
    async fn social_change(
        &mut self,
        user: Uuid,
        name: &str,
        ignore: bool,
        add: bool,
    ) -> Result<Body> {
        let other = repo::target(&self.db, name).await?;
        let snapshot = repo::social_change(&self.db, user, other, ignore, add).await?;
        let player = self.players.get_mut(&user).unwrap();
        player.friends = snapshot.friends;
        player.ignores = snapshot.ignores;
        self.update(user).await;
        if ignore && add {
            self.trades.cancel_for(user).await;
            let ids: Vec<_> = self
                .offers
                .values()
                .filter(|o| (o.from == user && o.to == other) || (o.from == other && o.to == user))
                .map(|o| o.id)
                .collect();
            for id in ids {
                self.end_offer(id, "cancelled");
            }
        }
        Ok(self.social_body(user))
    }
    async fn chat(&mut self, user: Uuid, value: pb::ChatSend) -> Result<Body> {
        let text = super::text(&value.text, 300)?;
        let now = Instant::now();
        let p = self.players.get_mut(&user).unwrap();
        limit(
            &mut p.chat,
            if p.info.verified { 5 } else { 2 },
            Duration::from_secs(10),
        )?;
        let signature = format!("{}:{}:{}", value.channel, value.to, text.to_lowercase());
        if p.last_chat.as_ref().is_some_and(|(last, at)| {
            *last == signature && now.duration_since(*at) < Duration::from_secs(10)
        }) {
            return Err(Error("RATE_LIMITED"));
        }
        p.last_chat = Some((signature, now));
        let mut message = pb::ChatMessage {
            id: Uuid::now_v7().to_string(),
            channel: value.channel.clone(),
            room: p.room.clone(),
            from_id: user.to_string(),
            from_login: p.info.login.clone(),
            to: String::new(),
            text,
            at_ms: now_ms(),
        };
        match value.channel.as_str() {
            "room" => {
                let room = p.room.clone();
                if room.is_empty() {
                    return Err(Error("CONFLICT"));
                }
                let allowed = self.room_access(user, &room).await?;
                let clan = room
                    .strip_prefix("clan:")
                    .and_then(|id| Uuid::parse_str(id).ok());
                repo::log_chat(&self.db, &message, clan).await?;
                self.rooms
                    .get(&room)
                    .ok_or(Error("CONFLICT"))?
                    .chat(message, allowed)?;
            }
            "private" => {
                let other = repo::target(&self.db, &value.to).await?;
                if other == user || !self.online(other) {
                    return Err(Error("NOT_FOUND"));
                }
                let (ignored, invited) = repo::relationship(&self.db, user, other).await?;
                if ignored || (!self.players[&user].info.verified && !invited) {
                    return Err(Error("FORBIDDEN"));
                }
                message.to = other.to_string();
                message.room.clear();
                repo::log_chat(&self.db, &message, None).await?;
                self.notify(other, Body::ChatMessage(message.clone()));
                self.notify(user, Body::ChatMessage(message));
            }
            "clan" => {
                let clan = repo::clan(&self.db, user)
                    .await?
                    .ok_or(Error("FORBIDDEN"))?;
                let allowed = repo::clan_members(&self.db, clan).await?;
                message.room = format!("clan:{clan}");
                repo::log_chat(&self.db, &message, Some(clan)).await?;
                for (id, p) in &self.players {
                    if allowed.contains(&id.to_string())
                        && p.filter != "private"
                        && !p.ignores.iter().any(|i| i.id == user.to_string())
                    {
                        self.notify(*id, Body::ChatMessage(message.clone()));
                    }
                }
            }
            _ => return Err(Error("INVALID_MESSAGE")),
        }
        Ok(ok())
    }
    fn is_busy(&self, user: Uuid) -> bool {
        self.trades.active_for(user)
            || self.duels.active_for(user).is_some()
            || self
                .reservations
                .values()
                .any(|r| r.duel.players.iter().any(|p| p.user == user))
    }
    fn cancel_for(&mut self, user: Uuid, status: &str) {
        let ids: Vec<_> = self
            .offers
            .values()
            .filter(|o| o.from == user || o.to == user)
            .map(|o| o.id)
            .collect();
        for id in ids {
            self.end_offer(id, status);
        }
    }
    fn end_offer(&mut self, id: Uuid, status: &str) {
        if let Some(offer) = self.offers.remove(&id) {
            let body = Body::ChallengeUpdate(pb::ChallengeUpdate {
                challenge_id: id.to_string(),
                status: status.into(),
                duel_id: String::new(),
            });
            self.notify(offer.from, body.clone());
            self.notify(offer.to, body);
        }
    }
    async fn challenge(&mut self, user: Uuid, name: &str) -> Result<Body> {
        limit(
            &mut self.players.get_mut(&user).unwrap().challenges,
            10,
            Duration::from_secs(60),
        )?;
        let other = repo::target(&self.db, name).await?;
        if other == user || !self.online(other) {
            return Err(Error("NOT_FOUND"));
        }
        let from = &self.players[&user];
        let to = &self.players[&other];
        if from.room != "arena" || to.room != "arena" {
            return Err(Error("FORBIDDEN"));
        }
        if self.is_busy(user)
            || self.is_busy(other)
            || self.offers.values().any(|o| o.from == user)
            || self.offers.values().filter(|o| o.to == other).count() >= 16
            || to.info.afk
        {
            return Err(Error("BUSY"));
        }
        if from.info.avatar_id.is_empty() || to.info.avatar_id.is_empty() {
            return Err(Error("CONFLICT"));
        }
        if to.info.challenge_filter == "none"
            || (to.info.challenge_filter == "level_pm2"
                && to.info.level.abs_diff(from.info.level) > 2)
        {
            return Err(Error("FORBIDDEN"));
        }
        if repo::relationship(&self.db, user, other).await?.0 {
            return Err(Error("FORBIDDEN"));
        }
        let id = Uuid::now_v7();
        let at_ms = now_ms() + self.settings.challenge_ttl.as_millis() as i64;
        let received = pb::ChallengeReceived {
            challenge_id: id.to_string(),
            from: Some(from.info.clone()),
            to: Some(to.info.clone()),
            expires_at_ms: at_ms,
        };
        self.offers.insert(
            id,
            Offer {
                id,
                from: user,
                to: other,
                from_avatar: from.info.avatar_id.clone(),
                to_avatar: to.info.avatar_id.clone(),
                expires: Instant::now() + self.settings.challenge_ttl,
            },
        );
        self.notify(other, Body::ChallengeReceived(Box::new(received.clone())));
        Ok(Body::ChallengeReceived(Box::new(received)))
    }
    async fn respond(&mut self, user: Uuid, value: pb::ChallengeRespond) -> Result<Body> {
        if self.settings.run_duels && !self.duels.has_capacity() {
            return Err(Error("BUSY"));
        }
        let id = Uuid::parse_str(&value.challenge_id).map_err(|_| Error("INVALID_MESSAGE"))?;
        let offer = self.offers.get(&id).ok_or(Error("NOT_FOUND"))?;
        if offer.to != user {
            return Err(Error("FORBIDDEN"));
        }
        if offer.expires <= Instant::now() {
            self.end_offer(id, "expired");
            return Err(Error("TIMEOUT"));
        }
        if !value.accept {
            self.end_offer(id, "declined");
            return Ok(ok());
        }
        let from = offer.from;
        let to = offer.to;
        let expected = [offer.from_avatar.clone(), offer.to_avatar.clone()];
        if !self.online(from) || !self.online(to) || self.is_busy(from) || self.is_busy(to) {
            return Err(Error("BUSY"));
        }
        let a = &self.players[&from].info;
        let b = &self.players[&to].info;
        if self.players[&from].room != "arena"
            || self.players[&to].room != "arena"
            || b.challenge_filter == "none"
            || (b.challenge_filter == "level_pm2" && b.level.abs_diff(a.level) > 2)
            || a.afk
            || b.afk
        {
            return Err(Error("FORBIDDEN"));
        }
        if repo::relationship(&self.db, from, to).await?.0 {
            return Err(Error("FORBIDDEN"));
        }
        let mut tx = self.db.begin().await?;
        let mut ordered = [from, to];
        ordered.sort();
        for user in ordered {
            sqlx::query!(
                "SELECT id FROM users WHERE id=$1 AND status='active' FOR UPDATE",
                user
            )
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(Error("CONFLICT"))?;
        }
        let first = repo::prepare_player(&mut tx, from, &expected[0]).await?;
        let second = repo::prepare_player(&mut tx, to, &expected[1]).await?;
        let mut seed = [0; 32];
        getrandom::fill(&mut seed).map_err(|_| Error("INTERNAL"))?;
        let setup = engine::DuelSetup {
            rules: engine_content::rules(),
            seed,
            players: [first.setup.clone(), second.setup.clone()],
        };
        engine::new_duel(&setup, &engine_content::content()).map_err(|_| Error("INVALID_DECK"))?;
        let duel = Uuid::now_v7();
        let runtime = if self.settings.run_duels {
            let seats = [
                Seat::player(
                    first.user,
                    first.avatar,
                    &first.setup,
                    &self.players[&from].info,
                ),
                Seat::player(
                    second.user,
                    second.avatar,
                    &second.setup,
                    &self.players[&to].info,
                ),
            ];
            let record = Record::new(duel, "arena", seats, seed, &self.settings.duels)?;
            duel::journal::create(
                &mut tx,
                &record,
                self.duels.instance(),
                self.settings.duels.lease_ms(),
            )
            .await?;
            Some(record)
        } else {
            None
        };
        tx.commit().await?;
        self.offers.remove(&id);
        self.cancel_for(from, "cancelled");
        self.cancel_for(to, "cancelled");
        self.reservations.insert(
            duel,
            Reservation {
                duel: PreparedDuel {
                    id: duel,
                    players: [first, second],
                    seed,
                },
                expires: Instant::now() + self.settings.reservation_ttl,
                claimed: runtime.is_some(),
            },
        );
        if let Some(record) = runtime {
            self.duels.start(record);
        }
        for user in [from, to] {
            self.players.get_mut(&user).unwrap().info.busy = true;
            self.update(user).await;
            self.notify(
                user,
                Body::ChallengeUpdate(pb::ChallengeUpdate {
                    challenge_id: id.to_string(),
                    status: "accepted".into(),
                    duel_id: duel.to_string(),
                }),
            );
        }
        Ok(Body::ChallengeUpdate(pb::ChallengeUpdate {
            challenge_id: id.to_string(),
            status: "accepted".into(),
            duel_id: duel.to_string(),
        }))
    }
    async fn release(&mut self, id: Uuid, status: &str) {
        if let Some(reservation) = self.reservations.remove(&id) {
            for player in reservation.duel.players {
                if let Some(p) = self.players.get_mut(&player.user) {
                    p.info.busy = false;
                }
                self.update(player.user).await;
                self.notify(
                    player.user,
                    Body::ChallengeUpdate(pb::ChallengeUpdate {
                        challenge_id: String::new(),
                        status: status.into(),
                        duel_id: id.to_string(),
                    }),
                );
            }
        }
    }
}
fn limit(queue: &mut VecDeque<Instant>, max: usize, window: Duration) -> Result<()> {
    let now = Instant::now();
    while queue
        .front()
        .is_some_and(|at| now.duration_since(*at) >= window)
    {
        queue.pop_front();
    }
    if queue.len() >= max {
        return Err(Error("RATE_LIMITED"));
    }
    queue.push_back(now);
    Ok(())
}

fn duel_id(value: &str) -> Result<Uuid> {
    Uuid::parse_str(value).map_err(|_| Error("INVALID_MESSAGE"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cancelled_handoff_does_not_lock_a_duel_and_duplicate_does_not_unlock_it() {
        let id = Uuid::now_v7();
        let player = PreparedPlayer {
            user: Uuid::now_v7(),
            avatar: Uuid::now_v7(),
            setup: engine::PlayerSetup {
                avatar: engine_content::avatar(engine_content::STARTERS[0].avatar)
                    .unwrap()
                    .snapshot(1),
                deck: Default::default(),
            },
        };
        let reservation = Reservation {
            duel: PreparedDuel {
                id,
                players: [player.clone(), player],
                seed: [0; 32],
            },
            expires: Instant::now() + Duration::from_secs(30),
            claimed: false,
        };
        let (notices, _receiver) = mpsc::channel(4);
        let duels = Manager::new(
            sqlx::postgres::PgPoolOptions::new()
                .connect_lazy("postgres://unused:unused@127.0.0.1/unused")
                .unwrap(),
            crate::duel::Settings::default(),
            notices,
            false,
        );
        let trades = crate::trade::Manager::new(
            sqlx::postgres::PgPoolOptions::new()
                .connect_lazy("postgres://unused:unused@127.0.0.1/unused")
                .unwrap(),
            mpsc::channel(4).0,
        );
        let mut state = State {
            trades,
            duels,
            db: sqlx::postgres::PgPoolOptions::new()
                .connect_lazy("postgres://unused:unused@127.0.0.1/unused")
                .unwrap(),
            settings: Arc::new(Settings::default()),
            players: HashMap::new(),
            rooms: HashMap::new(),
            offers: HashMap::new(),
            reservations: HashMap::from([(id, reservation)]),
        };
        let (tx, rx) = oneshot::channel();
        drop(rx);
        state.take(id, tx);
        assert!(!state.reservations[&id].claimed);
        let (tx, rx) = oneshot::channel();
        state.take(id, tx);
        assert_eq!(rx.await.unwrap().unwrap().id, id);
        assert!(state.reservations[&id].claimed);
        let (tx, rx) = oneshot::channel();
        state.take(id, tx);
        assert_eq!(rx.await.unwrap().unwrap_err().0, "NOT_FOUND");
        assert!(state.reservations[&id].claimed);
    }
}
