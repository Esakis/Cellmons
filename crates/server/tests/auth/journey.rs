use super::*;
use uuid::Uuid;

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL"]
async fn account_journey_enforces_locations_and_tutorial_rewards(pool: PgPool) {
    let f = Fixture::new(pool.clone());
    f.register().await;
    let login = f.login(false).await;
    let cookie = cookie(&login);
    let headers = [("cookie", cookie.as_str())];
    let state = f.call("GET", "/api/journey", Value::Null, &headers).await;
    assert_eq!(state.body["level"], 1);
    assert_eq!(state.body["tutorial_step"], 0);
    for path in [
        "/api/store",
        "/api/clans",
        "/api/expeditions",
        "/api/tournaments",
    ] {
        let result = f.call("GET", path, Value::Null, &headers).await;
        assert_eq!(result.body["error"], "LEVEL_REQUIRED", "{path}");
    }
    let skipped = f
        .call("POST", "/api/journey", json!({"step":2}), &headers)
        .await;
    assert_eq!(skipped.body["error"], "INVALID_MESSAGE");
    for step in [0, 0, 1, 1] {
        let result = f
            .call("POST", "/api/journey", json!({"step":step}), &headers)
            .await;
        assert_eq!(result.body["tutorial_step"], step + 1);
        assert_eq!(result.body["xp"], 0);
    }
    let user: Uuid = sqlx::query_scalar("SELECT id FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    let mut tx = pool.begin().await.unwrap();
    server::journey::award(&mut tx, user, true, false, 0)
        .await
        .unwrap();
    assert_eq!(server::journey::load(&mut tx, user).await.unwrap().xp, 0);
    server::journey::award(&mut tx, user, true, false, 5)
        .await
        .unwrap();
    server::journey::award(&mut tx, user, true, true, 5)
        .await
        .unwrap();
    let after = server::journey::load(&mut tx, user).await.unwrap();
    assert_eq!((after.xp, after.level, after.tutorial_step), (250, 3, 3));
    tx.commit().await.unwrap();
    assert!(server::journey::require(&pool, user, "arena").await.is_ok());
    assert!(
        matches!(server::journey::require(&pool, user, "castle").await, Err(e) if e.0 == "LEVEL_REQUIRED")
    );
    sqlx::query("UPDATE avatars SET xp=0,level=1 WHERE user_id=$1")
        .bind(user)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        f.call("GET", "/api/journey", Value::Null, &headers)
            .await
            .body["level"],
        3
    );
    for (place, threshold) in [
        ("portal", 450_i64),
        ("shop", 1000),
        ("clan", 3050),
        ("tower", 3050),
        ("castle", 16000),
        ("bazaar", 20000),
    ] {
        sqlx::query("UPDATE users SET account_xp=$2 WHERE id=$1")
            .bind(user)
            .bind(threshold - 1)
            .execute(&pool)
            .await
            .unwrap();
        assert!(
            matches!(server::journey::require(&pool, user, place).await, Err(e) if e.0 == "LEVEL_REQUIRED")
        );
        sqlx::query("UPDATE users SET account_xp=$2 WHERE id=$1")
            .bind(user)
            .bind(threshold)
            .execute(&pool)
            .await
            .unwrap();
        assert!(server::journey::require(&pool, user, place).await.is_ok());
    }
}
