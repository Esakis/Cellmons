use super::*;

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn tournament_readiness_requires_live_castle_presence_and_clears_on_leave(pool: PgPool) {
    let player = player(&pool, 0, true).await;
    let avatar: Uuid = sqlx::query_scalar("SELECT id FROM avatars WHERE user_id=$1 AND is_active")
        .bind(player.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let id = Uuid::now_v7();
    server::tournaments::create(
        &pool,
        player.id,
        id,
        "Ready test",
        server::tournaments::Terms {
            teams: 2,
            team_size: 1,
            fee: 0,
            max_games: 3,
            min_level: 1,
            max_level: 20,
            tiers: vec![0],
            only_clan: None,
            allowed_clans: vec![],
        },
    )
    .await
    .unwrap();
    server::tournaments::join(&pool, player.id, id, Uuid::now_v7(), avatar)
        .await
        .unwrap();
    let server = Server::new(pool.clone(), Settings::default()).await;
    let mut socket = server.connect(&player, None).await;
    let ready = || Client::TournamentReady(pb::TournamentReady { ready: true });
    assert_error(socket.call(ready()).await, "FORBIDDEN");
    socket.join("castle").await;
    assert_ok(socket.call(ready()).await);
    let since: Option<i64> = sqlx::query_scalar(
        "SELECT ready_since_ms FROM tournament_entries WHERE tournament_id=$1 AND user_id=$2",
    )
    .bind(id)
    .bind(player.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(since.is_some());
    socket.join("arena").await;
    let since: Option<i64> = sqlx::query_scalar(
        "SELECT ready_since_ms FROM tournament_entries WHERE tournament_id=$1 AND user_id=$2",
    )
    .bind(id)
    .bind(player.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(since, None);
    socket.join("castle").await;
    assert_ok(socket.call(ready()).await);
    drop(socket);
    tokio::time::timeout(Duration::from_secs(3),async {loop {
        let since:Option<i64>=sqlx::query_scalar("SELECT ready_since_ms FROM tournament_entries WHERE tournament_id=$1 AND user_id=$2").bind(id).bind(player.id).fetch_one(&pool).await.unwrap();
        if since.is_none(){break;}tokio::time::sleep(Duration::from_millis(10)).await;
    }}).await.unwrap();
    server.stop().await;
}
