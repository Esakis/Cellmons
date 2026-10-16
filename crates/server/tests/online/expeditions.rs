use super::*;

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn expedition_chat_rechecks_participation_and_moves_finished_players_to_portal(pool: PgPool) {
    let a = player(&pool, 0, true).await;
    let b = player(&pool, 1, true).await;
    let outsider = player(&pool, 2, true).await;
    let mut ids = vec![];
    for user in [a.id, b.id] {
        let avatar: Uuid =
            sqlx::query_scalar("SELECT id FROM avatars WHERE user_id=$1 AND is_active")
                .bind(user)
                .fetch_one(&pool)
                .await
                .unwrap();
        let id = Uuid::now_v7();
        server::expeditions::start(&pool, user, id, avatar, "walk")
            .await
            .unwrap();
        ids.push(id);
    }
    let server = Server::new(pool.clone(), Settings::default()).await;
    let mut first = server.connect(&a, None).await;
    let mut second = server.connect(&b, None).await;
    let mut other = server.connect(&outsider, None).await;
    first.join("expedition").await;
    second.join("expedition").await;
    assert_error(
        other
            .call(Client::RoomJoin(pb::RoomJoin {
                room: "expedition".into(),
            }))
            .await,
        "FORBIDDEN",
    );
    assert_ok(first.call(chat("private expedition message")).await);
    second.event(|body|matches!(body,Body::ChatMessage(message)if message.text=="private expedition message")).await;
    let response=reqwest::Client::new().post(format!("http://{}/api/expeditions",server.addr)).header("origin","http://localhost:5173").header("x-requested-with","app").header("cookie",format!("game_session={}",b.token)).json(&serde_json::json!({"action":"retreat","id":ids[1],"request":Uuid::now_v7(),"revision":0})).send().await.unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    second
        .event(|body| matches!(body,Body::RoomSnapshot(room)if room.room=="portal"))
        .await;
    assert_error(
        second
            .call(Client::RoomJoin(pb::RoomJoin {
                room: "expedition".into(),
            }))
            .await,
        "FORBIDDEN",
    );
    assert_ok(first.call(chat("private after expedition")).await);
    first.event(|body|matches!(body,Body::ChatMessage(message)if message.text=="private after expedition")).await;
    assert!(matches!(
        second.call(Client::Ping(pb::Ping::default())).await,
        Body::Pong(_)
    ));
    assert!(!second.events.iter().any(
        |body| matches!(body,Body::ChatMessage(message)if message.text=="private after expedition")
    ));
    drop(first);
    drop(second);
    drop(other);
    server.stop().await;
}
