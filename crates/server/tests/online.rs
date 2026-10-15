//! Real HTTP/WebSocket acceptance tests on isolated PostgreSQL databases.
use futures_util::{SinkExt as _, StreamExt as _};
use protocol::{
    Message as _,
    game::v1::{self as pb, client_message::Body as Client, server_message::Body},
};
use server::{
    gateway::{self, AppState},
    online::Settings,
};
use sha2::{Digest as _, Sha256};
use sqlx::PgPool;
use std::{collections::VecDeque, net::SocketAddr, time::Duration};
use tokio::{net::TcpStream, sync::oneshot};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::{Message, client::IntoClientRequest as _},
};
use uuid::Uuid;

struct Player {
    id: Uuid,
    login: String,
    token: String,
}
async fn player(db: &PgPool, index: usize, verified: bool) -> Player {
    let id = Uuid::now_v7();
    let login = format!("Player{index}");
    let email = format!("player{index}@example.test");
    let token = format!("{}{index:032x}", id.simple());
    let hash = Sha256::digest(token.as_bytes()).to_vec();
    sqlx::query("INSERT INTO users(id,login,email,password_hash,start_school,email_verified_at) VALUES ($1,$2,$3,'$argon2id$v=19$m=19456,t=2,p=1$fixture$fixture','combat',CASE WHEN $4 THEN now() ELSE NULL END)").bind(id).bind(&login).bind(email).bind(verified).execute(db).await.unwrap();
    sqlx::query("INSERT INTO sessions(id_hash,kind,user_id,expires_at) VALUES ($1,'web',$2,now()+interval '1 day')").bind(hash).bind(id).execute(db).await.unwrap();
    let avatar = Uuid::now_v7();
    sqlx::query("INSERT INTO unlocked_templates(user_id,template_id,source) VALUES ($1,1,'start')")
        .bind(id)
        .execute(db)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO avatars(id,user_id,slot,template_id,is_active) VALUES ($1,$2,0,1,true)",
    )
    .bind(avatar)
    .bind(id)
    .execute(db)
    .await
    .unwrap();
    let starter = &engine_content::STARTERS[0];
    for (card, qty) in starter.cards {
        sqlx::query("INSERT INTO collection(user_id,card_id,qty) VALUES ($1,$2,$3)")
            .bind(id)
            .bind(*card as i16)
            .bind(i32::from(*qty))
            .execute(db)
            .await
            .unwrap();
    }
    sqlx::query("INSERT INTO decks(avatar_id,cards) VALUES ($1,$2)")
        .bind(avatar)
        .bind(starter.deck.iter().map(|id| *id as i16).collect::<Vec<_>>())
        .execute(db)
        .await
        .unwrap();
    sqlx::query("INSERT INTO ratings(user_id) VALUES ($1)")
        .bind(id)
        .execute(db)
        .await
        .unwrap();
    let mut tx = db.begin().await.unwrap();
    server::economy::grant_initial(&mut tx, id).await.unwrap();
    tx.commit().await.unwrap();
    Player { id, login, token }
}
struct Server {
    state: AppState,
    addr: SocketAddr,
    halt: Option<oneshot::Sender<()>>,
    task: Option<tokio::task::JoinHandle<()>>,
}
impl Server {
    async fn new(db: PgPool, settings: Settings) -> Self {
        let state = AppState::new(db, server::auth::Auth::local(), settings);
        let app = gateway::router(state.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (halt, stop) = oneshot::channel();
        let online = state.online.clone();
        let task = tokio::spawn(async move {
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .with_graceful_shutdown(async move {
                let _ = stop.await;
                online.hub.shutdown().await;
            })
            .await
            .unwrap();
        });
        Self {
            state,
            addr,
            halt: Some(halt),
            task: Some(task),
        }
    }
    async fn stop(mut self) {
        let _ = self.halt.take().unwrap().send(());
        tokio::time::timeout(Duration::from_secs(5), self.task.take().unwrap())
            .await
            .unwrap()
            .unwrap();
    }
    async fn connect(&self, player: &Player, resume: Option<String>) -> ClientSocket {
        let mut request = format!("ws://{}/ws", self.addr)
            .into_client_request()
            .unwrap();
        request
            .headers_mut()
            .insert("origin", "http://localhost:5173".parse().unwrap());
        request.headers_mut().insert(
            "cookie",
            format!("game_session={}", player.token).parse().unwrap(),
        );
        let (socket, _) = connect_async(request).await.unwrap();
        let mut client = ClientSocket {
            socket,
            seq: 0,
            events: VecDeque::new(),
            resume: String::new(),
        };
        let body = resume
            .map(|resume_token| Client::Resume(pb::Resume { resume_token }))
            .unwrap_or_else(hello);
        let welcome = client.call(body).await;
        let Body::Welcome(welcome) = welcome else {
            panic!("expected welcome: {welcome:?}");
        };
        client.resume = welcome.resume_token;
        client
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        if let Some(halt) = self.halt.take() {
            let _ = halt.send(());
        }
    }
}
fn hello() -> Client {
    Client::Hello(pb::Hello {
        protocol_major: 1,
        protocol_minor: 0,
        build: "test".into(),
        content_version: engine_content::CONTENT_VERSION.to_vec(),
        locale: "en".into(),
        client_kind: "web".into(),
        native_version: String::new(),
    })
}
struct ClientSocket {
    socket: WebSocketStream<MaybeTlsStream<TcpStream>>,
    seq: u32,
    events: VecDeque<Body>,
    resume: String,
}
impl ClientSocket {
    async fn send(&mut self, body: Client) -> u32 {
        self.seq += 1;
        let seq = self.seq;
        self.socket
            .send(Message::Binary(
                pb::ClientMessage {
                    seq,
                    body: Some(body),
                }
                .encode_to_vec()
                .into(),
            ))
            .await
            .unwrap();
        seq
    }
    async fn read(&mut self) -> pb::ServerMessage {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                match self.socket.next().await.unwrap().unwrap() {
                    Message::Binary(bytes) => return pb::ServerMessage::decode(bytes).unwrap(),
                    Message::Ping(bytes) => {
                        self.socket.send(Message::Pong(bytes)).await.unwrap();
                    }
                    other => panic!("unexpected frame: {other:?}"),
                }
            }
        })
        .await
        .expect("WebSocket reply deadline")
    }
    async fn reply(&mut self, seq: u32) -> Body {
        loop {
            let reply = self.read().await;
            let body = reply.body.unwrap();
            if reply.reply_to == seq {
                return body;
            }
            self.events.push_back(body);
        }
    }
    async fn call(&mut self, body: Client) -> Body {
        let seq = self.send(body).await;
        self.reply(seq).await
    }
    async fn event(&mut self, predicate: impl Fn(&Body) -> bool) -> Body {
        if let Some(index) = self.events.iter().position(&predicate) {
            return self.events.remove(index).unwrap();
        }
        loop {
            let message = self.read().await;
            let body = message.body.unwrap();
            if predicate(&body) {
                return body;
            }
            self.events.push_back(body);
        }
    }
    async fn join(&mut self, room: &str) -> pb::RoomSnapshot {
        match self
            .call(Client::RoomJoin(pb::RoomJoin { room: room.into() }))
            .await
        {
            Body::RoomSnapshot(snapshot) => snapshot,
            body => panic!("room join: {body:?}"),
        }
    }
}
fn chat(text: &str) -> Client {
    Client::ChatSend(pb::ChatSend {
        channel: "room".into(),
        to: String::new(),
        text: text.into(),
    })
}
fn target(name: &str) -> pb::PlayerTarget {
    pb::PlayerTarget {
        player: name.into(),
    }
}
fn assert_ok(body: Body) {
    assert!(matches!(body, Body::Ok(_)), "{body:?}");
}
fn assert_error(body: Body, expected: &str) {
    assert!(
        matches!(&body,Body::Error(error) if error.code==expected),
        "{body:?}, expected {expected}"
    );
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn f63_hello_binary_ping_resume_replacement_and_session_revocation(pool: PgPool) {
    let a = player(&pool, 1, true).await;
    let server = Server::new(pool.clone(), Settings::default()).await;
    let mut first = server.connect(&a, None).await;
    let resume = first.resume.clone();
    assert!(matches!(
        first.call(Client::Ping(pb::Ping {})).await,
        Body::Pong(_)
    ));
    first.join("arena").await;
    let mut second = server.connect(&a, Some(resume.clone())).await;
    assert!(matches!(
        first.event(|b| matches!(b, Body::SessionReplaced(_))).await,
        Body::SessionReplaced(_)
    ));
    let room = second.event(|b| matches!(b, Body::RoomSnapshot(_))).await;
    assert!(matches!(room,Body::RoomSnapshot(r) if r.room=="arena"));
    assert_ne!(resume, second.resume);
    let client = reqwest::Client::new();
    let response = client
        .post(format!("http://{}/api/auth/logout", server.addr))
        .header("x-requested-with", "app")
        .header("cookie", format!("game_session={}", a.token))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 204);
    assert!(
        matches!(second.event(|b|matches!(b,Body::Error(_))).await,Body::Error(e) if e.code=="UNAUTHENTICATED")
    );
    drop(first);
    drop(second);
    server.stop().await;
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn r_kom_01_room_chat_presence_and_filters(pool: PgPool) {
    let a = player(&pool, 1, true).await;
    let b = player(&pool, 2, true).await;
    let server = Server::new(pool.clone(), Settings::default()).await;
    let mut one = server.connect(&a, None).await;
    let mut two = server.connect(&b, None).await;
    assert_eq!(one.join("arena").await.players.len(), 1);
    assert_eq!(two.join("arena").await.players.len(), 2);
    assert!(
        matches!(one.event(|b|matches!(b,Body::RoomDelta(_))).await,Body::RoomDelta(delta) if delta.joined[0].login==b.login)
    );
    assert_ok(one.call(chat("<script>safe text</script>")).await);
    assert!(
        matches!(two.event(|b|matches!(b,Body::ChatMessage(_))).await,Body::ChatMessage(message) if message.text=="<script>safe text</script>" && message.from_id==a.id.to_string())
    );
    assert_ok(two.call(Client::SetAfk(pb::SetAfk { afk: true })).await);
    assert!(
        matches!(one.event(|b|matches!(b,Body::RoomDelta(d) if !d.updated.is_empty())).await,Body::RoomDelta(d) if d.updated[0].afk)
    );
    assert_ok(one.call(Client::IgnoreAdd(target(&b.login))).await.map_ok());
    // A filter affects both live delivery and the current room's history snapshot.
    assert_ok(
        one.call(Client::SetChatFilter(pb::SetChatFilter {
            filter: "private".into(),
        }))
        .await,
    );
    assert_ok(two.call(chat("hidden room line")).await);
    assert!(one.join("arena").await.messages.is_empty());
    assert_error(
        one.call(Client::ChatSend(pb::ChatSend {
            channel: "private".into(),
            to: b.login.clone(),
            text: "blocked".into(),
        }))
        .await,
        "FORBIDDEN",
    );
    let stored: i64 = sqlx::query_scalar("SELECT count(*) FROM chat_log")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stored, 2);
    drop(one);
    drop(two);
    server.stop().await;
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn r_knt_02_unverified_private_chat_requires_recipient_friend_and_mail_is_owned(
    pool: PgPool,
) {
    let a = player(&pool, 1, false).await;
    let b = player(&pool, 2, true).await;
    let c = player(&pool, 3, true).await;
    let server = Server::new(pool.clone(), Settings::default()).await;
    let mut one = server.connect(&a, None).await;
    let mut two = server.connect(&b, None).await;
    let mut three = server.connect(&c, None).await;
    let private = || {
        Client::ChatSend(pb::ChatSend {
            channel: "private".into(),
            to: b.login.clone(),
            text: "private greeting".into(),
        })
    };
    assert_error(one.call(private()).await, "FORBIDDEN");
    assert!(matches!(
        two.call(Client::FriendAdd(target(&a.login))).await,
        Body::SocialSnapshot(_)
    ));
    let mut allowed = private();
    if let Client::ChatSend(ref mut msg) = allowed {
        msg.text = "invited greeting".into();
    }
    assert_ok(one.call(allowed).await);
    assert!(
        matches!(two.event(|b|matches!(b,Body::ChatMessage(_))).await,Body::ChatMessage(message) if message.text=="invited greeting")
    );
    assert_ok(
        one.call(Client::MailSend(pb::MailSend {
            to: b.login.clone(),
            text: "offline letter".into(),
        }))
        .await,
    );
    let inbox = two.call(Client::MailList(pb::MailList::default())).await;
    let Body::MailSnapshot(inbox) = inbox else {
        panic!()
    };
    assert_eq!(inbox.messages.len(), 1);
    let id = inbox.messages[0].id.clone();
    assert_ok(
        three
            .call(Client::MailRead(pb::MailTarget { id: id.clone() }))
            .await,
    );
    let read: bool = sqlx::query_scalar("SELECT read_at IS NOT NULL FROM mail WHERE id=$1::uuid")
        .bind(&id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(!read);
    assert_ok(
        two.call(Client::MailRead(pb::MailTarget { id: id.clone() }))
            .await,
    );
    assert_ok(
        two.call(Client::MailDelete(pb::MailTarget { id: id.clone() }))
            .await,
    );
    assert!(
        matches!(two.call(Client::MailList(pb::MailList::default())).await,Body::MailSnapshot(m) if m.messages.is_empty())
    );
    assert!(
        matches!(one.call(Client::MailList(pb::MailList{sent:true,..Default::default()})).await,Body::MailSnapshot(m) if m.messages.len()==1 && m.messages[0].read)
    );
    drop(one);
    drop(two);
    drop(three);
    server.stop().await;
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn r_lob_03_06_filters_ownership_races_and_immutable_duel_handoff(pool: PgPool) {
    let a = player(&pool, 1, true).await;
    let b = player(&pool, 2, true).await;
    let c = player(&pool, 3, true).await;
    let server = Server::new(
        pool.clone(),
        Settings {
            run_duels: false,
            ..Settings::default()
        },
    )
    .await;
    let mut one = server.connect(&a, None).await;
    let mut two = server.connect(&b, None).await;
    let mut three = server.connect(&c, None).await;
    one.join("arena").await;
    two.join("arena").await;
    three.join("arena").await;
    assert_ok(
        two.call(Client::SetChallengeFilter(pb::SetChallengeFilter {
            filter: "none".into(),
        }))
        .await,
    );
    assert_error(
        one.call(Client::ChallengeSend(pb::ChallengeSend {
            target: b.login.clone(),
        }))
        .await,
        "FORBIDDEN",
    );
    assert_ok(
        two.call(Client::SetChallengeFilter(pb::SetChallengeFilter {
            filter: "all".into(),
        }))
        .await,
    );
    let offer = one
        .call(Client::ChallengeSend(pb::ChallengeSend {
            target: b.login.clone(),
        }))
        .await;
    let Body::ChallengeReceived(offer) = offer else {
        panic!("{offer:?}")
    };
    let id = offer.challenge_id;
    assert!(server.state.online.hub.is_locked(a.id).await.unwrap());
    assert!(!server.state.online.hub.is_locked(b.id).await.unwrap());
    assert_error(
        one.call(Client::ChallengeSend(pb::ChallengeSend {
            target: c.login.clone(),
        }))
        .await,
        "BUSY",
    );
    assert_error(
        three
            .call(Client::ChallengeRespond(pb::ChallengeRespond {
                challenge_id: id.clone(),
                accept: true,
            }))
            .await,
        "FORBIDDEN",
    );
    let other = three
        .call(Client::ChallengeSend(pb::ChallengeSend {
            target: b.login.clone(),
        }))
        .await;
    let Body::ChallengeReceived(other) = other else {
        panic!()
    };
    let first = two
        .send(Client::ChallengeRespond(pb::ChallengeRespond {
            challenge_id: id,
            accept: true,
        }))
        .await;
    let second = two
        .send(Client::ChallengeRespond(pb::ChallengeRespond {
            challenge_id: other.challenge_id,
            accept: true,
        }))
        .await;
    let accepted = two.reply(first).await;
    let Body::ChallengeUpdate(accepted) = accepted else {
        panic!("{accepted:?}")
    };
    assert_eq!(accepted.status, "accepted");
    assert!(server.state.online.hub.is_locked(a.id).await.unwrap());
    assert!(server.state.online.hub.is_locked(b.id).await.unwrap());
    assert_error(two.reply(second).await, "NOT_FOUND");
    let duel = Uuid::parse_str(&accepted.duel_id).unwrap();
    let prepared = server.state.online.hub.claim_duel(duel).await.unwrap();
    assert_eq!(prepared.players[0].user, a.id);
    assert_eq!(prepared.players[1].user, b.id);
    assert_eq!(prepared.players[0].setup.deck.len(), 20);
    assert!(server.state.online.hub.claim_duel(duel).await.is_err());
    sqlx::query("UPDATE decks SET cards='{}'")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(prepared.players[0].setup.deck.len(), 20);
    assert_error(
        one.call(Client::ChallengeSend(pb::ChallengeSend {
            target: c.login.clone(),
        }))
        .await,
        "BUSY",
    );
    server.state.online.hub.finish_duel(duel).await;
    assert!(!server.state.online.hub.is_locked(a.id).await.unwrap());
    assert!(matches!(
        one.call(Client::ChallengeSend(pb::ChallengeSend {
            target: c.login.clone()
        }))
        .await,
        Body::ChallengeReceived(_)
    ));
    drop(one);
    drop(two);
    drop(three);
    server.stop().await;
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn r_lob_04_expiry_cancel_disconnect_and_resume_replay(pool: PgPool) {
    let a = player(&pool, 1, true).await;
    let b = player(&pool, 2, true).await;
    let settings = Settings {
        challenge_ttl: Duration::from_millis(150),
        ..Default::default()
    };
    let server = Server::new(pool.clone(), settings).await;
    let mut one = server.connect(&a, None).await;
    let mut two = server.connect(&b, None).await;
    one.join("arena").await;
    two.join("arena").await;
    let offer = one
        .call(Client::ChallengeSend(pb::ChallengeSend {
            target: b.login.clone(),
        }))
        .await;
    assert!(matches!(offer, Body::ChallengeReceived(_)));
    assert!(matches!(
        one.event(|b| matches!(b,Body::ChallengeUpdate(u) if u.status=="expired"))
            .await,
        Body::ChallengeUpdate(_)
    ));
    assert!(matches!(
        one.call(Client::ChallengeSend(pb::ChallengeSend {
            target: b.login.clone()
        }))
        .await,
        Body::ChallengeReceived(_)
    ));
    one.socket.close(None).await.unwrap();
    assert!(matches!(
        two.event(|b| matches!(b,Body::ChallengeUpdate(u) if u.status=="cancelled"))
            .await,
        Body::ChallengeUpdate(_)
    ));
    let saved = one.resume.clone();
    let mut resumed = server.connect(&a, Some(saved)).await;
    assert!(matches!(
        resumed.call(Client::Ping(pb::Ping {})).await,
        Body::Pong(_)
    ));
    drop(one);
    drop(resumed);
    drop(two);
    server.stop().await;
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn f63_thousand_authenticated_connections_and_binary_pong(pool: PgPool) {
    let start = std::time::Instant::now();
    let settings = Settings {
        per_ip: 1200,
        ..Default::default()
    };
    let server = Server::new(pool.clone(), settings).await;
    let mut ids = Vec::new();
    let mut names = Vec::new();
    let mut emails = Vec::new();
    let mut tokens = Vec::new();
    let mut hashes = Vec::new();
    for i in 0..1000 {
        let id = Uuid::now_v7();
        let token = format!("{}{i:032x}", id.simple());
        ids.push(id);
        names.push(format!("Load{i}"));
        emails.push(format!("load{i}@example.test"));
        hashes.push(Sha256::digest(token.as_bytes()).to_vec());
        tokens.push(token);
    }
    sqlx::query("INSERT INTO users(id,login,email,password_hash,start_school) SELECT id,login,email,'$argon2id$v=19$m=19456,t=2,p=1$fixture$fixture','combat' FROM unnest($1::uuid[],$2::text[],$3::text[]) t(id,login,email)").bind(&ids).bind(&names).bind(&emails).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO sessions(id_hash,kind,user_id,expires_at) SELECT hash,'web',id,now()+interval '1 day' FROM unnest($1::bytea[],$2::uuid[]) t(hash,id)").bind(&hashes).bind(&ids).execute(&pool).await.unwrap();
    let mut clients = Vec::new();
    for batch in 0..20 {
        let connections = (batch * 50..(batch + 1) * 50).map(|i| {
            let server = &server;
            let p = Player {
                id: ids[i],
                login: names[i].clone(),
                token: tokens[i].clone(),
            };
            async move {
                let mut client = server.connect(&p, None).await;
                assert!(matches!(
                    client.call(Client::Ping(pb::Ping {})).await,
                    Body::Pong(_)
                ));
                client
            }
        });
        clients.extend(futures_util::future::join_all(connections).await);
    }
    assert_eq!(clients.len(), 1000);
    println!(
        "1000 authenticated WebSockets and correlated Pongs: {:?}",
        start.elapsed()
    );
    drop(clients);
    server.stop().await;
}

trait SocialOk {
    fn map_ok(self) -> Body;
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn review_leave_arena_cancels_offer_and_resume_preserves_afk(pool: PgPool) {
    let a = player(&pool, 1, true).await;
    let b = player(&pool, 2, true).await;
    let server = Server::new(pool, Settings::default()).await;
    let mut one = server.connect(&a, None).await;
    let mut two = server.connect(&b, None).await;
    one.join("arena").await;
    two.join("arena").await;
    let Body::ChallengeReceived(offer) = one
        .call(Client::ChallengeSend(pb::ChallengeSend {
            target: b.login.clone(),
        }))
        .await
    else {
        panic!()
    };
    one.join("castle").await;
    assert_error(
        two.call(Client::ChallengeRespond(pb::ChallengeRespond {
            challenge_id: offer.challenge_id,
            accept: true,
        }))
        .await,
        "NOT_FOUND",
    );
    assert_ok(one.call(Client::SetAfk(pb::SetAfk { afk: true })).await);
    let saved = one.resume.clone();
    let mut resumed = server.connect(&a, Some(saved.clone())).await;
    let Body::PlayerInfo(profile) = resumed
        .call(Client::WhoIs(pb::PlayerTarget {
            player: a.login.clone(),
        }))
        .await
    else {
        panic!()
    };
    assert!(profile.player.unwrap().afk);
    let mut replay = raw_connection(&server, Some(&a.token), "http://localhost:5173", "")
        .await
        .unwrap();
    assert_error(
        replay
            .call(Client::Resume(pb::Resume {
                resume_token: saved.clone(),
            }))
            .await,
        "RESUME_EXPIRED",
    );
    // The rotated token cannot be replayed, even with the same authenticated account.
    assert_error(
        resumed
            .call(Client::Resume(pb::Resume {
                resume_token: saved,
            }))
            .await,
        "CONFLICT",
    );
    drop(one);
    drop(two);
    drop(resumed);
    drop(replay);
    server.stop().await;
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn review_clan_authorization_and_revoked_members_cannot_receive_chat(pool: PgPool) {
    let a = player(&pool, 1, true).await;
    let b = player(&pool, 2, true).await;
    let c = player(&pool, 3, true).await;
    let clan = Uuid::now_v7();
    sqlx::query("INSERT INTO clans(id,name,status) VALUES ($1,'TestClan','active')")
        .bind(clan)
        .execute(&pool)
        .await
        .unwrap();
    for user in [a.id, b.id] {
        sqlx::query("INSERT INTO clan_members(clan_id,user_id,role) VALUES ($1,$2,'member')")
            .bind(clan)
            .bind(user)
            .execute(&pool)
            .await
            .unwrap();
    }
    let server = Server::new(pool.clone(), Settings::default()).await;
    let mut one = server.connect(&a, None).await;
    let mut two = server.connect(&b, None).await;
    let mut three = server.connect(&c, None).await;
    let room = format!("clan:{clan}");
    one.join(&room).await;
    two.join(&room).await;
    assert_ok(
        two.call(Client::SetChatFilter(pb::SetChatFilter {
            filter: "clan_private".into(),
        }))
        .await,
    );
    assert_error(
        three
            .call(Client::RoomJoin(pb::RoomJoin { room: room.clone() }))
            .await,
        "FORBIDDEN",
    );
    assert_ok(
        one.call(Client::ChatSend(pb::ChatSend {
            channel: "clan".into(),
            text: "members only".into(),
            ..Default::default()
        }))
        .await,
    );
    two.event(|b| matches!(b,Body::ChatMessage(m) if m.text=="members only"))
        .await;
    assert_ok(
        one.call(Client::ChatSend(pb::ChatSend {
            channel: "room".into(),
            text: "clan room with filter".into(),
            ..Default::default()
        }))
        .await,
    );
    two.event(|b| matches!(b,Body::ChatMessage(m) if m.text=="clan room with filter"))
        .await;
    sqlx::query("DELETE FROM clan_members WHERE user_id=$1")
        .bind(b.id)
        .execute(&pool)
        .await
        .unwrap();
    assert_error(
        two.call(Client::ChatSend(pb::ChatSend {
            channel: "room".into(),
            text: "revoked".into(),
            ..Default::default()
        }))
        .await,
        "FORBIDDEN",
    );
    assert_ok(
        one.call(Client::ChatSend(pb::ChatSend {
            channel: "room".into(),
            text: "not for revoked member".into(),
            ..Default::default()
        }))
        .await,
    );
    two.events.clear();
    assert!(matches!(
        two.call(Client::Ping(pb::Ping {})).await,
        Body::Pong(_)
    ));
    assert!(
        !two.events
            .iter()
            .any(|body| matches!(body,Body::ChatMessage(m) if m.text=="not for revoked member"))
    );
    drop(one);
    drop(two);
    drop(three);
    server.stop().await;
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn review_heartbeat_revalidates_sessions_and_content_version_is_required(pool: PgPool) {
    let a = player(&pool, 1, true).await;
    let server = Server::new(
        pool.clone(),
        Settings {
            heartbeat: Duration::from_millis(30),
            idle: Duration::from_millis(200),
            ..Settings::default()
        },
    )
    .await;
    let mut invalid = raw_connection(&server, Some(&a.token), "http://localhost:5173", "")
        .await
        .unwrap();
    let Client::Hello(mut message) = hello() else {
        panic!()
    };
    message.content_version[0] ^= 1;
    assert_error(
        invalid.call(Client::Hello(message)).await,
        "VERSION_MISMATCH",
    );
    drop(invalid);
    let mut one = server.connect(&a, None).await;
    sqlx::query("DELETE FROM sessions WHERE user_id=$1")
        .bind(a.id)
        .execute(&pool)
        .await
        .unwrap();
    one.event(|body| matches!(body,Body::Error(e) if e.code=="UNAUTHENTICATED"))
        .await;
    drop(one);
    server.stop().await;
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn review_level_filter_boundary_cancel_and_unclaimed_reservation_release(pool: PgPool) {
    let a = player(&pool, 1, true).await;
    let b = player(&pool, 2, true).await;
    sqlx::query("UPDATE avatars SET level=4 WHERE user_id=$1")
        .bind(b.id)
        .execute(&pool)
        .await
        .unwrap();
    let server = Server::new(
        pool.clone(),
        Settings {
            reservation_ttl: Duration::from_millis(100),
            run_duels: false,
            ..Settings::default()
        },
    )
    .await;
    let mut one = server.connect(&a, None).await;
    let mut two = server.connect(&b, None).await;
    one.join("arena").await;
    two.join("arena").await;
    assert_ok(
        two.call(Client::SetChallengeFilter(pb::SetChallengeFilter {
            filter: "level_pm2".into(),
        }))
        .await,
    );
    assert_error(
        one.call(Client::ChallengeSend(pb::ChallengeSend {
            target: b.login.clone(),
        }))
        .await,
        "FORBIDDEN",
    );
    sqlx::query("UPDATE avatars SET level=3 WHERE user_id=$1")
        .bind(b.id)
        .execute(&pool)
        .await
        .unwrap();
    let mut refreshed = server.connect(&b, Some(two.resume.clone())).await;
    let Body::ChallengeReceived(offer) = one
        .call(Client::ChallengeSend(pb::ChallengeSend {
            target: b.login.clone(),
        }))
        .await
    else {
        panic!()
    };
    assert!(server.state.online.hub.is_locked(a.id).await.unwrap());
    assert_error(
        refreshed
            .call(Client::ChallengeCancel(pb::ChallengeCancel {
                challenge_id: offer.challenge_id.clone(),
            }))
            .await,
        "FORBIDDEN",
    );
    assert_ok(
        one.call(Client::ChallengeCancel(pb::ChallengeCancel {
            challenge_id: offer.challenge_id,
        }))
        .await,
    );
    assert!(!server.state.online.hub.is_locked(a.id).await.unwrap());
    let Body::ChallengeReceived(offer) = one
        .call(Client::ChallengeSend(pb::ChallengeSend {
            target: b.login.clone(),
        }))
        .await
    else {
        panic!()
    };
    assert!(
        matches!(refreshed.call(Client::ChallengeRespond(pb::ChallengeRespond{challenge_id:offer.challenge_id,accept:true})).await,Body::ChallengeUpdate(u) if u.status=="accepted")
    );
    one.event(|body| matches!(body,Body::ChallengeUpdate(u) if u.status=="unavailable"))
        .await;
    assert!(!server.state.online.hub.is_locked(a.id).await.unwrap());
    assert!(!server.state.online.hub.is_locked(b.id).await.unwrap());
    drop(one);
    drop(two);
    drop(refreshed);
    server.stop().await;
}

async fn raw_connection(
    server: &Server,
    token: Option<&str>,
    origin: &str,
    query: &str,
) -> Result<ClientSocket, tokio_tungstenite::tungstenite::Error> {
    let mut request = format!("ws://{}/ws{}", server.addr, query)
        .into_client_request()
        .unwrap();
    request
        .headers_mut()
        .insert("origin", origin.parse().unwrap());
    if let Some(token) = token {
        request
            .headers_mut()
            .insert("cookie", format!("game_session={token}").parse().unwrap());
    }
    let (socket, _) = connect_async(request).await?;
    Ok(ClientSocket {
        socket,
        seq: 0,
        events: VecDeque::new(),
        resume: String::new(),
    })
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn review_origin_auth_connection_limits_and_binary_validation(pool: PgPool) {
    let a = player(&pool, 1, true).await;
    let server = Server::new(
        pool,
        Settings {
            per_ip: 1,
            ..Settings::default()
        },
    )
    .await;
    for (token, origin, expected) in [
        (Some(a.token.as_str()), "https://evil.test", 403),
        (None, "http://localhost:5173", 401),
    ] {
        let Err(tokio_tungstenite::tungstenite::Error::Http(response)) =
            raw_connection(&server, token, origin, "").await
        else {
            panic!("unauthorized upgrade")
        };
        assert_eq!(response.status().as_u16(), expected);
    }
    let mut first = server.connect(&a, None).await;
    let Err(tokio_tungstenite::tungstenite::Error::Http(response)) =
        raw_connection(&server, Some(&a.token), "http://localhost:5173", "").await
    else {
        panic!("missing IP limit")
    };
    assert_eq!(response.status().as_u16(), 429);
    // Unknown future oneof fields are tolerated; malformed frames are bounded.
    first.seq += 1;
    first
        .socket
        .send(Message::Binary(
            pb::ClientMessage {
                seq: first.seq,
                body: None,
            }
            .encode_to_vec()
            .into(),
        ))
        .await
        .unwrap();
    assert_ok(first.reply(first.seq).await);
    for _ in 0..3 {
        first
            .socket
            .send(Message::Binary(vec![0xff].into()))
            .await
            .unwrap();
    }
    first
        .event(|b| matches!(b, Body::Error(e) if e.code=="INVALID_MESSAGE"))
        .await;
    drop(first);
    server.stop().await;
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn review_app_ticket_real_upgrade_is_single_use(pool: PgPool) {
    let a = player(&pool, 1, true).await;
    sqlx::query("UPDATE sessions SET kind='app' WHERE user_id=$1")
        .bind(a.id)
        .execute(&pool)
        .await
        .unwrap();
    let server = Server::new(pool, Settings::default()).await;
    let response = reqwest::Client::new()
        .post(format!("http://{}/api/auth/ws-ticket", server.addr))
        .header("origin", "https://localhost")
        .header("x-requested-with", "app")
        .header("x-native-version", "1.0.0")
        .bearer_auth(&a.token)
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
    let response: serde_json::Value = response.json().await.unwrap();
    let query = format!("?ticket={}", response["ticket"].as_str().unwrap());
    let mut first = raw_connection(&server, None, "https://localhost", &query)
        .await
        .unwrap();
    let Client::Hello(mut message) = hello() else {
        panic!()
    };
    message.client_kind = "android".into();
    message.native_version = "1.0.0".into();
    assert!(matches!(
        first.call(Client::Hello(message)).await,
        Body::Welcome(_)
    ));
    let Err(tokio_tungstenite::tungstenite::Error::Http(response)) =
        raw_connection(&server, None, "https://localhost", &query).await
    else {
        panic!("ticket replay")
    };
    assert_eq!(response.status().as_u16(), 401);
    drop(first);
    server.stop().await;
}
impl SocialOk for Body {
    fn map_ok(self) -> Body {
        assert!(matches!(self, Body::SocialSnapshot(_)));
        Body::Ok(pb::NetOk {})
    }
}

#[path = "online/duels.rs"]
mod duels;

#[path = "online/trade.rs"]
mod trade;
