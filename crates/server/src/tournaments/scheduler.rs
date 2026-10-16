//! Scheduler implementation follows the pure bracket; all work is committed before actor start.
use super::*;
use crate::{
    duel::{
        Settings,
        actor::{Action, Notice},
        manager::Manager as Duels,
        record::{ExternalEnd, Record, Seat},
    },
    economy::{self, Account},
    online::now_ms,
};
use sqlx::PgPool;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::sync::mpsc;

struct Inner {
    stopping: AtomicBool,
}
#[derive(Clone)]
pub struct Manager(Arc<Inner>);
impl Manager {
    pub(crate) fn disabled() -> Self {
        Self(Arc::new(Inner {
            stopping: AtomicBool::new(false),
        }))
    }
    pub fn new(
        db: PgPool,
        duels: Duels,
        settings: Settings,
        notices: mpsc::Sender<Notice>,
        enabled: bool,
    ) -> Self {
        let this = Self::disabled();
        if enabled {
            let this = this.clone();
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_millis(500));
                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                while !this.0.stopping.load(Ordering::Acquire) {
                    interval.tick().await;
                    let ids=sqlx::query_scalar::<_,Uuid>("SELECT id FROM tournaments WHERE status IN ('registering','running') AND engine_version=$1 AND content_version=$2 ORDER BY scheduler_until_ms,created_at LIMIT 64").bind(engine::VERSION).bind(&engine_content::CONTENT_VERSION[..]).fetch_all(&db).await;
                    if let Ok(ids) = ids {
                        for id in ids {
                            if this.0.stopping.load(Ordering::Acquire) || !duels.has_capacity() {
                                break;
                            }
                            match tokio::time::timeout(
                                Duration::from_secs(4),
                                tick(&db, id, duels.instance(), &settings, now_ms()),
                            )
                            .await
                            {
                                Ok(Ok(work)) => {
                                    for record in work.duels {
                                        let users =
                                            record.seats.iter().filter_map(|s| s.user).collect();
                                        let id = record.id;
                                        duels.start(record);
                                        let _ = notices.send(Notice::Recovered(id, users)).await;
                                    }
                                    for (user, id) in work.forfeits {
                                        let _ = duels.request(user, id, Action::Forfeit).await;
                                    }
                                    if !work.changed.is_empty() {
                                        let _ = notices
                                            .send(Notice::ProfilesChanged(work.changed))
                                            .await;
                                    }
                                }
                                Ok(Err(error)) => {
                                    tracing::warn!(tournament=%id,code=error.0,"tournament scheduler retry")
                                }
                                Err(_) => {
                                    tracing::warn!(tournament=%id,"tournament scheduler timed out")
                                }
                            }
                        }
                    }
                }
            });
        }
        this
    }
    pub fn shutdown(&self) {
        self.0.stopping.store(true, Ordering::Release);
    }
}
#[derive(Default)]
pub struct Work {
    pub duels: Vec<Record>,
    pub forfeits: Vec<(Uuid, Uuid)>,
    pub changed: Vec<Uuid>,
}
#[derive(sqlx::FromRow)]
struct Pending {
    id: Uuid,
    descriptor: Vec<u8>,
    status: String,
    first_fight: bool,
    created_ms: i64,
    deadline_ms: i64,
}
pub async fn tick(
    db: &PgPool,
    id: Uuid,
    instance: Uuid,
    settings: &Settings,
    now: i64,
) -> Result<Work> {
    let mut tx = db.begin().await?;
    let users: Vec<Uuid> = sqlx::query_scalar(
        "SELECT user_id FROM tournament_entries WHERE tournament_id=$1 ORDER BY user_id",
    )
    .bind(id)
    .fetch_all(&mut *tx)
    .await?;
    lock_users(&mut tx, users.clone()).await?;
    let tournament = locked(&mut tx, id).await?;
    tournament.compatible()?;
    let mut entries = entries(&mut tx, id).await?;
    let mut work = Work::default();
    if !matches!(tournament.status.as_str(), "registering" | "running")
        || entries.iter().any(|e| !users.contains(&e.user_id))
    {
        return Ok(work);
    }
    if tournament
        .scheduler_instance
        .is_some_and(|owner| owner != instance)
        && tournament.scheduler_until_ms > now
    {
        return Ok(work);
    }
    let takeover = tournament
        .scheduler_instance
        .is_some_and(|owner| owner != instance);
    let rules = &crate::rules::RULES.tournament;
    if takeover {
        sqlx::query("UPDATE tournament_entries SET ready_since_ms=NULL,ready_until_ms=0 WHERE tournament_id=$1").bind(id).execute(&mut *tx).await?;
        sqlx::query("UPDATE tournament_fights SET created_ms=$2,deadline_ms=$2+$3 WHERE tournament_id=$1 AND status='pending'").bind(id).bind(now).bind((rules.first_fight_s+rules.ready_grace_s)*1000).execute(&mut *tx).await?;
        for entry in &mut entries {
            entry.ready_since_ms = None;
            entry.ready_until_ms = 0;
        }
    }
    sqlx::query("UPDATE tournaments SET scheduler_instance=$2,scheduler_until_ms=$3 WHERE id=$1")
        .bind(id)
        .bind(instance)
        .bind(now + 10000)
        .execute(&mut *tx)
        .await?;
    let terms: Terms = decode(&tournament.terms)?;
    let mut bracket: bracket::Bracket;
    if tournament.status == "registering" {
        if now - tournament.created_ms >= rules.registration_hours * 3600000 {
            repository::refund(&mut tx, id, &entries).await?;
            work.changed = users;
            tx.commit().await?;
            return Ok(work);
        }
        let enrolled: Vec<_> = entries.iter().filter(|e| e.status == "enrolled").collect();
        if enrolled.len() != terms.teams * terms.team_size {
            tx.commit().await?;
            return Ok(work);
        }
        let mut groups: BTreeMap<Uuid, Vec<Uuid>> = BTreeMap::new();
        for entry in enrolled {
            groups.entry(entry.team_id).or_default().push(entry.user_id);
        }
        if groups.len() != terms.teams || groups.values().any(|g| g.len() != terms.team_size) {
            return Err(Error("INVALID_JOURNAL"));
        }
        let teams = groups
            .into_iter()
            .map(|(id, users)| bracket::Team { id, users })
            .collect();
        bracket = bracket::Bracket::new(
            teams,
            terms.max_games,
            tournament
                .seed
                .as_slice()
                .try_into()
                .map_err(|_| Error("INVALID_JOURNAL"))?,
        )?;
        sqlx::query("UPDATE tournaments SET status='running',started_at=now() WHERE id=$1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        work.changed = users.clone();
    } else {
        bracket = decode(
            tournament
                .bracket
                .as_deref()
                .ok_or(Error("INVALID_JOURNAL"))?,
        )?;
    }
    // Verification can be revoked after registration; such a seat forfeits, never starts a duel.
    let allowed: Vec<Uuid>=sqlx::query_scalar("SELECT id FROM users WHERE id=ANY($1) AND status='active' AND email_verified_at IS NOT NULL").bind(&users).fetch_all(&mut *tx).await?;
    for entry in entries
        .iter()
        .filter(|e| e.status == "enrolled" && !allowed.contains(&e.user_id))
    {
        bracket.withdraw(entry.user_id)?;
    }
    let pending: Vec<Pending>=sqlx::query_as("SELECT id,descriptor,status,first_fight,created_ms,deadline_ms FROM tournament_fights WHERE tournament_id=$1 AND status<>'done' ORDER BY id").bind(id).fetch_all(&mut *tx).await?;
    for fight in pending.iter().filter(|f| f.status == "active") {
        let descriptor: bracket::Fight = decode(&fight.descriptor)?;
        let winner: Option<i16> = sqlx::query_scalar("SELECT winner FROM matches WHERE id=$1")
            .bind(fight.id)
            .fetch_optional(&mut *tx)
            .await?;
        if let Some(winner) = winner {
            let score = match winner {
                0 => bracket::Score::Draw,
                1 => bracket::Score::First,
                2 => bracket::Score::Second,
                _ => return Err(Error("INVALID_JOURNAL")),
            };
            bracket.score(descriptor, score)?;
            sqlx::query("UPDATE tournament_fights SET status='done',score=$2 WHERE id=$1")
                .bind(fight.id)
                .bind(winner)
                .execute(&mut *tx)
                .await?;
        } else {
            for user in descriptor.users {
                if bracket.departed.contains(&user) {
                    work.forfeits.push((user, fight.id));
                }
            }
        }
    }
    let mut started = 0;
    for fight in pending.iter().filter(|f| f.status == "pending") {
        if started >= 32 {
            break;
        }
        let descriptor: bracket::Fight = decode(&fight.descriptor)?;
        let seats = descriptor.users.map(|user| {
            entries
                .iter()
                .find(|e| e.user_id == user)
                .ok_or(Error("INVALID_JOURNAL"))
        });
        let [first, second] = seats;
        let seats = [first?, second?];
        let departed = descriptor.users.map(|u| bracket.departed.contains(&u));
        let ready = seats.map(|e| e.ready_until_ms > now && e.ready_since_ms.is_some());
        let earliest = if fight.first_fight {
            fight.created_ms + rules.first_fight_s * 1000
        } else {
            seats
                .iter()
                .filter_map(|e| e.ready_since_ms)
                .max()
                .unwrap_or(now)
                + rules.next_fight_s * 1000
        };
        let forced = departed.iter().any(|v| *v);
        let normal = ready == [true, true] && !forced;
        if normal && now < earliest {
            continue;
        }
        if !normal && !forced && now < fight.deadline_ms {
            continue;
        }
        let present = if forced {
            [!departed[0], !departed[1]]
        } else {
            ready
        };
        if !normal && present == [false, false] {
            bracket.score(descriptor, bracket::Score::BothAbsent)?;
            sqlx::query("UPDATE tournament_fights SET status='done',score=3 WHERE id=$1")
                .bind(fight.id)
                .execute(&mut *tx)
                .await?;
            continue;
        }
        let [first, second] = seats.map(|e| decode::<Seat>(&e.seat));
        let mut seed = [0_u8; 32];
        getrandom::fill(&mut seed).map_err(|_| Error("INTERNAL"))?;
        let mut record = Record::new(fight.id, "tournament", [first?, second?], seed, settings)?;
        record.connected = [false, false];
        record.grace_until = [Some(now + settings.reconnect_ms()); 2];
        if !normal {
            record.ended = Some(ExternalEnd {
                loser: Some(if present[0] { 1 } else { 0 }),
                cause: 2,
            });
        }
        crate::duel::journal::create_for_activity(
            &mut tx,
            &record,
            instance,
            settings.lease_ms(),
            "tournament",
            id,
        )
        .await?;
        sqlx::query("UPDATE tournament_fights SET status='active' WHERE id=$1")
            .bind(fight.id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE tournament_entries SET ready_since_ms=NULL,ready_until_ms=0 WHERE tournament_id=$1 AND user_id=ANY($2)").bind(id).bind(descriptor.users.as_slice()).execute(&mut *tx).await?;
        work.duels.push(record);
        started += 1;
    }
    let active_teams: BTreeSet<Uuid> = if bracket.finished {
        bracket.champion.into_iter().collect()
    } else {
        bracket
            .rounds
            .last()
            .ok_or(Error("INVALID_JOURNAL"))?
            .pairs
            .iter()
            .flat_map(|p| {
                if p.resolved {
                    vec![p.winner]
                } else {
                    p.teams.to_vec()
                }
            })
            .flatten()
            .collect()
    };
    for entry in entries
        .iter()
        .filter(|e| e.status == "enrolled" && !active_teams.contains(&e.team_id))
    {
        repository::unlock(&mut tx, id, entry.user_id).await?;
        sqlx::query("UPDATE tournament_entries SET status='eliminated',ready_since_ms=NULL,ready_until_ms=0 WHERE tournament_id=$1 AND user_id=$2").bind(id).bind(entry.user_id).execute(&mut *tx).await?;
        work.changed.push(entry.user_id);
    }
    if bracket.finished {
        let mut winners: Vec<_> = entries
            .iter()
            .filter(|e| Some(e.team_id) == bracket.champion && e.status == "enrolled")
            .map(|e| e.user_id)
            .collect();
        winners.sort();
        if winners.is_empty() {
            economy::transfer(
                &mut tx,
                Account::Tournament(id),
                Account::Sink,
                tournament.balance,
                "tournament_prize",
                id,
            )
            .await?;
        } else {
            let count = winners.len() as i64;
            for (index, user) in winners.iter().enumerate() {
                if let Some(reward) = &tournament.system_reward {
                    rewards::grant(&mut tx, *user, id, reward).await?;
                }
                let amount = tournament.balance / count
                    + i64::from((index as i64) < tournament.balance % count);
                economy::transfer(
                    &mut tx,
                    Account::Tournament(id),
                    Account::Player(*user),
                    amount,
                    "tournament_prize",
                    id,
                )
                .await?;
                repository::unlock(&mut tx, id, *user).await?;
                sqlx::query("UPDATE tournament_entries SET status='champion',ready_since_ms=NULL,ready_until_ms=0 WHERE tournament_id=$1 AND user_id=$2").bind(id).bind(user).execute(&mut *tx).await?;
                work.changed.push(*user);
            }
        }
        sqlx::query("UPDATE tournaments SET status='finished',finished_at=now() WHERE id=$1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
    } else {
        for descriptor in bracket.plan() {
            let first = descriptor.round == 0 && descriptor.game == 0;
            let grace = (rules.ready_grace_s + if first { rules.first_fight_s } else { 0 }) * 1000;
            sqlx::query("INSERT INTO tournament_fights(id,tournament_id,descriptor,first_fight,created_ms,deadline_ms) VALUES ($1,$2,$3,$4,$5,$6)").bind(Uuid::now_v7()).bind(id).bind(encode(&descriptor)?).bind(first).bind(now).bind(now+grace).execute(&mut *tx).await?;
        }
    }
    sqlx::query("UPDATE tournaments SET bracket=$2,revision=revision+1 WHERE id=$1")
        .bind(id)
        .bind(encode(&bracket)?)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    work.changed.sort();
    work.changed.dedup();
    Ok(work)
}
