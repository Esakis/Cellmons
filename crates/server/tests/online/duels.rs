use super::*;

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL"]
async fn deployment_drain_keeps_live_fight_and_replay_intact(pool: PgPool) {
    let a = player(&pool, 1, true).await;
    let b = player(&pool, 2, true).await;
    let old = Server::new(pool.clone(), Settings::default()).await;
    let mut one = old.connect(&a, None).await;
    let mut two = old.connect(&b, None).await;
    let id = begin(&mut one, &mut two, &b.login).await;
    server::operations::drain(&pool, true).await.unwrap();
    let status = server::operations::status(&pool).await.unwrap();
    assert!(status.draining && !status.idle());
    assert_eq!(status.duels, 1);
    finish(&mut one, &mut two, &id).await;
    one.event(|body| matches!(body, Body::DuelEnd(_))).await;
    assert!(server::operations::status(&pool).await.unwrap().idle());
    drop(one);
    drop(two);
    old.stop().await;
    let replacement = Server::new(pool.clone(), Settings::default()).await;
    let mut one = replacement.connect(&a, None).await;
    assert!(snapshot(&mut one, &id).await.result.is_some());
    assert!(matches!(
        one.call(Client::MatchReplay(pb::MatchReplay { match_id: id }))
            .await,
        Body::MatchReplayData(_)
    ));
    server::operations::drain(&pool, false).await.unwrap();
    drop(one);
    replacement.stop().await;
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL"]
async fn blocked_journal_write_does_not_stall_the_lobby(pool: PgPool) {
    let a = player(&pool, 1, true).await;
    let b = player(&pool, 2, true).await;
    let server = Server::new(pool.clone(), Settings::default()).await;
    let mut one = server.connect(&a, None).await;
    let mut two = server.connect(&b, None).await;
    let id = begin(&mut one, &mut two, &b.login).await;
    let view = snapshot(&mut one, &id).await.view.unwrap();
    let mut lock = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM duel_journal WHERE id=$1 FOR UPDATE")
        .bind(Uuid::parse_str(&id).unwrap())
        .execute(&mut *lock)
        .await
        .unwrap();
    let request = one.send(submit(&id, &view)).await;
    assert!(
        tokio::time::timeout(Duration::from_millis(50), one.reply(request))
            .await
            .is_err()
    );
    let pong = tokio::time::timeout(
        Duration::from_millis(250),
        two.call(Client::Ping(pb::Ping {})),
    )
    .await
    .expect("unrelated lobby requests must remain responsive");
    assert!(matches!(pong, Body::Pong(_)));
    lock.rollback().await.unwrap();
    assert!(matches!(
        one.reply(request).await,
        Body::DuelMoveAccepted(_)
    ));
    drop(one);
    drop(two);
    server.stop().await;
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn incompatible_workers_do_not_claim_or_extend_other_releases(pool: PgPool) {
    let a = player(&pool, 1, true).await;
    let b = player(&pool, 2, true).await;
    let server = Server::new(pool.clone(), Settings::default()).await;
    let mut one = server.connect(&a, None).await;
    let mut two = server.connect(&b, None).await;
    let id = Uuid::parse_str(&begin(&mut one, &mut two, &b.login).await).unwrap();
    drop(one);
    drop(two);
    server.stop().await;
    let previous = Uuid::now_v7();
    let candidate = Uuid::now_v7();
    for (engine, content) in [
        ("retained-engine", engine_content::CONTENT_VERSION.to_vec()),
        (engine::VERSION, vec![0; 32]),
    ] {
        sqlx::query("UPDATE duel_journal SET engine_version=$2,content_version=$3,owner_instance=$4,lease_until=now()-interval '1 second' WHERE id=$1")
            .bind(id).bind(engine).bind(content).bind(previous).execute(&pool).await.unwrap();
        assert!(
            server::duel::journal::recover(&pool, candidate, 30000)
                .await
                .unwrap()
                .is_empty()
        );
        let (owner, expired) = sqlx::query_as::<_, (Uuid, bool)>(
            "SELECT owner_instance,lease_until<=clock_timestamp() FROM duel_journal WHERE id=$1",
        )
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(owner, previous);
        assert!(expired);
    }
    sqlx::query("UPDATE duel_journal SET engine_version=$2,content_version=$3 WHERE id=$1")
        .bind(id)
        .bind(engine::VERSION)
        .bind(&engine_content::CONTENT_VERSION[..])
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        server::duel::journal::recover(&pool, candidate, 30000)
            .await
            .unwrap()
            .len(),
        1
    );
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn f10_shade_experience_is_ascent_and_last_level_grants_the_twenty_first_point(pool: PgPool) {
    let a = player(&pool, 1, true).await;
    let b = player(&pool, 2, true).await;
    sqlx::query("UPDATE avatars SET template_id=29,is_shade=true,shade_school='arcane',level=19,xp=49950,dev_points_unspent=19 WHERE user_id=$1").bind(a.id).execute(&pool).await.unwrap();
    sqlx::query("UPDATE avatars SET level=19,xp=45000 WHERE user_id=$1")
        .bind(b.id)
        .execute(&pool)
        .await
        .unwrap();
    let server = Server::new(pool.clone(), Settings::default()).await;
    let mut one = server.connect(&a, None).await;
    let mut two = server.connect(&b, None).await;
    let id = begin(&mut one, &mut two, &b.login).await;
    assert_ok(
        two.call(Client::DuelForfeit(pb::DuelTarget { duel_id: id }))
            .await,
    );
    let Body::DuelEnd(end) = one.event(|b| matches!(b, Body::DuelEnd(_))).await else {
        panic!()
    };
    assert_eq!(end.xp_gained, 61); // 100 × 1900 / (19 × 130 + 600), not 100.
    let progress = sqlx::query_as::<_, (i32, i16, i16)>(
        "SELECT xp,level,dev_points_unspent FROM avatars WHERE user_id=$1",
    )
    .bind(a.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(progress, (50011, 20, 21));
    let Body::DuelEnd(lost) = two.event(|b| matches!(b, Body::DuelEnd(_))).await else {
        panic!()
    };
    assert_eq!(lost.xp_gained, 0);
    let id = begin(&mut one, &mut two, &b.login).await;
    assert_ok(
        one.call(Client::DuelForfeit(pb::DuelTarget { duel_id: id }))
            .await,
    );
    let Body::DuelEnd(end) = two.event(|b| matches!(b, Body::DuelEnd(_))).await else {
        panic!()
    };
    assert_eq!(end.xp_gained, 168); // The winner also receives the shade opponent's Ascent valuation.
    drop(one);
    drop(two);
    server.stop().await;
}

async fn begin(one: &mut ClientSocket, two: &mut ClientSocket, name: &str) -> String {
    one.join("arena").await;
    two.join("arena").await;
    let Body::ChallengeReceived(offer) = one
        .call(Client::ChallengeSend(pb::ChallengeSend {
            target: name.into(),
        }))
        .await
    else {
        panic!("offer")
    };
    let Body::ChallengeUpdate(update) = two
        .call(Client::ChallengeRespond(pb::ChallengeRespond {
            challenge_id: offer.challenge_id,
            accept: true,
        }))
        .await
    else {
        panic!("accept")
    };
    update.duel_id
}
async fn snapshot(client: &mut ClientSocket, id: &str) -> pb::DuelSnapshot {
    match client
        .call(Client::DuelSnapshot(pb::DuelTarget { duel_id: id.into() }))
        .await
    {
        Body::DuelSnapshot(s) => *s,
        other => panic!("snapshot: {other:?}"),
    }
}
fn submit(id: &str, view: &pb::TrainingView) -> Client {
    let me = view.me.as_ref().unwrap();
    Client::DuelSubmitMove(pb::DuelSubmitMove {
        duel_id: id.into(),
        round: view.round,
        r#move: Some(pb::DuelMove {
            attacks: me.attacks.clone(),
            defenses: me.defenses.clone(),
            ..Default::default()
        }),
    })
}
async fn finish(one: &mut ClientSocket, two: &mut ClientSocket, id: &str) {
    for _ in 0..6 {
        let a = snapshot(one, id).await;
        let b = snapshot(two, id).await;
        let av = a.view.as_ref().unwrap();
        let bv = b.view.as_ref().unwrap();
        assert_eq!(av.me, bv.opponent);
        assert_eq!(av.opponent, bv.me);
        assert_eq!(a.state_hash, b.state_hash);
        assert_eq!(a.seed_commitment, b.seed_commitment);
        if av.outcome.is_some() {
            return;
        }
        if !a.submitted {
            assert!(matches!(
                one.call(submit(id, av)).await,
                Body::DuelMoveAccepted(_)
            ));
        }
        if !b.submitted {
            assert!(matches!(
                two.call(submit(id, bv)).await,
                Body::DuelMoveAccepted(_)
            ));
        }
    }
    panic!("duel did not finish")
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn f7_full_fight_private_views_results_history_and_replay(pool: PgPool) {
    let a = player(&pool, 1, true).await;
    let b = player(&pool, 2, true).await;
    let c = player(&pool, 3, true).await;
    let server = Server::new(pool.clone(), Settings::default()).await;
    let mut one = server.connect(&a, None).await;
    let mut two = server.connect(&b, None).await;
    let mut stranger = server.connect(&c, None).await;
    let id = begin(&mut one, &mut two, &b.login).await;
    assert_error(
        stranger
            .call(Client::DuelSnapshot(pb::DuelTarget {
                duel_id: id.clone(),
            }))
            .await,
        "FORBIDDEN",
    );
    assert_error(
        stranger
            .call(Client::MatchReplay(pb::MatchReplay {
                match_id: id.clone(),
            }))
            .await,
        "NOT_FOUND",
    );
    let current = snapshot(&mut one, &id).await;
    assert_eq!(current.view.as_ref().unwrap().hand.len(), 5);
    let mut invalid = submit(&id, current.view.as_ref().unwrap());
    if let Client::DuelSubmitMove(ref mut mv) = invalid {
        mv.round = 99;
    }
    assert_error(one.call(invalid).await, "CONFLICT");
    let first_move = submit(&id, current.view.as_ref().unwrap());
    assert!(matches!(
        one.call(first_move.clone()).await,
        Body::DuelMoveAccepted(_)
    ));
    assert_error(one.call(first_move).await, "CONFLICT");
    let pending = snapshot(&mut two, &id).await;
    assert!(pending.opponent_ready);
    assert!(!pending.submitted);
    assert!(
        pending
            .events
            .iter()
            .all(|event| event.kind != "draw" || event.side == 0)
    );
    finish(&mut one, &mut two, &id).await;
    let Body::DuelEnd(end) = one.event(|b| matches!(b, Body::DuelEnd(_))).await else {
        panic!()
    };
    assert_eq!(end.seed.len(), 32);
    assert_eq!(
        Sha256::digest(&end.seed).as_slice(),
        current.seed_commitment
    );
    assert_eq!(end.rating_delta, 0);
    assert_eq!(end.xp_gained, 50);
    let Body::MatchHistory(history) = one
        .call(Client::MatchHistoryGet(pb::MatchHistoryGet {
            limit: 10,
            ..Default::default()
        }))
        .await
    else {
        panic!()
    };
    assert_eq!(history.matches.len(), 1);
    assert_eq!(history.matches[0].id, id);
    let Body::MatchReplayData(replay) = stranger
        .call(Client::MatchReplay(pb::MatchReplay {
            match_id: id.clone(),
        }))
        .await
    else {
        panic!()
    };
    assert_eq!(replay.rounds.len(), 6);
    assert!(
        replay
            .rounds
            .last()
            .unwrap()
            .view
            .as_ref()
            .unwrap()
            .outcome
            .is_some()
    );
    let Body::RankingSnapshot(rank) = one
        .call(Client::RankingGet(pb::RankingGet {
            limit: 50,
            ..Default::default()
        }))
        .await
    else {
        panic!()
    };
    assert_eq!(rank.players.len(), 3);
    assert!(rank.players.iter().all(|p| p.verified));
    let games = sqlx::query_scalar::<_, i32>("SELECT games FROM ratings WHERE user_id=$1")
        .bind(a.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(games, 1);
    drop(one);
    drop(two);
    drop(stranger);
    server.stop().await;
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn f7_forfeit_exactly_once_and_historical_reward(pool: PgPool) {
    let a = player(&pool, 1, true).await;
    let b = player(&pool, 2, true).await;
    let server = Server::new(pool.clone(), Settings::default()).await;
    let mut one = server.connect(&a, None).await;
    let mut two = server.connect(&b, None).await;
    let id = begin(&mut one, &mut two, &b.login).await;
    assert_ok(
        one.call(Client::DuelForfeit(pb::DuelTarget {
            duel_id: id.clone(),
        }))
        .await,
    );
    let Body::DuelEnd(end) = one.event(|b| matches!(b, Body::DuelEnd(_))).await else {
        panic!()
    };
    assert_eq!(
        (end.rating_new, end.rating_delta, end.xp_gained),
        (990, -10, 0)
    );
    let record = server::duel::journal::load(&pool, Uuid::parse_str(&id).unwrap())
        .await
        .unwrap();
    let (state, _) = record.rebuild().unwrap();
    sqlx::query("UPDATE ratings SET rating=1234 WHERE user_id=$1")
        .bind(a.id)
        .execute(&pool)
        .await
        .unwrap();
    for _ in 0..2 {
        let reward = server::duel::result::settle(&pool, Uuid::nil(), &record, &state)
            .await
            .unwrap();
        assert_eq!(reward.rating, [990, 1010]);
        assert_eq!(reward.xp, [0, 100]);
    }
    assert_eq!(
        sqlx::query_scalar::<_, i32>("SELECT games FROM ratings WHERE user_id=$1")
            .bind(a.id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM duel_locks")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    drop(one);
    drop(two);
    server.stop().await;
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn f7_timeouts_and_training_do_not_reward_or_change_statistics(pool: PgPool) {
    let a = player(&pool, 1, true).await;
    let b = player(&pool, 2, true).await;
    let server = Server::new(
        pool.clone(),
        Settings {
            duels: server::duel::Settings {
                round: Duration::from_millis(500),
                after_opponent: Duration::from_millis(500),
                tick: Duration::from_millis(10),
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .await;
    let mut one = server.connect(&a, None).await;
    let mut two = server.connect(&b, None).await;
    let id = begin(&mut one, &mut two, &b.login).await;
    let Body::DuelEnd(end) = one.event(|b| matches!(b, Body::DuelEnd(_))).await else {
        panic!()
    };
    assert_eq!(end.outcome.unwrap().reason, "timeout");
    let record = server::duel::journal::load(&pool, Uuid::parse_str(&id).unwrap())
        .await
        .unwrap();
    assert_eq!(record.moves.len(), 2);
    // Wait for lobby cleanup as well as the persisted result.
    one.event(|b| matches!(b,Body::ChallengeUpdate(c) if c.status=="finished"))
        .await;
    one.join("training").await;
    let Body::TrainingBots(bots) = one.call(Client::TrainingBots(pb::Empty {})).await else {
        panic!()
    };
    assert_eq!(bots.bots.len(), 8);
    assert!(bots.bots.iter().all(|b| b.login.starts_with("[AI]")));
    let Body::DuelSnapshot(training) = one
        .call(Client::TrainingChallengeBot(pb::TrainingChallengeBot {
            bot_id: bots.bots[0].id.clone(),
        }))
        .await
    else {
        panic!()
    };
    let id = training.duel_id;
    one.event(|b| matches!(b,Body::DuelSnapshot(s) if s.duel_id==id && s.opponent_ready))
        .await;
    assert_ok(
        one.call(Client::DuelForfeit(pb::DuelTarget {
            duel_id: id.clone(),
        }))
        .await,
    );
    let Body::DuelEnd(end) = one
        .event(|b| matches!(b,Body::DuelEnd(e) if e.duel_id==id))
        .await
    else {
        panic!()
    };
    assert_eq!((end.xp_gained, end.rating_delta), (0, 0));
    assert_eq!(
        sqlx::query_scalar::<_, i32>("SELECT games FROM ratings WHERE user_id=$1")
            .bind(a.id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    drop(one);
    drop(two);
    server.stop().await;
}

struct Process(std::process::Child);
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
async fn process(pool: &PgPool, addr: SocketAddr) -> Process {
    let options = pool.connect_options();
    let mut url =
        reqwest::Url::parse(&std::env::var("DATABASE_URL").expect("test database URL")).unwrap();
    url.set_path(options.get_database().unwrap());
    let child = std::process::Command::new(env!("CARGO_BIN_EXE_server"))
        .env("DATABASE_URL", url.as_str())
        .env("LISTEN_ADDR", addr.to_string())
        .env("PUBLIC_ORIGIN", "http://localhost:5173")
        .env("MAIL_MODE", "file")
        .env("DB_POOL_SIZE", "4")
        .env("LOG_LEVEL", "off")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let mut child = Process(child);
    for _ in 0..100 {
        assert!(child.0.try_wait().unwrap().is_none(), "server exited");
        if reqwest::get(format!("http://{addr}/readyz"))
            .await
            .is_ok_and(|r| r.status().is_success())
        {
            return child;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("server startup deadline")
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn f7_chaos_kill_process_after_move_commit_then_recover_and_finish(pool: PgPool) {
    let a = player(&pool, 1, true).await;
    let b = player(&pool, 2, true).await;
    let address = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    let child = process(&pool, address).await;
    // Only use Server's connection helper; its independent hub owns no fight.
    let mut proxy = Server::new(
        pool.clone(),
        Settings {
            run_duels: false,
            ..Default::default()
        },
    )
    .await;
    proxy.addr = address;
    let mut one = proxy.connect(&a, None).await;
    let mut two = proxy.connect(&b, None).await;
    let id = begin(&mut one, &mut two, &b.login).await;
    let opening = snapshot(&mut one, &id).await;
    assert!(matches!(
        one.call(submit(&id, opening.view.as_ref().unwrap())).await,
        Body::DuelMoveAccepted(_)
    ));
    let opening = snapshot(&mut two, &id).await;
    assert!(matches!(
        two.call(submit(&id, opening.view.as_ref().unwrap())).await,
        Body::DuelMoveAccepted(_)
    ));
    let initial = snapshot(&mut one, &id).await;
    let request = submit(&id, initial.view.as_ref().unwrap());
    assert!(matches!(
        one.call(request.clone()).await,
        Body::DuelMoveAccepted(_)
    ));
    drop(child);
    drop(one);
    drop(two);
    let child = process(&pool, address).await;
    // The former owner's lease must expire before the replacement can act.
    for _ in 0..500 {
        let owner=sqlx::query_scalar::<_,bool>("SELECT lease_until>clock_timestamp() AND owner_instance<>$2 FROM duel_journal WHERE id=$1").bind(Uuid::parse_str(&id).unwrap()).bind(Uuid::nil()).fetch_one(&pool).await.unwrap();
        let record = server::duel::journal::load(&pool, Uuid::parse_str(&id).unwrap())
            .await
            .unwrap();
        if owner && !record.connected[0] {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let mut one = proxy.connect(&a, None).await;
    let mut two = proxy.connect(&b, None).await;
    let restored = snapshot(&mut one, &id).await;
    assert!(restored.submitted);
    assert_eq!(restored.view, initial.view);
    assert_eq!(restored.state_hash, initial.state_hash);
    assert_error(one.call(request).await, "CONFLICT");
    finish(&mut one, &mut two, &id).await;
    one.event(|b| matches!(b, Body::DuelEnd(_))).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM matches WHERE id=$1")
            .bind(Uuid::parse_str(&id).unwrap())
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    drop(one);
    drop(two);
    drop(child);
    proxy.stop().await;
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn f7_reconnect_preserves_move_and_expired_grace_loses(pool: PgPool) {
    let a = player(&pool, 1, true).await;
    let b = player(&pool, 2, true).await;
    let server = Server::new(
        pool.clone(),
        Settings {
            duels: server::duel::Settings {
                reconnect: Duration::from_millis(800),
                tick: Duration::from_millis(10),
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .await;
    let mut one = server.connect(&a, None).await;
    let mut two = server.connect(&b, None).await;
    let id = begin(&mut one, &mut two, &b.login).await;
    let view = snapshot(&mut one, &id).await;
    let request = submit(&id, view.view.as_ref().unwrap());
    assert!(matches!(one.call(request).await, Body::DuelMoveAccepted(_)));
    let token = one.resume.clone();
    drop(one);
    two.event(|body| matches!(body,Body::DuelSnapshot(s) if !s.opponent_connected))
        .await;
    let mut resumed = server.connect(&a, Some(token)).await;
    let view = snapshot(&mut resumed, &id).await;
    assert!(view.submitted);
    assert!(view.your_move.is_some());
    two.event(|body| matches!(body,Body::DuelSnapshot(s) if s.opponent_connected))
        .await;
    drop(resumed);
    let Body::DuelEnd(end) = two.event(|body| matches!(body, Body::DuelEnd(_))).await else {
        panic!()
    };
    assert_eq!(end.outcome.as_ref().unwrap().reason, "disconnect");
    assert_eq!(end.outcome.unwrap().winner, 1);
    let disconnects =
        sqlx::query_scalar::<_, i32>("SELECT disconnects FROM ratings WHERE user_id=$1")
            .bind(a.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(disconnects, 1);
    drop(two);
    server.stop().await;
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn f7_stale_owner_cannot_acknowledge_or_persist_a_move(pool: PgPool) {
    let a = player(&pool, 1, true).await;
    let b = player(&pool, 2, true).await;
    let server = Server::new(pool.clone(), Settings::default()).await;
    let mut one = server.connect(&a, None).await;
    let mut two = server.connect(&b, None).await;
    let id = begin(&mut one, &mut two, &b.login).await;
    let uuid = Uuid::parse_str(&id).unwrap();
    let view = snapshot(&mut one, &id).await;
    sqlx::query(
        "UPDATE duel_journal SET owner_instance=$2,lease_until=now()+interval '1 hour' WHERE id=$1",
    )
    .bind(uuid)
    .bind(Uuid::nil())
    .execute(&pool)
    .await
    .unwrap();
    assert_error(
        one.call(submit(&id, view.view.as_ref().unwrap())).await,
        "LEASE_LOST",
    );
    let stored = server::duel::journal::load(&pool, uuid).await.unwrap();
    assert!(stored.pending.iter().all(Option::is_none));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM duel_locks WHERE duel_id=$1")
            .bind(uuid)
            .fetch_one(&pool)
            .await
            .unwrap(),
        2
    );
    drop(one);
    drop(two);
    server.stop().await;
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn f8_foreign_school_deck_challenge_lock_and_guaranteed_arena_unlock(pool: PgPool) {
    let a = player(&pool, 1, true).await;
    let b = player(&pool, 2, true).await;
    let server = Server::new(pool.clone(), Settings::default()).await;
    let mut one = server.connect(&a, None).await;
    let mut two = server.connect(&b, None).await;
    let avatar = sqlx::query_scalar::<_, Uuid>("SELECT id FROM avatars WHERE user_id=$1")
        .bind(a.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let own = engine_content::avatar(1).unwrap().snapshot(1);
    let card = engine_content::content()
        .cards()
        .iter()
        .find(|c| !own.schools.meets(&c.requires))
        .unwrap()
        .id as i16;
    sqlx::query("INSERT INTO collection(user_id,card_id,qty) VALUES ($1,$2,4) ON CONFLICT(user_id,card_id) DO UPDATE SET qty=4").bind(a.id).bind(card).execute(&pool).await.unwrap();
    let http = reqwest::Client::new();
    let update = |body: serde_json::Value| {
        http.post(format!("http://{}/api/game", server.addr))
            .header("cookie", format!("game_session={}", a.token))
            .header("x-requested-with", "app")
            .json(&body)
            .send()
    };
    let response = update(
        serde_json::json!({"action":"deck","id":avatar,"revision":0,"cards":[card,card,card,card]}),
    )
    .await
    .unwrap();
    assert!(response.status().is_success());
    sqlx::query("INSERT INTO arena_progress(user_id,wins) VALUES ($1,4)")
        .bind(a.id)
        .execute(&pool)
        .await
        .unwrap();
    one.join("arena").await;
    two.join("arena").await;
    let Body::ChallengeReceived(offer) = one
        .call(Client::ChallengeSend(pb::ChallengeSend {
            target: b.login.clone(),
        }))
        .await
    else {
        panic!()
    };
    let response = update(serde_json::json!({"action":"deck","id":avatar,"revision":1,"cards":[]}))
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::CONFLICT);
    let Body::ChallengeUpdate(accepted) = two
        .call(Client::ChallengeRespond(pb::ChallengeRespond {
            challenge_id: offer.challenge_id,
            accept: true,
        }))
        .await
    else {
        panic!()
    };
    let response = update(serde_json::json!({"action":"activate","id":avatar}))
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::CONFLICT);
    assert_ok(
        two.call(Client::DuelForfeit(pb::DuelTarget {
            duel_id: accepted.duel_id.clone(),
        }))
        .await,
    );
    let Body::DuelEnd(end) = one.event(|b| matches!(b, Body::DuelEnd(_))).await else {
        panic!()
    };
    assert_ne!(end.unlocked_template, 0);
    assert_ne!(end.unlocked_template, 1);
    let unlocks =
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM unlocked_templates WHERE user_id=$1")
            .bind(a.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(unlocks, 2);
    let record = server::duel::journal::load(&pool, Uuid::parse_str(&accepted.duel_id).unwrap())
        .await
        .unwrap();
    let (state, _) = record.rebuild().unwrap();
    let repeated = server::duel::result::settle(&pool, Uuid::nil(), &record, &state)
        .await
        .unwrap();
    assert_eq!(
        repeated.unlock[0].map(u32::from),
        Some(end.unlocked_template)
    );
    assert_eq!(
        sqlx::query_scalar::<_, i32>("SELECT wins FROM arena_progress WHERE user_id=$1")
            .bind(a.id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        5
    );
    let mut obsolete = record.clone();
    obsolete.engine_version = "previous-release".into();
    sqlx::query("UPDATE duel_journal SET data=$2 WHERE id=$1")
        .bind(record.id)
        .bind(obsolete.bytes().unwrap())
        .execute(&pool)
        .await
        .unwrap();
    let final_view = snapshot(&mut one, &accepted.duel_id).await;
    assert_eq!(
        final_view.result.unwrap().unlocked_template,
        end.unlocked_template
    );
    assert!(matches!(
        two.call(Client::MatchReplay(pb::MatchReplay {
            match_id: accepted.duel_id
        }))
        .await,
        Body::MatchReplayData(_)
    ));
    drop(one);
    drop(two);
    server.stop().await;
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL"]
async fn blocked_public_chat_write_does_not_stall_the_lobby(pool: PgPool) {
    let a = player(&pool, 1, true).await;
    let b = player(&pool, 2, true).await;
    let server = Server::new(pool.clone(), Settings::default()).await;
    let mut one = server.connect(&a, None).await;
    let mut two = server.connect(&b, None).await;
    one.join("arena").await;
    two.join("arena").await;
    let mut lock = pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE chat_log IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *lock)
        .await
        .unwrap();
    let request = one.send(chat("A stalled durable public message")).await;
    assert!(
        tokio::time::timeout(Duration::from_millis(50), one.reply(request))
            .await
            .is_err()
    );
    let pong = tokio::time::timeout(
        Duration::from_millis(250),
        two.call(Client::Ping(pb::Ping {})),
    )
    .await
    .expect("public chat persistence must not stall unrelated requests");
    assert!(matches!(pong, Body::Pong(_)));
    assert_ok(two.call(Client::SetAfk(pb::SetAfk { afk: true })).await);
    tokio::time::timeout(
        Duration::from_millis(250),
        one.event(
            |body| matches!(body,Body::RoomDelta(delta) if delta.updated.iter().any(|p|p.afk)),
        ),
    )
    .await
    .expect("a pending write must not pause the same socket's outgoing events");
    lock.rollback().await.unwrap();
    assert_ok(one.reply(request).await);
    let body = two.event(|body| matches!(body, Body::ChatMessage(_))).await;
    assert!(
        matches!(body,Body::ChatMessage(message) if message.text=="A stalled durable public message")
    );
    drop(one);
    drop(two);
    server.stop().await;
}
