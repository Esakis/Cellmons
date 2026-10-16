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

// Presence lists need no biography, character UUID, clan details or combat statistics.
// Full profiles are fetched by WhoIs; the owner continues to receive its full own profile.
fn presence(player: &pb::PublicPlayer) -> pb::PublicPlayer {
    pb::PublicPlayer {
        id: player.id.clone(),
        login: player.login.clone(),
        template_id: player.template_id,
        path_id: player.path_id,
        level: player.level,
        rating: player.rating,
        title: player.title.clone(),
        afk: player.afk,
        verified: player.verified,
        busy: player.busy,
        mentor: player.mentor,
        tier: player.tier,
        ..Default::default()
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
                        let joined = Output::encode(Body::RoomDelta(pb::RoomDelta {
                            room: name.clone(),
                            joined: vec![presence(&player)],
                            ..Default::default()
                        }));
                        if !members.contains_key(&player.id) {
                            for other in members.values() {
                                other.output.shared(&joined);
                            }
                        }
                        let id = player.id.clone();
                        members.insert(id.clone(), member);
                        let mut players: Vec<_> = members
                            .values()
                            .map(|m| {
                                if m.player.id == id {
                                    m.player.clone()
                                } else {
                                    presence(&m.player)
                                }
                            })
                            .collect();
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
                            motd: String::new(),
                        });
                    }
                    Command::Leave(id) => {
                        if members.remove(&id).is_some() {
                            let left = Output::encode(Body::RoomDelta(pb::RoomDelta {
                                room: name.clone(),
                                left: vec![id],
                                ..Default::default()
                            }));
                            for member in members.values() {
                                member.output.shared(&left);
                            }
                        }
                    }
                    Command::Update(member) => {
                        let player = member.player.clone();
                        if members.contains_key(&player.id) {
                            members.insert(player.id.clone(), member);
                            let updated = Output::encode(Body::RoomDelta(pb::RoomDelta {
                                room: name.clone(),
                                updated: vec![presence(&player)],
                                ..Default::default()
                            }));
                            for member in members.values() {
                                if member.player.id != player.id {
                                    member.output.shared(&updated);
                                    continue;
                                }
                                member.output.send(
                                    Body::RoomDelta(pb::RoomDelta {
                                        room: name.clone(),
                                        updated: vec![if member.player.id == player.id {
                                            player.clone()
                                        } else {
                                            presence(&player)
                                        }],
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
                        let shared = Output::encode(Body::ChatMessage(message.clone()));
                        for member in members.values() {
                            if member.receives_room(&name)
                                && !member.ignores.contains(&message.from_id)
                            {
                                member.output.shared(&shared);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::online::hub::Outbound;
    use protocol::Message as _;

    fn member(id: &str) -> (Member, mpsc::Receiver<(Outbound, u32)>) {
        let (data, rx) = mpsc::channel(16);
        let (close, _) = tokio::sync::watch::channel(None);
        (
            Member {
                player: pb::PublicPlayer {
                    id: id.into(),
                    login: id.into(),
                    description: "Full biography".into(),
                    avatar_id: "private-character-id".into(),
                    stats: vec![30; 6],
                    ..Default::default()
                },
                output: Output { data, close },
                ignores: HashSet::new(),
                filter: "all".into(),
            },
            rx,
        )
    }
    async fn delta(rx: &mut mpsc::Receiver<(Outbound, u32)>) -> pb::RoomDelta {
        let (out, _) = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
            .await
            .unwrap()
            .unwrap();
        let body = match out {
            Outbound::Body(body) => body,
            Outbound::Encoded(raw) => pb::ServerMessage::decode(raw).unwrap().body.unwrap(),
        };
        match body {
            Body::RoomDelta(delta) => delta,
            _ => panic!("expected room delta"),
        }
    }
    #[tokio::test]
    async fn rejoining_is_idempotent_and_updates_preserve_the_owners_full_profile() {
        let room = Room::new("arena".into());
        let (mut owner, mut own_events) = member("owner");
        let (observer, mut observed_events) = member("observer");
        room.join(owner.clone(), None).await.unwrap();
        let snapshot = room.join(observer, None).await.unwrap();
        assert!(
            snapshot
                .players
                .iter()
                .find(|p| p.id == "owner")
                .unwrap()
                .description
                .is_empty()
        );
        delta(&mut own_events).await;
        let snapshot = room.join(owner.clone(), None).await.unwrap();
        assert_eq!(
            snapshot
                .players
                .iter()
                .find(|p| p.id == "owner")
                .unwrap()
                .description,
            "Full biography"
        );
        assert!(observed_events.try_recv().is_err());
        assert!(own_events.try_recv().is_err());
        owner.player.afk = true;
        room.update(owner).await.unwrap();
        let private = delta(&mut own_events).await.updated.remove(0);
        let public = delta(&mut observed_events).await.updated.remove(0);
        assert!(private.afk && public.afk);
        assert_eq!(private.description, "Full biography");
        assert_eq!(private.stats, vec![30; 6]);
        assert!(
            public.description.is_empty() && public.stats.is_empty() && public.avatar_id.is_empty()
        );
    }
    #[test]
    fn room_presence_fits_the_typical_500_player_budget_without_full_profiles() {
        let players = (0..500)
            .map(|i| {
                presence(&pb::PublicPlayer {
                    id: format!("00000000-0000-0000-0000-{i:012}"),
                    login: format!("Player{i}"),
                    template_id: 1,
                    level: 20,
                    rating: 5000,
                    verified: true,
                    title: "rank-master".into(),
                    description: "Biography only available in a full profile".into(),
                    avatar_id: "character UUID".into(),
                    stats: vec![30; 6],
                    ..Default::default()
                })
            })
            .collect::<Vec<_>>();
        assert!(
            players
                .iter()
                .all(|p| p.description.is_empty() && p.avatar_id.is_empty() && p.stats.is_empty())
        );
        let snapshot = pb::RoomSnapshot {
            players,
            ..Default::default()
        };
        assert!(snapshot.encoded_len() <= 40 * 1024);
    }
}
