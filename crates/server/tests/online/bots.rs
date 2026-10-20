use super::*;
use server::bots::{Population, Settings as BotSettings};

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL"]
async fn managed_seed_is_repeatable_and_preserves_progress(pool: PgPool) {
    let initial = server::bots::seed(&pool).await.unwrap();
    assert_eq!((initial.created, initial.existing), (40, 0));
    assert!(initial.logins.iter().all(|name| !name.contains("[AI]")));
    sqlx::query("UPDATE users SET description='Kept biography' WHERE login='Astar'")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE bot_accounts SET enabled=false WHERE roster_key='astar'")
        .execute(&pool)
        .await
        .unwrap();
    let again = server::bots::seed(&pool).await.unwrap();
    assert_eq!((again.created, again.existing), (0, 40));
    let (enabled, description) = sqlx::query_as::<_, (bool, String)>(
        "SELECT b.enabled,u.description FROM bot_accounts b JOIN users u ON u.id=b.user_id WHERE b.roster_key='astar'"
    ).fetch_one(&pool).await.unwrap();
    assert!(!enabled);
    assert_eq!(description, "Kept biography");
    let rows = sqlx::query_as::<_, (i32, i16, Vec<i16>)>(
        "SELECT a.template_id,a.level,d.cards FROM bot_accounts b JOIN avatars a ON a.user_id=b.user_id JOIN decks d ON d.avatar_id=a.id"
    ).fetch_all(&pool).await.unwrap();
    assert_eq!(rows.len(), 40);
    for (template, level, cards) in rows {
        let mut deck = arrayvec::ArrayVec::new();
        for card in cards {
            deck.push(card as u16);
        }
        let setup = engine::PlayerSetup {
            avatar: engine_content::avatar(template as u16)
                .unwrap()
                .snapshot(level as u8),
            deck,
        };
        engine::new_duel(
            &engine::DuelSetup {
                rules: engine_content::rules(),
                seed: [0; 32],
                players: [setup.clone(), setup],
            },
            &engine_content::content(),
        )
        .unwrap();
    }
    let missing = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM (SELECT a.user_id,card,count(*) AS qty FROM avatars a JOIN decks d ON d.avatar_id=a.id CROSS JOIN unnest(d.cards) card GROUP BY a.user_id,card) used LEFT JOIN collection c ON c.user_id=used.user_id AND c.card_id=used.card WHERE c.qty IS NULL OR c.qty<used.qty"
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(missing, 0);

    // Upgrade a managed account from a retired release without losing its history.
    let (user, old): (Uuid, Uuid) = sqlx::query_as(
        "SELECT a.user_id,a.id FROM avatars a JOIN users u ON u.id=a.user_id WHERE u.login='Astar' AND a.is_active"
    ).fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO unlocked_templates(user_id,template_id,source) VALUES($1,6,'admin')")
        .bind(user)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE avatars SET template_id=6,xp=25 WHERE id=$1")
        .bind(old)
        .execute(&pool)
        .await
        .unwrap();
    server::bots::seed(&pool).await.unwrap();
    let (replacement, template, level, xp): (Uuid, i32, i16, i32) = sqlx::query_as(
        "SELECT id,template_id,level,xp FROM avatars WHERE user_id=$1 AND is_active",
    )
    .bind(user)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_ne!(replacement, old);
    assert!(engine_content::is_playable(template as u16));
    assert_eq!((level, xp), (1, 25));
    assert!(sqlx::query_scalar::<_, bool>(
        "SELECT NOT is_active AND deleted_at IS NULL AND template_id=6 AND xp=25 FROM avatars WHERE id=$1"
    ).bind(old).fetch_one(&pool).await.unwrap());
    let quantity: i64 =
        sqlx::query_scalar("SELECT sum(qty)::bigint FROM collection WHERE user_id=$1")
            .bind(user)
            .fetch_one(&pool)
            .await
            .unwrap();
    server::bots::seed(&pool).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM avatars WHERE user_id=$1")
            .bind(user)
            .fetch_one(&pool)
            .await
            .unwrap(),
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT sum(qty)::bigint FROM collection WHERE user_id=$1")
            .bind(user)
            .fetch_one(&pool)
            .await
            .unwrap(),
        quantity
    );
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL"]
async fn managed_seed_never_claims_an_existing_nickname(pool: PgPool) {
    let human = player(&pool, 1, true).await;
    sqlx::query("UPDATE users SET login='Valen' WHERE id=$1")
        .bind(human.id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(server::bots::seed(&pool).await.is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM bot_accounts")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM users")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM users WHERE login='Valen'")
            .fetch_one(&pool)
            .await
            .unwrap(),
        human.id
    );
}

