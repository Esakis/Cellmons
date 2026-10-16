use crate::{
    Config, Measurements,
    fixture::{self, Player},
    network::Client,
};
use anyhow::{Context as _, ensure};
use futures_util::{StreamExt as _, TryStreamExt as _};
use protocol::game::v1::{self as pb, client_message::Body as Request, server_message::Body};
use server::{
    gateway::{self, AppState},
    online::Settings,
};
use sqlx::PgPool;
use std::{
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::sync::oneshot;
use uuid::Uuid;

pub async fn run(db: &PgPool, config: &Config, metrics: Arc<Measurements>) -> anyhow::Result<()> {
    println!("Preparing {} synthetic accounts", config.connections);
    let players = fixture::players(db, config.connections).await?;
    let settings = Settings {
        max_connections: config.connections + 64,
        per_ip: config.connections + 64,
        duels: server::duel::Settings {
            max_active: 2048.max(config.duels),
            ..Default::default()
        },
        ..Default::default()
    };
    let state = AppState::new(db.clone(), server::auth::Auth::local(), settings);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let (halt, stop) = oneshot::channel();
    let hub = state.online.hub.clone();
    let task = tokio::spawn(async move {
        axum::serve(
            gateway::realtime_listener(listener),
            gateway::router(state).into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(async move {
            let _ = stop.await;
            hub.shutdown().await;
        })
        .await
    });
    let result = async {
        let mut clients = futures_util::stream::iter(&players)
            .map(|player| Client::connect(addr, player, None, metrics.clone()))
            .buffered(32)
            .try_collect::<Vec<_>>()
            .await?;
        println!("{} real WebSockets connected", clients.len());
        match config.scenario.as_str() {
            "lobby" => {
                lobby(&clients, config, &metrics).await?;
                storm(&mut clients, &players, addr, metrics.clone()).await?;
            }
            "duels" => duels(db, &mut clients, &players, addr, config, metrics.clone()).await?,
            "economy" => {
                economy(db, &players, &metrics).await?;
                trades(db, &players, &clients, &metrics).await?;
            }
            "tournaments" => tournaments(db, &players, &clients, config, &metrics).await?,
            "soak" => {
                let (fighting, idle) = clients.split_at_mut(config.duels * 2);
                let (fighters, observers) = players.split_at(config.duels * 2);
                let _ = observers;
                tokio::try_join!(
                    duel_slice(db, fighting, fighters, config, metrics.clone()),
                    lobby(idle, config, &metrics)
                )?;
            }
            _ => unreachable!(),
        }
        futures_util::stream::iter(&clients)
            .map(|c| {
                let metrics = &metrics;
                async move {
                    let t = Instant::now();
                    c.ping().await?;
                    metrics.record("ping", t);
                    Ok::<_, anyhow::Error>(())
                }
            })
            .buffer_unordered(32)
            .try_collect::<Vec<_>>()
            .await?;
        ensure!(
            server::economy::reconcile(db)
                .await
                .map_err(|e| anyhow::anyhow!(e.0))?
                .is_ok(),
            "ledger reconciliation failed"
        );
        Ok::<_, anyhow::Error>(())
    }
    .await;
    let _ = halt.send(());
    tokio::time::timeout(Duration::from_secs(15), task)
        .await
        .context("fixture shutdown timed out")???;
    result
}
async fn lobby(clients: &[Client], config: &Config, metrics: &Measurements) -> anyhow::Result<()> {
    let members = config.room_members.min(clients.len());
    ensure!(members > 0, "lobby needs observers");
    let observer_rooms = ["training", "castle", "portal", "bazaar", "shop", "tower"];
    ensure!(
        clients.len() - members <= observer_rooms.len() * 1000,
        "not enough public rooms for all observers"
    );
    for (index, client) in clients[members..].iter().enumerate() {
        client.join(observer_rooms[index / 1000]).await?;
    }
    for client in &clients[..members] {
        let t = Instant::now();
        let bytes = client.join("arena").await?;
        metrics.record("room_join", t);
        metrics.value("room_snapshot_bytes", bytes as u64);
    }
    println!(
        "{members} room members; sending {} chat messages/s",
        config.chat_per_second
    );
    let started = Instant::now();
    let deadline = started + Duration::from_secs(config.seconds);
    let chat = async {
        let index = if config.chat_per_second == 0 {
            tokio::time::sleep(Duration::from_secs(config.seconds)).await;
            0
        } else {
            let period = Duration::from_micros(
                (members as u64) * 1_000_000 / (config.chat_per_second as u64),
            );
            let counts: Vec<usize> = futures_util::stream::iter(0..members)
                .map(|member| async move {
                    let offset = Duration::from_micros(
                        (member as u64) * 1_000_000 / (config.chat_per_second as u64),
                    );
                    let mut interval = tokio::time::interval_at(
                        tokio::time::Instant::from_std(started + offset),
                        period,
                    );
                    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                    let mut sent_count = 0;
                    while tokio::time::timeout_at(
                        tokio::time::Instant::from_std(deadline),
                        interval.tick(),
                    )
                    .await
                    .is_ok()
                    {
                        let sent = SystemTime::now().duration_since(UNIX_EPOCH)?.as_micros();
                        let t = Instant::now();
                        clients[member]
                            .call(Request::ChatSend(pb::ChatSend {
                                channel: "room".into(),
                                text: format!("load:{sent}"),
                                ..Default::default()
                            }))
                            .await?;
                        metrics.record("chat_ack", t);
                        sent_count += 1;
                    }
                    Ok::<_, anyhow::Error>(sent_count)
                })
                .buffer_unordered(members)
                .try_collect()
                .await?;
            counts.into_iter().sum()
        };
        metrics.value("chat_messages_sent", index as u64);
        metrics.record("chat_run_elapsed", started);
        ensure!(
            started.elapsed() <= Duration::from_secs(config.seconds + 1),
            "chat backlog grew past the measurement window"
        );
        ensure!(
            index as u64 >= (config.chat_per_second as u64) * config.seconds * 9 / 10,
            "chat throughput below 90% of requested load"
        );
        Ok::<_, anyhow::Error>(())
    };
    let churn = async {
        let observers = &clients[members..];
        if observers.is_empty() {
            return Ok::<_, anyhow::Error>(());
        }
        let changes_per_minute = (clients.len() * 5 / 100).max(1);
        let mut interval =
            tokio::time::interval(Duration::from_millis(60000 / changes_per_minute as u64));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut count = 0;
        while tokio::time::timeout_at(tokio::time::Instant::from_std(deadline), interval.tick())
            .await
            .is_ok()
        {
            let index = count % observers.len();
            let started = Instant::now();
            observers[index]
                .call(Request::RoomLeave(pb::RoomLeave {}))
                .await?;
            observers[index].join(observer_rooms[index / 1000]).await?;
            metrics.record("presence_rejoin", started);
            count += 1;
        }
        metrics.value("presence_changes_sent", count as u64);
        Ok(())
    };
    tokio::try_join!(chat, churn)?;
    Ok(())
}
async fn storm(
    clients: &mut Vec<Client>,
    players: &[Player],
    addr: SocketAddr,
    metrics: Arc<Measurements>,
) -> anyhow::Result<()> {
    let half = clients.len() / 2;
    let start = Instant::now();
    let old: Vec<_> = clients.drain(..half).collect();
    let resumes: Vec<_> = old.iter().map(|c| c.resume.clone()).collect();
    drop(old);
    tokio::time::sleep(Duration::from_millis(100)).await;
    let restored = futures_util::stream::iter(players[..half].iter().zip(resumes))
        .map(|(player, resume)| Client::connect(addr, player, Some(resume), metrics.clone()))
        .buffered(32)
        .try_collect::<Vec<_>>()
        .await?;
    clients.splice(0..0, restored);
    metrics.record("reconnect_storm_total", start);
    println!(
        "Reconnected {half} clients in {} ms",
        start.elapsed().as_millis()
    );
    Ok(())
}
async fn start_pair(a: &Client, b: &Client, opponent: &str) -> anyhow::Result<String> {
    a.join("arena").await?;
    b.join("arena").await?;
    let Body::ChallengeReceived(offer) = a
        .call(Request::ChallengeSend(pb::ChallengeSend {
            target: opponent.into(),
        }))
        .await?
    else {
        anyhow::bail!("expected challenge")
    };
    let Body::ChallengeUpdate(update) = b
        .call(Request::ChallengeRespond(pb::ChallengeRespond {
            challenge_id: offer.challenge_id,
            accept: true,
        }))
        .await?
    else {
        anyhow::bail!("expected duel")
    };
    a.call(Request::RoomLeave(pb::RoomLeave {})).await?;
    b.call(Request::RoomLeave(pb::RoomLeave {})).await?;
    ensure!(!update.duel_id.is_empty(), "duel not started");
    Ok(update.duel_id)
}
async fn snapshot(client: &Client, id: &str) -> anyhow::Result<pb::DuelSnapshot> {
    let Body::DuelSnapshot(snapshot) = client
        .call(Request::DuelSnapshot(pb::DuelTarget { duel_id: id.into() }))
        .await?
    else {
        anyhow::bail!("expected duel snapshot")
    };
    Ok(*snapshot)
}
fn submit(snapshot: &pb::DuelSnapshot) -> anyhow::Result<Request> {
    let view = snapshot.view.as_ref().context("missing duel view")?;
    let me = view.me.as_ref().context("missing own view")?;
    Ok(Request::DuelSubmitMove(pb::DuelSubmitMove {
        duel_id: snapshot.duel_id.clone(),
        round: view.round,
        r#move: Some(pb::DuelMove {
            attacks: me.attacks.clone(),
            defenses: me.defenses.clone(),
            ..Default::default()
        }),
    }))
}
async fn play(
    a: &Client,
    b: &Client,
    id: &str,
    config: &Config,
    metrics: &Measurements,
) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(config.seconds);
    while Instant::now() < deadline {
        let one = snapshot(a, id).await?;
        if one.result.is_some() {
            return Ok(());
        }
        let two = snapshot(b, id).await?;
        a.call(submit(&one)?).await?;
        let t = Instant::now();
        b.call(submit(&two)?).await?;
        loop {
            let next = snapshot(a, id).await?;
            if next.result.is_some()
                || next
                    .view
                    .as_ref()
                    .is_some_and(|v| Some(v.round) != one.view.as_ref().map(|v| v.round))
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        metrics.record("round_result", t);
        tokio::time::sleep(Duration::from_millis(config.think_ms)).await;
    }
    Ok(())
}
// End the synthetic fixture with bounded settlement concurrency. A simultaneous mass-forfeit
// is a different workload from S2's steady round cadence, and must not contaminate its samples.
async fn finish(a: &Client, id: &str, metrics: &Measurements) -> anyhow::Result<()> {
    if snapshot(a, id).await?.result.is_some() {
        return Ok(());
    }
    let t = Instant::now();
    a.call(Request::DuelForfeit(pb::DuelTarget { duel_id: id.into() }))
        .await?;
    let deadline = Instant::now() + Duration::from_secs(30);
    while snapshot(a, id).await?.result.is_none() {
        ensure!(
            Instant::now() < deadline,
            "settlement acknowledgement deadline"
        );
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    metrics.record("forfeit_and_settlement", t);
    Ok(())
}
async fn finish_pairs(
    clients: &[Client],
    ids: &[String],
    metrics: &Measurements,
) -> anyhow::Result<()> {
    futures_util::stream::iter(clients.as_chunks::<2>().0.iter().zip(ids))
        .map(|(pair, id)| finish(&pair[0], id, metrics))
        .buffer_unordered(32)
        .try_collect::<Vec<_>>()
        .await?;
    Ok(())
}
async fn duel_slice(
    db: &PgPool,
    clients: &[Client],
    players: &[Player],
    config: &Config,
    metrics: Arc<Measurements>,
) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(config.seconds);
    let mut cycles = 0;
    while Instant::now() < deadline {
        let mut cycle = config.clone();
        cycle.seconds = deadline
            .saturating_duration_since(Instant::now())
            .as_secs()
            .clamp(1, 60);
        duel_cycle(db, clients, players, &cycle, metrics.clone()).await?;
        cycles += 1;
    }
    metrics.value("soak_cycles", cycles);
    Ok(())
}
async fn duel_cycle(
    db: &PgPool,
    clients: &[Client],
    players: &[Player],
    config: &Config,
    metrics: Arc<Measurements>,
) -> anyhow::Result<()> {
    let ids = futures_util::stream::iter(
        clients
            .as_chunks::<2>()
            .0
            .iter()
            .zip(players.as_chunks::<2>().0.iter()),
    )
    .map(|(pair, users)| async move { start_pair(&pair[0], &pair[1], &users[1].login).await })
    .buffered(16)
    .try_collect::<Vec<_>>()
    .await?;
    let active: i64 = sqlx::query_scalar("SELECT count(*) FROM duel_journal WHERE status='active'")
        .fetch_one(db)
        .await?;
    ensure!(
        active >= ids.len() as i64,
        "duels ended before simultaneous load was reached"
    );
    metrics.value("simultaneous_duels", active as u64);
    println!("{active} simultaneous durable duels");
    let stagger_count = ids.len().max(1) as u64;
    let metrics = &metrics;
    futures_util::stream::iter(
        clients
            .as_chunks::<2>()
            .0
            .iter()
            .zip(ids.iter())
            .enumerate(),
    )
    .map(|(index, (pair, id))| async move {
        tokio::time::sleep(Duration::from_millis(
            index as u64 * config.think_ms / stagger_count,
        ))
        .await;
        play(&pair[0], &pair[1], id, config, metrics).await
    })
    .buffer_unordered(config.duels.max(1))
    .try_collect::<Vec<_>>()
    .await?;
    finish_pairs(clients, &ids, metrics).await?;
    Ok(())
}
async fn duels(
    db: &PgPool,
    clients: &mut Vec<Client>,
    players: &[Player],
    addr: SocketAddr,
    config: &Config,
    metrics: Arc<Measurements>,
) -> anyhow::Result<()> {
    let count = config.duels * 2;
    ensure!(count > 0, "duel scenario needs duels");
    let ids = futures_util::stream::iter(
        clients[..count]
            .as_chunks::<2>()
            .0
            .iter()
            .zip(players[..count].as_chunks::<2>().0.iter()),
    )
    .map(|(pair, users)| async move { start_pair(&pair[0], &pair[1], &users[1].login).await })
    .buffered(16)
    .try_collect::<Vec<_>>()
    .await?;
    let active: i64 = sqlx::query_scalar("SELECT count(*) FROM duel_journal WHERE status='active'")
        .fetch_one(db)
        .await?;
    ensure!(
        active == config.duels as i64,
        "not all duels active together"
    );
    metrics.value("simultaneous_duels", active as u64);
    println!("{active} simultaneous durable duels; reconnecting half of the sockets");
    storm(clients, players, addr, metrics.clone()).await?;
    futures_util::stream::iter(
        clients[..count]
            .as_chunks::<2>()
            .0
            .iter()
            .zip(ids.iter())
            .enumerate(),
    )
    .map(|(index, (pair, id))| {
        let metrics = &metrics;
        async move {
            tokio::time::sleep(Duration::from_millis(
                index as u64 * config.think_ms / config.duels as u64,
            ))
            .await;
            play(&pair[0], &pair[1], id, config, metrics).await
        }
    })
    .buffer_unordered(config.duels)
    .try_collect::<Vec<_>>()
    .await?;
    finish_pairs(&clients[..count], &ids, &metrics).await?;
    let finished: i64 = sqlx::query_scalar("SELECT count(*) FROM matches")
        .fetch_one(db)
        .await?;
    ensure!(
        finished == config.duels as i64,
        "duel settlement count differs"
    );
    Ok(())
}
async fn economy(db: &PgPool, players: &[Player], metrics: &Measurements) -> anyhow::Result<()> {
    futures_util::stream::iter(players.iter().take(200))
        .map(|player| async move {
            let inventory = server::store::inventory(db, player.id)
                .await
                .map_err(|e| anyhow::anyhow!(e.0))?;
            let offer = inventory
                .offers
                .into_iter()
                .find(|o| o.id == "basic")
                .context("basic offer")?;
            let key = Uuid::now_v7();
            let request = || server::store::Request {
                key,
                operation: server::store::Operation::Buy {
                    kind: "basic".into(),
                    quote: offer.quote.clone(),
                },
            };
            let t = Instant::now();
            let purchase = server::store::execute(db, player.id, request())
                .await
                .map_err(|e| anyhow::anyhow!(e.0))?;
            let retry = server::store::execute(db, player.id, request())
                .await
                .map_err(|e| anyhow::anyhow!(e.0))?;
            ensure!(purchase.booster == retry.booster, "duplicate purchase");
            metrics.record("purchase_with_retry", t);
            let t = Instant::now();
            let opened = server::store::execute(
                db,
                player.id,
                server::store::Request {
                    key: Uuid::now_v7(),
                    operation: server::store::Operation::Open {
                        id: purchase.booster.context("booster receipt")?,
                    },
                },
            )
            .await
            .map_err(|e| anyhow::anyhow!(e.0))?;
            ensure!(opened.cards.len() == 12, "pack size changed");
            metrics.record("open", t);
            Ok::<_, anyhow::Error>(())
        })
        .buffer_unordered(200)
        .try_collect::<Vec<_>>()
        .await?;
    Ok(())
}
fn trade_snapshot(body: Body) -> anyhow::Result<pb::TradeSnapshot> {
    match body {
        Body::TradeSnapshot(value) => Ok(value),
        _ => anyhow::bail!("expected trade snapshot"),
    }
}
async fn trades(
    db: &PgPool,
    players: &[Player],
    clients: &[Client],
    metrics: &Measurements,
) -> anyhow::Result<()> {
    ensure!(
        clients.len() >= 400,
        "economy scenario needs 400 accounts for 200 concurrent trades"
    );
    for client in &clients[..400] {
        client.join("bazaar").await?;
    }
    futures_util::stream::iter(0..200).map(|index| async move {
        let (a,b) = (&clients[index],&clients[index+200]);
        let (card, before): (i16,i32) = sqlx::query_as("SELECT card_id,qty FROM collection WHERE user_id=$1 AND qty>0 ORDER BY card_id LIMIT 1").bind(players[index].id).fetch_one(db).await?;
        let started = Instant::now();
        let offer = trade_snapshot(a.call(Request::TradeInvite(pb::TradeInvite{target:players[index+200].login.clone()})).await?)?;
        let id = offer.trade_id;
        let offer = trade_snapshot(b.call(Request::TradeRespond(pb::TradeRespond{trade_id:id.clone(),accept:true})).await?)?;
        let offer = trade_snapshot(a.call(Request::TradeUpdate(pb::TradeUpdate{trade_id:id.clone(),version:offer.version,offer:Some(pb::TradeOffer{cards:vec![pb::TradeCard{card_id:card as u32,qty:1}],ducats:0})})).await?)?;
        let offer = trade_snapshot(b.call(Request::TradeUpdate(pb::TradeUpdate{trade_id:id.clone(),version:offer.version,offer:Some(pb::TradeOffer{cards:vec![],ducats:1})})).await?)?;
        for _ in 0..2 { a.call(Request::TradeConfirm(pb::TradeConfirm{trade_id:id.clone(),version:offer.version})).await?; }
        for _ in 0..2 {
            let result = trade_snapshot(b.call(Request::TradeConfirm(pb::TradeConfirm{trade_id:id.clone(),version:offer.version})).await?)?;
            ensure!(result.status=="completed", "trade {index} did not settle: {}",result.status);
        }
        metrics.record("trade_with_retry",started);
        let after: i32 = sqlx::query_scalar("SELECT coalesce((SELECT qty FROM collection WHERE user_id=$1 AND card_id=$2),0)").bind(players[index].id).bind(card).fetch_one(db).await?;
        ensure!(after==before-1,"duplicate card transfer");
        let balance: i64 = sqlx::query_scalar("SELECT balance FROM wallets WHERE user_id=$1").bind(players[index].id).fetch_one(db).await?;
        ensure!(balance==1,"duplicate currency transfer");
        Ok::<_,anyhow::Error>(())
    }).buffer_unordered(200).try_collect::<Vec<_>>().await?;
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM ledger_transactions WHERE reason='trade'")
            .fetch_one(db)
            .await?;
    ensure!(count == 200, "trade ledger count differs");
    Ok(())
}
async fn tournaments(
    db: &PgPool,
    players: &[Player],
    clients: &[Client],
    config: &Config,
    metrics: &Measurements,
) -> anyhow::Result<()> {
    let count = config.tournaments * 64;
    ensure!(
        config.tournaments > 0 && players.len() >= count,
        "tournament load needs 64 connections per tournament"
    );
    for roster in players[..count].as_chunks::<64>().0 {
        let id = Uuid::now_v7();
        server::tournaments::create(
            db,
            roster[0].id,
            id,
            "Load tournament",
            server::tournaments::Terms {
                teams: 64,
                team_size: 1,
                fee: 0,
                max_games: 1,
                min_level: 1,
                max_level: 20,
                tiers: vec![0],
                only_clan: None,
                allowed_clans: vec![],
            },
        )
        .await
        .map_err(|e| anyhow::anyhow!(e.0))?;
        for user in roster {
            server::tournaments::join(db, user.id, id, Uuid::now_v7(), user.avatar)
                .await
                .map_err(|e| anyhow::anyhow!(e.0))?;
        }
    }
    let start = Instant::now();
    let deadline = start + Duration::from_secs(60.max(config.seconds));
    loop {
        for user in &players[..count] {
            if let Err(e) = server::tournaments::ready(db, user.id, true).await {
                ensure!(e.0 == "BUSY", "tournament readiness failed");
            }
        }
        let active: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM duel_journal WHERE kind='tournament' AND status='active'",
        )
        .fetch_one(db)
        .await?;
        if active == count as i64 / 2 {
            metrics.value("tournament_simultaneous_duels", active as u64);
            break;
        }
        ensure!(Instant::now() < deadline, "tournament start deadline");
        tokio::time::sleep(Duration::from_secs(3)).await;
    }
    metrics.record("tournament_first_round_ready_wait", start);
    for (user, client) in players[..count].iter().zip(&clients[..count]) {
        let id: Uuid = sqlx::query_scalar("SELECT duel_id FROM duel_locks WHERE user_id=$1")
            .bind(user.id)
            .fetch_one(db)
            .await?;
        snapshot(client, &id.to_string()).await?;
    }
    Ok(())
}
