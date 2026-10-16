use super::*;
use crate::{
    duel::{Settings, record::Record},
    online::repo,
};
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

#[derive(Clone, Deserialize, Serialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum Mutation {
    Start {
        id: Uuid,
        avatar: Uuid,
        difficulty: String,
    },
    Choose {
        id: Uuid,
        request: Uuid,
        revision: i32,
        choice: u8,
    },
    Retreat {
        id: Uuid,
        request: Uuid,
        revision: i32,
    },
}
pub struct Choice {
    pub id: Uuid,
    pub request: Uuid,
    pub revision: i32,
    pub choice: u8,
}
#[derive(sqlx::FromRow)]
pub(super) struct Run {
    pub id: Uuid,
    pub user_id: Uuid,
    pub avatar_id: Uuid,
    pub difficulty: String,
    pub content_version: Vec<u8>,
    pub engine_version: String,
    pub plan: Vec<u8>,
    pub status: String,
    pub stage: i16,
    pub xp_accum: i32,
    pub active_duel: Option<Uuid>,
    pub revision: i32,
    pub award: Option<Vec<u8>>,
}
pub(super) async fn locked(connection: &mut PgConnection, user: Uuid, id: Uuid) -> Result<Run> {
    sqlx::query("SELECT id FROM users WHERE id=$1 FOR UPDATE")
        .bind(user)
        .fetch_one(&mut *connection)
        .await?;
    let run:Run=sqlx::query_as("SELECT id,user_id,avatar_id,difficulty,content_version,engine_version,plan,status,stage,xp_accum,active_duel,revision,award FROM expeditions WHERE id=$1 AND user_id=$2 FOR UPDATE").bind(id).bind(user).fetch_optional(connection).await?.ok_or(Error("NOT_FOUND"))?;
    if run.content_version != engine_content::CONTENT_VERSION
        || run.engine_version != engine::VERSION
    {
        return Err(Error("CONTENT_MISMATCH"));
    }
    Ok(run)
}
async fn eligible(connection: &mut PgConnection, user: Uuid) -> Result<()> {
    let allowed: bool = sqlx::query_scalar(
        "SELECT status='active' AND email_verified_at IS NOT NULL FROM users WHERE id=$1",
    )
    .bind(user)
    .fetch_optional(connection)
    .await?
    .unwrap_or(false);
    if allowed {
        Ok(())
    } else {
        Err(Error("FORBIDDEN"))
    }
}
pub async fn start(
    db: &PgPool,
    user: Uuid,
    id: Uuid,
    avatar: Uuid,
    difficulty: &str,
) -> Result<()> {
    CONFIG.difficulty(difficulty)?;
    let mut tx = db.begin().await?;
    sqlx::query("SELECT id FROM users WHERE id=$1 FOR UPDATE")
        .bind(user)
        .fetch_one(&mut *tx)
        .await?;
    eligible(&mut tx, user).await?;
    if let Some((owner, old_avatar, old_difficulty)) = sqlx::query_as::<_, (Uuid, Uuid, String)>(
        "SELECT user_id,avatar_id,difficulty FROM expeditions WHERE id=$1",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?
    {
        if owner == user && old_avatar == avatar && old_difficulty == difficulty {
            return Ok(());
        }
        return Err(Error("CONFLICT"));
    }
    crate::activity::ensure_idle(&mut tx, user).await?;
    let prepared = repo::prepare_player(&mut tx, user, &avatar.to_string()).await?;
    let profile = repo::player_on(&mut tx, user).await?;
    let owned: Vec<i32> =
        sqlx::query_scalar("SELECT template_id FROM unlocked_templates WHERE user_id=$1")
            .bind(user)
            .fetch_all(&mut *tx)
            .await?;
    let mut seed = [0; 32];
    getrandom::fill(&mut seed).map_err(|_| Error("INTERNAL"))?;
    let plan = model::Plan::new(
        crate::duel::record::Seat::player(user, avatar, &prepared.setup, &profile),
        difficulty,
        &CONFIG,
        &owned,
        seed,
    )?;
    let available: bool = sqlx::query_scalar("SELECT balance<=$2 FROM wallets WHERE user_id=$1")
        .bind(user)
        .bind(
            i64::MAX
                - i64::from(plan.difficulty.fixed_ducats)
                - i64::from(plan.difficulty.random_ducats),
        )
        .fetch_one(&mut *tx)
        .await?;
    if !available {
        return Err(Error("BALANCE_LIMIT"));
    }
    sqlx::query("INSERT INTO expeditions(id,user_id,avatar_id,difficulty,content_version,plan,engine_version) VALUES ($1,$2,$3,$4,$5,$6,$7)")
        .bind(id).bind(user).bind(avatar).bind(difficulty).bind(engine_content::CONTENT_VERSION.as_slice()).bind(encode(&plan)?).bind(engine::VERSION).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO activity_locks(user_id,kind,owner_id) VALUES ($1,'expedition',$2)")
        .bind(user)
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}
async fn repeated(
    connection: &mut PgConnection,
    user: Uuid,
    key: Uuid,
    id: Uuid,
    action: &str,
    revision: i32,
    choice: Option<u8>,
) -> Result<bool> {
    let fingerprint = Sha256::digest(encode(&(id, action, revision, choice))?).to_vec();
    if let Some(old) = sqlx::query_scalar::<_, Vec<u8>>(
        "SELECT fingerprint FROM expedition_requests WHERE user_id=$1 AND request_id=$2",
    )
    .bind(user)
    .bind(key)
    .fetch_optional(&mut *connection)
    .await?
    {
        if old != fingerprint {
            return Err(Error("CONFLICT"));
        }
        return Ok(true);
    }
    sqlx::query("INSERT INTO expedition_requests(user_id,request_id,expedition_id,fingerprint) VALUES ($1,$2,$3,$4)").bind(user).bind(key).bind(id).bind(fingerprint).execute(connection).await?;
    Ok(false)
}
pub async fn choose(
    db: &PgPool,
    user: Uuid,
    request: Choice,
    instance: Uuid,
    settings: &Settings,
) -> Result<Option<Record>> {
    let Choice {
        id,
        request: key,
        revision,
        choice,
    } = request;
    let mut tx = db.begin().await?;
    let run = locked(&mut tx, user, id).await?;
    if repeated(&mut tx, user, key, id, "choose", revision, Some(choice)).await? {
        return Ok(None);
    }
    eligible(&mut tx, user).await?;
    if run.status != "active" || run.revision != revision {
        return Err(Error("CONFLICT"));
    }
    if run.active_duel.is_some() {
        return Err(Error("BUSY"));
    }
    let plan: model::Plan = decode(&run.plan)?;
    let stage = u8::try_from(run.stage).map_err(|_| Error("INVALID_JOURNAL"))?;
    let enemy = plan.enemy(stage, choice)?;
    let now = crate::online::now_ms();
    let mut record = Record::new(
        Uuid::now_v7(),
        "expedition",
        [plan.human.clone(), enemy.seat.clone()],
        plan.duel_seed(stage),
        settings,
    )?;
    record.connected = [false, true];
    record.grace_until = [Some(now + settings.reconnect_ms()), None];
    crate::duel::journal::create_for_activity(
        &mut tx,
        &record,
        instance,
        settings.lease_ms(),
        "expedition",
        id,
    )
    .await?;
    sqlx::query(
        "INSERT INTO expedition_fights(duel_id,expedition_id,stage,choice) VALUES ($1,$2,$3,$4)",
    )
    .bind(record.id)
    .bind(id)
    .bind(run.stage)
    .bind(i16::from(choice))
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE expeditions SET active_duel=$2,revision=revision+1 WHERE id=$1")
        .bind(id)
        .bind(record.id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Some(record))
}
pub async fn retreat(db: &PgPool, user: Uuid, id: Uuid, key: Uuid, revision: i32) -> Result<()> {
    let mut tx = db.begin().await?;
    let run = locked(&mut tx, user, id).await?;
    if repeated(&mut tx, user, key, id, "retreat", revision, None).await? {
        return Ok(());
    }
    if run.status != "active" || run.revision != revision {
        return Err(Error("CONFLICT"));
    }
    if run.active_duel.is_some() {
        return Err(Error("BUSY"));
    }
    let plan: model::Plan = decode(&run.plan)?;
    let nominal = (u64::try_from(run.xp_accum).map_err(|_| Error("INVALID_JOURNAL"))?
        * run.stage as u64
        / u64::from(plan.total_stages())) as u32;
    let xp = super::rewards::grant_xp(&mut tx, user, run.avatar_id, nominal).await?;
    finish(
        &mut tx,
        &run,
        "retreated",
        &model::Award {
            xp,
            ..Default::default()
        },
    )
    .await?;
    tx.commit().await?;
    Ok(())
}
pub(super) async fn finish(
    connection: &mut PgConnection,
    run: &Run,
    status: &str,
    award: &model::Award,
) -> Result<()> {
    sqlx::query("UPDATE expeditions SET status=$2,active_duel=NULL,award=$3,ended_at=now(),revision=revision+1 WHERE id=$1").bind(run.id).bind(status).bind(encode(award)?).execute(&mut *connection).await?;
    sqlx::query(
        "DELETE FROM activity_locks WHERE user_id=$1 AND kind='expedition' AND owner_id=$2",
    )
    .bind(run.user_id)
    .bind(run.id)
    .execute(connection)
    .await?;
    Ok(())
}
