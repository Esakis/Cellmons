use super::*;
use uuid::Uuid;

async fn post(f: &Fixture, token: &str, body: Value) -> Reply {
    f.call("POST", "/api/game", body, &[("cookie", token)])
        .await
}
async fn get(f: &Fixture, token: &str) -> Reply {
    f.call("GET", "/api/game", Value::Null, &[("cookie", token)])
        .await
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn f8_pool_slots_activation_soft_delete_and_serialized_creation(pool: PgPool) {
    let f = Fixture::new(pool.clone());
    f.register().await;
    let token = cookie(&f.login(false).await);
    assert_eq!(get(&f, "").await.status, StatusCode::UNAUTHORIZED);
    let current = get(&f, &token).await;
    let initial = current.body["avatars"][0]["id"].as_str().unwrap();
    let copies = sqlx::query_scalar::<_, i64>("SELECT sum(qty)::bigint FROM collection")
        .fetch_one(&pool)
        .await
        .unwrap();
    for _ in 0..8 {
        assert_eq!(
            post(
                &f,
                &token,
                json!({"action":"create","id":Uuid::now_v7(),"template_id":1})
            )
            .await
            .status,
            StatusCode::OK
        );
    }
    let left = Uuid::now_v7();
    let right = Uuid::now_v7();
    let (a, b) = tokio::join!(
        post(
            &f,
            &token,
            json!({"action":"create","id":left,"template_id":1})
        ),
        post(
            &f,
            &token,
            json!({"action":"create","id":right,"template_id":1})
        )
    );
    assert_eq!(
        usize::from(a.status == StatusCode::OK) + usize::from(b.status == StatusCode::OK),
        1
    );
    let winner = if a.status == StatusCode::OK {
        left
    } else {
        right
    };
    assert_eq!(
        post(
            &f,
            &token,
            json!({"action":"create","id":winner,"template_id":1})
        )
        .await
        .status,
        StatusCode::OK
    );
    let denied = post(
        &f,
        &token,
        json!({"action":"delete","id":initial,"revision":0}),
    )
    .await;
    assert_eq!(denied.body["error"], "ACTIVE_AVATAR");
    assert_eq!(
        post(&f, &token, json!({"action":"activate","id":winner}))
            .await
            .status,
        StatusCode::OK
    );
    let deleted = post(
        &f,
        &token,
        json!({"action":"delete","id":initial,"revision":1}),
    )
    .await;
    assert_eq!(deleted.status, StatusCode::OK);
    assert_eq!(deleted.body["avatars"].as_array().unwrap().len(), 9);
    let next = post(
        &f,
        &token,
        json!({"action":"create","id":Uuid::now_v7(),"template_id":1}),
    )
    .await;
    assert_eq!(next.status, StatusCode::OK);
    assert_eq!(
        next.body["avatars"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|a| a["active"] == true)
            .count(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT sum(qty)::bigint FROM collection")
            .fetch_one(&pool)
            .await
            .unwrap(),
        copies
    );
    assert_eq!(
        post(
            &f,
            &token,
            json!({"action":"create","id":Uuid::now_v7(),"template_id":999})
        )
        .await
        .status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        post(&f, &token, json!({"action":"activate","id":Uuid::now_v7()}))
            .await
            .status,
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn f8_points_stale_retries_and_deck_ownership_rules(pool: PgPool) {
    let f = Fixture::new(pool.clone());
    f.register().await;
    let token = cookie(&f.login(false).await);
    sqlx::query("UPDATE avatars SET level=5,xp=4000,dev_points_unspent=3")
        .execute(&pool)
        .await
        .unwrap();
    let before = get(&f, &token).await;
    let avatar = &before.body["avatars"][0];
    let id = avatar["id"].as_str().unwrap();
    let request = json!({"action":"spend","id":id,"revision":0,"points":[1,0,0,0,1,1]});
    let after = post(&f, &token, request.clone()).await;
    assert_eq!(after.status, StatusCode::OK);
    let updated = &after.body["avatars"][0];
    assert_eq!(updated["unspent"], 0);
    assert_eq!(
        updated["stats"][0].as_i64().unwrap(),
        avatar["stats"][0].as_i64().unwrap() + 1
    );
    assert_eq!(
        updated["stats"][4].as_i64().unwrap(),
        avatar["stats"][4].as_i64().unwrap() + 7
    );
    assert_eq!(
        updated["stats"][5].as_i64().unwrap(),
        avatar["stats"][5].as_i64().unwrap() + 7
    );
    assert_eq!(post(&f, &token, request).await.body["error"], "CONFLICT");
    assert_eq!(
        post(
            &f,
            &token,
            json!({"action":"spend","id":id,"revision":1,"points":[-1,0,0,0,0,0]})
        )
        .await
        .status,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        post(
            &f,
            &token,
            json!({"action":"deck","id":id,"revision":0,"cards":[999]})
        )
        .await
        .body["error"],
        "INVALID_DECK"
    );
    let stack = before.body["collection"][0]["card_id"].as_i64().unwrap();
    assert_eq!(
        post(
            &f,
            &token,
            json!({"action":"deck","id":id,"revision":0,"cards":[stack,stack,stack,stack,stack]})
        )
        .await
        .body["error"],
        "INVALID_DECK"
    );
    let empty = post(
        &f,
        &token,
        json!({"action":"deck","id":id,"revision":0,"cards":[]}),
    )
    .await;
    assert_eq!(empty.status, StatusCode::OK);
    assert_eq!(empty.body["avatars"][0]["deck_revision"], 1);
    assert_eq!(
        post(
            &f,
            &token,
            json!({"action":"deck","id":id,"revision":0,"cards":[]})
        )
        .await
        .body["error"],
        "CONFLICT"
    );
    let preset = Uuid::now_v7();
    assert_eq!(
        post(
            &f,
            &token,
            json!({"action":"preset_save","id":preset,"revision":null,"name":"Empty","cards":[]})
        )
        .await
        .status,
        StatusCode::OK
    );
    let other = Uuid::now_v7();
    assert_eq!(
        post(
            &f,
            &token,
            json!({"action":"preset_save","id":other,"revision":null,"name":"Empty","cards":[]})
        )
        .await
        .body["error"],
        "CONFLICT"
    );
    assert_eq!(
        post(
            &f,
            &token,
            json!({"action":"preset_delete","id":preset,"revision":0})
        )
        .await
        .status,
        StatusCode::OK
    );
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn f8_inventory_reduction_prunes_all_decks_and_presets_atomically(pool: PgPool) {
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
    let card = engine_content::STARTERS[0].cards[0].0 as i16;
    sqlx::query("UPDATE collection SET qty=4 WHERE card_id=$1")
        .bind(card)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE decks SET cards=$2 WHERE avatar_id=$1")
        .bind(avatar)
        .bind(vec![card; 4])
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO deck_presets(id,user_id,name,cards) VALUES ($1,$2,'Before trade',$3)")
        .bind(Uuid::now_v7())
        .bind(user)
        .bind(vec![card; 4])
        .execute(&pool)
        .await
        .unwrap();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM users WHERE id=$1 FOR UPDATE")
        .bind(user)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("UPDATE collection SET qty=1 WHERE user_id=$1 AND card_id=$2")
        .bind(user)
        .bind(card)
        .execute(&mut *tx)
        .await
        .unwrap();
    server::game::prune(&mut tx, user).await.unwrap();
    tx.commit().await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, Vec<i16>>("SELECT cards FROM decks WHERE avatar_id=$1")
            .bind(avatar)
            .fetch_one(&pool)
            .await
            .unwrap(),
        vec![card]
    );
    assert_eq!(
        sqlx::query_scalar::<_, Vec<i16>>("SELECT cards FROM deck_presets WHERE user_id=$1")
            .bind(user)
            .fetch_one(&pool)
            .await
            .unwrap(),
        vec![card]
    );
}
