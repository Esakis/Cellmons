use super::{
    Error, Result, Settings,
    actor::{Action, Actor, Command, Notice},
    journal,
    record::Record,
};
use protocol::game::v1::server_message::Body;
use sqlx::PgPool;
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

#[derive(Clone)]
struct Handle {
    sender: mpsc::Sender<Command>,
    abort: tokio::task::AbortHandle,
}
struct Shared {
    db: PgPool,
    instance: Uuid,
    settings: Settings,
    notices: mpsc::Sender<Notice>,
    actors: Mutex<HashMap<Uuid, Handle>>,
    users: Mutex<HashMap<Uuid, Uuid>>,
    stopping: AtomicBool,
}
#[derive(Clone)]
pub struct Manager(Arc<Shared>);
impl Manager {
    pub fn new(
        db: PgPool,
        settings: Settings,
        notices: mpsc::Sender<Notice>,
        enabled: bool,
    ) -> Self {
        let manager = Self(Arc::new(Shared {
            db,
            instance: Uuid::now_v7(),
            settings,
            notices,
            actors: Mutex::new(HashMap::new()),
            users: Mutex::new(HashMap::new()),
            stopping: AtomicBool::new(false),
        }));
        if enabled {
            let recovering = manager.clone();
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_millis(500));
                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                loop {
                    interval.tick().await;
                    if recovering.0.stopping.load(Ordering::Acquire) {
                        break;
                    }
                    if !recovering.has_capacity() {
                        continue;
                    }
                    if let Ok(Ok(records)) = tokio::time::timeout(
                        Duration::from_secs(3),
                        journal::recover(
                            &recovering.0.db,
                            recovering.instance(),
                            recovering.0.settings.lease_ms(),
                        ),
                    )
                    .await
                    {
                        for mut record in records {
                            if recovering.contains(record.id) {
                                continue;
                            }
                            if journal::verify_rounds(&recovering.0.db, &record)
                                .await
                                .is_err()
                            {
                                tracing::error!(duel=%record.id,"duel journal checksum mismatch");
                                continue;
                            }
                            let Ok((state, _)) = record.rebuild() else {
                                tracing::error!(duel=%record.id,"duel journal cannot be replayed");
                                continue;
                            };
                            let previous = record.clone();
                            if !state.is_finished() {
                                for i in 0..2 {
                                    if record.seats[i].user.is_some() {
                                        record.connected[i] = false;
                                        record.grace_until[i] = Some(
                                            crate::online::now_ms()
                                                + recovering.0.settings.reconnect_ms(),
                                        );
                                    }
                                }
                                let timeout = if record.pending.iter().any(Option::is_some) {
                                    recovering.0.settings.opponent_ms()
                                } else {
                                    recovering.0.settings.round_ms()
                                };
                                record.deadline_ms = crate::online::now_ms() + timeout;
                                record.revision = record.revision.saturating_add(1);
                                if journal::save(
                                    &recovering.0.db,
                                    recovering.instance(),
                                    &previous,
                                    &record,
                                    &state,
                                )
                                .await
                                .is_err()
                                {
                                    continue;
                                }
                            }
                            let users = record.seats.iter().filter_map(|s| s.user).collect();
                            let id = record.id;
                            recovering.start(record);
                            let _ = recovering
                                .0
                                .notices
                                .send(Notice::Recovered(id, users))
                                .await;
                        }
                    }
                }
            });
        }
        manager
    }
    pub fn instance(&self) -> Uuid {
        self.0.instance
    }
    pub fn has_capacity(&self) -> bool {
        self.0.actors.lock().expect("duel registry").len() < self.0.settings.max_active
    }
    pub fn contains(&self, id: Uuid) -> bool {
        self.0
            .actors
            .lock()
            .expect("duel registry")
            .contains_key(&id)
    }
    pub fn active_for(&self, user: Uuid) -> Option<Uuid> {
        self.0.users.lock().expect("duel users").get(&user).copied()
    }
    pub fn start(&self, record: Record) {
        let id = record.id;
        let users: Vec<_> = record.seats.iter().filter_map(|s| s.user).collect();
        let mut actors = self.0.actors.lock().expect("duel registry");
        if actors.contains_key(&id)
            || actors.len() >= self.0.settings.max_active
            || self.0.stopping.load(Ordering::Acquire)
        {
            return;
        }
        for user in &users {
            self.0.users.lock().expect("duel users").insert(*user, id);
        }
        let (sender, receiver) = mpsc::channel(64);
        let actor = Actor {
            db: self.0.db.clone(),
            instance: self.instance(),
            record,
            settings: self.0.settings.clone(),
            notices: self.0.notices.clone(),
            receiver,
        };
        let manager = self.clone();
        let task = tokio::spawn(async move {
            if let Err(error) = actor.run().await {
                tracing::error!(duel=%id,code=error.0,"duel actor stopped");
            }
            manager.0.actors.lock().expect("duel registry").remove(&id);
            for user in &users {
                let mut map = manager.0.users.lock().expect("duel users");
                if map.get(user) == Some(&id) {
                    map.remove(user);
                }
            }
            let _ = manager.0.notices.send(Notice::Finished(id, users)).await;
        });
        actors.insert(
            id,
            Handle {
                sender,
                abort: task.abort_handle(),
            },
        );
    }
    pub fn connection(&self, user: Uuid, connected: bool) {
        if let Some(id) = self.active_for(user)
            && let Some(handle) = self.0.actors.lock().expect("duel registry").get(&id)
        {
            let _ = handle.sender.try_send(Command::Connection(user, connected));
        }
    }
    pub async fn request(&self, user: Uuid, id: Uuid, action: Action) -> Result<Body> {
        let sender = self
            .0
            .actors
            .lock()
            .expect("duel registry")
            .get(&id)
            .map(|h| h.sender.clone())
            .ok_or(Error("NOT_FOUND"))?;
        let (reply, receiver) = oneshot::channel();
        sender
            .try_send(Command::Request {
                user,
                action,
                reply,
            })
            .map_err(|_| Error("BUSY"))?;
        tokio::time::timeout(Duration::from_secs(4), receiver)
            .await
            .map_err(|_| Error("TIMEOUT"))?
            .map_err(|_| Error("UNAVAILABLE"))?
    }
    pub async fn shutdown(&self) {
        self.0.stopping.store(true, Ordering::Release);
        let handles: Vec<_> = self
            .0
            .actors
            .lock()
            .expect("duel registry")
            .values()
            .cloned()
            .collect();
        futures_util::future::join_all(handles.into_iter().map(|handle| async move {
            let (reply, receiver) = oneshot::channel();
            let stopped = tokio::time::timeout(Duration::from_secs(5), async {
                handle
                    .sender
                    .send(Command::Stop(reply))
                    .await
                    .map_err(|_| ())?;
                receiver.await.map_err(|_| ())
            })
            .await;
            if !matches!(stopped, Ok(Ok(()))) {
                handle.abort.abort();
            }
        }))
        .await;
    }
}
