use super::{Error, Result, hub::Output};
use protocol::game::v1::{self as pb, server_message::Body};
use std::collections::{HashMap, HashSet, VecDeque};
use tokio::sync::{mpsc, oneshot};

#[derive(Clone)]
pub struct Member {
    pub player: pb::PublicPlayer,
    pub output: Output,
    pub ignores: HashSet<String>,
    pub filter: String,
}
impl Member {
    fn receives_room(&self, name: &str) -> bool {
        self.filter == "all" || (name.starts_with("clan:") && self.filter == "clan_private")
    }
}
enum Command {
    Join(
        Member,
        Option<HashSet<String>>,
        oneshot::Sender<pb::RoomSnapshot>,
    ),
    Leave(String),
    Update(Member),
    Chat(pb::ChatMessage, Option<HashSet<String>>),
}
#[derive(Clone)]
pub struct Room(mpsc::Sender<Command>);
impl Room {
    pub fn new(name: String) -> Self {
        let (tx, mut rx) = mpsc::channel::<Command>(256);
        tokio::spawn(async move {
            let mut members = HashMap::<String, Member>::new();
            let mut history = VecDeque::<pb::ChatMessage>::new();
            while let Some(command) = rx.recv().await {
                match command {
                    Command::Join(member, allowed, reply) => {
                        if let Some(allowed) = allowed {
                            members.retain(|id, _| allowed.contains(id));
                        }
                        let player = member.player.clone();
                        for other in members.values() {
                            other.output.send(
                                Body::RoomDelta(pb::RoomDelta {
                                    room: name.clone(),
                                    joined: vec![player.clone()],
                                    ..Default::default()
                                }),
                                0,
                            );
                        }
                        let id = player.id.clone();
                        members.insert(id.clone(), member);
                        let mut players: Vec<_> =
                            members.values().map(|m| m.player.clone()).collect();
                        players.sort_by_key(|a| a.login.to_lowercase());
                        let own = &members[&id];
                        let messages = if own.receives_room(&name) {
                            history
                                .iter()
                                .filter(|message| !own.ignores.contains(&message.from_id))
                                .cloned()
                                .collect()
                        } else {
                            Vec::new()
                        };
                        let _ = reply.send(pb::RoomSnapshot {
                            room: name.clone(),
                            players,
                            messages,
                            recent_duels: Vec::new(),
                        });
                    }
                    Command::Leave(id) => {
                        if members.remove(&id).is_some() {
                            for member in members.values() {
                                member.output.send(
                                    Body::RoomDelta(pb::RoomDelta {
                                        room: name.clone(),
                                        left: vec![id.clone()],
                                        ..Default::default()
                                    }),
                                    0,
                                );
                            }
                        }
                    }
                    Command::Update(member) => {
                        let player = member.player.clone();
                        if members.contains_key(&player.id) {
                            members.insert(player.id.clone(), member);
                            for member in members.values() {
                                member.output.send(
                                    Body::RoomDelta(pb::RoomDelta {
                                        room: name.clone(),
                                        updated: vec![player.clone()],
                                        ..Default::default()
                                    }),
                                    0,
                                );
                            }
                        }
                    }
                    Command::Chat(message, allowed) => {
                        if let Some(allowed) = allowed {
                            members.retain(|id, _| allowed.contains(id));
                        }
                        for member in members.values() {
                            if member.receives_room(&name)
                                && !member.ignores.contains(&message.from_id)
                            {
                                member.output.send(Body::ChatMessage(message.clone()), 0);
                            }
                        }
                        history.push_back(message);
                        while history.len() > 50 {
                            history.pop_front();
                        }
                    }
                }
            }
        });
        Self(tx)
    }
    pub async fn join(
        &self,
        member: Member,
        allowed: Option<HashSet<String>>,
    ) -> Result<pb::RoomSnapshot> {
        let (tx, rx) = oneshot::channel();
        self.0
            .try_send(Command::Join(member, allowed, tx))
            .map_err(|_| Error("BUSY"))?;
        rx.await.map_err(|_| Error("INTERNAL"))
    }
    pub async fn leave(&self, id: String) -> Result<()> {
        self.0
            .send(Command::Leave(id))
            .await
            .map_err(|_| Error("BUSY"))
    }
    pub async fn update(&self, member: Member) -> Result<()> {
        self.0
            .send(Command::Update(member))
            .await
            .map_err(|_| Error("BUSY"))
    }
    pub fn chat(&self, message: pb::ChatMessage, allowed: Option<HashSet<String>>) -> Result<()> {
        self.0
            .try_send(Command::Chat(message, allowed))
            .map_err(|_| Error("BUSY"))
    }
}
