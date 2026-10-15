use super::*;
use server::economy::{self, Account};
use uuid::Uuid;

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn f9_initial_wallet_and_ledger_are_atomic_private_and_append_only(pool: PgPool) {
    let f = Fixture::new(pool.clone());
    f.register().await;
    let token = cookie(&f.login(false).await);
    let user = sqlx::query_scalar::<_, Uuid>("SELECT id FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    let view = f
        .call("GET", "/api/economy", Value::Null, &[("cookie", &token)])
        .await;
    assert_eq!(view.status, StatusCode::OK);
    assert_eq!(view.body["balance"], "40");
    assert_eq!(view.body["entries"].as_array().unwrap().len(), 1);
    assert_eq!(view.headers[header::CACHE_CONTROL], "no-store");
    assert_eq!(
        f.call("GET", "/api/economy", Value::Null, &[]).await.status,
        StatusCode::UNAUTHORIZED
    );
    let mut tx = pool.begin().await.unwrap();
    economy::grant_initial(&mut tx, user).await.unwrap();
    tx.commit().await.unwrap();
    assert!(economy::reconcile(&pool).await.unwrap().is_ok());
    let error = sqlx::query("DELETE FROM ledger_entries")
        .execute(&pool)
        .await
        .unwrap_err();
    assert_eq!(
        error.as_database_error().unwrap().code().as_deref(),
        Some("23514")
    );
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("UPDATE wallets SET balance=41 WHERE user_id=$1")
        .bind(user)
        .execute(&mut *tx)
        .await
        .unwrap();
    assert!(tx.commit().await.is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT balance FROM wallets WHERE user_id=$1")
            .bind(user)
            .fetch_one(&pool)
            .await
            .unwrap(),
        40
    );
    let mut tx = pool.begin().await.unwrap();
    let id = Uuid::now_v7();
    sqlx::query("INSERT INTO ledger_transactions(id,reason) VALUES ($1,'admin')")
        .bind(id)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("INSERT INTO ledger_entries(tx_id,account,delta) VALUES ($1,'system:faucet',3)")
        .bind(id)
        .execute(&mut *tx)
        .await
        .unwrap();
    assert!(tx.commit().await.is_err());
    assert!(economy::reconcile(&pool).await.unwrap().is_ok());
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn f9_one_hundred_concurrent_spends_cannot_overdraw_the_wallet(pool: PgPool) {
    let f = Fixture::new(pool.clone());
    f.register().await;
    let user = sqlx::query_scalar::<_, Uuid>("SELECT id FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    let results = futures_util::future::join_all((0..100).map(|_| {
        let pool = pool.clone();
        async move {
            let mut tx = pool.begin().await.unwrap();
            sqlx::query("SELECT id FROM users WHERE id=$1 FOR UPDATE")
                .bind(user)
                .execute(&mut *tx)
                .await
                .unwrap();
            match economy::transfer(
                &mut tx,
                Account::Player(user),
                Account::Sink,
                10,
                "booster",
                Uuid::now_v7(),
            )
            .await
            {
                Ok(()) => {
                    tx.commit().await.unwrap();
                    true
                }
                Err(error) => {
                    assert_eq!(error.0, "INSUFFICIENT_FUNDS");
                    false
                }
            }
        }
    }))
    .await;
    assert_eq!(results.iter().filter(|v| **v).count(), 4);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT balance FROM wallets WHERE user_id=$1")
            .bind(user)
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM ledger_entries")
            .fetch_one(&pool)
            .await
            .unwrap(),
        10
    );
    assert!(economy::reconcile(&pool).await.unwrap().is_ok());
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn f9_arena_daily_bonus_and_ducat_cap_use_utc_and_conserve_ledger(pool: PgPool) {
    let f = Fixture::new(pool.clone());
    f.register().await;
    let user = sqlx::query_scalar::<_, Uuid>("SELECT id FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    let mut awards = Vec::new();
    for _ in 0..12 {
        let mut tx = pool.begin().await.unwrap();
        sqlx::query("SELECT id FROM users WHERE id=$1 FOR UPDATE")
            .bind(user)
            .execute(&mut *tx)
            .await
            .unwrap();
        awards.push(
            economy::award_arena(&mut tx, user, Uuid::now_v7())
                .await
                .unwrap(),
        );
        tx.commit().await.unwrap();
    }
    assert_eq!(&awards[..5], &[12, 2, 2, 2, 2]);
    assert!(awards[5..].iter().all(|v| *v == 0));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT balance FROM wallets WHERE user_id=$1")
            .bind(user)
            .fetch_one(&pool)
            .await
            .unwrap(),
        60
    );
    assert!(economy::reconcile(&pool).await.unwrap().is_ok());
}