fn quiet_settings() -> BotSettings {
    BotSettings {
        autonomous_duels: false,
        action_interval: Duration::from_secs(300),
        health_interval: Duration::from_millis(100),
        accept_delay: Duration::from_millis(30),
    }
}
async fn wait_for_population(client: &mut ClientSocket, expected: usize) -> pb::RoomSnapshot {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        let snapshot = client.join("arena").await;
        if snapshot.players.len() >= expected {
            return snapshot;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "population did not connect: {}",
            snapshot.players.len()
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
async fn full_duel(client: &mut ClientSocket, id: &str) -> pb::DuelEnd {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        assert!(
            tokio::time::Instant::now() < deadline,
            "automated opponent stopped playing"
        );
        let Body::DuelSnapshot(snapshot) = client
            .call(Client::DuelSnapshot(pb::DuelTarget { duel_id: id.into() }))
            .await
        else {
            panic!("snapshot")
        };
        if let Some(result) = snapshot.result {
            return result;
        }
        let view = snapshot.view.unwrap();
        if view.outcome.is_none() && !snapshot.submitted {
            let me = view.me.unwrap();
            assert!(matches!(
                client
                    .call(Client::DuelSubmitMove(pb::DuelSubmitMove {
                        duel_id: id.into(),
                        round: view.round,
                        r#move: Some(pb::DuelMove {
                            attacks: me.attacks,
                            defenses: me.defenses,
                            ..Default::default()
                        }),
                    }))
                    .await,
                Body::DuelMoveAccepted(_)
            ));
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL"]
async fn managed_accounts_are_playable_restartable_and_can_be_disabled(pool: PgPool) {
    let human = player(&pool, 1, true).await;
    let server = Server::new(pool.clone(), Settings::default()).await;
    let population = Population::start(
        pool.clone(),
        server.state.online.hub.clone(),
        quiet_settings(),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(
        Population::start(
            pool.clone(),
            server.state.online.hub.clone(),
            quiet_settings()
        )
        .await
        .unwrap()
        .is_none()
    );
    let mut client = server.connect(&human, None).await;
    let room = wait_for_population(&mut client, 29).await;
    let astar = room.players.iter().find(|p| p.login == "Astar").unwrap();
    let id = astar.id.clone();
    assert!(Uuid::parse_str(&id).is_ok());
    assert!(astar.verified && astar.level == 1 && !astar.busy);
    let Body::PlayerInfo(info) = client.call(Client::WhoIs(target("Astar"))).await else {
        panic!("profile")
    };
    assert!(info.online);
    assert_eq!(info.player.unwrap().login, "Astar");
    let Body::ChallengeReceived(offer) = client
        .call(Client::ChallengeSend(pb::ChallengeSend {
            target: "Astar".into(),
        }))
        .await
    else {
        panic!("challenge")
    };
    let Body::ChallengeUpdate(accepted) = client.event(|body| matches!(body,Body::ChallengeUpdate(update) if update.challenge_id==offer.challenge_id && update.status=="accepted")).await else { panic!("acceptance") };
    let match_id = Uuid::parse_str(&accepted.duel_id).unwrap();
    let record = server::duel::journal::load(&pool, match_id).await.unwrap();
    assert_eq!(record.kind, "arena");
    assert!(record.seats[1].bot.is_some() && record.seats[1].bot_profile.is_some());
    full_duel(&mut client, &accepted.duel_id).await;
    let before = sqlx::query_scalar::<_, i64>("SELECT sum(rating)::bigint FROM ratings")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(before, 41000);
    assert_eq!(
        sqlx::query_scalar::<_, i32>("SELECT games FROM ratings WHERE user_id=$1")
            .bind(human.id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    let replacement = Population::spawn(
        pool.clone(),
        server.state.online.hub.clone(),
        quiet_settings(),
    );
    // A standby process must take over once the old controller releases its database lock.
    population.shutdown().await;
    let population = replacement;
    let room = wait_for_population(&mut client, 29).await;
    assert_eq!(
        room.players.iter().find(|p| p.login == "Astar").unwrap().id,
        id
    );
    sqlx::query("UPDATE bot_accounts SET enabled=false")
        .execute(&pool)
        .await
        .unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let room = client.join("arena").await;
        if room.players.len() == 1 {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "disabled accounts stayed online"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    population.shutdown().await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM sessions WHERE device_label='managed-controller'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    drop(client);
    server.stop().await;
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL"]
async fn managed_exhibitions_use_real_engine_without_rewards(pool: PgPool) {
    let server = Server::new(pool.clone(), Settings::default()).await;
    let mut settings = quiet_settings();
    settings.autonomous_duels = true;
    settings.action_interval = Duration::from_secs(1);
    let population = Population::start(pool.clone(), server.state.online.hub.clone(), settings)
        .await
        .unwrap()
        .unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(40);
    let id = loop {
        if let Some(id) =
            sqlx::query_scalar::<_, Uuid>("SELECT id FROM matches WHERE kind='training' LIMIT 1")
                .fetch_optional(&pool)
                .await
                .unwrap()
        {
            break id;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "autonomous duel did not finish"
        );
        tokio::time::sleep(Duration::from_millis(200)).await;
    };
    let record = server::duel::journal::load(&pool, id).await.unwrap();
    assert!(
        record
            .seats
            .iter()
            .all(|s| s.user.is_some() && s.bot.is_some())
    );
    assert!(!record.moves.is_empty());
    assert!(record.rebuild().unwrap().0.is_finished());
    let rewards = sqlx::query_as::<_, (i32, i32, i32, i32)>(
        "SELECT first_rating_delta,second_rating_delta,first_xp,second_xp FROM matches WHERE id=$1",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(rewards, (0, 0, 0, 0));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT sum(games)::bigint FROM ratings")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    population.shutdown().await;
    server.stop().await;
}
