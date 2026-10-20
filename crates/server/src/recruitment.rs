//! Guaranteed, duplicate-free class choices earned by persistent account level.
use crate::{
    game,
    online::{Error, Result},
};
use serde::Serialize;
use sqlx::PgConnection;
use uuid::Uuid;

pub const MILESTONES: [u8; 7] = [5, 8, 12, 16, 20, 24, 28];

#[derive(Serialize)]
pub struct Snapshot {
    pub level: u8,
    pub available: Vec<u16>,
    pub milestones: Vec<Milestone>,
}
#[derive(Serialize)]
pub struct Milestone {
    pub level: u8,
    pub claimed: bool,
    pub template_id: Option<i32>,
    pub avatar_id: Option<Uuid>,
}
pub async fn load(db: &mut PgConnection, user: Uuid, owned: &[i32]) -> Result<Snapshot> {
    let xp: i64 = sqlx::query_scalar("SELECT account_xp FROM users WHERE id=$1")
        .bind(user)
        .fetch_one(&mut *db)
        .await?;
    let claims: Vec<(i16, Option<i32>, Option<Uuid>)> = sqlx::query_as(
        "SELECT milestone,template_id,avatar_id FROM recruitment_claims WHERE user_id=$1",
    )
    .bind(user)
    .fetch_all(db)
    .await?;
    Ok(Snapshot {
        level: crate::journey::level(xp),
        available: engine_content::AVATARS
            .iter()
            .filter(|a| {
                a.playable
                    && a.rarity != engine_content::AvatarRarity::Shade
                    && !owned.contains(&i32::from(a.id))
            })
            .map(|a| a.id)
            .collect(),
        milestones: MILESTONES
            .into_iter()
            .map(|level| {
                let claim = claims.iter().find(|(n, _, _)| *n == i16::from(level));
                Milestone {
                    level,
                    claimed: claim.is_some(),
                    template_id: claim.and_then(|(_, template, _)| *template),
                    avatar_id: claim.and_then(|(_, _, avatar)| *avatar),
                }
            })
            .collect(),
    })
}
pub async fn pending(db: &mut PgConnection, user: Uuid, level: u8) -> Result<usize> {
    let claimed: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM recruitment_claims WHERE user_id=$1 AND milestone<=$2",
    )
    .bind(user)
    .bind(i16::from(level))
    .fetch_one(db)
    .await?;
    Ok(MILESTONES
        .iter()
        .filter(|n| **n <= level)
        .count()
        .saturating_sub(claimed as usize))
}

/// Caller holds the user row lock and the activity guard, in the game mutation transaction.
pub async fn claim(
    db: &mut PgConnection,
    user: Uuid,
    snapshot: &game::Snapshot,
    level: u8,
    template: Option<u16>,
) -> Result<()> {
    let progress = &snapshot.recruitment;
    let milestone = progress
        .milestones
        .iter()
        .find(|m| m.level == level)
        .ok_or(Error("INVALID_MILESTONE"))?;
    if milestone.claimed {
        return if milestone.template_id == template.map(i32::from) {
            Ok(())
        } else {
            Err(Error("CONFLICT"))
        };
    }
    if progress.level < level {
        return Err(Error("LEVEL_REQUIRED"));
    }
    let avatar = if progress.available.is_empty() {
        if template.is_some() {
            return Err(Error("RECRUIT_UNAVAILABLE"));
        }
        None
    } else {
        let template = template.ok_or(Error("RECRUIT_CHOICE_REQUIRED"))?;
        if !progress.available.contains(&template) {
            return Err(Error("RECRUIT_UNAVAILABLE"));
        }
        let id = Uuid::now_v7();
        sqlx::query("INSERT INTO unlocked_templates(user_id,template_id,source) VALUES($1,$2,'recruitment')")
            .bind(user).bind(i32::from(template)).execute(&mut *db).await?;
        game::create_avatar(db, user, snapshot, id, template).await?;
        Some(id)
    };
    sqlx::query("INSERT INTO recruitment_claims(user_id,milestone,template_id,avatar_id) VALUES($1,$2,$3,$4)")
        .bind(user).bind(i16::from(level)).bind(template.map(i32::from)).bind(avatar)
        .execute(db).await?;
    Ok(())
}
