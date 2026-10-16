use super::*;
use crate::{
    duel::{record::Record, result},
    economy::{self, Account},
};
use sqlx::PgConnection;
use uuid::Uuid;

/// Caller already holds the user row. Only completion or retreat releases escrowed expedition XP.
pub(super) async fn grant_xp(
    connection: &mut PgConnection,
    user: Uuid,
    avatar: Uuid,
    gained: u32,
) -> Result<u32> {
    let (xp,level,template,verified):(i32,i16,i32,bool)=sqlx::query_as("SELECT a.xp,a.level,coalesce(a.path_id,a.template_id),u.email_verified_at IS NOT NULL FROM avatars a JOIN users u ON u.id=a.user_id WHERE a.id=$1 AND a.user_id=$2 FOR UPDATE OF a")
        .bind(avatar).bind(user).fetch_one(&mut *connection).await?;
    let cap = if verified {
        crate::rules::RULES.xp.cap
    } else {
        crate::rules::RULES.xp.unverified_cap
    };
    let updated = xp
        .saturating_add(i32::try_from(gained).unwrap_or(i32::MAX))
        .min(cap)
        .max(xp);
    let actual = (updated - xp) as u32;
    if actual == 0 {
        return Ok(0);
    }
    let new_level = result::level_for_xp(updated).max(level);
    let definition =
        engine_content::avatar(u16::try_from(template).map_err(|_| Error("CONTENT_MISMATCH"))?)
            .ok_or(Error("CONTENT_MISMATCH"))?;
    let points: i16 = definition
        .dev_points
        .iter()
        .filter(|(at, _)| i16::from(*at) > level && i16::from(*at) <= new_level)
        .map(|(_, n)| i16::from(*n))
        .sum();
    sqlx::query("UPDATE avatars SET xp=$2,level=$3,dev_points_unspent=dev_points_unspent+$4,revision=revision+1 WHERE id=$1").bind(avatar).bind(updated).bind(new_level).bind(points).execute(connection).await?;
    Ok(actual)
}
pub(crate) async fn settle_fight(
    connection: &mut PgConnection,
    record: &Record,
    winner: i16,
) -> Result<model::Award> {
    let user = record.seats[0].user.ok_or(Error("INVALID_JOURNAL"))?;
    let id: Uuid =
        sqlx::query_scalar("SELECT expedition_id FROM expedition_fights WHERE duel_id=$1")
            .bind(record.id)
            .fetch_one(&mut *connection)
            .await?;
    let run = repository::locked(connection, user, id).await?;
    if run.status != "active"
        || run.active_duel != Some(record.id)
        || record.seats[0].avatar != Some(run.avatar_id)
        || record.seats[1].user.is_some()
    {
        return Err(Error("INVALID_JOURNAL"));
    }
    let plan: model::Plan = decode(&run.plan)?;
    let allowed: bool = sqlx::query_scalar(
        "SELECT status='active' AND email_verified_at IS NOT NULL FROM users WHERE id=$1",
    )
    .bind(user)
    .fetch_one(&mut *connection)
    .await?;
    let human = record.seats[0].public()?;
    let npc = record.seats[1].public()?;
    let won = winner == 1 && allowed;
    let nominal = if won {
        result::xp_award(
            human.level,
            result::xp_tier(human.template_id as u16, human.tier as i16),
            npc.level,
            result::xp_tier(npc.template_id as u16, npc.tier as i16),
            true,
            false,
        )
    } else {
        0
    };
    sqlx::query("UPDATE expedition_fights SET winner=$2,xp_nominal=$3 WHERE duel_id=$1")
        .bind(record.id)
        .bind(winner)
        .bind(nominal as i32)
        .execute(&mut *connection)
        .await?;
    if !won {
        let award = model::Award::default();
        repository::finish(connection, &run, "lost", &award).await?;
        return Ok(award);
    }
    let accumulated = run
        .xp_accum
        .checked_add(nominal as i32)
        .ok_or(Error("INTERNAL"))?;
    sqlx::query("UPDATE expeditions SET stage=stage+1,xp_accum=$2,active_duel=NULL,revision=revision+1 WHERE id=$1").bind(id).bind(accumulated).execute(&mut *connection).await?;
    if usize::try_from(run.stage).ok() != Some(plan.stages.len()) {
        return Ok(model::Award::default());
    }
    let owned: Vec<i32> =
        sqlx::query_scalar("SELECT template_id FROM unlocked_templates WHERE user_id=$1")
            .bind(user)
            .fetch_all(&mut *connection)
            .await?;
    let mut award = plan.reward(&owned);
    award.xp = grant_xp(connection, user, run.avatar_id, accumulated as u32).await?;
    if let Some(template) = award.template {
        sqlx::query("INSERT INTO unlocked_templates(user_id,template_id,source) VALUES ($1,$2,'expedition') ON CONFLICT DO NOTHING").bind(user).bind(i32::from(template)).execute(&mut *connection).await?;
    }
    if let Some(card) = award.card {
        let changed=sqlx::query("INSERT INTO collection(user_id,card_id,qty) VALUES ($1,$2,1) ON CONFLICT(user_id,card_id) DO UPDATE SET qty=collection.qty+1 WHERE collection.qty<2147483647")
            .bind(user).bind(card as i16).execute(&mut *connection).await?;
        if changed.rows_affected() == 0 {
            award.card = None;
        }
    }
    economy::transfer(
        connection,
        Account::Faucet,
        Account::Player(user),
        i64::from(award.ducats),
        "expedition",
        id,
    )
    .await?;
    repository::finish(connection, &run, "won", &award).await?;
    Ok(award)
}
