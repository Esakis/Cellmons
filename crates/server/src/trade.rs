//! Bounded trade actors. Database leases fence stale actors; owner rows serialize inventory use.
use crate::{
    duel::actor::Notice,
    economy::{self, Account},
    online::{Error, Result, now_ms},
};
use protocol::game::v1::{self as pb, server_message::Body};
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, PgPool};
use std::{
    collections::{BTreeMap, HashMap},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU8, Ordering},
    },
    time::Duration,
};
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

#[derive(Clone, Default, Serialize, Deserialize)]
struct Offer {
    cards: BTreeMap<u16, i32>,
    ducats: i64,
}
#[derive(Clone, Serialize, Deserialize)]
struct Record {
    id: Uuid,
    users: [Uuid; 2],
    logins: [String; 2],
    status: String,
    version: u32,
    offers: [Offer; 2],
    accepted: [bool; 2],
    deadline: i64,
    created: i64,
    revision: i64,
}
impl Record {
    fn active(&self) -> bool {
        matches!(self.status.as_str(), "invited" | "offering")
    }
    fn snapshot(&self) -> Body {
        Body::TradeSnapshot(pb::TradeSnapshot {
            trade_id: self.id.to_string(),
            users: self.users.map(|id| id.to_string()).to_vec(),
            logins: self.logins.to_vec(),
            status: self.status.clone(),
            version: self.version,
            offers: self
                .offers
                .iter()
                .map(|o| pb::TradeOffer {
                    cards: o
                        .cards
                        .iter()
                        .map(|(id, qty)| pb::TradeCard {
                            card_id: u32::from(*id),
                            qty: *qty as u32,
                        })
                        .collect(),
                    ducats: o.ducats,
                })
                .collect(),
            accepted: self.accepted.to_vec(),
            deadline_ms: self.deadline,
            revision: self.revision as u64,
        })
    }
}
pub enum Action {
    Snapshot,
    Expire,
    Respond(bool),
    Update(u32, pb::TradeOffer),
    Confirm(u32),
    Cancel,
}
struct Command {
    user: Uuid,
    action: Action,
    reply: oneshot::Sender<Result<Body>>,
}
#[derive(Clone)]
struct Handle {
    tx: mpsc::Sender<Command>,
    users: [Uuid; 2],
    busy: Arc<AtomicU8>,
}
struct Inner {
    db: PgPool,
    instance: Uuid,
    actors: Mutex<HashMap<Uuid, Handle>>,
    notices: mpsc::Sender<Notice>,
    stopping: AtomicBool,
}
#[derive(Clone)]
pub struct Manager(Arc<Inner>);
impl Manager {
    pub fn new(db: PgPool, notices: mpsc::Sender<Notice>) -> Self {
        let this = Self(Arc::new(Inner {
            db,
            instance: Uuid::now_v7(),
            actors: Mutex::new(HashMap::new()),
            notices,
            stopping: AtomicBool::new(false),
        }));
        let manager = this.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(2));
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            while !manager.0.stopping.load(Ordering::Acquire) {
                tick.tick().await;
                if let Ok(Ok(recovered)) =
                    tokio::time::timeout(Duration::from_secs(3), recover_records(&manager.0.db))
                        .await
                {
                    for record in recovered {
                        for user in record.users {
                            let _ = manager
                                .0
                                .notices
                                .send(Notice::Send(user, record.snapshot()))
                                .await;
                        }
                        let _ = manager
                            .0
                            .notices
                            .send(Notice::ProfilesChanged(record.users.to_vec()))
                            .await;
                    }
                }
            }
        });
        this
    }
    pub fn active_for(&self, user: Uuid) -> bool {
        self.0
            .actors
            .lock()
            .expect("trade registry")
            .values()
            .any(|h| {
                h.users
                    .iter()
                    .enumerate()
                    .any(|(i, id)| *id == user && h.busy.load(Ordering::Acquire) & (1 << i) != 0)
            })
    }
    pub fn for_user(&self, user: Uuid) -> Option<Uuid> {
        self.0
            .actors
            .lock()
            .expect("trade registry")
            .iter()
            .find_map(|(id, h)| h.users.contains(&user).then_some(*id))
    }
    pub async fn invite(&self, users: [Uuid; 2], logins: [String; 2]) -> Result<Body> {
        if self.0.stopping.load(Ordering::Acquire)
            || self.0.actors.lock().expect("trade registry").len() >= 1024
        {
            return Err(Error("BUSY"));
        }
        let record = create(&self.0.db, self.0.instance, users, logins).await?;
        let body = record.snapshot();
        let id = record.id;
        let busy = Arc::new(AtomicU8::new(1));
        let (tx, rx) = mpsc::channel(32);
        self.0.actors.lock().expect("trade registry").insert(
            id,
            Handle {
                tx,
                users,
                busy: busy.clone(),
            },
        );
        let this = self.clone();
        tokio::spawn(async move {
            this.run(record, rx, busy).await;
        });
        let _ = self
            .0
            .notices
            .send(Notice::ProfilesChanged(users.to_vec()))
            .await;
        Ok(body)
    }
    pub async fn request(&self, user: Uuid, id: Uuid, action: Action) -> Result<Body> {
        let handle = self
            .0
            .actors
            .lock()
            .expect("trade registry")
            .get(&id)
            .cloned();
        if let Some(handle) = handle {
            if !handle.users.contains(&user) {
                return Err(Error("FORBIDDEN"));
            }
            let (reply, rx) = oneshot::channel();
            handle
                .tx
                .try_send(Command {
                    user,
                    action,
                    reply,
                })
                .map_err(|_| Error("BUSY"))?;
            return tokio::time::timeout(Duration::from_secs(3), rx)
                .await
                .map_err(|_| Error("TIMEOUT"))?
                .map_err(|_| Error("UNAVAILABLE"))?;
        }
        let record = load(&self.0.db, id).await?;
        if !record.users.contains(&user) {
            return Err(Error("FORBIDDEN"));
        }
        if record.active() {
            return Err(Error("BUSY"));
        }
        Ok(record.snapshot())
    }
    pub async fn cancel_for(&self, user: Uuid) {
        if let Some(id) = self.for_user(user) {
            let _ = self.request(user, id, Action::Cancel).await;
        }
    }
    pub async fn shutdown(&self) {
        self.0.stopping.store(true, Ordering::Release);
        let handles: Vec<_> = self
            .0
            .actors
            .lock()
            .expect("trade registry")
            .iter()
            .map(|(id, h)| (*id, h.users[0]))
            .collect();
        futures_util::future::join_all(
            handles
                .into_iter()
                .map(|(id, user)| self.request(user, id, Action::Cancel)),
        )
        .await;
    }
    async fn run(&self, mut record: Record, mut rx: mpsc::Receiver<Command>, busy: Arc<AtomicU8>) {
        let mut tick = tokio::time::interval(Duration::from_secs(1));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut publish = true;
        loop {
            if publish {
                for user in record.users {
                    let _ = self
                        .0
                        .notices
                        .send(Notice::Send(user, record.snapshot()))
                        .await;
                }
                publish = false;
            }
            if !record.active() {
                break;
            }
            tokio::select! {
                command=rx.recv()=>{
                    let Some(command)=command else {break;};
                    if command.reply.is_closed(){continue;}
                    match tokio::time::timeout(Duration::from_secs(3),mutate(&self.0.db,self.0.instance,record.id,command.user,command.action)).await {
                        Ok(Ok(next))=>{let changed=next.revision!=record.revision;let profile_changed=next.status!=record.status;record=next;busy.store(if record.status=="offering" {3}else if record.status=="invited"{1}else{0},Ordering::Release);let _=command.reply.send(Ok(record.snapshot()));publish=changed;if profile_changed {let _=self.0.notices.send(Notice::ProfilesChanged(record.users.to_vec())).await;}},
                        Ok(Err(error))=>{let _=command.reply.send(Err(error));if error.0=="LEASE_LOST"{break;}},
                        Err(_)=>{let _=command.reply.send(Err(Error("TIMEOUT")));},
                    }
                },
                _=tick.tick()=>{
                    if record.deadline<=now_ms() {
                        match mutate(&self.0.db,self.0.instance,record.id,record.users[0],Action::Expire).await {Ok(next)=>{record=next;publish=true;},Err(e) if e.0=="LEASE_LOST"=>break,Err(_)=>{}}
                    } else {
                        let renewed=sqlx::query("UPDATE trades SET lease_until=clock_timestamp()+interval '5 seconds' WHERE id=$1 AND owner_instance=$2 AND lease_until>clock_timestamp() AND status IN ('invited','offering')").bind(record.id).bind(self.0.instance).execute(&self.0.db).await;
                        if matches!(renewed,Ok(result) if result.rows_affected()==0){break;}
                    }
                }
            }
        }
        self.0
            .actors
            .lock()
            .expect("trade registry")
            .remove(&record.id);
        let _ = self
            .0
            .notices
            .send(Notice::ProfilesChanged(record.users.to_vec()))
            .await;
    }
}
async fn lock_users(tx: &mut PgConnection, users: [Uuid; 2]) -> Result<()> {
    let mut ordered = users;
    ordered.sort_unstable();
    for user in ordered {
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM users WHERE id=$1 FOR UPDATE")
            .bind(user)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(Error("UNAUTHENTICATED"))?;
    }
    Ok(())
}
async fn load(db: &PgPool, id: Uuid) -> Result<Record> {
    let bytes = sqlx::query_scalar::<_, Vec<u8>>("SELECT state FROM trades WHERE id=$1")
        .bind(id)
        .fetch_optional(db)
        .await?
        .ok_or(Error("NOT_FOUND"))?;
    serde_json::from_slice(&bytes).map_err(|_| Error("INTERNAL"))
}
async fn create(
    db: &PgPool,
    instance: Uuid,
    users: [Uuid; 2],
    logins: [String; 2],
) -> Result<Record> {
    if users[0] == users[1] {
        return Err(Error("INVALID_MESSAGE"));
    }
    let mut tx = db.begin().await?;
    lock_users(&mut tx, users).await?;
    let active = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM users WHERE id=ANY($1) AND status='active'",
    )
    .bind(&users[..])
    .fetch_one(&mut *tx)
    .await?;
    if active != 2 {
        return Err(Error("UNAUTHENTICATED"));
    }
    let verified = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM users WHERE id=ANY($1) AND email_verified_at IS NOT NULL",
    )
    .bind(&users[..])
    .fetch_one(&mut *tx)
    .await?;
    if verified != 2 {
        return Err(Error("EMAIL_UNVERIFIED"));
    }
    for user in users {
        crate::activity::ensure_idle(&mut tx, user).await?;
    }
    let pending=sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM trades WHERE status IN ('invited','offering') AND (from_user=ANY($1) OR to_user=ANY($1)))").bind(&users[..]).fetch_one(&mut *tx).await?;
    if pending {
        return Err(Error("BUSY"));
    }
    let now = now_ms();
    let record = Record {
        id: Uuid::now_v7(),
        users,
        logins,
        status: "invited".into(),
        version: 0,
        offers: Default::default(),
        accepted: [false; 2],
        deadline: now + 30_000,
        created: now,
        revision: 0,
    };
    let bytes = serde_json::to_vec(&record).map_err(|_| Error("INTERNAL"))?;
    sqlx::query("INSERT INTO trades(id,from_user,to_user,status,state,owner_instance,lease_until) VALUES ($1,$2,$3,'invited',$4,$5,clock_timestamp()+interval '5 seconds')").bind(record.id).bind(users[0]).bind(users[1]).bind(bytes).bind(instance).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO activity_locks(user_id,kind,owner_id) VALUES ($1,'trade',$2)")
        .bind(users[0])
        .bind(record.id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(record)
}
fn parse_offer(value: pb::TradeOffer) -> Result<Offer> {
    if value.cards.len() > 200 || value.ducats < 0 {
        return Err(Error("INVALID_MESSAGE"));
    }
    let mut offer = Offer {
        ducats: value.ducats,
        ..Default::default()
    };
    for stack in value.cards {
        let id = u16::try_from(stack.card_id).map_err(|_| Error("INVALID_MESSAGE"))?;
        if stack.qty == 0 || engine_content::content().get(id).is_none() {
            return Err(Error("INVALID_MESSAGE"));
        }
        let qty = offer.cards.entry(id).or_default();
        *qty = qty
            .checked_add(i32::try_from(stack.qty).map_err(|_| Error("INVALID_MESSAGE"))?)
            .ok_or(Error("INVALID_MESSAGE"))?;
    }
    Ok(offer)
}
async fn validate_offer(tx: &mut PgConnection, user: Uuid, offer: &Offer) -> Result<()> {
    let balance = sqlx::query_scalar::<_, i64>("SELECT balance FROM wallets WHERE user_id=$1")
        .bind(user)
        .fetch_one(&mut *tx)
        .await?;
    if balance < offer.ducats {
        return Err(Error("INSUFFICIENT_FUNDS"));
    }
    for (id, qty) in &offer.cards {
        let owned = sqlx::query_scalar::<_, i32>(
            "SELECT qty FROM collection WHERE user_id=$1 AND card_id=$2",
        )
        .bind(user)
        .bind(*id as i16)
        .fetch_optional(&mut *tx)
        .await?
        .unwrap_or(0);
        if owned < *qty {
            return Err(Error("INSUFFICIENT_CARDS"));
        }
    }
    Ok(())
}
async fn settle(tx: &mut PgConnection, record: &Record) -> Result<()> {
    for (user, offer) in record.users.iter().zip(&record.offers) {
        validate_offer(tx, *user, offer).await?;
    }
    let mut net = BTreeMap::<u16, i64>::new();
    for (i, offer) in record.offers.iter().enumerate() {
        for (id, qty) in &offer.cards {
            *net.entry(*id).or_default() += if i == 0 {
                i64::from(*qty)
            } else {
                -i64::from(*qty)
            };
        }
    }
    for (id, qty) in net {
        if qty == 0 {
            continue;
        }
        let (from, to) = if qty > 0 {
            (record.users[0], record.users[1])
        } else {
            (record.users[1], record.users[0])
        };
        let qty = qty.unsigned_abs() as i32;
        let changed = sqlx::query(
            "UPDATE collection SET qty=qty-$3 WHERE user_id=$1 AND card_id=$2 AND qty>=$3",
        )
        .bind(from)
        .bind(id as i16)
        .bind(qty)
        .execute(&mut *tx)
        .await?;
        if changed.rows_affected() != 1 {
            return Err(Error("INSUFFICIENT_CARDS"));
        }
        let changed=sqlx::query("INSERT INTO collection(user_id,card_id,qty) VALUES ($1,$2,$3) ON CONFLICT(user_id,card_id) DO UPDATE SET qty=collection.qty+$3 WHERE collection.qty<=$4").bind(to).bind(id as i16).bind(qty).bind(i32::MAX-qty).execute(&mut *tx).await?;
        if changed.rows_affected() != 1 {
            return Err(Error("CARD_LIMIT"));
        }
    }
    let net = record.offers[0].ducats - record.offers[1].ducats;
    if net != 0 {
        let (from, to) = if net > 0 {
            (record.users[0], record.users[1])
        } else {
            (record.users[1], record.users[0])
        };
        economy::transfer(
            tx,
            Account::Player(from),
            Account::Player(to),
            net.abs(),
            "trade",
            record.id,
        )
        .await?;
    }
    for user in record.users {
        crate::game::prune(tx, user).await?;
    }
    Ok(())
}
async fn mutate(
    db: &PgPool,
    instance: Uuid,
    id: Uuid,
    user: Uuid,
    action: Action,
) -> Result<Record> {
    let current = load(db, id).await?;
    let side = current
        .users
        .iter()
        .position(|u| *u == user)
        .ok_or(Error("FORBIDDEN"))?;
    let mut tx = db.begin().await?;
    lock_users(&mut tx, current.users).await?;
    let row=sqlx::query_as::<_,(Vec<u8>,Uuid,bool)>("SELECT state,owner_instance,lease_until>clock_timestamp() FROM trades WHERE id=$1 FOR UPDATE").bind(id).fetch_one(&mut *tx).await?;
    let mut record: Record = serde_json::from_slice(&row.0).map_err(|_| Error("INTERNAL"))?;
    if !record.active() || matches!(action, Action::Snapshot) {
        return Ok(record);
    }
    if row.1 != instance || !row.2 {
        return Err(Error("LEASE_LOST"));
    }
    let expired = record.deadline <= now_ms() || record.created + 900_000 <= now_ms();
    let available = matches!(action, Action::Cancel | Action::Expire)
        || sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM users WHERE id=ANY($1) AND status='active' AND email_verified_at IS NOT NULL",
        )
        .bind(&record.users[..])
        .fetch_one(&mut *tx)
        .await?
            == 2;
    if !available {
        record.status = "cancelled".into();
        record.accepted = [false; 2];
    } else if expired {
        record.status = "expired".into();
        record.accepted = [false; 2];
    } else {
        match action {
            Action::Snapshot => unreachable!(),
            Action::Expire => return Ok(record),
            Action::Cancel => {
                record.status = "cancelled".into();
                record.accepted = [false; 2];
            }
            Action::Respond(accept) => {
                if side == 1 && accept && record.status == "offering" {
                    return Ok(record);
                }
                if side != 1 || record.status != "invited" {
                    return Err(Error("CONFLICT"));
                }
                if accept {
                    crate::activity::ensure_idle(&mut tx, user).await?;
                    sqlx::query(
                        "INSERT INTO activity_locks(user_id,kind,owner_id) VALUES ($1,'trade',$2)",
                    )
                    .bind(user)
                    .bind(id)
                    .execute(&mut *tx)
                    .await?;
                    record.status = "offering".into();
                    record.deadline = now_ms() + 120_000;
                } else {
                    record.status = "cancelled".into();
                }
            }
            Action::Update(version, value) => {
                if record.status != "offering" || record.version != version {
                    return Err(Error("CONFLICT"));
                }
                let offer = parse_offer(value)?;
                validate_offer(&mut tx, user, &offer).await?;
                record.offers[side] = offer;
                record.accepted = [false; 2];
                record.version = record.version.checked_add(1).ok_or(Error("CONFLICT"))?;
                record.deadline = (now_ms() + 120_000).min(record.created + 900_000);
            }
            Action::Confirm(version) => {
                if record.status != "offering" || record.version != version {
                    return Err(Error("CONFLICT"));
                }
                if record.accepted[side] {
                    return Ok(record);
                }
                validate_offer(&mut tx, user, &record.offers[side]).await?;
                record.accepted[side] = true;
                if record.accepted == [true; 2] {
                    settle(&mut tx, &record).await?;
                    record.status = "completed".into();
                }
            }
        }
    }
    record.revision = record.revision.checked_add(1).ok_or(Error("CONFLICT"))?;
    if !record.active() {
        sqlx::query("DELETE FROM activity_locks WHERE kind='trade' AND owner_id=$1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    let bytes = serde_json::to_vec(&record).map_err(|_| Error("INTERNAL"))?;
    sqlx::query("UPDATE trades SET status=$2,state=$3,revision=$4,finished_at=CASE WHEN $5 THEN NULL ELSE now() END WHERE id=$1").bind(id).bind(&record.status).bind(bytes).bind(record.revision).bind(record.active()).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(record)
}
/// Expired actors are cancelled, never replayed into a transfer. Live instances retain their leases.
async fn recover_records(db: &PgPool) -> Result<Vec<Record>> {
    let rows=sqlx::query_as::<_,(Uuid,Uuid,Uuid)>("SELECT id,from_user,to_user FROM trades WHERE status IN ('invited','offering') AND lease_until<=clock_timestamp() ORDER BY id LIMIT 20").fetch_all(db).await?;
    let mut users = Vec::new();
    for (id, a, b) in rows {
        let mut tx = db.begin().await?;
        lock_users(&mut tx, [a, b]).await?;
        let row=sqlx::query_scalar::<_,Vec<u8>>("SELECT state FROM trades WHERE id=$1 AND status IN ('invited','offering') AND lease_until<=clock_timestamp() FOR UPDATE").bind(id).fetch_optional(&mut *tx).await?;
        let Some(bytes) = row else {
            continue;
        };
        let mut record: Record = serde_json::from_slice(&bytes).map_err(|_| Error("INTERNAL"))?;
        record.status = "cancelled".into();
        record.accepted = [false; 2];
        record.revision += 1;
        sqlx::query("UPDATE trades SET status='cancelled',finished_at=now(),revision=$2,state=$3 WHERE id=$1").bind(id).bind(record.revision).bind(serde_json::to_vec(&record).map_err(|_|Error("INTERNAL"))?).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM activity_locks WHERE kind='trade' AND owner_id=$1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        users.push(record);
    }
    Ok(users)
}
pub async fn recover(db: &PgPool) -> Result<Vec<Uuid>> {
    Ok(recover_records(db)
        .await?
        .into_iter()
        .flat_map(|r| r.users)
        .collect())
}
