use super::*;
use crate::gateway::AppState;
use axum::{
    Json,
    extract::{Query, State},
    http::HeaderMap,
};
use sqlx::PgConnection;
use uuid::Uuid;

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Parameters {
    id: Option<Uuid>,
}
#[derive(Serialize)]
pub struct Enemy {
    choice: u8,
    template_id: u16,
    level: u8,
    color: &'static str,
    boss: bool,
}
#[derive(Serialize)]
pub struct View {
    id: Uuid,
    avatar: Uuid,
    difficulty: String,
    status: String,
    stage: i16,
    stages: u32,
    xp_accum: i32,
    retreat_xp: u32,
    active_duel: Option<Uuid>,
    revision: i32,
    choices: Vec<Enemy>,
    award: Option<model::Award>,
}
#[derive(Serialize)]
pub struct Difficulty {
    id: String,
    stages: u32,
    fixed_ducats: u32,
    random_ducats: u32,
    rarity: String,
}
#[derive(Serialize)]
pub struct Snapshot {
    content_version: String,
    account_level: u8,
    run: Option<View>,
    difficulties: Vec<Difficulty>,
    avatar_chance_bps: u32,
    card_chance_bps: u32,
    ducats_chance_bps: u32,
}
pub async fn load(connection: &mut PgConnection, user: Uuid, id: Option<Uuid>) -> Result<Snapshot> {
    let run:Option<repository::Run>=sqlx::query_as("SELECT id,user_id,avatar_id,difficulty,content_version,engine_version,plan,status,stage,xp_accum,active_duel,revision,award FROM expeditions WHERE user_id=$1 AND ($2::uuid IS NULL OR id=$2) ORDER BY (status='active') DESC,created_at DESC LIMIT 1").bind(user).bind(id).fetch_optional(&mut *connection).await?;
    if id.is_some() && run.is_none() {
        return Err(Error("NOT_FOUND"));
    }
    let run = if let Some(run) = run {
        if run.content_version != engine_content::CONTENT_VERSION && run.status == "active" {
            return Err(Error("CONTENT_MISMATCH"));
        }
        let plan: model::Plan = decode(&run.plan)?;
        let level = plan.human.public()?.level as i16;
        let choices = if run.status != "active" || run.active_duel.is_some() {
            vec![]
        } else if run.stage as usize == plan.stages.len() {
            vec![(0, &plan.boss)]
        } else {
            plan.stages
                .get(run.stage as usize)
                .ok_or(Error("INVALID_JOURNAL"))?
                .iter()
                .enumerate()
                .map(|(i, e)| (i as u8, e))
                .collect()
        }
        .into_iter()
        .map(|(choice, enemy)| Enemy {
            choice,
            template_id: enemy.template,
            level: enemy.level,
            color: match i16::from(enemy.level) - level {
                ..0 => "lower",
                0 => "equal",
                1..=2 => "higher",
                _ => "danger",
            },
            boss: enemy.boss,
        })
        .collect();
        let retreat_xp =
            (run.xp_accum as u64 * run.stage as u64 / u64::from(plan.total_stages())) as u32;
        Some(View {
            id: run.id,
            avatar: run.avatar_id,
            difficulty: run.difficulty,
            status: run.status,
            stage: run.stage,
            stages: plan.total_stages(),
            xp_accum: run.xp_accum,
            retreat_xp,
            active_duel: run.active_duel,
            revision: run.revision,
            choices,
            award: run.award.as_deref().map(decode).transpose()?,
        })
    } else {
        None
    };
    Ok(Snapshot {
        content_version: crate::auth::crypto::hex(&engine_content::CONTENT_VERSION),
        account_level: crate::journey::level(
            sqlx::query_scalar("SELECT account_xp FROM users WHERE id=$1")
                .bind(user)
                .fetch_one(&mut *connection)
                .await?,
        ),
        run,
        difficulties: CONFIG
            .difficulties
            .iter()
            .map(|d| Difficulty {
                id: d.id.clone(),
                stages: u32::from(d.stages) + 1,
                fixed_ducats: d.fixed_ducats,
                random_ducats: d.random_ducats,
                rarity: d.rarity.clone(),
            })
            .collect(),
        avatar_chance_bps: CONFIG.avatar_chance_bps,
        card_chance_bps: CONFIG.card_chance_bps,
        ducats_chance_bps: CONFIG.ducats_chance_bps,
    })
}
pub async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
    query: std::result::Result<Query<Parameters>, axum::extract::rejection::QueryRejection>,
) -> std::result::Result<Json<Snapshot>, crate::game::HttpError> {
    let session = crate::auth::security::session(&state, &headers)
        .await
        .map_err(crate::game::HttpError)?;
    crate::journey::require(&state.db, session.user, "portal")
        .await
        .map_err(crate::game::response)?;
    state
        .auth
        .limit("expedition-read", &session.user.to_string(), 60, 60)
        .map_err(crate::game::HttpError)?;
    let Query(query) = query.map_err(|_| crate::game::response(Error("INVALID_MESSAGE")))?;
    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|e| crate::game::response(e.into()))?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await
        .map_err(|e| crate::game::response(e.into()))?;
    let view = load(&mut tx, session.user, query.id)
        .await
        .map_err(crate::game::response)?;
    tx.commit()
        .await
        .map_err(|e| crate::game::response(e.into()))?;
    Ok(Json(view))
}
pub async fn post(
    State(state): State<AppState>,
    headers: HeaderMap,
    mutation: std::result::Result<Json<Mutation>, axum::extract::rejection::JsonRejection>,
) -> std::result::Result<Json<Uuid>, crate::game::HttpError> {
    let session = crate::auth::security::session(&state, &headers)
        .await
        .map_err(crate::game::HttpError)?;
    crate::journey::require(&state.db, session.user, "portal")
        .await
        .map_err(crate::game::response)?;
    state
        .auth
        .limit("expedition-write", &session.user.to_string(), 30, 60)
        .map_err(crate::game::HttpError)?;
    let Json(mutation) = mutation.map_err(|_| crate::game::response(Error("INVALID_MESSAGE")))?;
    let id = match mutation {
        Mutation::Start {
            id,
            avatar,
            difficulty,
        } => {
            state
                .auth
                .limit("expedition-start", &session.user.to_string(), 10, 60)
                .map_err(crate::game::HttpError)?;
            start(&state.db, session.user, id, avatar, &difficulty)
                .await
                .map_err(crate::game::response)?;
            id
        }
        Mutation::Choose {
            id,
            request,
            revision,
            choice,
        } => {
            let record = choose(
                &state.db,
                session.user,
                Choice {
                    id,
                    request,
                    revision,
                    choice,
                },
                state.online.hub.duel_instance(),
                &state.online.hub.settings().duels,
            )
            .await
            .map_err(crate::game::response)?;
            if let Some(record) = record {
                state.online.hub.start_durable(record);
            }
            id
        }
        Mutation::Retreat {
            id,
            request,
            revision,
        } => {
            retreat(&state.db, session.user, id, request, revision)
                .await
                .map_err(crate::game::response)?;
            id
        }
    };
    state.online.hub.activity_changed(vec![session.user]).await;
    Ok(Json(id))
}
