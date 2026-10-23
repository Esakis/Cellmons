//! Real engine, PostgreSQL escrow, retries, readiness and scheduler recovery.
use engine::{EventBuf, Side};
use engine_ai::{Bot, Level};
use protocol::Message as _;
use server::{
    duel::{
        self, Settings,
        record::{ExternalEnd, Record},
    },
    tournaments::{self, Terms},
};
use sqlx::PgPool;
use std::time::Duration;
use uuid::Uuid;

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn incompatible_release_cannot_join_or_claim_a_tournament(db: PgPool) {
    let (a, _) = user(&db, 0).await;
    let (b, avatar) = user(&db, 1).await;
    let id = create(&db, a, terms(2, 0)).await;
    sqlx::query("UPDATE tournaments SET engine_version='retained-release' WHERE id=$1")
        .bind(id)
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(
        tournaments::join(&db, b, id, Uuid::now_v7(), avatar)
            .await
            .unwrap_err()
            .0,
        "CONTENT_MISMATCH"
    );
    assert_eq!(
        tournaments::tick(
            &db,
            id,
            Uuid::now_v7(),
            &settings(),
            server::online::now_ms()
        )
        .await
        .err()
        .unwrap()
        .0,
        "CONTENT_MISMATCH"
    );
    let claimed: bool =
        sqlx::query_scalar("SELECT scheduler_instance IS NOT NULL FROM tournaments WHERE id=$1")
            .bind(id)
            .fetch_one(&db)
            .await
            .unwrap();
    assert!(!claimed);
}

