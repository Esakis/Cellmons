use super::*;

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL"]
async fn new_account_can_chat_on_square_but_cannot_enter_locked_rooms(pool: PgPool) {
    let user = player(&pool, 901, false).await;
    sqlx::query("UPDATE users SET account_xp=0,tutorial_step=0 WHERE id=$1")
        .bind(user.id)
        .execute(&pool)
        .await
        .unwrap();
    let server = Server::new(pool.clone(), Settings::default()).await;
    let mut socket = server.connect(&user, None).await;
    socket.join("square").await;
    let reply = socket
        .call(Client::ChatSend(pb::ChatSend {
            channel: "room".into(),
            text: "Hello from the map".into(),
            ..Default::default()
        }))
        .await;
    assert!(!matches!(reply, Body::Error(_)), "{reply:?}");
    let message = socket
        .event(|body| matches!(body, Body::ChatMessage(m) if m.text == "Hello from the map"))
        .await;
    assert!(matches!(message, Body::ChatMessage(m) if m.room == "square"));
    for room in ["arena", "castle", "portal", "bazaar", "shop", "tower"] {
        let reply = socket
            .call(Client::RoomJoin(pb::RoomJoin { room: room.into() }))
            .await;
        assert!(
            matches!(reply, Body::Error(e) if e.code == "LEVEL_REQUIRED"),
            "{room}"
        );
    }
    sqlx::query("UPDATE users SET account_xp=250 WHERE id=$1")
        .bind(user.id)
        .execute(&pool)
        .await
        .unwrap();
    socket.join("arena").await;
    server.stop().await;
}
