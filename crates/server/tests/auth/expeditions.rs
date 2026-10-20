use super::*;
use uuid::Uuid;

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn expedition_http_rejects_guests_unverified_foreign_avatars_and_private_plan_fields(
    pool: PgPool,
) {
    let f = Fixture::new(pool.clone());
    assert_eq!(
        f.call("GET", "/api/expeditions", Value::Null, &[])
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    f.register().await;
    sqlx::query("UPDATE users SET account_xp=450,tutorial_step=3")
        .execute(&pool)
        .await
        .unwrap();
    let session = cookie(&f.login(false).await);
    let avatar: Uuid = sqlx::query_scalar("SELECT id FROM avatars WHERE is_active")
        .fetch_one(&pool)
        .await
        .unwrap();
    let id = Uuid::now_v7();
    let start = json!({"action":"start","id":id,"avatar":avatar,"difficulty":"walk"});
    assert_eq!(
        f.call(
            "POST",
            "/api/expeditions",
            start.clone(),
            &[("cookie", &session)]
        )
        .await
        .status,
        StatusCode::FORBIDDEN
    );
    sqlx::query("UPDATE users SET email_verified_at=now()")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        f.call(
            "POST",
            "/api/expeditions",
            start.clone(),
            &[("cookie", &session), ("origin", "https://other.test")]
        )
        .await
        .status,
        StatusCode::FORBIDDEN
    );
    let bad =
        json!({"action":"start","id":Uuid::now_v7(),"avatar":Uuid::now_v7(),"difficulty":"walk"});
    assert_eq!(
        f.call("POST", "/api/expeditions", bad, &[("cookie", &session)])
            .await
            .status,
        StatusCode::CONFLICT
    );
    for _ in 0..2 {
        let reply = f
            .call(
                "POST",
                "/api/expeditions",
                start.clone(),
                &[("cookie", &session)],
            )
            .await;
        assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    }
    let view = f
        .call(
            "GET",
            "/api/expeditions",
            Value::Null,
            &[("cookie", &session)],
        )
        .await;
    assert_eq!(view.status, StatusCode::OK, "{}", view.body);
    assert_eq!(view.body["run"]["choices"].as_array().unwrap().len(), 3);
    assert_eq!(view.body["run"]["stages"], 5);
    for field in [
        "seed",
        "plan",
        "reward_kind",
        "reward_selector",
        "card_pool",
        "seat",
    ] {
        assert!(!view.body.to_string().contains(&format!("\"{field}\"")));
    }
    let extra = json!({"action":"choose","id":id,"request":Uuid::now_v7(),"revision":0,"choice":0,"seed":"chosen"});
    let bad = f
        .call("POST", "/api/expeditions", extra, &[("cookie", &session)])
        .await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);
    assert_eq!(bad.body["error"], "INVALID_MESSAGE");
    f.state.online.hub.shutdown().await;
}
