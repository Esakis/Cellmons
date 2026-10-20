use super::*;
use uuid::Uuid;

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL"]
async fn only_three_starting_classes_register_with_a_legal_complete_deck(pool: PgPool) {
    for avatar in engine_content::AVATARS
        .iter()
        .filter(|a| a.playable && [1, 2, 4].contains(&a.id))
    {
        let f = Fixture::new(pool.clone());
        let starter = engine_content::starter(avatar.id).unwrap();
        let school = server::progression::school_name(starter.school);
        let reply = f.call("POST", "/api/auth/register", json!({
            "login":format!("Class{}",avatar.id),"email":format!("class{}@example.test",avatar.id),
            "password":PASSWORD,"school":school,"template_id":avatar.id,
            "terms_version":server::auth::LEGAL_VERSION,"age_confirmed":true
        }), &[]).await;
        assert_eq!(
            reply.status,
            StatusCode::CREATED,
            "{}: {}",
            avatar.key,
            reply.body
        );
        let (id, user, deck): (Uuid, Uuid, Vec<i16>) = sqlx::query_as("SELECT a.id,a.user_id,d.cards FROM avatars a JOIN decks d ON d.avatar_id=a.id WHERE a.template_id=$1")
            .bind(i32::from(avatar.id)).fetch_one(&pool).await.unwrap();
        assert_eq!(
            deck,
            starter.deck.iter().map(|c| *c as i16).collect::<Vec<_>>()
        );
        let snapshot = server::game::load(&mut pool.acquire().await.unwrap(), user)
            .await
            .unwrap();
        assert_eq!(snapshot.avatars[0].id, id);
        assert_eq!(snapshot.templates, [i32::from(avatar.id)]);
        assert!(starter.deck.iter().all(|card| {
            avatar
                .schools
                .meets(&engine_content::content().get(*card).unwrap().requires)
        }));
    }
    for id in [3, 5, 11, 13, 32] {
        let f = Fixture::new(pool.clone());
        let starter = engine_content::starter(id).unwrap();
        let reply = f.call("POST", "/api/auth/register", json!({
            "login":format!("Locked{id}"),"email":format!("locked{id}@example.test"),
            "password":PASSWORD,"school":server::progression::school_name(starter.school),"template_id":id,
            "terms_version":server::auth::LEGAL_VERSION,"age_confirmed":true
        }), &[]).await;
        assert_eq!(reply.body["error"], "AVATAR_LOCKED");
    }
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL"]
async fn retired_templates_cannot_register_create_activate_or_reincarnate(pool: PgPool) {
    let f = Fixture::new(pool.clone());
    let rejected = f.call("POST", "/api/auth/register", json!({
        "login":"LockedClass","email":"locked@example.test","password":PASSWORD,
        "school":"tech","template_id":6,"terms_version":server::auth::LEGAL_VERSION,"age_confirmed":true
    }), &[]).await;
    assert_eq!(rejected.body["error"], "AVATAR_LOCKED");
    f.register().await;
    let user: Uuid = sqlx::query_scalar("SELECT id FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    let old = Uuid::now_v7();
    sqlx::query("INSERT INTO unlocked_templates(user_id,template_id,source) VALUES($1,6,'admin')")
        .bind(user)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO avatars(id,user_id,slot,template_id,level,xp) VALUES($1,$2,1,6,20,65000)",
    )
    .bind(old)
    .bind(user)
    .execute(&pool)
    .await
    .unwrap();
    let before = sqlx::query_scalar::<_, i64>("SELECT count(*) FROM avatars")
        .fetch_one(&pool)
        .await
        .unwrap();
    for body in [
        json!({"action":"create","id":Uuid::now_v7(),"template_id":6}),
        json!({"action":"activate","id":old}),
        json!({"action":"reincarnate","id":old,"revision":0,"choice":{"kind":"path","id":1012}}),
    ] {
        let result = server::game::mutate(&pool, user, serde_json::from_value(body).unwrap()).await;
        assert!(matches!(result, Err(e) if e.0 == "AVATAR_LOCKED"));
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM avatars")
            .fetch_one(&pool)
            .await
            .unwrap(),
        before
    );
    let snapshot = server::game::load(&mut pool.acquire().await.unwrap(), user)
        .await
        .unwrap();
    assert!(!snapshot.templates.contains(&6));
    assert!(snapshot.avatars.iter().any(|a| a.id == old)); // keep legacy data visible
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL"]
async fn vampire_pack_is_granted_once_even_after_delete_and_recreate(pool: PgPool) {
    let f = Fixture::new(pool.clone());
    f.register().await;
    let user: Uuid = sqlx::query_scalar("SELECT id FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO unlocked_templates(user_id,template_id,source) VALUES($1,32,'arena')")
        .bind(user)
        .execute(&pool)
        .await
        .unwrap();
    let id = Uuid::now_v7();
    let create =
        |id| serde_json::from_value(json!({"action":"create","id":id,"template_id":32})).unwrap();
    let result = server::game::mutate(&pool, user, create(id)).await.unwrap();
    assert_eq!(
        result
            .avatars
            .iter()
            .find(|a| a.id == id)
            .unwrap()
            .deck
            .len(),
        20
    );
    let cards: Vec<(i16, i32)> = result
        .collection
        .iter()
        .map(|s| (s.card_id, s.qty))
        .collect();
    server::game::mutate(&pool, user, create(id)).await.unwrap();
    server::game::mutate(
        &pool,
        user,
        serde_json::from_value(json!({"action":"delete","id":id,"revision":0})).unwrap(),
    )
    .await
    .unwrap();
    let result = server::game::mutate(&pool, user, create(Uuid::now_v7()))
        .await
        .unwrap();
    assert_eq!(
        result
            .collection
            .iter()
            .map(|s| (s.card_id, s.qty))
            .collect::<Vec<_>>(),
        cards
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM starter_claims WHERE user_id=$1 AND template_id=32"
        )
        .bind(user)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
}
