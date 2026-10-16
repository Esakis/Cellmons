use super::*;
use uuid::Uuid;
async fn post(f: &Fixture, token: &str, body: Value) -> Reply {
    f.call("POST", "/api/game", body, &[("cookie", token)])
        .await
}
async fn advance(pool: &PgPool, id: Uuid) {
    sqlx::query("UPDATE avatars SET level=20,xp=65000,dev_points_unspent=7,dp_phys_atk=2,dp_hp=1 WHERE id=$1").bind(id).execute(pool).await.unwrap();
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
async fn f10_invalid_profiles_and_foreign_sacrifices_leave_every_character_unchanged(pool: PgPool) {
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
    fixture_tier(&pool, id, 1, 3).await;
    let shade = Uuid::now_v7();
    mutation(
        &pool,
        user,
        json!({"action":"shade","id":id,"revision":0,"new_id":shade,"school":"arcane"}),
    )
    .await
    .unwrap();
    advance(&pool, shade).await;
    let registered=f.call("POST","/api/auth/register",json!({"login":"ForeignPlayer","email":"foreign@example.test","password":PASSWORD,"school":"combat","terms_version":"2026-10-16","age_confirmed":true}),&[]).await;
    assert_eq!(registered.status, StatusCode::CREATED);
    let foreign = sqlx::query_scalar::<_, Uuid>("SELECT id FROM avatars WHERE user_id<>$1")
        .bind(user)
        .fetch_one(&pool)
        .await
        .unwrap();
    fixture_tier(&pool, foreign, 1, 1).await;
    let path = engine_content::PATHS
        .iter()
        .find(|p| p.base_avatar == 29)
        .unwrap();
    assert!(
        matches!(mutation(&pool,user,json!({"action":"reincarnate","id":shade,"revision":0,"choice":{"kind":"path","id":path.avatar.id},"sacrifice":foreign})).await,Err(e) if e.0=="NOT_FOUND")
    );
    assert!(
        sqlx::query_scalar::<_, bool>("SELECT deleted_at IS NULL FROM avatars WHERE id=$1")
            .bind(foreign)
            .fetch_one(&pool)
            .await
            .unwrap()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i16>("SELECT tier FROM avatars WHERE id=$1")
            .bind(shade)
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
async fn f10_shade_sacrifice_uses_the_same_full_slot_and_never_unlocks_free_copies(pool: PgPool) {
    let f = Fixture::new(pool.clone());
    f.register().await;
    let (user, source) = sqlx::query_as::<_, (Uuid, Uuid)>("SELECT user_id,id FROM avatars")
        .fetch_one(&pool)
        .await
        .unwrap();
    for _ in 1..10 {
        mutation(
            &pool,
            user,
            json!({"action":"create","id":Uuid::now_v7(),"template_id":1}),
        )
        .await
        .unwrap();
    }
    fixture_tier(&pool, source, 1, 3).await;
    advance(&pool, source).await;
    let child = Uuid::now_v7();
    // A conflicting destination must roll back deletion of the source and its active flag.
    let existing = sqlx::query_scalar::<_, Uuid>("SELECT id FROM avatars WHERE id<>$1 LIMIT 1")
        .bind(source)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(
        matches!(mutation(&pool,user,json!({"action":"shade","id":source,"revision":0,"new_id":existing,"school":"arcane"})).await,Err(e) if e.0=="CONFLICT")
    );
    let result = mutation(
        &pool,
        user,
        json!({"action":"shade","id":source,"revision":0,"new_id":child,"school":"arcane"}),
    )
    .await
    .unwrap();
    assert_eq!(result.avatars.len(), 10);
    assert!(!result.avatars.iter().any(|a| a.id == source));
    let shade = result.avatars.iter().find(|a| a.id == child).unwrap();
    assert_eq!(shade.slot, 0);
    assert!(shade.active && shade.is_shade);
    assert_eq!(shade.template_id, 29);
    assert_eq!(shade.shade_school.as_deref(), Some("arcane"));
    assert_eq!(shade.stats, [2, 2, 2, 2, 35, 40]);
    assert_eq!(shade.schools, [0, 0, 0, 1, 0, 0]);
    assert!(shade.deck.is_empty());
    assert_eq!(shade.xp, 0);
    assert_eq!(shade.unspent, 0);
    assert!(!result.templates.contains(&29));
    assert!(
        matches!(mutation(&pool,user,json!({"action":"create","id":Uuid::now_v7(),"template_id":29})).await,Err(e) if e.0=="NOT_FOUND")
    );
    assert!(
        matches!(mutation(&pool,user,json!({"action":"shade","id":source,"revision":0,"new_id":Uuid::now_v7(),"school":"arcane"})).await,Err(e) if e.0=="NOT_FOUND")
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM avatar_sacrifices")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn f10_each_shade_reincarnation_consumes_exactly_the_required_tier_and_checks_school(
    pool: PgPool,
) {
    let f = Fixture::new(pool.clone());
    f.register().await;
    let (user, source) = sqlx::query_as::<_, (Uuid, Uuid)>("SELECT user_id,id FROM avatars")
        .fetch_one(&pool)
        .await
        .unwrap();
    fixture_tier(&pool, source, 1, 3).await;
    advance(&pool, source).await;
    let shade = Uuid::now_v7();
    mutation(
        &pool,
        user,
        json!({"action":"shade","id":source,"revision":0,"new_id":shade,"school":"arcane"}),
    )
    .await
    .unwrap();
    let path = engine_content::PATHS
        .iter()
        .find(|p| p.base_avatar == 29 && p.variant == 1)
        .unwrap();
    for tier in 0..3 {
        advance(&pool, shade).await;
        let choice = match tier {
            0 => json!({"kind":"path","id":path.avatar.id}),
            1 => json!({"kind":"specialization","name":"essence"}),
            _ => json!({"kind":"gift","card_id":path.gifts[0]}),
        };
        let request = json!({"action":"reincarnate","id":shade,"revision":tier,"choice":choice});
        assert!(
            matches!(mutation(&pool,user,request.clone()).await,Err(e) if e.0=="DONOR_REQUIRED")
        );
        let donor = Uuid::now_v7();
        mutation(
            &pool,
            user,
            json!({"action":"create","id":donor,"template_id":1}),
        )
        .await
        .unwrap();
        fixture_tier(&pool, donor, 1, if tier == 0 { 2 } else { tier }).await;
        let mut request = request;
        request["sacrifice"] = json!(donor);
        assert!(
            matches!(mutation(&pool,user,request.clone()).await,Err(e) if e.0=="INVALID_DONOR")
        );
        fixture_tier(&pool, donor, 1, tier + 1).await;
        // The donor's current path, not just its original template, defines proficiency.
        sqlx::query("INSERT INTO unlocked_templates(user_id,template_id,source) VALUES ($1,4,'admin') ON CONFLICT DO NOTHING").bind(user).execute(&pool).await.unwrap();
        let knows = Uuid::now_v7();
        mutation(
            &pool,
            user,
            json!({"action":"create","id":knows,"template_id":4}),
        )
        .await
        .unwrap();
        fixture_tier(&pool, knows, 4, tier + 1).await;
        request["sacrifice"] = json!(knows);
        assert!(
            matches!(mutation(&pool,user,request.clone()).await,Err(e) if e.0=="INVALID_DONOR")
        );
        request["sacrifice"] = json!(shade);
        assert!(
            matches!(mutation(&pool,user,request.clone()).await,Err(e) if e.0=="INVALID_DONOR")
        );
        request["sacrifice"] = json!(donor);
        if tier == 2 {
            // Consuming the active donor atomically activates the surviving shade.
            sqlx::query("UPDATE avatars SET is_active=false WHERE user_id=$1")
                .bind(user)
                .execute(&pool)
                .await
                .unwrap();
            sqlx::query("UPDATE avatars SET is_active=true WHERE id=$1")
                .bind(donor)
                .execute(&pool)
                .await
                .unwrap();
        }
        let results = futures_util::future::join_all(
            (0..100).map(|_| mutation(&pool, user, request.clone())),
        )
        .await;
        assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
        assert!(
            results
                .iter()
                .all(|r| r.is_ok() || matches!(r,Err(e) if e.0=="CONFLICT"))
        );
        let alive =
            sqlx::query_scalar::<_, bool>("SELECT deleted_at IS NULL FROM avatars WHERE id=$1")
                .bind(donor)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(!alive);
        let result = results.into_iter().find_map(Result::ok).unwrap();
        let current = result.avatars.iter().find(|a| a.id == shade).unwrap();
        assert_eq!(current.tier, tier + 1);
        assert_eq!(current.level, 1);
        assert_eq!(current.unspent, (tier + 1) * 2);
        assert!(current.active);
    }
    fixture_tier(&pool, shade, 29, 3).await;
    advance(&pool, shade).await;
    assert!(
        matches!(mutation(&pool,user,json!({"action":"shade","id":shade,"revision":3,"new_id":Uuid::now_v7(),"school":"combat"})).await,Err(e) if e.0=="INVALID_DONOR")
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM avatar_sacrifices")
            .fetch_one(&pool)
            .await
            .unwrap(),
        4
    );
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn f10_reincarnation_resets_progress_keeps_deck_and_permanently_selects_path_spec_gift(
    pool: PgPool,
) {
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
    assert_eq!(avatar["unspent"], 2);
    assert_eq!(avatar["spent"], json!([0, 0, 0, 0, 0, 0]));
    assert_eq!(avatar["path_id"], path.avatar.id);
    assert_eq!(avatar["deck"], before["deck"]);
    assert_eq!(first.body["collection"], initial.body["collection"]);
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
    assert_eq!(second.body["avatars"][0]["unspent"], 4);
    advance(&pool, id).await;
    let invalid = post(
        &f,
        &token,
        json!({"action":"reincarnate","id":id,"revision":2,"choice":{"kind":"gift","card_id":1}}),
    )
    .await;
    assert_eq!(invalid.body["error"], "INVALID_CHOICE");
    let third=post(&f,&token,json!({"action":"reincarnate","id":id,"revision":2,"choice":{"kind":"gift","card_id":path.gifts[0]}})).await;
    assert_eq!(third.status, StatusCode::OK, "{}", third.body);
    assert_eq!(third.body["avatars"][0]["tier"], 3);
    assert_eq!(third.body["avatars"][0]["gift_id"], path.gifts[0]);
    assert_eq!(third.body["avatars"][0]["unspent"], 6);
    advance(&pool, id).await;
    assert_eq!(post(&f,&token,json!({"action":"reincarnate","id":id,"revision":3,"choice":{"kind":"gift","card_id":path.gifts[1]}})).await.body["error"],"FINAL_TIER");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM reincarnation_history")
            .fetch_one(&pool)
            .await
            .unwrap(),
        3
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
