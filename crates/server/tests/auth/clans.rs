use super::*;
use uuid::Uuid;

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn clan_http_requires_a_session_and_rejects_cross_origin_writes(pool: PgPool) {
    let f = Fixture::new(pool.clone());
    assert_eq!(
        f.call("GET", "/api/clans", Value::Null, &[]).await.status,
        StatusCode::UNAUTHORIZED
    );
    f.register().await;
    sqlx::query("UPDATE users SET account_xp=8000,tutorial_step=3")
        .execute(&pool)
        .await
        .unwrap();
    let session = cookie(&f.login(false).await);
    let clan = Uuid::now_v7();
    let create = json!({"action":"create","id":clan,"name":"HTTP Guild"});
    let blocked = f
        .call(
            "POST",
            "/api/clans",
            create.clone(),
            &[("cookie", &session), ("origin", "https://other.test")],
        )
        .await;
    assert_eq!(blocked.status, StatusCode::FORBIDDEN);
    let result = f
        .call(
            "POST",
            "/api/clans",
            create.clone(),
            &[("cookie", &session)],
        )
        .await;
    assert_eq!(result.status, StatusCode::OK, "{}", result.body);
    assert_eq!(result.body["revision"], 0);
    let retry = f
        .call("POST", "/api/clans", create, &[("cookie", &session)])
        .await;
    assert_eq!(retry.status, StatusCode::OK);
    let view = f
        .call("GET", "/api/clans", Value::Null, &[("cookie", &session)])
        .await;
    assert_eq!(view.status, StatusCode::OK, "{}", view.body);
    assert_eq!(view.body["own"], clan.to_string());
    assert_eq!(view.body["own_role"], "master");
    let unknown=f.call("POST","/api/clans",json!({"action":"edit","clan":clan,"revision":0,"description":"","motd":"","www":"","email":"","role":"master"}),&[("cookie",&session)]).await;
    assert_eq!(unknown.status, StatusCode::BAD_REQUEST);
    assert_eq!(unknown.body["error"], "INVALID_MESSAGE");
    let ledger = server::economy::reconcile(&pool).await.unwrap();
    assert!(ledger.is_ok());
    f.state.online.hub.shutdown().await;
}
