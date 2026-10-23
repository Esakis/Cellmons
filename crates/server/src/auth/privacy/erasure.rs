use super::*;
use protocol::{Message as _, game::v1 as pb};
use serde_json::Value;
use sqlx::{PgConnection, PgPool};

fn profile(value: &mut Option<pb::PublicPlayer>, user: Uuid) {
    if let Some(p) = value.as_mut().filter(|p| p.id == user.to_string()) {
        p.login = "Former player".into();
        p.title.clear();
        p.description.clear();
        p.clan_id.clear();
        p.clan_name.clear();
        p.challenge_filter = "none".into();
        p.verified = false;
        p.mentor = false;
    }
}
fn snapshot(value: &mut pb::DuelSnapshot, user: Uuid) {
    profile(&mut value.you, user);
    profile(&mut value.opponent, user);
}
fn frozen(value: &mut Value, user: Uuid, depth: usize) -> Result<()> {
    if depth > 64 {
        return Err(Error::busy());
    }
    match value {
        Value::Array(values) => {
            for value in values {
                frozen(value, user, depth + 1)?;
            }
        }
        Value::Object(values) => {
            if values.get("user").and_then(Value::as_str) == Some(user.to_string().as_str())
                && let Some(bytes) = values.get_mut("profile")
            {
                let raw: Vec<u8> =
                    serde_json::from_value(bytes.clone()).map_err(|_| Error::busy())?;
                let mut player =
                    Some(pb::PublicPlayer::decode(raw.as_slice()).map_err(|_| Error::busy())?);
                profile(&mut player, user);
                *bytes = serde_json::to_value(player.ok_or_else(Error::busy)?.encode_to_vec())
                    .map_err(|_| Error::busy())?;
            }
            for value in values.values_mut() {
                frozen(value, user, depth + 1)?;
            }
        }
        _ => {}
    }
    Ok(())
}
fn redact_json(bytes: Vec<u8>, user: Uuid) -> Result<Vec<u8>> {
    let mut value: Value = serde_json::from_slice(&bytes).map_err(|_| Error::busy())?;
    frozen(&mut value, user, 0)?;
    serde_json::to_vec(&value).map_err(|_| Error::busy())
}
async fn redact_history(tx: &mut PgConnection, user: Uuid) -> Result<()> {
    let ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM matches WHERE first_user=$1 OR second_user=$1 ORDER BY id",
    )
    .bind(user)
    .fetch_all(&mut *tx)
    .await?;
    for id in ids {
        let data:Option<(Vec<u8>,Vec<u8>,Vec<u8>)>=sqlx::query_as("SELECT replay,first_snapshot,second_snapshot FROM match_projections WHERE match_id=$1 FOR UPDATE").bind(id).fetch_optional(&mut *tx).await?;
        if let Some((replay, one, two)) = data {
            let mut replay =
                pb::MatchReplayData::decode(replay.as_slice()).map_err(|_| Error::busy())?;
            for round in &mut replay.rounds {
                snapshot(round, user);
            }
            let mut one = pb::DuelSnapshot::decode(one.as_slice()).map_err(|_| Error::busy())?;
            let mut two = pb::DuelSnapshot::decode(two.as_slice()).map_err(|_| Error::busy())?;
            snapshot(&mut one, user);
            snapshot(&mut two, user);
            sqlx::query("UPDATE match_projections SET replay=$2,first_snapshot=$3,second_snapshot=$4 WHERE match_id=$1").bind(id).bind(replay.encode_to_vec()).bind(one.encode_to_vec()).bind(two.encode_to_vec()).execute(&mut *tx).await?;
        }
        let data: Option<Vec<u8>> = sqlx::query_scalar(
            "SELECT data FROM duel_journal WHERE id=$1 AND status='finished' FOR UPDATE",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(data) = data {
            sqlx::query("UPDATE duel_journal SET data=$2 WHERE id=$1")
                .bind(id)
                .bind(redact_json(data, user)?)
                .execute(&mut *tx)
                .await?;
        }
    }
    let entries: Vec<(Uuid, Vec<u8>)> = sqlx::query_as(
        "SELECT tournament_id,seat FROM tournament_entries WHERE user_id=$1 FOR UPDATE",
    )
    .bind(user)
    .fetch_all(&mut *tx)
    .await?;
    for (id, seat) in entries {
        sqlx::query("UPDATE tournament_entries SET seat=$3 WHERE tournament_id=$1 AND user_id=$2")
            .bind(id)
            .bind(user)
            .bind(redact_json(seat, user)?)
            .execute(&mut *tx)
            .await?;
    }
    let expeditions: Vec<(Uuid, Vec<u8>)> =
        sqlx::query_as("SELECT id,plan FROM expeditions WHERE user_id=$1 FOR UPDATE")
            .bind(user)
            .fetch_all(&mut *tx)
            .await?;
    for (id, plan) in expeditions {
        sqlx::query("UPDATE expeditions SET plan=$2 WHERE id=$1")
            .bind(id)
            .bind(redact_json(plan, user)?)
            .execute(&mut *tx)
            .await?;
    }
    Ok(())
}
/// Scheduled separately from realtime actors. Each account commits or rolls back as a unit.
pub async fn cleanup(db: &PgPool) -> Result<u64> {
    let candidates:Vec<Uuid>=sqlx::query_scalar("SELECT id FROM users WHERE status='pending_deletion' AND deletion_requested_at<=now()-interval '30 days' ORDER BY id LIMIT 100").fetch_all(db).await?;
    let mut erased = 0;
    let mut failures = 0;
    for user in candidates {
        match erase_one(db, user).await {
            Ok(true) => erased += 1,
            Ok(false) => {}
            Err(_) => {
                failures += 1;
                tracing::error!(account=%user,"account erasure rolled back; operator review required");
            }
        }
    }
    if failures > 0 {
        return Err(Error::busy());
    }
    Ok(erased)
}
async fn erase_one(db: &PgPool, user: Uuid) -> Result<bool> {
    let mut tx = db.begin().await?;
    sqlx::query("SET LOCAL lock_timeout='5s'")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL statement_timeout='30s'")
        .execute(&mut *tx)
        .await?;
    let exists:Option<Uuid>=sqlx::query_scalar("SELECT id FROM users WHERE id=$1 AND status='pending_deletion' AND deletion_requested_at<=now()-interval '30 days' FOR UPDATE SKIP LOCKED").bind(user).fetch_optional(&mut *tx).await?;
    if exists.is_none() {
        return Ok(false);
    }
    let busy:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM duel_locks WHERE user_id=$1) OR EXISTS(SELECT 1 FROM activity_locks WHERE user_id=$1) OR EXISTS(SELECT 1 FROM clan_members WHERE user_id=$1)").bind(user).fetch_one(&mut *tx).await?;
    if busy {
        tracing::warn!("account erasure deferred until activity release");
        return Ok(false);
    }
    redact_history(&mut tx, user).await?;
    let balance: Option<i64> =
        sqlx::query_scalar("SELECT balance FROM wallets WHERE user_id=$1 FOR UPDATE")
            .bind(user)
            .fetch_optional(&mut *tx)
            .await?;
    if let Some(balance) = balance {
        crate::economy::transfer(
            &mut tx,
            crate::economy::Account::Player(user),
            crate::economy::Account::Sink,
            balance,
            "admin",
            Uuid::now_v7(),
        )
        .await
        .map_err(|_| Error::busy())?;
    }
    macro_rules! delete {
        ($sql:literal) => {
            sqlx::query($sql).bind(user).execute(&mut *tx).await?;
        };
    }
    delete!("DELETE FROM sessions WHERE user_id=$1");
    delete!("DELETE FROM email_tokens WHERE user_id=$1");
    delete!("DELETE FROM admin_totp WHERE user_id=$1");
    delete!("DELETE FROM friends WHERE user_id=$1 OR friend_id=$1");
    delete!("DELETE FROM ignores WHERE user_id=$1 OR ignored_id=$1");
    delete!("DELETE FROM mail WHERE from_user=$1 OR to_user=$1");
    delete!("DELETE FROM chat_log WHERE from_user=$1 OR to_user=$1");
    delete!("DELETE FROM decks WHERE avatar_id IN (SELECT id FROM avatars WHERE user_id=$1)");
    delete!("DELETE FROM deck_presets WHERE user_id=$1");
    delete!("DELETE FROM collection WHERE user_id=$1");
    delete!("DELETE FROM boosters WHERE user_id=$1");
    delete!("DELETE FROM junkyard WHERE user_id=$1");
    delete!("DELETE FROM daily_counters WHERE user_id=$1");
    delete!("DELETE FROM arena_progress WHERE user_id=$1");
    delete!("DELETE FROM recruitment_claims WHERE user_id=$1");
    delete!("DELETE FROM class_contracts WHERE user_id=$1");
    delete!("DELETE FROM card_mastery WHERE user_id=$1");
    delete!("DELETE FROM daily_quests WHERE user_id=$1");
    delete!("DELETE FROM path_packs WHERE user_id=$1");
    delete!(
        "DELETE FROM incarnation_decks WHERE avatar_id IN (SELECT id FROM avatars WHERE user_id=$1)"
    );
    delete!(
        "DELETE FROM class_trials WHERE avatar_id IN (SELECT id FROM avatars WHERE user_id=$1)"
    );
    delete!(
        "DELETE FROM avatar_advancement WHERE avatar_id IN (SELECT id FROM avatars WHERE user_id=$1)"
    );
    delete!(
        "DELETE FROM legacy_progression WHERE avatar_id IN (SELECT id FROM avatars WHERE user_id=$1)"
    );
    delete!("DELETE FROM idempotency_keys WHERE user_id=$1");
    delete!("DELETE FROM user_notices WHERE user_id=$1");
    delete!("DELETE FROM abuse_signals WHERE user_id=$1");
    sqlx::query(
        "UPDATE avatars SET deleted_at=COALESCE(deleted_at,now()),is_active=false WHERE user_id=$1",
    )
    .bind(user)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE users SET status='deleted',login='deleted_'||replace(id::text,'-',''),email=NULL,password_hash='$argon2id$erased-unrecoverable-account',email_verified_at=NULL,role='player',description='',challenge_filter='none',last_login_at=NULL,muted_until=NULL,banned_until=NULL,deletion_previous_status=NULL WHERE id=$1").bind(user).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO privacy_events(id,user_id,event) VALUES($1,$2,'erased')")
        .bind(Uuid::now_v7())
        .bind(user)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(true)
}
