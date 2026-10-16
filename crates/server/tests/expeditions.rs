//! Forced outcomes test transactions; the separate expedition_audit example uses real L3 moves.
use server::{
    duel::{
        self, Settings,
        record::{ExternalEnd, Record},
    },
    expeditions::{self, Choice},
};
use sqlx::PgPool;
use std::time::Duration;
use uuid::Uuid;

async fn user(db: &PgPool, index: usize) -> (Uuid, Uuid) {
    let user = Uuid::from_u128(index as u128 + 5000);
    let avatar = Uuid::from_u128(index as u128 + 6000);
    let mut tx = db.begin().await.unwrap();
    sqlx::query("INSERT INTO users(id,login,email,password_hash,start_school,email_verified_at) VALUES ($1,$2,$3,'$argon2id$v=19$m=19456,t=2,p=1$fixture$fixture','combat',now())")
        .bind(user).bind(format!("Explorer{index}")).bind(format!("explorer{index}@example.test")).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO ratings(user_id) VALUES ($1)")
        .bind(user)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("INSERT INTO unlocked_templates(user_id,template_id,source) VALUES ($1,1,'start')")
        .bind(user)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO avatars(id,user_id,slot,template_id,is_active) VALUES ($1,$2,0,1,true)",
    )
    .bind(avatar)
    .bind(user)
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::query("INSERT INTO decks(avatar_id) VALUES ($1)")
        .bind(avatar)
        .execute(&mut *tx)
        .await
        .unwrap();
    server::economy::grant_initial(&mut tx, user).await.unwrap();
    tx.commit().await.unwrap();
    (user, avatar)
}
fn settings() -> Settings {
    Settings {
        lease: Duration::from_secs(60),
        ..Default::default()
    }
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn incompatible_engine_cannot_advance_or_reserve_a_duel(db: PgPool) {
    let (user, avatar) = user(&db, 0).await;
    let id = run(&db, user, avatar).await;
    sqlx::query("UPDATE expeditions SET engine_version='other-engine' WHERE id=$1")
        .bind(id)
        .execute(&db)
        .await
        .unwrap();
    let err = expeditions::choose(
        &db,
        user,
        Choice {
            id,
            request: Uuid::now_v7(),
            revision: 0,
            choice: 0,
        },
        Uuid::now_v7(),
        &settings(),
    )
    .await
    .err()
    .unwrap();
    assert_eq!(err.0, "CONTENT_MISMATCH");
    assert_eq!(revision(&db, id).await, 0);
    let fights: i64 = sqlx::query_scalar("SELECT count(*) FROM duel_journal")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(fights, 0);
}
async fn run(db: &PgPool, user: Uuid, avatar: Uuid) -> Uuid {
    let id = Uuid::now_v7();
    expeditions::start(db, user, id, avatar, "walk")
        .await
        .unwrap();
    id
}
async fn revision(db: &PgPool, id: Uuid) -> i32 {
    sqlx::query_scalar("SELECT revision FROM expeditions WHERE id=$1")
        .bind(id)
        .fetch_one(db)
        .await
        .unwrap()
}
async fn fight(
    db: &PgPool,
    user: Uuid,
    id: Uuid,
    instance: Uuid,
    won: bool,
) -> (Record, engine::DuelState) {
    let revision = revision(db, id).await;
    let before = expeditions::choose(
        db,
        user,
        Choice {
            id,
            request: Uuid::now_v7(),
            revision,
            choice: 0,
        },
        instance,
        &settings(),
    )
    .await
    .unwrap()
    .unwrap();
    let mut record = before.clone();
    record.ended = Some(ExternalEnd {
        loser: Some(if won { 1 } else { 0 }),
        cause: 0,
    });
    record.revision += 1;
    let (state, _) = record.rebuild().unwrap();
    duel::journal::save(db, instance, &before, &record, &state)
        .await
        .unwrap();
    (record, state)
}
async fn xp(db: &PgPool, avatar: Uuid) -> i32 {
    sqlx::query_scalar("SELECT xp FROM avatars WHERE id=$1")
        .bind(avatar)
        .fetch_one(db)
        .await
        .unwrap()
}
async fn status(db: &PgPool, id: Uuid) -> String {
    sqlx::query_scalar("SELECT status FROM expeditions WHERE id=$1")
        .bind(id)
        .fetch_one(db)
        .await
        .unwrap()
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn expedition_xp_is_escrowed_and_retreat_pays_the_exact_fraction_once(db: PgPool) {
    let (user, avatar) = user(&db, 0).await;
    let id = run(&db, user, avatar).await;
    let instance = Uuid::now_v7();
    for _ in 0..2 {
        let (record, state) = fight(&db, user, id, instance, true).await;
        let reward = duel::result::settle(&db, instance, &record, &state)
            .await
            .unwrap();
        assert_eq!(reward.xp, [0, 0]);
    }
    assert_eq!(xp(&db, avatar).await, 0);
    let accumulated: i32 = sqlx::query_scalar("SELECT xp_accum FROM expeditions WHERE id=$1")
        .bind(id)
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(accumulated, 200);
    let revision = revision(&db, id).await;
    let request = Uuid::now_v7();
    let mut jobs = vec![];
    for _ in 0..100 {
        let db = db.clone();
        jobs.push(tokio::spawn(async move {
            expeditions::retreat(&db, user, id, request, revision).await
        }));
    }
    for job in jobs {
        job.await.unwrap().unwrap();
    }
    assert_eq!(xp(&db, avatar).await, 80);
    assert_eq!(status(&db, id).await, "retreated");
    let rating: (i32, i32) = sqlx::query_as("SELECT rating,games FROM ratings WHERE user_id=$1")
        .bind(user)
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(rating, (1000, 0));
    let balance: i64 = sqlx::query_scalar("SELECT balance FROM wallets WHERE user_id=$1")
        .bind(user)
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(balance, 40);
    assert!(server::economy::reconcile(&db).await.unwrap().is_ok());
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn boss_completion_grants_all_xp_and_one_exclusive_reward_exactly_once(db: PgPool) {
    let (user, avatar) = user(&db, 0).await;
    let id = run(&db, user, avatar).await;
    let instance = Uuid::now_v7();
    for stage in 0..5 {
        let (record, state) = fight(&db, user, id, instance, true).await;
        let reward = duel::result::settle(&db, instance, &record, &state)
            .await
            .unwrap();
        let again = duel::result::settle(&db, instance, &record, &state)
            .await
            .unwrap();
        assert_eq!(reward.xp, again.xp);
        assert_eq!(reward.xp[0], if stage == 4 { 600 } else { 0 });
    }
    assert_eq!(status(&db, id).await, "won");
    assert_eq!(xp(&db, avatar).await, 600);
    let (raw,balance):(Vec<u8>,i64)=sqlx::query_as("SELECT e.award,w.balance FROM expeditions e JOIN wallets w ON w.user_id=e.user_id WHERE e.id=$1").bind(id).fetch_one(&db).await.unwrap();
    let award: expeditions::model::Award = serde_json::from_slice(&raw).unwrap();
    assert_eq!(award.xp, 600);
    assert_eq!(balance, 40 + i64::from(award.ducats));
    assert!(award.ducats == 5 || award.ducats == 15);
    assert!(
        u8::from(award.template.is_some())
            + u8::from(award.card.is_some())
            + u8::from(award.ducats > 5)
            <= 1
    );
    if let Some(id) = award.template {
        assert!(matches!(
            engine_content::avatar(id).unwrap().rarity,
            engine_content::AvatarRarity::Common
        ));
        assert_ne!(id, 1);
    }
    let grants: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ledger_transactions WHERE reason='expedition' AND ref_id=$1",
    )
    .bind(id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(grants, 1);
    let history: i64 = sqlx::query_scalar("SELECT sum(first_xp)::bigint FROM matches")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(history, 600);
    let locks: i64 = sqlx::query_scalar("SELECT count(*) FROM activity_locks WHERE user_id=$1")
        .bind(user)
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(locks, 0);
    assert!(server::economy::reconcile(&db).await.unwrap().is_ok());
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn competing_choices_start_only_one_duel_and_loss_discards_escrow(db: PgPool) {
    let (user, avatar) = user(&db, 0).await;
    let id = run(&db, user, avatar).await;
    let instance = Uuid::now_v7();
    let request = Uuid::now_v7();
    let mut jobs = vec![];
    for _ in 0..100 {
        let db = db.clone();
        jobs.push(tokio::spawn(async move {
            expeditions::choose(
                &db,
                user,
                Choice {
                    id,
                    request,
                    revision: 0,
                    choice: 0,
                },
                instance,
                &settings(),
            )
            .await
        }));
    }
    let mut started = vec![];
    for job in jobs {
        if let Some(record) = job.await.unwrap().unwrap() {
            started.push(record);
        }
    }
    assert_eq!(started.len(), 1);
    assert_eq!(
        expeditions::retreat(&db, user, id, Uuid::now_v7(), 1)
            .await
            .unwrap_err()
            .0,
        "BUSY"
    );
    let before = &started[0];
    let mut record = before.clone();
    record.ended = Some(ExternalEnd {
        loser: Some(1),
        cause: 0,
    });
    record.revision += 1;
    let (state, _) = record.rebuild().unwrap();
    duel::journal::save(&db, instance, before, &record, &state)
        .await
        .unwrap();
    duel::result::settle(&db, instance, &record, &state)
        .await
        .unwrap();
    let (record, state) = fight(&db, user, id, instance, false).await;
    duel::result::settle(&db, instance, &record, &state)
        .await
        .unwrap();
    assert_eq!(status(&db, id).await, "lost");
    assert_eq!(xp(&db, avatar).await, 0);
    assert!(
        expeditions::choose(
            &db,
            user,
            Choice {
                id,
                request,
                revision: 0,
                choice: 0
            },
            instance,
            &settings()
        )
        .await
        .unwrap()
        .is_none()
    );
    // Replaying Start for a completed run cannot create another run or reserve the avatar again.
    expeditions::start(&db, user, id, avatar, "walk")
        .await
        .unwrap();
    let locks: i64 = sqlx::query_scalar("SELECT count(*) FROM activity_locks WHERE user_id=$1")
        .bind(user)
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(locks, 0);
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn revoked_verification_cannot_collect_a_reward_and_wallet_capacity_is_checked_before_start(
    db: PgPool,
) {
    let (user, avatar) = user(&db, 0).await;
    let id = run(&db, user, avatar).await;
    let instance = Uuid::now_v7();
    let (record, state) = fight(&db, user, id, instance, true).await;
    sqlx::query("UPDATE users SET email_verified_at=NULL WHERE id=$1")
        .bind(user)
        .execute(&db)
        .await
        .unwrap();
    let award = duel::result::settle(&db, instance, &record, &state)
        .await
        .unwrap();
    assert_eq!(award.xp, [0, 0]);
    assert_eq!(status(&db, id).await, "lost");
    sqlx::query("UPDATE users SET email_verified_at=now() WHERE id=$1")
        .bind(user)
        .execute(&db)
        .await
        .unwrap();
    let mut tx = db.begin().await.unwrap();
    sqlx::query("SELECT id FROM users WHERE id=$1 FOR UPDATE")
        .bind(user)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    server::economy::transfer(
        &mut tx,
        server::economy::Account::Faucet,
        server::economy::Account::Player(user),
        i64::MAX - 40,
        "admin",
        Uuid::now_v7(),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(
        expeditions::start(&db, user, Uuid::now_v7(), avatar, "walk")
            .await
            .unwrap_err()
            .0,
        "BALANCE_LIMIT"
    );
    assert!(server::economy::reconcile(&db).await.unwrap().is_ok());
}
