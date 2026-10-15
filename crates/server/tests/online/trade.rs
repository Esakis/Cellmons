use super::*;
use server::trade::{Action, Manager};

fn snapshot(body: Body) -> pb::TradeSnapshot {
    match body {
        Body::TradeSnapshot(value) => value,
        other => panic!("expected trade: {other:?}"),
    }
}
fn offer(cards: &[(u32, u32)], ducats: i64) -> pb::TradeOffer {
    pb::TradeOffer {
        cards: cards
            .iter()
            .map(|(card_id, qty)| pb::TradeCard {
                card_id: *card_id,
                qty: *qty,
            })
            .collect(),
        ducats,
    }
}
async fn quantity(db: &PgPool, user: Uuid, card: i16) -> i32 {
    sqlx::query_scalar("SELECT qty FROM collection WHERE user_id=$1 AND card_id=$2")
        .bind(user)
        .bind(card)
        .fetch_optional(db)
        .await
        .unwrap()
        .unwrap_or(0)
}
async fn manager(db: PgPool) -> Manager {
    let (tx, mut rx) = tokio::sync::mpsc::channel(4096);
    tokio::spawn(async move { while rx.recv().await.is_some() {} });
    Manager::new(db, tx)
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn r_knt_02_trade_requires_verified_email_and_rechecks_it_before_settlement(pool: PgPool) {
    let a = player(&pool, 1, false).await;
    let b = player(&pool, 2, true).await;
    let manager = manager(pool.clone()).await;
    assert!(
        matches!(manager.invite([a.id,b.id],[a.login.clone(),b.login.clone()]).await,Err(e) if e.0=="EMAIL_UNVERIFIED")
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM activity_locks")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    sqlx::query("UPDATE users SET email_verified_at=now() WHERE id=$1")
        .bind(a.id)
        .execute(&pool)
        .await
        .unwrap();
    let invite = snapshot(
        manager
            .invite([a.id, b.id], [a.login, b.login])
            .await
            .unwrap(),
    );
    let id = Uuid::parse_str(&invite.trade_id).unwrap();
    manager
        .request(b.id, id, Action::Respond(true))
        .await
        .unwrap();
    manager
        .request(a.id, id, Action::Update(0, offer(&[(1, 1)], 10)))
        .await
        .unwrap();
    manager.request(a.id, id, Action::Confirm(1)).await.unwrap();
    sqlx::query("UPDATE users SET email_verified_at=NULL WHERE id=$1")
        .bind(a.id)
        .execute(&pool)
        .await
        .unwrap();
    let cancelled = snapshot(manager.request(b.id, id, Action::Confirm(1)).await.unwrap());
    assert_eq!(cancelled.status, "cancelled");
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM ledger_transactions WHERE reason='trade'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM activity_locks")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    manager.shutdown().await;
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn f9_trade_transfer_failure_rolls_back_every_item_balance_and_acceptance(pool: PgPool) {
    let a = player(&pool, 1, true).await;
    let b = player(&pool, 2, true).await;
    let manager = manager(pool.clone()).await;
    sqlx::query("UPDATE collection SET qty=$2 WHERE user_id=$1 AND card_id=1")
        .bind(b.id)
        .bind(i32::MAX)
        .execute(&pool)
        .await
        .unwrap();
    let invite = snapshot(
        manager
            .invite([a.id, b.id], [a.login, b.login])
            .await
            .unwrap(),
    );
    let id = Uuid::parse_str(&invite.trade_id).unwrap();
    manager
        .request(b.id, id, Action::Respond(true))
        .await
        .unwrap();
    manager
        .request(a.id, id, Action::Update(0, offer(&[(1, 1)], 40)))
        .await
        .unwrap();
    manager.request(a.id, id, Action::Confirm(1)).await.unwrap();
    let before = quantity(&pool, a.id, 1).await;
    assert!(
        matches!(manager.request(b.id,id,Action::Confirm(1)).await,Err(e) if e.0=="CARD_LIMIT")
    );
    assert_eq!(quantity(&pool, a.id, 1).await, before);
    assert_eq!(quantity(&pool, b.id, 1).await, i32::MAX);
    let state = snapshot(manager.request(a.id, id, Action::Snapshot).await.unwrap());
    assert_eq!(state.accepted, [true, false]);
    assert_eq!(state.status, "offering");
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM ledger_transactions WHERE reason='trade'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT min(balance) FROM wallets")
            .fetch_one(&pool)
            .await
            .unwrap(),
        40
    );
    manager.request(a.id, id, Action::Cancel).await.unwrap();
    assert!(server::economy::reconcile(&pool).await.unwrap().is_ok());
    manager.shutdown().await;
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn f9_trade_versions_clear_acceptance_and_one_hundred_races_never_duplicate(pool: PgPool) {
    let a = player(&pool, 1, true).await;
    let b = player(&pool, 2, true).await;
    let manager = manager(pool.clone()).await;
    let invite = snapshot(
        manager
            .invite([a.id, b.id], [a.login.clone(), b.login.clone()])
            .await
            .unwrap(),
    );
    let id = Uuid::parse_str(&invite.trade_id).unwrap();
    manager
        .request(b.id, id, Action::Respond(true))
        .await
        .unwrap();
    manager
        .request(a.id, id, Action::Update(0, offer(&[(1, 1)], 10)))
        .await
        .unwrap();
    manager.request(a.id, id, Action::Confirm(1)).await.unwrap();
    let next = snapshot(
        manager
            .request(b.id, id, Action::Update(1, offer(&[(2, 1)], 0)))
            .await
            .unwrap(),
    );
    assert_eq!(next.accepted, [false, false]);
    assert_eq!(next.version, 2);
    assert!(matches!(manager.request(a.id,id,Action::Confirm(1)).await,Err(e) if e.0=="CONFLICT"));
    let before = [
        quantity(&pool, a.id, 1).await,
        quantity(&pool, b.id, 1).await,
        quantity(&pool, a.id, 2).await,
        quantity(&pool, b.id, 2).await,
    ];
    let a_id = a.id;
    let results=futures_util::future::join_all((0..100).map(|_|{let db=pool.clone();async move {server::store::execute(&db,a_id,serde_json::from_value(serde_json::json!({"key":Uuid::now_v7(),"operation":{"action":"discard","cards":[{"card_id":1,"qty":1}]}})).unwrap()).await}})).await;
    assert!(results.iter().all(|r| matches!(r,Err(e) if e.0=="BUSY")));
    let results = futures_util::future::join_all(
        (0..100).map(|_| manager.request(a.id, id, Action::Confirm(2))),
    )
    .await;
    assert!(results.iter().any(Result::is_ok));
    assert!(
        results
            .iter()
            .all(|r| r.is_ok() || matches!(r,Err(e) if e.0=="BUSY"))
    );
    let results = futures_util::future::join_all(
        (0..100).map(|_| manager.request(b.id, id, Action::Confirm(2))),
    )
    .await;
    assert!(results.iter().any(Result::is_ok));
    let result = snapshot(manager.request(a.id, id, Action::Snapshot).await.unwrap());
    assert_eq!(result.status, "completed");
    assert_eq!(
        [
            quantity(&pool, a.id, 1).await,
            quantity(&pool, b.id, 1).await,
            quantity(&pool, a.id, 2).await,
            quantity(&pool, b.id, 2).await
        ],
        [before[0] - 1, before[1] + 1, before[2] + 1, before[3] - 1]
    );
    let wallets =
        sqlx::query_as::<_, (Uuid, i64)>("SELECT user_id,balance FROM wallets ORDER BY user_id")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert!(wallets.contains(&(a.id, 30)));
    assert!(wallets.contains(&(b.id, 50)));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM ledger_transactions WHERE reason='trade'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM activity_locks")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert!(server::economy::reconcile(&pool).await.unwrap().is_ok());
    manager.shutdown().await;
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn f9_trade_recovery_does_not_cancel_live_lease_or_allow_old_actor_to_settle(pool: PgPool) {
    let a = player(&pool, 1, true).await;
    let b = player(&pool, 2, true).await;
    let manager = manager(pool.clone()).await;
    let invite = snapshot(
        manager
            .invite([a.id, b.id], [a.login, b.login])
            .await
            .unwrap(),
    );
    let id = Uuid::parse_str(&invite.trade_id).unwrap();
    manager
        .request(b.id, id, Action::Respond(true))
        .await
        .unwrap();
    manager
        .request(a.id, id, Action::Update(0, offer(&[(1, 1)], 40)))
        .await
        .unwrap();
    manager.request(a.id, id, Action::Confirm(1)).await.unwrap();
    assert!(server::trade::recover(&pool).await.unwrap().is_empty());
    sqlx::query("UPDATE trades SET lease_until=clock_timestamp()-interval '1 second' WHERE id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    server::trade::recover(&pool).await.unwrap();
    let result = snapshot(manager.request(b.id, id, Action::Confirm(1)).await.unwrap());
    assert_eq!(result.status, "cancelled");
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM ledger_transactions WHERE reason='trade'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM activity_locks")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert!(server::economy::reconcile(&pool).await.unwrap().is_ok());
    manager.shutdown().await;
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn f9_trade_websocket_bazaar_ownership_busy_and_disconnect_cancel(pool: PgPool) {
    let a = player(&pool, 1, true).await;
    let b = player(&pool, 2, true).await;
    let c = player(&pool, 3, true).await;
    let server = Server::new(pool.clone(), Settings::default()).await;
    let mut ca = server.connect(&a, None).await;
    let mut cb = server.connect(&b, None).await;
    let mut cc = server.connect(&c, None).await;
    ca.join("arena").await;
    cb.join("arena").await;
    assert!(
        matches!(ca.call(Client::TradeInvite(pb::TradeInvite {target:b.login.clone()})).await,Body::Error(e) if e.code=="FORBIDDEN")
    );
    ca.join("bazaar").await;
    cb.join("bazaar").await;
    let invite = snapshot(
        ca.call(Client::TradeInvite(pb::TradeInvite {
            target: b.login.clone(),
        }))
        .await,
    );
    let trade_id = invite.trade_id;
    cb.event(|e| matches!(e,Body::TradeSnapshot(s) if s.trade_id==trade_id && s.status=="invited"))
        .await;
    assert!(
        matches!(cc.call(Client::TradeSnapshot(pb::TradeTarget {trade_id:trade_id.clone()})).await,Body::Error(e) if e.code=="FORBIDDEN")
    );
    let active = snapshot(
        cb.call(Client::TradeRespond(pb::TradeRespond {
            trade_id: trade_id.clone(),
            accept: true,
        }))
        .await,
    );
    assert_eq!(active.status, "offering");
    ca.join("training").await;
    let cancelled = snapshot(
        cb.event(
            |e| matches!(e,Body::TradeSnapshot(s) if s.trade_id==trade_id && s.status=="cancelled"),
        )
        .await,
    );
    assert_eq!(cancelled.accepted, [false, false]);
    ca.join("bazaar").await;
    let invite = snapshot(
        ca.call(Client::TradeInvite(pb::TradeInvite {
            target: b.login.clone(),
        }))
        .await,
    );
    let trade_id = invite.trade_id;
    cb.call(Client::TradeRespond(pb::TradeRespond {
        trade_id: trade_id.clone(),
        accept: true,
    }))
    .await;
    cb.socket.close(None).await.unwrap();
    ca.event(
        |e| matches!(e,Body::TradeSnapshot(s) if s.trade_id==trade_id && s.status=="cancelled"),
    )
    .await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM activity_locks")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    server.stop().await;
}
