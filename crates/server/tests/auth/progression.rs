use super::*;
use uuid::Uuid;
async fn post(f: &Fixture, token: &str, body: Value) -> Reply {
    f.call("POST", "/api/game", body, &[("cookie", token)])
        .await
}
async fn advance(pool: &PgPool, id: Uuid) {
    sqlx::query("UPDATE avatars SET level=20,xp=65000,dev_points_unspent=7,dp_phys_atk=2,dp_hp=1 WHERE id=$1").bind(id).execute(pool).await.unwrap();
    sqlx::query(
        "UPDATE users SET account_xp=42000 WHERE id=(SELECT user_id FROM avatars WHERE id=$1)",
    )
    .bind(id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO class_trials(avatar_id,tier,wins) SELECT id,tier,3 FROM avatars WHERE id=$1 AND tier<2 ON CONFLICT(avatar_id,tier) DO UPDATE SET wins=3").bind(id).execute(pool).await.unwrap();
}

async fn fixture_tier(pool: &PgPool, id: Uuid, base: u16, tier: i16) {
    let path = engine_content::PATHS
        .iter()
        .find(|p| p.base_avatar == base)
        .unwrap();
    sqlx::query("UPDATE avatars SET tier=$2,path_id=$3,specialization=$4,gift_id=$5 WHERE id=$1")
        .bind(id)
        .bind(tier)
        .bind(if tier > 0 {
            Some(i32::from(path.avatar.id))
        } else {
            None
        })
        .bind(if tier >= 2 {
            Some(server::progression::specialization_name(
                path.specializations[0],
            ))
        } else {
            None
        })
        .bind(if tier == 3 {
            Some(i32::from(path.gifts[0]))
        } else {
            None
        })
        .execute(pool)
        .await
        .unwrap();
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn f10_invalid_profiles_leave_every_character_unchanged(pool: PgPool) {
    let f = Fixture::new(pool.clone());
    f.register().await;
    let (user, id) = sqlx::query_as::<_, (Uuid, Uuid)>("SELECT user_id,id FROM avatars")
        .fetch_one(&pool)
        .await
        .unwrap();
    fixture_tier(&pool, id, 1, 2).await;
    advance(&pool, id).await;
    let path = engine_content::PATHS
        .iter()
        .find(|p| p.base_avatar == 1)
        .unwrap();
    sqlx::query("UPDATE avatars SET specialization=NULL WHERE id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        matches!(mutation(&pool,user,json!({"action":"reincarnate","id":id,"revision":0,"choice":{"kind":"gift","card_id":path.gifts[0]}})).await,Err(e) if e.0=="CONTENT_MISMATCH")
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM reincarnation_history")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}

async fn mutation(
    pool: &PgPool,
    user: Uuid,
    body: Value,
) -> server::online::Result<server::game::Snapshot> {
    server::game::mutate(pool, user, serde_json::from_value(body).unwrap()).await
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn retired_shades_cannot_consume_a_ready_gold_character(pool: PgPool) {
    let f = Fixture::new(pool.clone());
    f.register().await;
    let (user, id): (Uuid, Uuid) = sqlx::query_as("SELECT user_id,id FROM avatars")
        .fetch_one(&pool)
        .await
        .unwrap();
    fixture_tier(&pool, id, 1, 3).await;
    advance(&pool, id).await;
    for school in ["combat", "divine", "element", "arcane", "necro", "tech"] {
        let result = mutation(
            &pool,
            user,
            json!({"action":"shade","id":id,"revision":0,"new_id":Uuid::now_v7(),"school":school}),
        )
        .await;
        assert!(matches!(result, Err(e) if e.0 == "AVATAR_LOCKED"));
    }
    let (active, tier, xp, deleted): (bool, i16, i32, bool) =
        sqlx::query_as("SELECT is_active,tier,xp,deleted_at IS NOT NULL FROM avatars WHERE id=$1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!((active, tier, xp, deleted), (true, 3, 65000, false));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM avatar_sacrifices")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn reincarnation_archives_decks_and_grants_three_then_six_permanent_points(pool: PgPool) {
    let f = Fixture::new(pool.clone());
    f.register().await;
    let token = cookie(&f.login(false).await);
    let initial = f
        .call("GET", "/api/game", Value::Null, &[("cookie", &token)])
        .await;
    let before = &initial.body["avatars"][0];
    let id = Uuid::parse_str(before["id"].as_str().unwrap()).unwrap();
    let path = engine_content::PATHS
        .iter()
        .find(|p| p.base_avatar == 1 && p.variant == 0)
        .unwrap();
    let request = json!({"action":"reincarnate","id":id,"revision":0,"choice":{"kind":"path","id":path.avatar.id}});
    assert_eq!(
        post(&f, &token, request.clone()).await.body["error"],
        "NOT_READY"
    );
    advance(&pool, id).await;
    let first = post(&f, &token, request.clone()).await;
    assert_eq!(first.status, StatusCode::OK, "{}", first.body);
    let avatar = &first.body["avatars"][0];
    assert_eq!(avatar["tier"], 1);
    assert_eq!(avatar["level"], 1);
    assert_eq!(avatar["xp"], 0);
    assert_eq!(avatar["unspent"], 3);
    assert_eq!(avatar["advancement"]["permanent_points"], 3);
    assert_eq!(avatar["spent"], json!([0, 0, 0, 0, 0, 0]));
    assert_eq!(avatar["path_id"], path.avatar.id);
    assert_eq!(avatar["archived_decks"][0], before["deck"]);
    assert_eq!(avatar["deck"].as_array().unwrap().len(), 20);
    assert_ne!(first.body["collection"], initial.body["collection"]);
    assert_eq!(avatar["stats"][0], path.avatar.base.phys_atk);
    assert_eq!(post(&f, &token, request).await.body["error"], "CONFLICT");
    advance(&pool, id).await;
    let wrong=post(&f,&token,json!({"action":"reincarnate","id":id,"revision":1,"choice":{"kind":"path","id":path.avatar.id}})).await;
    assert_eq!(wrong.body["error"], "INVALID_CHOICE");
    let spec = server::progression::specialization_name(path.specializations[0]);
    let second=post(&f,&token,json!({"action":"reincarnate","id":id,"revision":1,"choice":{"kind":"specialization","name":spec}})).await;
    assert_eq!(second.status, StatusCode::OK, "{}", second.body);
    assert_eq!(second.body["avatars"][0]["tier"], 2);
    assert_eq!(second.body["avatars"][0]["specialization"], spec);
    assert_eq!(second.body["avatars"][0]["unspent"], 6);
    assert_eq!(
        second.body["avatars"][0]["advancement"]["permanent_points"],
        6
    );
    assert_eq!(second.body["collection"], first.body["collection"]);
    advance(&pool, id).await;
    let invalid = post(
        &f,
        &token,
        json!({"action":"reincarnate","id":id,"revision":2,"choice":{"kind":"gift","card_id":1}}),
    )
    .await;
    assert_eq!(invalid.body["error"], "FINAL_TIER");
    let third=post(&f,&token,json!({"action":"reincarnate","id":id,"revision":2,"choice":{"kind":"gift","card_id":path.gifts[0]}})).await;
    assert_eq!(third.body["error"], "FINAL_TIER");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM reincarnation_history")
            .fetch_one(&pool)
            .await
            .unwrap(),
        2
    );
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn f10_reincarnation_concurrent_requests_cannot_advance_twice_or_ignore_activity(
    pool: PgPool,
) {
    let f = Fixture::new(pool.clone());
    f.register().await;
    let (user, id) = sqlx::query_as::<_, (Uuid, Uuid)>("SELECT user_id,id FROM avatars")
        .fetch_one(&pool)
        .await
        .unwrap();
    advance(&pool, id).await;
    let path = engine_content::PATHS
        .iter()
        .find(|p| p.base_avatar == 1)
        .unwrap()
        .avatar
        .id;
    let results=futures_util::future::join_all((0..100).map(|_|{let pool=pool.clone();async move {server::game::mutate(&pool,user,serde_json::from_value(json!({"action":"reincarnate","id":id,"revision":0,"choice":{"kind":"path","id":path}})).unwrap()).await}})).await;
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert!(
        results
            .iter()
            .all(|r| r.is_ok() || matches!(r,Err(e) if e.0=="CONFLICT"))
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM reincarnation_history")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    sqlx::query("INSERT INTO activity_locks(user_id,kind,owner_id) VALUES ($1,'trade',$2)")
        .bind(user)
        .bind(Uuid::now_v7())
        .execute(&pool)
        .await
        .unwrap();
    advance(&pool, id).await;
    let denied=server::game::mutate(&pool,user,serde_json::from_value(json!({"action":"reincarnate","id":id,"revision":1,"choice":{"kind":"specialization","name":"combat"}})).unwrap()).await;
    assert!(matches!(denied,Err(e) if e.0=="BUSY"));
    let denied = server::game::mutate(
        &pool,
        user,
        serde_json::from_value(json!({"action":"create","id":Uuid::now_v7(),"template_id":path}))
            .unwrap(),
    )
    .await;
    assert!(matches!(denied,Err(e) if e.0=="BUSY"));
}
