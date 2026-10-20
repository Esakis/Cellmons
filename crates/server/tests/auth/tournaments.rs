use super::*;
use uuid::Uuid;

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn tournament_http_requires_verified_session_and_never_exports_private_rng(pool: PgPool) {
    let f = Fixture::new(pool.clone());
    assert_eq!(
        f.call("GET", "/api/tournaments", Value::Null, &[])
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    f.register().await;
    sqlx::query("UPDATE users SET account_xp=16000,tutorial_step=3")
        .execute(&pool)
        .await
        .unwrap();
    let session = cookie(&f.login(false).await);
    let id = Uuid::now_v7();
    let mutation = json!({"action":"create","id":id,"name":"HTTP Tournament","terms":{"teams":2,"team_size":1,"fee":0,"max_games":3,"min_level":1,"max_level":20,"tiers":[0,1,2,3],"only_clan":null,"allowed_clans":[]}});
    assert_eq!(
        f.call(
            "POST",
            "/api/tournaments",
            mutation.clone(),
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
            "/api/tournaments",
            mutation.clone(),
            &[("cookie", &session), ("origin", "https://other.test")]
        )
        .await
        .status,
        StatusCode::FORBIDDEN
    );
    for _ in 0..2 {
        let result = f
            .call(
                "POST",
                "/api/tournaments",
                mutation.clone(),
                &[("cookie", &session)],
            )
            .await;
        assert_eq!(result.status, StatusCode::OK, "{}", result.body);
        assert_eq!(result.body, id.to_string());
    }
    let avatar: Uuid = sqlx::query_scalar("SELECT id FROM avatars WHERE is_active")
        .fetch_one(&pool)
        .await
        .unwrap();
    let join = json!({"action":"join","id":id,"request":Uuid::now_v7(),"avatar":avatar});
    assert_eq!(
        f.call("POST", "/api/tournaments", join, &[("cookie", &session)])
            .await
            .status,
        StatusCode::OK
    );
    let view = f
        .call(
            "GET",
            "/api/tournaments",
            Value::Null,
            &[("cookie", &session)],
        )
        .await;
    assert_eq!(view.status, StatusCode::OK, "{}", view.body);
    assert_eq!(view.body["own"], id.to_string());
    assert_eq!(
        view.body["profile"]["entries"][0]["avatar"],
        avatar.to_string()
    );
    assert!(!view.body.to_string().contains("\"seed\""));
    assert!(!view.body.to_string().contains("\"counter\""));
    let invalid = f
        .call(
            "POST",
            "/api/tournaments",
            json!({"action":"create","id":Uuid::now_v7(),"name":"bad","terms":{"teams":-1}}),
            &[("cookie", &session)],
        )
        .await;
    assert_eq!(invalid.status, StatusCode::BAD_REQUEST);
    assert_eq!(invalid.body["error"], "INVALID_MESSAGE");
    assert!(server::economy::reconcile(&pool).await.unwrap().is_ok());
    f.state.online.hub.shutdown().await;
}
