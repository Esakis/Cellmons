//! Public, bounded online projections. The private journal never leaves the server.
use super::{Error, Result, repo};
use crate::duel::{actor, journal, record::Record};
use protocol::{
    Message,
    game::v1::{self as pb, server_message::Body},
};
use sqlx::PgPool;
use uuid::Uuid;

pub fn bots() -> Vec<(pb::PublicPlayer, engine::PlayerSetup, String)> {
    let rotation = (super::now_ms()
        / (i64::from(crate::rules::RULES.training.bot_rotation_min) * 60_000))
        as usize;
    engine_ai::training_profiles()
        .bots
        .iter()
        .enumerate()
        .flat_map(|(index, bot)| {
            (0..2)
                .filter_map(move |slot| {
                    let level = ((rotation + index * 5 + slot * 10) % 20 + 1) as u8;
                    let position = rotation + slot;
                    let setup = bot.player(level, position)?;
                    let template = bot.avatar_pool[position % bot.avatar_pool.len()];
                    let profile = pb::PublicPlayer {
                        id: format!("ai:{rotation}:{index}:{slot}"),
                        login: format!("[AI] {} {level}", bot.name_en),
                        template_id: u32::from(template),
                        level: u32::from(level),
                        stats: engine::Stat::ALL
                            .map(|s| setup.avatar.stats.get(s))
                            .to_vec(),
                        schools: setup.avatar.schools.0.map(u32::from).to_vec(),
                        ..Default::default()
                    };
                    Some((profile, setup, bot.id.clone()))
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

pub async fn ranking(db: &PgPool, request: pb::RankingGet) -> Result<Body> {
    let offset = request.offset.min(10_000) as i64;
    let limit = request.limit.clamp(1, 50) as i64;
    let ids=sqlx::query_scalar!("SELECT r.user_id FROM ratings r JOIN users u ON u.id=r.user_id WHERE u.status='active' AND u.email_verified_at IS NOT NULL ORDER BY r.rating DESC,r.user_id LIMIT $1 OFFSET $2",limit+1,offset).fetch_all(db).await?;
    let more = ids.len() > limit as usize;
    let mut players = Vec::new();
    for user in ids.into_iter().take(limit as usize) {
        players.push(repo::player(db, user).await?);
    }
    Ok(Body::RankingSnapshot(pb::RankingSnapshot {
        players,
        next_offset: if more { (offset + limit) as u32 } else { 0 },
    }))
}
pub async fn history(db: &PgPool, user: Uuid, request: pb::MatchHistoryGet) -> Result<Body> {
    let user = if request.player.is_empty() {
        user
    } else {
        repo::target(db, &request.player).await?
    };
    let before = if request.before_id.is_empty() {
        None
    } else {
        Some(Uuid::parse_str(&request.before_id).map_err(|_| Error("INVALID_MESSAGE"))?)
    };
    let limit = i64::from(request.limit.clamp(1, 50));
    let mut rows=sqlx::query!("SELECT m.id,m.kind,a.login::text AS \"first!\",COALESCE(b.login::text,'AI') AS \"second!\",m.winner,m.reason,(extract(epoch FROM m.created_at)*1000)::bigint AS \"at!\" FROM matches m JOIN users a ON a.id=m.first_user LEFT JOIN users b ON b.id=m.second_user WHERE (m.first_user=$1 OR m.second_user=$1) AND ($2::uuid IS NULL OR m.id<$2) ORDER BY m.id DESC LIMIT $3",user,before,limit+1).fetch_all(db).await?;
    let more = rows.len() > limit as usize;
    rows.truncate(limit as usize);
    let next_cursor = if more {
        rows.last().map(|r| r.id.to_string()).unwrap_or_default()
    } else {
        String::new()
    };
    Ok(Body::MatchHistory(pb::MatchHistory {
        matches: rows
            .into_iter()
            .map(|r| pb::MatchSummary {
                id: r.id.to_string(),
                kind: r.kind,
                first: r.first,
                second: r.second,
                winner: r.winner as u32,
                reason: r.reason,
                at_ms: r.at,
            })
            .collect(),
        next_cursor,
    }))
}
async fn completed(db: &PgPool, id: Uuid) -> Result<Record> {
    if !sqlx::query_scalar!(
        "SELECT EXISTS(SELECT 1 FROM matches WHERE id=$1) AS \"found!\"",
        id
    )
    .fetch_one(db)
    .await?
    {
        return Err(Error("NOT_FOUND"));
    }
    journal::load(db, id).await
}
pub async fn finished_snapshot(db: &PgPool, user: Uuid, id: Uuid) -> Result<Body> {
    if let Some(data)=sqlx::query_scalar!("SELECT CASE WHEN m.first_user=$2 THEN p.first_snapshot ELSE p.second_snapshot END AS \"data!\" FROM match_projections p JOIN matches m ON m.id=p.match_id WHERE m.id=$1 AND (m.first_user=$2 OR m.second_user=$2)",id,user).fetch_optional(db).await? {
        return Ok(Body::DuelSnapshot(Box::new(pb::DuelSnapshot::decode(data.as_slice()).map_err(|_|Error("INVALID_JOURNAL"))?)));
    }
    let record = completed(db, id).await?;
    let side = record.side(user)?;
    let (state, events) = record.rebuild()?;
    let rewards = crate::duel::result::settle(db, Uuid::nil(), &record, &state).await?;
    let mut snapshot = actor::snapshot(&record, &state, &events, side)?;
    snapshot.result = Some(pb::DuelEnd {
        duel_id: id.to_string(),
        outcome: snapshot.view.as_ref().and_then(|v| v.outcome.clone()),
        xp_gained: rewards.xp[side.index()],
        rating_delta: rewards.delta[side.index()],
        rating_new: rewards.rating[side.index()],
        seed: record.seed.to_vec(),
        unlocked_template: rewards.unlock[side.index()].map_or(0, u32::from),
        ducats_gained: rewards.ducats[side.index()],
    });
    Ok(Body::DuelSnapshot(Box::new(snapshot)))
}
pub async fn replay(db: &PgPool, id: Uuid) -> Result<Body> {
    if let Some(data) =
        sqlx::query_scalar!("SELECT replay FROM match_projections WHERE match_id=$1", id)
            .fetch_optional(db)
            .await?
    {
        return Ok(Body::MatchReplayData(
            pb::MatchReplayData::decode(data.as_slice()).map_err(|_| Error("INVALID_JOURNAL"))?,
        ));
    }
    Ok(Body::MatchReplayData(actor::replay(
        &completed(db, id).await?,
    )?))
}