async fn user(db: &PgPool, index: usize) -> (Uuid, Uuid) {
    let id = Uuid::from_u128(index as u128 + 1000);
    let avatar = Uuid::from_u128(index as u128 + 2000);
    let starters: Vec<_> = engine_content::STARTERS
        .iter()
        .filter(|s| engine_content::is_playable(s.avatar))
        .collect();
    let starter = starters[index % starters.len()];
    let template = i32::from(starter.avatar);
    let mut tx = db.begin().await.unwrap();
    sqlx::query("INSERT INTO users(id,login,email,password_hash,start_school,email_verified_at) VALUES ($1,$2,$3,'$argon2id$v=19$m=19456,t=2,p=1$fixture$fixture','combat',now())")
        .bind(id).bind(format!("Tournament{index}")).bind(format!("tournament{index}@example.test")).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO ratings(user_id) VALUES ($1)")
        .bind(id)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO unlocked_templates(user_id,template_id,source) VALUES ($1,$2,'start')",
    )
    .bind(id)
    .bind(template)
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO avatars(id,user_id,slot,template_id,is_active) VALUES ($1,$2,0,$3,true)",
    )
    .bind(avatar)
    .bind(id)
    .bind(template)
    .execute(&mut *tx)
    .await
    .unwrap();
    for (card, qty) in starter.cards {
        sqlx::query("INSERT INTO collection(user_id,card_id,qty) VALUES ($1,$2,$3)")
            .bind(id)
            .bind(*card as i16)
            .bind(i32::from(*qty))
            .execute(&mut *tx)
            .await
            .unwrap();
    }
    sqlx::query("INSERT INTO decks(avatar_id,cards) VALUES ($1,$2)")
        .bind(avatar)
        .bind(starter.deck.iter().map(|id| *id as i16).collect::<Vec<_>>())
        .execute(&mut *tx)
        .await
        .unwrap();
    server::economy::grant_initial(&mut tx, id).await.unwrap();
    tx.commit().await.unwrap();
    (id, avatar)
}
fn terms(teams: usize, fee: i64) -> Terms {
    Terms {
        teams,
        team_size: 1,
        fee,
        max_games: 3,
        min_level: 1,
        max_level: 20,
        tiers: vec![0, 1, 2, 3],
        only_clan: None,
        allowed_clans: vec![],
    }
}
fn settings() -> Settings {
    Settings {
        lease: Duration::from_secs(60),
        ..Default::default()
    }
}
async fn create(db: &PgPool, owner: Uuid, terms: Terms) -> Uuid {
    let id = Uuid::now_v7();
    tournaments::create(db, owner, id, "Tournament test", terms)
        .await
        .unwrap();
    sqlx::query("UPDATE tournaments SET seed=$2 WHERE id=$1")
        .bind(id)
        .bind([31_u8; 32].as_slice())
        .execute(db)
        .await
        .unwrap();
    id
}
async fn balance(db: &PgPool, user: Uuid) -> i64 {
    sqlx::query_scalar("SELECT balance FROM wallets WHERE user_id=$1")
        .bind(user)
        .fetch_one(db)
        .await
        .unwrap()
}
async fn escrow(db: &PgPool, id: Uuid) -> i64 {
    sqlx::query_scalar("SELECT balance FROM tournaments WHERE id=$1")
        .bind(id)
        .fetch_one(db)
        .await
        .unwrap()
}
async fn ready_at(db: &PgPool, id: Uuid, now: i64) {
    sqlx::query("UPDATE tournament_entries SET ready_since_ms=$2-15000,ready_until_ms=$2+15000 WHERE tournament_id=$1 AND status='enrolled'").bind(id).bind(now).execute(db).await.unwrap();
}
async fn play(db: &PgPool, instance: Uuid, mut record: Record) {
    let (mut state, _) = record.rebuild().unwrap();
    let content = engine_content::content();
    let mut bots = [Bot::new(Level::L1, 11), Bot::new(Level::L1, 29)];
    while !state.is_finished() {
        let previous = record.clone();
        let moves = Side::BOTH.map(|s| bots[s.index()].choose(&engine::view(&state, s), &content));
        let mut events = EventBuf::new();
        engine::resolve_round(&mut state, &content, &moves, &mut events).unwrap();
        record.moves.push(
            moves
                .each_ref()
                .map(|m| protocol::duel::encode_move(m).encode_to_vec()),
        );
        record.revision += 1;
        duel::journal::save(db, instance, &previous, &record, &state)
            .await
            .unwrap();
        for side in Side::BOTH {
            bots[side.index()].observe(&engine::view(&state, side));
        }
    }
    duel::journal::verify_rounds(db, &record).await.unwrap();
    let reward = duel::result::settle(db, instance, &record, &state)
        .await
        .unwrap();
    let again = duel::result::settle(db, instance, &record, &state)
        .await
        .unwrap();
    assert_eq!(reward.xp, again.xp);
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn fee_retry_refund_and_request_fingerprints_preserve_the_ledger(db: PgPool) {
    let (owner, avatar) = user(&db, 0).await;
    let id = create(&db, owner, terms(2, 10)).await;
    let request = Uuid::now_v7();
    let mut jobs = vec![];
    for _ in 0..100 {
        let db = db.clone();
        jobs.push(tokio::spawn(async move {
            tournaments::join(&db, owner, id, request, avatar).await
        }));
    }
    for job in jobs {
        job.await.unwrap().unwrap();
    }
    assert_eq!(balance(&db, owner).await, 30);
    assert_eq!(escrow(&db, id).await, 10);
    let key = Uuid::now_v7();
    tournaments::leave(&db, owner, id, key).await.unwrap();
    tournaments::leave(&db, owner, id, key).await.unwrap();
    assert_eq!(balance(&db, owner).await, 40);
    assert_eq!(escrow(&db, id).await, 0);
    // A delayed old Join cannot enroll or charge the player again after withdrawal.
    tournaments::join(&db, owner, id, request, avatar)
        .await
        .unwrap();
    assert_eq!(balance(&db, owner).await, 40);
    assert_eq!(
        tournaments::leave(&db, owner, id, request)
            .await
            .unwrap_err()
            .0,
        "CONFLICT"
    );
    tournaments::join(&db, owner, id, Uuid::now_v7(), avatar)
        .await
        .unwrap();
    assert!(server::economy::reconcile(&db).await.unwrap().is_ok());
    let corrupt = sqlx::query("UPDATE tournaments SET balance=balance+1 WHERE id=$1")
        .bind(id)
        .execute(&db)
        .await
        .unwrap_err();
    assert_eq!(
        corrupt.as_database_error().unwrap().code().unwrap(),
        "23514"
    );
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn competing_registration_pins_exactly_one_activity_and_rechecks_eligibility(db: PgPool) {
    let (a, avatar) = user(&db, 0).await;
    let (b, other) = user(&db, 1).await;
    let first = create(&db, a, terms(2, 0)).await;
    let second = create(&db, b, terms(2, 0)).await;
    let mut jobs = vec![];
    for i in 0..100 {
        let db = db.clone();
        jobs.push(tokio::spawn(async move {
            tournaments::join(
                &db,
                a,
                if i % 2 == 0 { first } else { second },
                Uuid::now_v7(),
                avatar,
            )
            .await
        }));
    }
    let mut successes = 0;
    for job in jobs {
        successes += usize::from(job.await.unwrap().is_ok());
    }
    assert_eq!(successes, 1);
    assert_eq!(
        server::game::mutate(&db, a, server::game::Mutation::Activate { id: avatar })
            .await
            .err()
            .expect("activity blocks avatar changes")
            .0,
        "BUSY"
    );
    assert_eq!(
        tournaments::join(&db, b, first, Uuid::now_v7(), avatar)
            .await
            .unwrap_err()
            .0,
        "INELIGIBLE"
    );
    sqlx::query("UPDATE users SET email_verified_at=NULL WHERE id=$1")
        .bind(b)
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(
        tournaments::join(&db, b, first, Uuid::now_v7(), other)
            .await
            .unwrap_err()
            .0,
        "FORBIDDEN"
    );
    assert!(server::economy::reconcile(&db).await.unwrap().is_ok());
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn thirty_two_bots_finish_a_durable_tournament_and_pay_the_whole_pot_once(db: PgPool) {
    let mut users = vec![];
    for index in 0..32 {
        users.push(user(&db, index).await);
    }
    let id = create(&db, users[0].0, terms(32, 1)).await;
    for (user, avatar) in &users {
        tournaments::join(&db, *user, id, Uuid::now_v7(), *avatar)
            .await
            .unwrap();
    }
    let instance = Uuid::now_v7();
    let settings = settings();
    let mut now = server::online::now_ms();
    let mut games = 0;
    tournaments::tick(&db, id, instance, &settings, now)
        .await
        .unwrap();
    for _ in 0..100 {
        now += 20000;
        ready_at(&db, id, now).await;
        let work = tournaments::tick(&db, id, instance, &settings, now)
            .await
            .unwrap();
        for record in work.duels {
            play(&db, instance, record).await;
            games += 1;
        }
        let status: String = sqlx::query_scalar("SELECT status FROM tournaments WHERE id=$1")
            .bind(id)
            .fetch_one(&db)
            .await
            .unwrap();
        if status == "finished" {
            break;
        }
    }
    let (status, raw): (String, Vec<u8>) =
        sqlx::query_as("SELECT status,bracket FROM tournaments WHERE id=$1")
            .bind(id)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(status, "finished");
    let bracket: tournaments::bracket::Bracket = serde_json::from_slice(&raw).unwrap();
    let champion = bracket.champion.unwrap();
    assert!((62..=93).contains(&games), "{games}");
    // 40 starting Dukats - 1 entry + 32 prize + 5 daily quest.
    assert_eq!(balance(&db, champion).await, 76);
    assert_eq!(escrow(&db, id).await, 0);
    let rating: i64 = sqlx::query_scalar("SELECT sum(rating) FROM ratings")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(rating, 32000);
    let locks: i64 = sqlx::query_scalar("SELECT count(*) FROM activity_locks WHERE owner_id=$1")
        .bind(id)
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(locks, 0);
    let prizes: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ledger_transactions WHERE reason='tournament_prize' AND ref_id=$1",
    )
    .bind(id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(prizes, 1);
    tournaments::tick(&db, id, instance, &settings, now + 1000)
        .await
        .unwrap();
    assert_eq!(balance(&db, champion).await, 76);
    assert!(server::economy::reconcile(&db).await.unwrap().is_ok());
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn clan_rosters_advance_together_and_share_every_ducat(db: PgPool) {
    let mut users = vec![];
    for index in 0..32 {
        users.push(user(&db, index).await);
    }
    let mut clans = vec![];
    for index in 0..8 {
        let clan = Uuid::from_u128(3000 + index as u128);
        clans.push(clan);
        sqlx::query("INSERT INTO clans(id,name,status,founded_at) VALUES ($1,$2,'active',now())")
            .bind(clan)
            .bind(format!("Clan {index}"))
            .execute(&db)
            .await
            .unwrap();
        for (user, _) in &users[index * 4..index * 4 + 4] {
            sqlx::query("INSERT INTO clan_members(clan_id,user_id,role) VALUES ($1,$2,'member')")
                .bind(clan)
                .bind(user)
                .execute(&db)
                .await
                .unwrap();
        }
    }
    let mut config = terms(8, 1);
    config.team_size = 4;
    config.allowed_clans = clans;
    let id = create(&db, users[0].0, config).await;
    for (user, avatar) in &users {
        tournaments::join(&db, *user, id, Uuid::now_v7(), *avatar)
            .await
            .unwrap();
    }
    let instance = Uuid::now_v7();
    let settings = settings();
    let mut now = server::online::now_ms();
    let mut games = 0;
    tournaments::tick(&db, id, instance, &settings, now)
        .await
        .unwrap();
    for _ in 0..60 {
        now += 20000;
        ready_at(&db, id, now).await;
        let work = tournaments::tick(&db, id, instance, &settings, now)
            .await
            .unwrap();
        for record in work.duels {
            play(&db, instance, record).await;
            games += 1;
        }
        let status: String = sqlx::query_scalar("SELECT status FROM tournaments WHERE id=$1")
            .bind(id)
            .fetch_one(&db)
            .await
            .unwrap();
        if status == "finished" {
            break;
        }
    }
    let winners: Vec<Uuid> = sqlx::query_scalar(
        "SELECT user_id FROM tournament_entries WHERE tournament_id=$1 AND status='champion'",
    )
    .bind(id)
    .fetch_all(&db)
    .await
    .unwrap();
    assert_eq!(winners.len(), 4);
    assert!((56..=84).contains(&games), "{games}");
    for winner in winners {
        // Teams advance together: a champion may have played too few qualifying duels
        // after account level 4 to earn the daily quest. Its reward is separate from the pot.
        let daily: i64 =
            sqlx::query_scalar("SELECT count(*)*5 FROM daily_quests WHERE user_id=$1 AND rewarded")
                .bind(winner)
                .fetch_one(&db)
                .await
                .unwrap();
        assert!(daily == 0 || daily == 5);
        assert_eq!(balance(&db, winner).await, 47 + daily);
    }
    assert_eq!(escrow(&db, id).await, 0);
    assert!(server::economy::reconcile(&db).await.unwrap().is_ok());
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn scheduler_recovery_resets_readiness_and_absence_does_not_create_draw_xp(db: PgPool) {
    let (a, aa) = user(&db, 0).await;
    let (b, ba) = user(&db, 1).await;
    let id = create(&db, a, terms(2, 2)).await;
    for (user, avatar) in [(a, aa), (b, ba)] {
        tournaments::join(&db, user, id, Uuid::now_v7(), avatar)
            .await
            .unwrap();
    }
    let settings = settings();
    let instance = Uuid::now_v7();
    let now = server::online::now_ms();
    tournaments::tick(&db, id, instance, &settings, now)
        .await
        .unwrap();
    ready_at(&db, id, now + 20000).await;
    let replacement = Uuid::now_v7();
    let takeover = tournaments::tick(&db, id, replacement, &settings, now + 20000)
        .await
        .unwrap();
    assert!(takeover.duels.is_empty());
    let before=sqlx::query_scalar::<_,i64>("SELECT count(*) FROM tournament_entries WHERE tournament_id=$1 AND ready_since_ms IS NOT NULL").bind(id).fetch_one(&db).await.unwrap();
    assert_eq!(before, 0);
    let old = tournaments::tick(&db, id, instance, &settings, now + 20001)
        .await
        .unwrap();
    assert!(old.duels.is_empty());
    let finish = tournaments::tick(&db, id, replacement, &settings, now + 140000)
        .await
        .unwrap();
    assert!(finish.duels.is_empty());
    let status: String = sqlx::query_scalar("SELECT status FROM tournaments WHERE id=$1")
        .bind(id)
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(status, "finished");
    let matches: i64 = sqlx::query_scalar("SELECT count(*) FROM matches")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(matches, 0);
    assert_eq!(escrow(&db, id).await, 0);
    assert_eq!(balance(&db, a).await, 38);
    assert!(server::economy::reconcile(&db).await.unwrap().is_ok());
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn readiness_lease_keeps_first_return_time_and_one_absence_is_a_real_walkover(db: PgPool) {
    let (a, aa) = user(&db, 0).await;
    let (b, ba) = user(&db, 1).await;
    let id = create(&db, a, terms(2, 0)).await;
    for (user, avatar) in [(a, aa), (b, ba)] {
        tournaments::join(&db, user, id, Uuid::now_v7(), avatar)
            .await
            .unwrap();
    }
    tournaments::ready(&db, a, true).await.unwrap();
    let since: i64 = sqlx::query_scalar(
        "SELECT ready_since_ms FROM tournament_entries WHERE tournament_id=$1 AND user_id=$2",
    )
    .bind(id)
    .bind(a)
    .fetch_one(&db)
    .await
    .unwrap();
    tournaments::ready(&db, a, true).await.unwrap();
    let after: i64 = sqlx::query_scalar(
        "SELECT ready_since_ms FROM tournament_entries WHERE tournament_id=$1 AND user_id=$2",
    )
    .bind(id)
    .bind(a)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(since, after);
    let now = server::online::now_ms();
    let instance = Uuid::now_v7();
    let settings = settings();
    tournaments::tick(&db, id, instance, &settings, now)
        .await
        .unwrap();
    sqlx::query("UPDATE tournament_entries SET ready_since_ms=$3,ready_until_ms=$3+200000 WHERE tournament_id=$1 AND user_id=$2").bind(id).bind(a).bind(now).execute(&db).await.unwrap();
    let work = tournaments::tick(&db, id, instance, &settings, now + 111000)
        .await
        .unwrap();
    assert_eq!(work.duels.len(), 1);
    let record = &work.duels[0];
    assert!(record.ended.is_some());
    let (state, _) = record.rebuild().unwrap();
    let rewards = duel::result::settle(&db, instance, record, &state)
        .await
        .unwrap();
    assert_eq!(rewards.xp, [0, 0]);
    assert_eq!(rewards.delta.iter().sum::<i32>(), 0);
    let mut wrong = record.clone();
    wrong.id = Uuid::now_v7();
    wrong.ended = Some(ExternalEnd {
        loser: Some(0),
        cause: 0,
    });
    let mut tx = db.begin().await.unwrap();
    assert!(
        duel::journal::create_for_activity(
            &mut tx,
            &wrong,
            instance,
            60000,
            "tournament",
            Uuid::now_v7()
        )
        .await
        .is_err()
    );
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn expired_registration_refunds_and_concurrent_cancel_cannot_refund_twice(db: PgPool) {
    let (a, aa) = user(&db, 0).await;
    let (b, ba) = user(&db, 1).await;
    let id = create(&db, a, terms(3, 7)).await;
    for (user, avatar) in [(a, aa), (b, ba)] {
        tournaments::join(&db, user, id, Uuid::now_v7(), avatar)
            .await
            .unwrap();
    }
    let instance = Uuid::now_v7();
    let settings = settings();
    let now = server::online::now_ms();
    let mut jobs = vec![];
    for _ in 0..100 {
        let db = db.clone();
        let settings = settings.clone();
        jobs.push(tokio::spawn(async move {
            tournaments::tick(&db, id, instance, &settings, now + 86401000).await
        }));
    }
    for job in jobs {
        job.await.unwrap().unwrap();
    }
    assert_eq!(balance(&db, a).await, 40);
    assert_eq!(balance(&db, b).await, 40);
    assert_eq!(escrow(&db, id).await, 0);
    let locks: i64 = sqlx::query_scalar("SELECT count(*) FROM activity_locks WHERE owner_id=$1")
        .bind(id)
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(locks, 0);
    let id = create(&db, a, terms(3, 7)).await;
    tournaments::join(&db, a, id, Uuid::now_v7(), aa)
        .await
        .unwrap();
    let mutation = serde_json::json!({"action":"cancel","id":id,"request":Uuid::now_v7()});
    // The HTTP retry uses this same request key; concurrent scheduler expiry is a separate path.
    let request: server::tournaments::Mutation = serde_json::from_value(mutation).unwrap();
    let server::tournaments::Mutation::Cancel { request, .. } = request else {
        unreachable!()
    };
    tournaments::cancel(&db, a, id, request).await.unwrap();
    tournaments::cancel(&db, a, id, request).await.unwrap();
    assert_eq!(balance(&db, a).await, 40);
    assert_eq!(escrow(&db, id).await, 0);
    assert!(server::economy::reconcile(&db).await.unwrap().is_ok());
}
