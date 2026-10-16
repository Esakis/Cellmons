use super::*;
use server::store::{self, Request};
use uuid::Uuid;

async fn discard(pool: &PgPool, user: Uuid, key: Uuid, qty: u32) -> server::store::Receipt {
    store::execute(
        pool,
        user,
        serde_json::from_value(
            json!({"key":key,"operation":{"action":"discard","cards":[{"card_id":1,"qty":qty}]}}),
        )
        .unwrap(),
    )
    .await
    .unwrap()
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn f9_junkyard_thresholds_unique_characters_retry_and_deck_pruning(pool: PgPool) {
    let f = Fixture::new(pool.clone());
    f.register().await;
    let user = sqlx::query_scalar::<_, Uuid>("SELECT id FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    let avatar = sqlx::query_scalar::<_, Uuid>("SELECT id FROM avatars")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO collection(user_id,card_id,qty) VALUES ($1,1,800) ON CONFLICT(user_id,card_id) DO UPDATE SET qty=800").bind(user).execute(&pool).await.unwrap();
    sqlx::query("UPDATE decks SET cards=ARRAY[1,1,1,1]::smallint[] WHERE avatar_id=$1")
        .bind(avatar)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO deck_presets(id,user_id,name,cards) VALUES ($1,$2,'Kept',ARRAY[1,1,1,1]::smallint[])").bind(Uuid::now_v7()).bind(user).execute(&pool).await.unwrap();
    let key = Uuid::now_v7();
    let results =
        futures_util::future::join_all((0..100).map(|_| discard(&pool, user, key, 599))).await;
    assert!(
        results
            .iter()
            .all(|r| r.points == "2995" && r.unlocked == results[0].unlocked)
    );
    assert_eq!(results[0].unlocked.len(), 2);
    let third = discard(&pool, user, Uuid::now_v7(), 1).await;
    assert_eq!(third.unlocked.len(), 1);
    assert!(!results[0].unlocked.contains(&third.unlocked[0]));
    assert_eq!(third.ducats, "0");
    let cash = discard(&pool, user, Uuid::now_v7(), 200).await;
    assert!(cash.unlocked.is_empty());
    assert_eq!(cash.ducats, "40");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT balance FROM wallets WHERE user_id=$1")
            .bind(user)
            .fetch_one(&pool)
            .await
            .unwrap(),
        80
    );
    let progress = sqlx::query_as::<_, (i64, i64)>(
        "SELECT points,rewards_granted FROM junkyard WHERE user_id=$1",
    )
    .bind(user)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(progress, (4000, 4));
    let deck =
        sqlx::query_as::<_, (Vec<i16>, i32)>("SELECT cards,revision FROM decks WHERE avatar_id=$1")
            .bind(avatar)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(deck.0.is_empty());
    assert!(deck.1 > 0);
    let preset = sqlx::query_as::<_, (Vec<i16>, i32)>(
        "SELECT cards,revision FROM deck_presets WHERE user_id=$1",
    )
    .bind(user)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(preset.0.is_empty());
    assert!(preset.1 > 0);
    assert!(server::economy::reconcile(&pool).await.unwrap().is_ok());
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn f9_junkyard_rolls_back_partial_discard_and_respects_activity_lock(pool: PgPool) {
    let f = Fixture::new(pool.clone());
    f.register().await;
    let user = sqlx::query_scalar::<_, Uuid>("SELECT id FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    let before =
        sqlx::query_scalar::<_, i32>("SELECT qty FROM collection WHERE user_id=$1 AND card_id=1")
            .bind(user)
            .fetch_one(&pool)
            .await
            .unwrap();
    let key = Uuid::now_v7();
    let rejected=store::execute(&pool,user,serde_json::from_value(json!({"key":key,"operation":{"action":"discard","cards":[{"card_id":1,"qty":1},{"card_id":84,"qty":999}]}})).unwrap()).await;
    assert!(matches!(rejected,Err(e) if e.0=="INSUFFICIENT_CARDS"));
    assert_eq!(
        sqlx::query_scalar::<_, i32>("SELECT qty FROM collection WHERE user_id=$1 AND card_id=1")
            .bind(user)
            .fetch_one(&pool)
            .await
            .unwrap(),
        before
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM junkyard")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM idempotency_keys")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    sqlx::query("INSERT INTO activity_locks(user_id,kind,owner_id) VALUES ($1,'trade',$2)")
        .bind(user)
        .bind(Uuid::now_v7())
        .execute(&pool)
        .await
        .unwrap();
    let inventory = store::inventory(&pool, user).await.unwrap();
    let rejected=store::execute(&pool,user,serde_json::from_value(json!({"key":key,"operation":{"action":"buy","kind":"basic","quote":inventory.offers[0].quote}})).unwrap()).await;
    assert!(matches!(rejected,Err(e) if e.0=="BUSY"));
    assert!(server::economy::reconcile(&pool).await.unwrap().is_ok());
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn f9_purchase_and_open_retry_storm_does_not_duplicate_currency_or_cards(pool: PgPool) {
    let f = Fixture::new(pool.clone());
    f.register().await;
    let user = sqlx::query_scalar::<_, Uuid>("SELECT id FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    let inventory = store::inventory(&pool, user).await.unwrap();
    let quote = inventory
        .offers
        .into_iter()
        .find(|o| o.id == "basic")
        .unwrap()
        .quote;
    let key = Uuid::now_v7();
    let request = json!({"key":key,"operation":{"action":"buy","kind":"basic","quote":quote}});
    let results = futures_util::future::join_all((0..100).map(|_| {
        let pool = pool.clone();
        let request = request.clone();
        async move {
            store::execute(
                &pool,
                user,
                serde_json::from_value::<Request>(request).unwrap(),
            )
            .await
            .unwrap()
        }
    }))
    .await;
    let id = results[0].booster.unwrap();
    assert!(results.iter().all(|r| r.booster == Some(id)));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT balance FROM wallets WHERE user_id=$1")
            .bind(user)
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM boosters")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM idempotency_keys")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    let changed = store::execute(
        &pool,
        user,
        serde_json::from_value(json!({"key":key,"operation":{"action":"open","id":id}})).unwrap(),
    )
    .await;
    assert!(matches!(changed,Err(error) if error.0=="CONFLICT"));
    let request = json!({"key":Uuid::now_v7(),"operation":{"action":"open","id":id}});
    let opened = futures_util::future::join_all((0..100).map(|_| {
        let pool = pool.clone();
        let request = request.clone();
        async move {
            store::execute(
                &pool,
                user,
                serde_json::from_value::<Request>(request).unwrap(),
            )
            .await
            .unwrap()
        }
    }))
    .await;
    assert!(opened.iter().all(|r| r.cards == opened[0].cards));
    assert_eq!(opened[0].cards.len(), 12);
    let again = store::execute(
        &pool,
        user,
        serde_json::from_value(json!({"key":Uuid::now_v7(),"operation":{"action":"open","id":id}}))
            .unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(again.cards, opened[0].cards);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT sum(qty)::bigint FROM collection WHERE user_id=$1")
            .bind(user)
            .fetch_one(&pool)
            .await
            .unwrap(),
        52
    );
    assert!(server::economy::reconcile(&pool).await.unwrap().is_ok());
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn f9_store_quote_ownership_hidden_contents_and_insufficient_funds(pool: PgPool) {
    let f = Fixture::new(pool.clone());
    f.register().await;
    let token = cookie(&f.login(false).await);
    let inventory = f
        .call("GET", "/api/store", Value::Null, &[("cookie", &token)])
        .await;
    assert_eq!(inventory.status, StatusCode::OK);
    let quote = inventory.body["offers"][0]["quote"].as_str().unwrap();
    let wrong=f.call("POST","/api/store",json!({"key":Uuid::now_v7(),"operation":{"action":"buy","kind":"basic","quote":"obsolete"}}),&[("cookie",&token)]).await;
    assert_eq!(wrong.status, StatusCode::CONFLICT);
    assert_eq!(wrong.body["error"], "PRICE_CHANGED");
    let bought = f
        .call(
            "POST",
            "/api/store",
            json!({"key":Uuid::now_v7(),"operation":{"action":"buy","kind":"basic","quote":quote}}),
            &[("cookie", &token)],
        )
        .await;
    assert_eq!(bought.status, StatusCode::OK);
    let id = bought.body["booster"].as_str().unwrap();
    let inventory = f
        .call("GET", "/api/store", Value::Null, &[("cookie", &token)])
        .await;
    assert!(inventory.body["boosters"][0]["contents"].is_null());
    assert!(inventory.body["boosters"][0].get("seed").is_none());
    let denied=f.call("POST","/api/store",json!({"key":Uuid::now_v7(),"operation":{"action":"buy","kind":"bronze","quote":quote}}),&[("cookie",&token)]).await;
    assert_eq!(denied.body["error"], "INSUFFICIENT_FUNDS");
    let second=f.call("POST","/api/auth/register",json!({"login":"SecondUser","email":"second@example.test","password":PASSWORD,"school":"tech","terms_version":"2026-10-16","age_confirmed":true}),&[]).await;
    assert_eq!(second.status, StatusCode::CREATED);
    let login = f
        .call(
            "POST",
            "/api/auth/login",
            json!({"identity":"SecondUser","password":PASSWORD}),
            &[],
        )
        .await;
    let other = cookie(&login);
    let denied = f
        .call(
            "POST",
            "/api/store",
            json!({"key":Uuid::now_v7(),"operation":{"action":"open","id":id}}),
            &[("cookie", &other)],
        )
        .await;
    assert_eq!(denied.status, StatusCode::NOT_FOUND);
    assert!(server::economy::reconcile(&pool).await.unwrap().is_ok());
}
