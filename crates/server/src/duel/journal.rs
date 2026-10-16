use super::{Error, Result, record::Record};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

pub async fn create(
    tx: &mut PgConnection,
    record: &Record,
    instance: Uuid,
    lease_ms: i64,
) -> Result<()> {
    create_checked(tx, record, instance, lease_ms, None).await
}
/// The caller holds the participants' user rows; only this exact activity may start a duel.
pub async fn create_for_activity(
    tx: &mut PgConnection,
    record: &Record,
    instance: Uuid,
    lease_ms: i64,
    kind: &str,
    activity: Uuid,
) -> Result<()> {
    if record.kind != kind || !matches!(kind, "tournament" | "expedition") {
        return Err(Error("INVALID_MESSAGE"));
    }
    create_checked(tx, record, instance, lease_ms, Some((kind, activity))).await
}
async fn create_checked(
    tx: &mut PgConnection,
    record: &Record,
    instance: Uuid,
    lease_ms: i64,
    activity: Option<(&str, Uuid)>,
) -> Result<()> {
    for user in record.seats.iter().filter_map(|s| s.user) {
        if let Some((kind, owner)) = activity {
            let allowed: bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM activity_locks WHERE user_id=$1 AND kind=$2 AND owner_id=$3) AND NOT EXISTS(SELECT 1 FROM duel_locks WHERE user_id=$1)").bind(user).bind(kind).bind(owner).fetch_one(&mut *tx).await?;
            if !allowed {
                return Err(Error("BUSY"));
            }
        } else {
            crate::activity::ensure_idle(&mut *tx, user).await?;
        }
    }
    let data = record.bytes()?;
    sqlx::query!("INSERT INTO duel_journal(id,kind,engine_version,content_version,data,owner_instance,lease_until) VALUES ($1,$2,$3,$4,$5,$6,now()+($7::bigint*interval '1 millisecond'))",record.id,record.kind,record.engine_version,&record.content_version[..],data,instance,lease_ms).execute(&mut *tx).await?;
    let mut users: Vec<_> = record.seats.iter().filter_map(|s| s.user).collect();
    users.sort();
    for user in users {
        let changed = sqlx::query!(
            "INSERT INTO duel_locks(user_id,duel_id) VALUES ($1,$2) ON CONFLICT DO NOTHING",
            user,
            record.id
        )
        .execute(&mut *tx)
        .await?;
        if changed.rows_affected() != 1 {
            return Err(Error("BUSY"));
        }
    }
    Ok(())
}
pub async fn save(
    db: &PgPool,
    instance: Uuid,
    previous: &Record,
    next: &Record,
    state: &engine::DuelState,
) -> Result<()> {
    let data = next.bytes()?;
    let mut tx = db.begin().await?;
    let changed=sqlx::query!("UPDATE duel_journal SET data=$4,revision=$5,updated_at=now() WHERE id=$1 AND owner_instance=$2 AND revision=$3 AND status='active' AND lease_until>clock_timestamp()",next.id,instance,i64::from(previous.revision),data,i64::from(next.revision)).execute(&mut *tx).await?;
    if changed.rows_affected() != 1 {
        return Err(Error("LEASE_LOST"));
    }
    if next.moves.len() > previous.moves.len() {
        let round = next.moves.len() as i16;
        let moves = serde_json::to_vec(next.moves.last().ok_or(Error("INTERNAL"))?)
            .map_err(|_| Error("INTERNAL"))?;
        let hash = super::state_digest(state);
        sqlx::query!(
            "INSERT INTO duel_journal_rounds(duel_id,round,moves,state_hash) VALUES ($1,$2,$3,$4)",
            next.id,
            round,
            moves,
            &hash[..]
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}
pub async fn renew(db: &PgPool, id: Uuid, instance: Uuid, lease_ms: i64) -> Result<()> {
    let changed=sqlx::query!("UPDATE duel_journal SET lease_until=now()+($3::bigint*interval '1 millisecond') WHERE id=$1 AND owner_instance=$2 AND status='active' AND lease_until>clock_timestamp()",id,instance,lease_ms).execute(db).await?;
    if changed.rows_affected() != 1 {
        return Err(Error("LEASE_LOST"));
    }
    Ok(())
}
pub async fn release_lease(db: &PgPool, id: Uuid, instance: Uuid) -> Result<()> {
    sqlx::query!("UPDATE duel_journal SET lease_until=now() WHERE id=$1 AND owner_instance=$2 AND status='active'",id,instance).execute(db).await?;
    Ok(())
}
pub async fn recover(db: &PgPool, instance: Uuid, lease_ms: i64) -> Result<Vec<Record>> {
    // An incompatible worker must never claim a lease belonging to the retained release.
    let rows=sqlx::query_as::<_, (Uuid, Vec<u8>)>("UPDATE duel_journal SET owner_instance=$1,lease_until=now()+($2::bigint*interval '1 millisecond') WHERE id IN (SELECT id FROM duel_journal WHERE status='active' AND lease_until<=clock_timestamp() AND engine_version=$3 AND content_version=$4 ORDER BY lease_until LIMIT 100 FOR UPDATE SKIP LOCKED) RETURNING id,data")
        .bind(instance).bind(lease_ms).bind(engine::VERSION).bind(&engine_content::CONTENT_VERSION[..]).fetch_all(db).await?;
    Ok(rows
        .into_iter()
        .filter_map(|(id, data)| match Record::decode(&data) {
            Ok(record) if record.id == id => Some(record),
            _ => {
                tracing::error!(duel=%id,"invalid journal requires operator recovery");
                None
            }
        })
        .collect())
}
pub async fn load(db: &PgPool, id: Uuid) -> Result<Record> {
    let data = sqlx::query_scalar!("SELECT data FROM duel_journal WHERE id=$1", id)
        .fetch_optional(db)
        .await?
        .ok_or(Error("NOT_FOUND"))?;
    Record::decode(&data)
}

pub async fn verify_rounds(db: &PgPool, record: &Record) -> Result<()> {
    let rows = sqlx::query!(
        "SELECT round,moves,state_hash FROM duel_journal_rounds WHERE duel_id=$1 ORDER BY round",
        record.id
    )
    .fetch_all(db)
    .await?;
    if rows.len() != record.moves.len() {
        return Err(Error("INVALID_JOURNAL"));
    }
    for (index, row) in rows.into_iter().enumerate() {
        let moves: [Vec<u8>; 2] =
            serde_json::from_slice(&row.moves).map_err(|_| Error("INVALID_JOURNAL"))?;
        if row.round as usize != index + 1 || moves != record.moves[index] {
            return Err(Error("INVALID_JOURNAL"));
        }
        let mut partial = record.clone();
        partial.moves.truncate(index + 1);
        partial.ended = None;
        let (state, _) = partial.rebuild()?;
        if row.state_hash != super::state_digest(&state) {
            return Err(Error("INVALID_JOURNAL"));
        }
    }
    Ok(())
}
