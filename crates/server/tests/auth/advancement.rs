use super::*;
use protocol::Message;
use server::{
    duel::{
        self,
        record::{Record, Seat},
    },
    game::Mutation,
};
use uuid::Uuid;

async fn owner(db: &PgPool) -> (Uuid, Uuid) {
    sqlx::query_as("SELECT user_id,id FROM avatars WHERE is_active")
        .fetch_one(db)
        .await
        .unwrap()
}
async fn state(db: &PgPool, user: Uuid) -> server::game::Snapshot {
    server::game::load(&mut db.acquire().await.unwrap(), user)
        .await
        .unwrap()
}
async fn xp(db: &PgPool, user: Uuid, value: i64) {
    sqlx::query("UPDATE users SET account_xp=$2,email_verified_at=now() WHERE id=$1")
        .bind(user)
        .bind(value)
        .execute(db)
        .await
        .unwrap();
}
fn contract(template: u16, claim: bool) -> Mutation {
    Mutation::Contract {
        template_id: template,
        claim,
    }
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL"]
async fn contracts_gate_switch_retry_full_slots_and_keep_milestone_rewards(db: PgPool) {
    let f = Fixture::new(db.clone());
    f.register().await;
    let (user, _) = owner(&db).await;
    assert_eq!(
        server::game::mutate(&db, user, contract(2, false))
            .await
            .err()
            .unwrap()
            .0,
        "LEVEL_REQUIRED"
    );
    xp(&db, user, 450).await;
    for id in [1, 6, 29, 1000, u16::MAX] {
        assert_eq!(
            server::game::mutate(&db, user, contract(id, false))
                .await
                .err()
                .unwrap()
                .0,
            "RECRUIT_UNAVAILABLE"
        );
    }
    server::game::mutate(&db, user, contract(2, false))
        .await
        .unwrap();
    sqlx::query("UPDATE class_contracts SET fragments=29 WHERE user_id=$1")
        .bind(user)
        .execute(&db)
        .await
        .unwrap();
    server::game::mutate(&db, user, contract(4, false))
        .await
        .unwrap();
    let before = server::game::mutate(&db, user, contract(2, false))
        .await
        .unwrap();
    assert_eq!(
        before
            .advancement
            .contracts
            .iter()
            .find(|c| c.template_id == 2)
            .unwrap()
            .fragments,
        29
    );
    assert_eq!(
        before
            .advancement
            .contracts
            .iter()
            .filter(|c| c.active)
            .count(),
        1
    );
    assert_eq!(
        server::game::mutate(&db, user, contract(2, true))
            .await
            .err()
            .unwrap()
            .0,
        "NOT_READY"
    );
    sqlx::query("UPDATE class_contracts SET fragments=30 WHERE user_id=$1 AND template_id=2")
        .bind(user)
        .execute(&db)
        .await
        .unwrap();
    for slot in 1..10_i16 {
        sqlx::query("INSERT INTO avatars(id,user_id,slot,template_id) VALUES($1,$2,$3,1)")
            .bind(Uuid::now_v7())
            .bind(user)
            .bind(slot)
            .execute(&db)
            .await
            .unwrap();
    }
    assert_eq!(
        server::game::mutate(&db, user, contract(2, true))
            .await
            .err()
            .unwrap()
            .0,
        "SLOTS_FULL"
    );
    assert!(!state(&db, user).await.templates.contains(&2));
    sqlx::query("UPDATE avatars SET deleted_at=now() WHERE user_id=$1 AND slot=9")
        .bind(user)
        .execute(&db)
        .await
        .unwrap();
    let claims = futures_util::future::join_all(
        (0..10).map(|_| server::game::mutate(&db, user, contract(2, true))),
    )
    .await;
    assert!(claims.iter().all(Result::is_ok));
    let after = state(&db, user).await;
    assert_eq!(
        after.avatars.iter().filter(|a| a.template_id == 2).count(),
        1
    );
    assert!(after.recruitment.milestones.iter().all(|m| !m.claimed));
    let cards = serde_json::to_value(&after.collection).unwrap();
    let recruited = after.avatars.iter().find(|a| a.template_id == 2).unwrap();
    server::game::mutate(
        &db,
        user,
        Mutation::Delete {
            id: recruited.id,
            revision: recruited.revision,
        },
    )
    .await
    .unwrap();
    let retry = server::game::mutate(&db, user, contract(2, true))
        .await
        .unwrap();
    assert_eq!(serde_json::to_value(retry.collection).unwrap(), cards);
    assert!(retry.avatars.iter().all(|a| a.template_id != 2));
}

async fn completed(
    db: &PgPool,
    user: Uuid,
    avatar: Uuid,
    kind: &str,
) -> (Uuid, Record, engine::DuelState) {
    let snapshot = state(db, user).await;
    let a = snapshot.avatars.iter().find(|a| a.id == avatar).unwrap();
    let setup = engine::PlayerSetup {
        avatar: engine_content::avatar(a.template_id as u16)
            .unwrap()
            .snapshot(a.level as u8),
        deck: a.deck.iter().map(|n| *n as u16).collect(),
    };
    let profile = protocol::game::v1::PublicPlayer {
        id: user.to_string(),
        avatar_id: avatar.to_string(),
        template_id: a.template_id as u32,
        level: a.level as u32,
        tier: a.tier as u32,
        verified: true,
        ..Default::default()
    };
    let mut human = Seat::player(user, avatar, &setup, &profile);
    let mut npc = human.clone();
    npc.user = None;
    npc.avatar = None;
    npc.deck.clear();
    human.stats = [4, 5, 4, 5, 300, 150];
    npc.stats = [1, 0, 1, 0, 300, 150];
    let instance = Uuid::now_v7();
    let mut record = Record::new(
        Uuid::now_v7(),
        kind,
        [human, npc],
        [37; 32],
        &duel::Settings::default(),
    )
    .unwrap();
    let content = engine_content::content();
    let mut state = record.rebuild().unwrap().0;
    while !state.is_finished() {
        let mut moves = engine::Side::BOTH.map(|s| engine::default_move(&state, s));
        for index in 0..5 {
            let mut candidate = moves[0];
            candidate.card = engine::CardAction::Cast(index);
            if engine::validate_move(&state, &content, engine::Side::A, &candidate).is_ok() {
                moves[0] = candidate;
                break;
            }
        }
        record
            .moves
            .push(moves.map(|m| protocol::duel::encode_move(&m).encode_to_vec()));
        engine::resolve_round(&mut state, &content, &moves, &mut engine::EventBuf::new()).unwrap();
    }
    assert!(record.moves.len() >= 2);
    assert!(matches!(
        state.outcome(),
        Some(engine::Outcome::Win {
            winner: engine::Side::A,
            ..
        })
    ));
    assert!(!server::advancement::casts(&record).unwrap()[0].is_empty());
    let mut tx = db.begin().await.unwrap();
    duel::journal::create(&mut tx, &record, instance, 60000)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    (instance, record, state)
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL"]
async fn settlement_counts_unique_cards_casts_trials_daily_rewards_and_retries(db: PgPool) {
    let f = Fixture::new(db.clone());
    f.register().await;
    let (user, avatar) = owner(&db).await;
    xp(&db, user, 4700).await;
    sqlx::query("UPDATE avatars SET level=20,xp=6000 WHERE id=$1")
        .bind(avatar)
        .execute(&db)
        .await
        .unwrap();
    server::game::mutate(&db, user, contract(2, false))
        .await
        .unwrap();
    for round in 1..=3 {
        let (instance, record, duel) = completed(&db, user, avatar, "arena").await;
        let casts = server::advancement::casts(&record).unwrap();
        if round == 1 {
            for id in record.seats[0]
                .deck
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>()
            {
                sqlx::query("INSERT INTO card_mastery(user_id,card_id,xp) VALUES($1,$2,14)")
                    .bind(user)
                    .bind(id as i16)
                    .execute(&db)
                    .await
                    .unwrap();
            }
        }
        let reward = duel::result::settle(&db, instance, &record, &duel)
            .await
            .unwrap();
        assert_eq!(reward.xp[0], 120);
        let after = serde_json::to_value(state(&db, user).await).unwrap();
        duel::result::settle(&db, Uuid::nil(), &record, &duel)
            .await
            .unwrap();
        assert_eq!(serde_json::to_value(state(&db, user).await).unwrap(), after);
        let current = state(&db, user).await;
        assert_eq!(current.advancement.contracts[0].fragments, round * 2);
        assert_eq!(current.avatars[0].advancement.trial_wins, round);
        assert_eq!(current.advancement.daily_fights, round);
        for card in &current.advancement.mastery {
            assert_eq!(
                card.xp,
                14 + round
                    * if casts[0].contains(&(card.card_id as u16)) {
                        3
                    } else {
                        1
                    }
            );
        }
    }
    assert!(state(&db, user).await.advancement.daily_rewarded);
    let awards: i64 =
        sqlx::query_scalar("SELECT count(*) FROM ledger_transactions WHERE reason='daily_quest'")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(awards, 1);
    assert!(server::economy::reconcile(&db).await.unwrap().is_ok());
    let before = serde_json::to_value(state(&db, user).await).unwrap();
    let (instance, record, duel) = completed(&db, user, avatar, "training").await;
    duel::result::settle(&db, instance, &record, &duel)
        .await
        .unwrap();
    let after = serde_json::to_value(state(&db, user).await).unwrap();
    assert_eq!(after["advancement"], before["advancement"]);
    assert_eq!(after["avatars"], before["avatars"]);
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL"]
async fn incarnation_gates_account_trial_and_spends_refunds_permanent_pool(db: PgPool) {
    let f = Fixture::new(db.clone());
    f.register().await;
    let (user, id) = owner(&db).await;
    sqlx::query("UPDATE avatars SET level=20,xp=6000 WHERE id=$1")
        .bind(id)
        .execute(&db)
        .await
        .unwrap();
    let path = engine_content::PATHS
        .iter()
        .find(|p| p.base_avatar == 1)
        .unwrap();
    let make = || Mutation::Reincarnate {
        id,
        revision: 0,
        choice: server::progression::Choice::Path { id: path.avatar.id },
        sacrifice: None,
    };
    assert_eq!(
        server::game::mutate(&db, user, make())
            .await
            .err()
            .unwrap()
            .0,
        "LEVEL_REQUIRED"
    );
    xp(&db, user, 4700).await;
    assert_eq!(
        server::game::mutate(&db, user, make())
            .await
            .err()
            .unwrap()
            .0,
        "TRIAL_REQUIRED"
    );
    sqlx::query("INSERT INTO class_trials(avatar_id,tier,wins) VALUES($1,0,3)")
        .bind(id)
        .execute(&db)
        .await
        .unwrap();
    let silver = server::game::mutate(&db, user, make()).await.unwrap();
    let a = &silver.avatars[0];
    assert_eq!((a.level, a.xp, a.unspent), (1, 0, 3));
    let spent = server::game::mutate(
        &db,
        user,
        Mutation::Spend {
            id,
            revision: a.revision,
            points: [1, 0, 0, 0, 2, 0],
        },
    )
    .await
    .unwrap();
    assert_eq!(spent.avatars[0].advancement.permanent_unspent, 0);
    assert_eq!(spent.avatars[0].advancement.permanent_points, 3);
    assert_eq!(spent.avatars[0].stats[4], a.stats[4] + 14);
    assert_eq!(server::advancement::level_for_xp(8999, 1), 19);
    assert_eq!(server::advancement::level_for_xp(9000, 1), 20);
    assert_eq!(server::advancement::level_for_xp(11999, 2), 19);
    assert_eq!(server::advancement::level_for_xp(12000, 2), 20);
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL"]
async fn fifth_completed_arena_win_grants_one_released_class_and_replay_is_idempotent(db: PgPool) {
    let f = Fixture::new(db.clone());
    f.register().await;
    let (user, avatar) = owner(&db).await;
    xp(&db, user, 700).await;
    sqlx::query("INSERT INTO arena_progress(user_id,wins) VALUES($1,4)")
        .bind(user)
        .execute(&db)
        .await
        .unwrap();
    let (instance, record, duel) = completed(&db, user, avatar, "arena").await;
    let first = duel::result::settle(&db, instance, &record, &duel)
        .await
        .unwrap();
    let template = first.unlock[0].unwrap();
    assert_ne!(template, 1);
    assert!(engine_content::is_playable(template));
    let second = duel::result::settle(&db, Uuid::nil(), &record, &duel)
        .await
        .unwrap();
    assert_eq!(first.unlock, second.unlock);
    assert_eq!(state(&db, user).await.templates.len(), 2);
    let wins: i32 = sqlx::query_scalar("SELECT wins FROM arena_progress WHERE user_id=$1")
        .bind(user)
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(wins, 5);
}
