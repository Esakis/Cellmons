use super::*;
use server::admin::{
    control::{self, Operation},
    moderation, views,
};
impl Fixture {
    async fn authorized(&self) -> access::Identity {
        let id = self.identity("127.0.0.1:50000").await;
        let setup = access::begin_setup(&self.state, &id, PASSWORD.into())
            .await
            .unwrap();
        access::complete_factor(&self.state, &id, Uuid::now_v7(), true, None, code(&setup))
            .await
            .unwrap();
        id
    }
}
fn request(operation: Operation) -> control::Request {
    control::Request {
        request: Uuid::now_v7(),
        reason: "Evidence reviewed by test moderator".into(),
        operation,
    }
}
async fn player(db: &PgPool, n: u128) -> (Uuid, Uuid) {
    let user = Uuid::from_u128(10000 + n);
    let avatar = Uuid::from_u128(20000 + n);
    let mut tx = db.begin().await.unwrap();
    sqlx::query("INSERT INTO users(id,login,email,password_hash,start_school,email_verified_at) VALUES ($1,$2,$3,'$argon2id$v=19$m=19456,t=2,p=1$fixture$fixture','combat',now())").bind(user).bind(format!("Player{n}")).bind(format!("player{n}@example.test")).execute(&mut *tx).await.unwrap();
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
    sqlx::query("INSERT INTO decks(avatar_id,cards) VALUES ($1,$2)")
        .bind(avatar)
        .bind(
            engine_content::STARTERS[0]
                .deck
                .iter()
                .map(|c| *c as i16)
                .collect::<Vec<_>>(),
        )
        .execute(&mut *tx)
        .await
        .unwrap();
    for (card, qty) in engine_content::STARTERS[0].cards {
        sqlx::query("INSERT INTO collection(user_id,card_id,qty) VALUES ($1,$2,$3)")
            .bind(user)
            .bind(*card as i16)
            .bind(i32::from(*qty))
            .execute(&mut *tx)
            .await
            .unwrap();
    }
    server::economy::grant_initial(&mut tx, user).await.unwrap();
    tx.commit().await.unwrap();
    (user, avatar)
}
#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL"]
async fn moderation_preserves_an_existing_deletion_request(db: PgPool) {
    let f = Fixture::new(db.clone()).await;
    let identity = f.authorized().await;
    let (user, _) = player(&db, 1).await;
    sqlx::query("UPDATE users SET status='pending_deletion',deletion_previous_status='active',deletion_requested_at=now() WHERE id=$1")
        .bind(user).execute(&db).await.unwrap();
    for (operation, expected) in [
        (
            Operation::Ban {
                user,
                seconds: None,
            },
            "banned",
        ),
        (Operation::Unban { user }, "active"),
    ] {
        control::apply(&f.state, &identity, request(operation))
            .await
            .unwrap();
        let (status,previous,requested):(String,String,bool)=sqlx::query_as("SELECT status,deletion_previous_status,deletion_requested_at IS NOT NULL FROM users WHERE id=$1")
            .bind(user).fetch_one(&db).await.unwrap();
        assert_eq!(status, "pending_deletion");
        assert_eq!(previous, expected);
        assert!(requested);
    }
    f.state.online.hub.shutdown().await;
}
#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn concurrent_decisions_have_one_audit_and_retry_cannot_reapply_an_old_ban(db: PgPool) {
    let f = Fixture::new(db.clone()).await;
    let (user, _) = player(&db, 1).await;
    let id = f.identity("127.0.0.1:50000").await;
    assert_eq!(
        control::apply(&f.state, &id, request(Operation::Warn { user }))
            .await
            .err()
            .unwrap()
            .0,
        "STEP_UP_REQUIRED"
    );
    assert_eq!(
        views::load(&f.state, &id, views::Parameters::default())
            .await
            .err()
            .unwrap()
            .0,
        "STEP_UP_REQUIRED"
    );
    let id = f.authorized().await;
    let ban = request(Operation::Ban {
        user,
        seconds: Some(3600),
    });
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..100 {
        let state = f.state.clone();
        let id = id.clone();
        let req = ban.clone();
        tasks.spawn(async move { control::apply(&state, &id, req).await });
    }
    let mut applied = 0;
    while let Some(result) = tasks.join_next().await {
        applied += usize::from(result.unwrap().unwrap().applied);
    }
    assert_eq!(applied, 1);
    control::apply(&f.state, &id, request(Operation::Unban { user }))
        .await
        .unwrap();
    assert!(
        !control::apply(&f.state, &id, ban.clone())
            .await
            .unwrap()
            .applied
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM users WHERE id=$1")
            .bind(user)
            .fetch_one(&db)
            .await
            .unwrap(),
        "active"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM admin_audit WHERE action='ban'")
            .fetch_one(&db)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM user_notices WHERE user_id=$1")
            .bind(user)
            .fetch_one(&db)
            .await
            .unwrap(),
        2
    );
    let mut changed = ban;
    changed.reason = "Changed evidence must conflict".into();
    assert_eq!(
        control::apply(&f.state, &id, changed)
            .await
            .err()
            .unwrap()
            .0,
        "CONFLICT"
    );
    assert_eq!(
        control::apply(
            &f.state,
            &id,
            request(Operation::Ban {
                user: f.user,
                seconds: None
            })
        )
        .await
        .err()
        .unwrap()
        .0,
        "FORBIDDEN"
    );
    f.state.online.hub.shutdown().await;
}
#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn moderation_limits_filters_expiry_and_live_role_checks(db: PgPool) {
    let f = Fixture::new(db.clone()).await;
    let id = f.authorized().await;
    let (user, _) = player(&db, 2).await;
    control::apply(
        &f.state,
        &id,
        request(Operation::Filter {
            id: Uuid::now_v7(),
            phrase: "BAD Word".into(),
            enabled: true,
        }),
    )
    .await
    .unwrap();
    assert_eq!(
        moderation::can_speak(&db, user, "ＢＡＤ\u{200b} Word!")
            .await
            .unwrap_err()
            .0,
        "FILTERED"
    );
    assert!(
        moderation::can_speak(&db, user, "Not a bad wordsmith")
            .await
            .is_ok()
    );
    control::apply(
        &f.state,
        &id,
        request(Operation::Mute { user, seconds: 60 }),
    )
    .await
    .unwrap();
    assert_eq!(
        moderation::can_speak(&db, user, "Hello")
            .await
            .unwrap_err()
            .0,
        "MUTED"
    );
    sqlx::query("UPDATE users SET muted_until=now()-interval '1 second' WHERE id=$1")
        .bind(user)
        .execute(&db)
        .await
        .unwrap();
    assert!(moderation::can_speak(&db, user, "Hello").await.is_ok());
    control::apply(
        &f.state,
        &id,
        request(Operation::Ban {
            user,
            seconds: Some(60),
        }),
    )
    .await
    .unwrap();
    sqlx::query("UPDATE users SET banned_until=now()-interval '1 second' WHERE id=$1")
        .bind(user)
        .execute(&db)
        .await
        .unwrap();
    moderation::expire_ban(&db, None, Some("Player2"))
        .await
        .unwrap();
    assert!(moderation::can_speak(&db, user, "Hello").await.is_ok());
    sqlx::query("UPDATE users SET role='moderator' WHERE id=$1")
        .bind(f.user)
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(
        control::apply(
            &f.state,
            &id,
            request(Operation::Maintenance {
                enabled: true,
                notice: "Maintenance".into()
            })
        )
        .await
        .err()
        .unwrap()
        .0,
        "FORBIDDEN"
    );
    assert_eq!(
        control::apply(
            &f.state,
            &id,
            request(Operation::Ban {
                user,
                seconds: None
            })
        )
        .await
        .err()
        .unwrap()
        .0,
        "INVALID_MESSAGE"
    );
    assert_eq!(
        control::apply(
            &f.state,
            &id,
            request(Operation::Mute {
                user,
                seconds: 4 * 86400
            })
        )
        .await
        .err()
        .unwrap()
        .0,
        "INVALID_MESSAGE"
    );
    control::apply(&f.state, &id, request(Operation::Warn { user }))
        .await
        .unwrap();
    sqlx::query("UPDATE users SET role='player' WHERE id=$1")
        .bind(f.user)
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(
        control::apply(&f.state, &id, request(Operation::Warn { user }))
            .await
            .err()
            .unwrap()
            .0,
        "FORBIDDEN"
    );
    f.state.online.hub.shutdown().await;
}
#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn reports_capture_authorized_context_and_decisions_notify_only_the_reporter(db: PgPool) {
    let f = Fixture::new(db.clone()).await;
    let id = f.authorized().await;
    let (a, _) = player(&db, 3).await;
    let (b, _) = player(&db, 4).await;
    let (outsider, _) = player(&db, 5).await;
    let chat = Uuid::now_v7();
    sqlx::query("INSERT INTO chat_log(id,room,from_user,to_user,text) VALUES ($1,'',$2,$3,'Private report evidence')").bind(chat).bind(a).bind(b).execute(&db).await.unwrap();
    let report = moderation::Report {
        id: Uuid::now_v7(),
        kind: "chat".into(),
        target: chat,
        reason: "Offensive private message".into(),
    };
    assert_eq!(
        moderation::report(&db, outsider, report.clone())
            .await
            .err()
            .unwrap()
            .0,
        "NOT_FOUND"
    );
    for _ in 0..3 {
        assert_eq!(
            moderation::report(&db, b, report.clone()).await.unwrap(),
            report.id
        );
    }
    let rows = views::load(
        &f.state,
        &id,
        views::Parameters {
            category: "reports".into(),
            ..Default::default()
        },
    )
    .await
    .unwrap()
    .rows;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["context"]["text"], "Private report evidence");
    assert!(
        control::apply(
            &f.state,
            &id,
            request(Operation::Resolve {
                report: report.id,
                accepted: true
            })
        )
        .await
        .unwrap()
        .applied
    );
    assert_eq!(
        sqlx::query_scalar::<_, Uuid>("SELECT user_id FROM user_notices WHERE kind='report'")
            .fetch_one(&db)
            .await
            .unwrap(),
        b
    );
    for category in [
        "players",
        "chat",
        "audit",
        "filters",
        "announcements",
        "signals",
        "clans",
        "control",
    ] {
        views::load(
            &f.state,
            &id,
            views::Parameters {
                category: category.into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    }
    f.state.online.hub.shutdown().await;
}
#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn maintenance_and_owner_console_have_no_http_privilege_bootstrap(db: PgPool) {
    let f = Fixture::new(db.clone()).await;
    let id = f.authorized().await;
    let (user, _) = player(&db, 6).await;
    control::apply(
        &f.state,
        &id,
        request(Operation::Maintenance {
            enabled: true,
            notice: "Scheduled test maintenance".into(),
        }),
    )
    .await
    .unwrap();
    let mut connection = db.acquire().await.unwrap();
    assert_eq!(
        server::activity::ensure_idle(&mut connection, user)
            .await
            .unwrap_err()
            .0,
        "MAINTENANCE"
    );
    control::apply(
        &f.state,
        &id,
        request(Operation::Maintenance {
            enabled: false,
            notice: "".into(),
        }),
    )
    .await
    .unwrap();
    assert!(
        server::activity::ensure_idle(&mut connection, user)
            .await
            .is_ok()
    );
    drop(connection);
    assert_eq!(
        control::apply(
            &f.state,
            &id,
            request(Operation::Role {
                user,
                role: "admin".into()
            })
        )
        .await
        .err()
        .unwrap()
        .0,
        "INVALID_MESSAGE"
    );
    server::admin::console::run(
        &db,
        &["role", "Player6", "mentor", "Owner test promotion"].map(String::from),
    )
    .await
    .unwrap();
    assert_eq!(
        server::admin::console::run(
            &db,
            &["role", "StaffOwner", "player", "Owner test downgrade"].map(String::from)
        )
        .await
        .unwrap_err()
        .0,
        "LAST_ADMIN"
    );
    server::admin::console::run(
        &db,
        &[
            "reset-factor",
            "StaffOwner",
            "confirm",
            "Lost test factor only",
        ]
        .map(String::from),
    )
    .await
    .unwrap();
    assert!(!access::gate(&f.state, &id).await.unwrap().authorized);
    assert_eq!(server::admin::signals::scan(&db).await.unwrap(), 0);
    f.state.online.hub.shutdown().await;
}
#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn all_system_prize_types_settle_once_and_cancelled_cash_reconciles(db: PgPool) {
    use server::{
        duel,
        tournaments::{self, Reward, Terms},
    };
    let f = Fixture::new(db.clone()).await;
    let access = f.authorized().await;
    let (a, aa) = player(&db, 7).await;
    let (b, ba) = player(&db, 8).await;
    let terms = Terms {
        teams: 2,
        team_size: 1,
        fee: 0,
        max_games: 1,
        min_level: 1,
        max_level: 20,
        tiers: vec![0, 1, 2, 3],
        only_clan: None,
        allowed_clans: vec![],
    };
    for (index, reward) in [
        Reward::Ducats { amount: 100 },
        Reward::Avatar { template: 32 },
        Reward::Card { card: 330 },
        Reward::Booster {
            booster: "basic".into(),
        },
    ]
    .into_iter()
    .enumerate()
    {
        sqlx::query("UPDATE users SET role=CASE WHEN id=$1 AND $3=0 OR id=$2 AND $3=1 THEN 'mentor' ELSE 'player' END WHERE id IN ($1,$2)").bind(a).bind(b).bind(index as i32).execute(&db).await.unwrap();
        let id = Uuid::now_v7();
        let creation = request(Operation::SystemTournament {
            id,
            name: "System prize fixture".into(),
            terms: terms.clone(),
            reward: reward.clone(),
        });
        control::apply(&f.state, &access, creation.clone())
            .await
            .unwrap();
        assert!(
            !control::apply(&f.state, &access, creation)
                .await
                .unwrap()
                .applied
        );
        for (user, avatar) in [(a, aa), (b, ba)] {
            tournaments::join(&db, user, id, Uuid::now_v7(), avatar)
                .await
                .unwrap();
        }
        assert_eq!(
            tournaments::cancel(&db, f.user, id, Uuid::now_v7())
                .await
                .unwrap_err()
                .0,
            "FORBIDDEN"
        );
        let instance = Uuid::now_v7();
        let settings = duel::Settings::default();
        let now = server::online::now_ms();
        tournaments::tick(&db, id, instance, &settings, now)
            .await
            .unwrap();
        sqlx::query("UPDATE tournament_entries SET ready_since_ms=$3,ready_until_ms=$3+200000 WHERE tournament_id=$1 AND user_id=$2").bind(id).bind(a).bind(now).execute(&db).await.unwrap();
        let work = tournaments::tick(&db, id, instance, &settings, now + 111000)
            .await
            .unwrap();
        assert_eq!(work.duels.len(), 1);
        let record = &work.duels[0];
        let (state, _) = record.rebuild().unwrap();
        let rewards = duel::result::settle(&db, instance, record, &state)
            .await
            .unwrap();
        if index == 0 {
            assert_eq!(rewards.xp, [0, 0]);
            assert_eq!(rewards.delta, [0, 0]);
        } else {
            // The absent opponent loses rating, but a walkover grants no played-match XP.
            assert_eq!(rewards.xp, [0, 0]);
            assert!(rewards.delta.iter().any(|d| *d > 0));
        }
        tournaments::tick(&db, id, instance, &settings, now + 111001)
            .await
            .unwrap();
        tournaments::tick(&db, id, instance, &settings, now + 111002)
            .await
            .unwrap();
        let winner: Uuid = sqlx::query_scalar(
            "SELECT user_id FROM tournament_entries WHERE tournament_id=$1 AND status='champion'",
        )
        .bind(id)
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(winner, a);
        match reward {
            Reward::Ducats { .. } => assert_eq!(
                sqlx::query_scalar::<_, i64>("SELECT balance FROM wallets WHERE user_id=$1")
                    .bind(a)
                    .fetch_one(&db)
                    .await
                    .unwrap(),
                140
            ),
            Reward::Avatar { .. } => assert_eq!(
                sqlx::query_scalar::<_, i64>(
                    "SELECT count(*) FROM unlocked_templates WHERE user_id=$1 AND template_id=32"
                )
                .bind(a)
                .fetch_one(&db)
                .await
                .unwrap(),
                1
            ),
            Reward::Card { .. } => assert_eq!(
                sqlx::query_scalar::<_, i32>(
                    "SELECT qty FROM collection WHERE user_id=$1 AND card_id=330"
                )
                .bind(a)
                .fetch_one(&db)
                .await
                .unwrap(),
                1
            ),
            Reward::Booster { .. } => {
                let (price, source, seed, pools): (i64, String, Vec<u8>, Vec<u8>) =
                    sqlx::query_as("SELECT price,source,seed,pools FROM boosters WHERE user_id=$1")
                        .bind(a)
                        .fetch_one(&db)
                        .await
                        .unwrap();
                assert_eq!((price, source.as_str(), seed.len()), (0, "tournament", 32));
                assert!(!pools.is_empty());
            }
        }
        assert!(server::economy::reconcile(&db).await.unwrap().is_ok());
    }
    let expired = Uuid::now_v7();
    control::apply(
        &f.state,
        &access,
        request(Operation::SystemTournament {
            id: expired,
            name: "Expires and burns escrow".into(),
            terms: terms.clone(),
            reward: Reward::Ducats { amount: 100 },
        }),
    )
    .await
    .unwrap();
    tournaments::tick(
        &db,
        expired,
        Uuid::now_v7(),
        &duel::Settings::default(),
        server::online::now_ms() + 86401000,
    )
    .await
    .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT balance FROM tournaments WHERE id=$1")
            .bind(expired)
            .fetch_one(&db)
            .await
            .unwrap(),
        0
    );
    assert!(server::economy::reconcile(&db).await.unwrap().is_ok());
    let cancelled = Uuid::now_v7();
    control::apply(
        &f.state,
        &access,
        request(Operation::SystemTournament {
            id: cancelled,
            name: "Owner cancels before start".into(),
            terms,
            reward: Reward::Ducats { amount: 100 },
        }),
    )
    .await
    .unwrap();
    tournaments::join(&db, a, cancelled, Uuid::now_v7(), aa)
        .await
        .unwrap();
    let cancel = request(Operation::SystemCancel { id: cancelled });
    assert!(
        control::apply(&f.state, &access, cancel.clone())
            .await
            .unwrap()
            .applied
    );
    assert!(
        !control::apply(&f.state, &access, cancel)
            .await
            .unwrap()
            .applied
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT balance FROM tournaments WHERE id=$1")
            .bind(cancelled)
            .fetch_one(&db)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM activity_locks WHERE user_id=$1")
            .bind(a)
            .fetch_one(&db)
            .await
            .unwrap(),
        0
    );
    assert!(server::economy::reconcile(&db).await.unwrap().is_ok());
    f.state.online.hub.shutdown().await;
}
#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn abuse_signals_are_repeatable_private_review_leads_without_automatic_sanctions(db: PgPool) {
    let mut players = vec![];
    for n in 10..14 {
        let (user, _) = player(&db, n).await;
        players.push(user);
        sqlx::query("INSERT INTO sessions(id_hash,user_id,kind,ip_hash,expires_at) VALUES ($1,$2,'web',$3,now()+interval '1 day')").bind(vec![n as u8;32]).bind(user).bind(vec![95_u8;32]).execute(&db).await.unwrap();
    }
    let mut tx = db.begin().await.unwrap();
    sqlx::query("SELECT id FROM users WHERE id=$1 FOR UPDATE")
        .bind(players[0])
        .execute(&mut *tx)
        .await
        .unwrap();
    server::economy::transfer(
        &mut tx,
        server::economy::Account::Faucet,
        server::economy::Account::Player(players[0]),
        15000,
        "admin",
        Uuid::now_v7(),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(server::admin::signals::scan(&db).await.unwrap(), 5);
    sqlx::query("UPDATE abuse_signals SET reviewed=true")
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(server::admin::signals::scan(&db).await.unwrap(), 5);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM abuse_signals WHERE reviewed")
            .fetch_one(&db)
            .await
            .unwrap(),
        5
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM users WHERE status='active'")
            .fetch_one(&db)
            .await
            .unwrap(),
        4
    );
    let evidence: Vec<Vec<u8>> = sqlx::query_scalar("SELECT details FROM abuse_signals")
        .fetch_all(&db)
        .await
        .unwrap();
    for bytes in evidence {
        let evidence = String::from_utf8(bytes).unwrap();
        assert!(!evidence.contains("ip_hash"));
        assert!(!evidence.contains("5f5f5f5f"));
    }
    assert!(server::economy::reconcile(&db).await.unwrap().is_ok());
}
#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn http_requires_step_up_and_banned_credentials_only_read_their_own_notices(db: PgPool) {
    let f = Fixture::new(db.clone()).await;
    assert_eq!(
        f.call_at("/api/admin", "GET", Value::Null, "127.0.0.1:50000", None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let payload = serde_json::to_value(request(Operation::Warn { user: f.user })).unwrap();
    let (status, body) = f
        .call_at("/api/admin", "POST", payload, "127.0.0.1:50000", None)
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"], "STEP_UP_REQUIRED");
    let id = f.authorized().await;
    let (status, body) = f
        .call_at(
            "/api/admin?category=players",
            "GET",
            Value::Null,
            "127.0.0.1:50000",
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["rows"][0]["login"], "StaffOwner");
    assert!(!body.to_string().contains("password_hash"));
    assert!(!body.to_string().contains("secret"));
    let (other, _) = player(&db, 15).await;
    control::apply(&f.state, &id, request(Operation::Warn { user: other }))
        .await
        .unwrap();
    sqlx::query("INSERT INTO user_notices(id,user_id,kind,body) VALUES ($1,$2,'ban','Reason visible only to this account')").bind(Uuid::now_v7()).bind(f.user).execute(&db).await.unwrap();
    sqlx::query("UPDATE users SET status='banned',banned_until=now()+interval '1 day' WHERE id=$1")
        .bind(f.user)
        .execute(&db)
        .await
        .unwrap();
    let (status, body) = f
        .call_at("/api/notices", "GET", Value::Null, "127.0.0.1:50000", None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["notices"].as_array().unwrap().len(), 1);
    assert_eq!(
        body["notices"][0]["body"],
        "Reason visible only to this account"
    );
    assert_eq!(
        f.call_at("/api/me", "GET", Value::Null, "127.0.0.1:50000", None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        f.call_at("/api/admin", "GET", Value::Null, "127.0.0.1:50000", None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    f.state.online.hub.shutdown().await;
}
#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn native_token_rotation_revokes_step_up_without_a_foreign_key_failure(db: PgPool) {
    let mut f = Fixture::new(db.clone()).await;
    sqlx::query("UPDATE sessions SET kind='app' WHERE user_id=$1")
        .bind(f.user)
        .execute(&db)
        .await
        .unwrap();
    f.headers.insert(
        "authorization",
        format!("Bearer {}", "11".repeat(32)).parse().unwrap(),
    );
    f.headers
        .insert("x-native-version", "1.0.0".parse().unwrap());
    let id = f.authorized().await;
    assert!(access::gate(&f.state, &id).await.unwrap().authorized);
    let (status, body) = f
        .call_at(
            "/api/auth/rotate",
            "POST",
            serde_json::json!({}),
            "127.0.0.1:50000",
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let token = body["token"].as_str().unwrap();
    assert_eq!(token.len(), 64);
    assert_ne!(token, "11".repeat(32));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM admin_sessions")
            .fetch_one(&db)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        access::gate(&f.state, &id).await.err().unwrap().0,
        "FORBIDDEN"
    );
    f.headers
        .insert("authorization", format!("Bearer {token}").parse().unwrap());
    let new = f.identity("127.0.0.1:50000").await;
    assert!(!access::gate(&f.state, &new).await.unwrap().authorized);
    assert!(access::gate(&f.state, &new).await.unwrap().enrolled);
    f.state.online.hub.shutdown().await;
}
