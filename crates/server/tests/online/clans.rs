use super::*;
use server::clans::{self, Mutation as M};

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn clan_http_kick_moves_the_live_member_out_and_hides_motd_and_new_chat(pool: PgPool) {
    let mut players = Vec::new();
    for i in 0..6 {
        players.push(player(&pool, i, true).await);
    }
    let clan = Uuid::now_v7();
    clans::mutate(
        &pool,
        players[0].id,
        M::Create {
            id: clan,
            name: "Live Guild".into(),
        },
    )
    .await
    .unwrap();
    for i in 1..5 {
        clans::mutate(&pool, players[i].id, M::Apply { clan })
            .await
            .unwrap();
        clans::mutate(
            &pool,
            players[0].id,
            M::Accept {
                clan,
                user: players[i].id,
                revision: i as i32 - 1,
            },
        )
        .await
        .unwrap();
    }
    clans::mutate(
        &pool,
        players[0].id,
        M::Edit {
            clan,
            revision: 4,
            description: "Public".into(),
            motd: "Private clan notice".into(),
            www: String::new(),
            email: String::new(),
        },
    )
    .await
    .unwrap();
    let server = Server::new(pool.clone(), Settings::default()).await;
    let mut master = server.connect(&players[0], None).await;
    let mut member = server.connect(&players[1], None).await;
    let mut outsider = server.connect(&players[5], None).await;
    let room = format!("clan:{clan}");
    assert_eq!(master.join(&room).await.motd, "Private clan notice");
    assert_eq!(member.join(&room).await.motd, "Private clan notice");
    assert_error(
        outsider
            .call(Client::RoomJoin(pb::RoomJoin { room: room.clone() }))
            .await,
        "FORBIDDEN",
    );
    let response = reqwest::Client::new()
        .post(format!("http://{}/api/clans", server.addr))
        .header("origin", "http://localhost:5173")
        .header("x-requested-with", "app")
        .header("cookie", format!("game_session={}", players[0].token))
        .json(&serde_json::json!({"action":"kick","clan":clan,"user":players[1].id,"revision":5}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let moved = member
        .event(|body| matches!(body,Body::RoomSnapshot(snapshot)if snapshot.room=="arena"))
        .await;
    let Body::RoomSnapshot(moved) = moved else {
        unreachable!()
    };
    assert!(moved.motd.is_empty());
    assert_error(
        member.call(Client::RoomJoin(pb::RoomJoin { room })).await,
        "FORBIDDEN",
    );
    assert_ok(master.call(chat("private after removal")).await);
    master.event(|body|matches!(body,Body::ChatMessage(message)if message.text=="private after removal")).await;
    assert!(matches!(
        member.call(Client::Ping(pb::Ping::default())).await,
        Body::Pong(_)
    ));
    assert!(!member.events.iter().any(
        |body| matches!(body,Body::ChatMessage(message)if message.text=="private after removal")
    ));
    drop(master);
    drop(member);
    drop(outsider);
    server.stop().await;
}
