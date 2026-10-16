use server::{activity, operations};
use sqlx::PgPool;
use std::time::Duration;

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn live_metrics_are_aggregate_and_unavailable_on_the_public_router(db: PgPool) {
    use axum::{body::Body, extract::State, http::Request};
    use server::{
        auth::{Auth, AuthConfig, mail::Mailer},
        gateway::AppState,
        online::Settings,
    };
    use tower::ServiceExt as _;
    operations::maintenance(&db).await.unwrap();
    let state = AppState::new(
        db,
        Auth::new(AuthConfig::local(), Mailer::Capture(Default::default())),
        Settings::default(),
    );
    let response = server::gateway::router(state.clone())
        .oneshot(
            Request::builder()
                .uri("/metrics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 404);
    assert!(response.headers().contains_key("x-trace-id"));
    let response = server::telemetry::metrics::export(State(state.clone())).await;
    assert_eq!(response.status(), 200);
    let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
        .await
        .unwrap();
    let text = std::str::from_utf8(&bytes).unwrap();
    assert!(text.contains("operation_success{operation=\"maintenance\"} 1"));
    assert!(text.contains("wal_archive_failed 0"));
    assert!(text.contains("duel_round_latency_seconds_bucket{le=\"+Inf\"}"));
    assert!(!text.contains("user_id") && !text.contains("login=") && !text.contains("ip="));
    state.online.hub.shutdown().await;
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn weekly_report_handles_an_empty_database_without_writing_jobs_or_player_data(db: PgPool) {
    let report = operations::report::weekly(&db).await.unwrap();
    assert_eq!(report["overview"]["matches"], 0);
    assert_eq!(report["overview"]["xp_awarded"], "0");
    assert_eq!(report["sampled_latest_matches"], 0);
    assert_eq!(report["minimum_group_games"], 20);
    assert_eq!(report["templates"], serde_json::json!({}));
    assert_eq!(report["cards_attempted_in_matches"], serde_json::json!({}));
    let jobs: i64 = sqlx::query_scalar("SELECT count(*) FROM operation_jobs")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(jobs, 0);
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn maintenance_is_repeatable_and_publishes_only_aggregate_state(db: PgPool) {
    operations::maintenance(&db).await.unwrap();
    operations::maintenance(&db).await.unwrap();
    let (success, recent, summary): (bool,bool,String) = sqlx::query_as("SELECT success,last_success_at>now()-interval '1 minute',summary::text FROM operation_jobs WHERE name='maintenance'").fetch_one(&db).await.unwrap();
    assert!(success && recent);
    let summary: serde_json::Value = serde_json::from_str(&summary).unwrap();
    assert_eq!(summary["ledger_mismatches"], 0);
    assert_eq!(summary["erased_accounts"], 0);
    assert_eq!(summary.as_object().unwrap().len(), 5);
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn drain_is_a_transaction_barrier_and_never_overrides_maintenance(db: PgPool) {
    let mut starting = db.begin().await.unwrap();
    activity::ensure_available(&mut starting).await.unwrap();
    let pool = db.clone();
    let mut drain = tokio::spawn(async move { operations::drain(&pool, true).await });
    assert!(
        tokio::time::timeout(Duration::from_millis(100), &mut drain)
            .await
            .is_err()
    );
    starting.commit().await.unwrap();
    drain.await.unwrap().unwrap();
    let status = operations::status(&db).await.unwrap();
    assert!(status.draining && status.idle());
    let mut tx = db.begin().await.unwrap();
    assert_eq!(
        activity::ensure_available(&mut tx).await.unwrap_err().0,
        "MAINTENANCE"
    );
    tx.rollback().await.unwrap();
    sqlx::query("UPDATE server_controls SET maintenance=true WHERE singleton")
        .execute(&db)
        .await
        .unwrap();
    operations::drain(&db, false).await.unwrap();
    let mut tx = db.begin().await.unwrap();
    assert_eq!(
        activity::ensure_available(&mut tx).await.unwrap_err().0,
        "MAINTENANCE"
    );
    tx.rollback().await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM admin_audit WHERE action='deployment_drain'"
        )
        .fetch_one(&db)
        .await
        .unwrap(),
        2
    );
}
