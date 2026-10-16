//! Staff mutations are serialized on user rows; every committed action has a receipt and audit.
use super::access::{self, Identity};
use crate::{
    gateway::AppState,
    online::{Error, Result},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::PgConnection;
use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    Warn {
        user: Uuid,
    },
    Ban {
        user: Uuid,
        seconds: Option<i64>,
    },
    Unban {
        user: Uuid,
    },
    Mute {
        user: Uuid,
        seconds: i64,
    },
    Unmute {
        user: Uuid,
    },
    Role {
        user: Uuid,
        role: String,
    },
    ClanEdit {
        clan: Uuid,
        description: String,
        motd: String,
    },
    Filter {
        id: Uuid,
        phrase: String,
        enabled: bool,
    },
    Announcement {
        id: Uuid,
        body: String,
        seconds: i64,
        enabled: bool,
    },
    Maintenance {
        enabled: bool,
        notice: String,
    },
    Resolve {
        report: Uuid,
        accepted: bool,
    },
    SystemTournament {
        id: Uuid,
        name: String,
        terms: crate::tournaments::Terms,
        reward: crate::tournaments::Reward,
    },
    SystemCancel {
        id: Uuid,
    },
    ReviewSignal {
        id: Uuid,
    },
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub request: Uuid,
    pub reason: String,
    pub operation: Operation,
}
#[derive(Serialize)]
pub struct Receipt {
    pub request: Uuid,
    pub applied: bool,
}

pub(super) async fn notice(
    db: &mut PgConnection,
    user: Uuid,
    kind: &str,
    body: &str,
) -> Result<()> {
    let body = if body.chars().count() > 2000 {
        format!("{}…", body.chars().take(1999).collect::<String>())
    } else {
        body.to_owned()
    };
    sqlx::query("INSERT INTO user_notices(id,user_id,kind,body) VALUES ($1,$2,$3,$4)")
        .bind(Uuid::now_v7())
        .bind(user)
        .bind(kind)
        .bind(body)
        .execute(db)
        .await?;
    Ok(())
}
pub async fn apply(state: &AppState, id: &Identity, request: Request) -> Result<Receipt> {
    let reason = crate::online::text(&request.reason, 2000)?;
    if reason.chars().count() < 5 {
        return Err(Error("INVALID_MESSAGE"));
    }
    let fingerprint = Sha256::digest(
        serde_json::to_vec(&("admin-action-v1", &request)).map_err(|_| Error("INTERNAL"))?,
    )
    .to_vec();
    let mut tx = state.db.begin().await?;
    let target = match &request.operation {
        Operation::Warn { user }
        | Operation::Ban { user, .. }
        | Operation::Unban { user }
        | Operation::Mute { user, .. }
        | Operation::Unmute { user }
        | Operation::Role { user, .. } => Some(*user),
        Operation::Resolve { report, .. } => {
            sqlx::query_scalar("SELECT reporter FROM moderation_reports WHERE id=$1")
                .bind(report)
                .fetch_optional(&mut *tx)
                .await?
        }
        _ => None,
    };
    let mut users = vec![id.user()];
    users.extend(target);
    if let Operation::SystemCancel { id } = request.operation {
        users.extend(
            sqlx::query_scalar::<_, Uuid>(
                "SELECT user_id FROM tournament_entries WHERE tournament_id=$1",
            )
            .bind(id)
            .fetch_all(&mut *tx)
            .await?,
        );
    }
    users.sort();
    users.dedup();
    let locked_users = users.clone();
    for user in users {
        sqlx::query("SELECT id FROM users WHERE id=$1 FOR UPDATE")
            .bind(user)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(Error("NOT_FOUND"))?;
    }
    let role = access::require(&mut tx, id).await?;
    if let Some(old) = sqlx::query_scalar::<_, Vec<u8>>(
        "SELECT fingerprint FROM admin_requests WHERE actor=$1 AND request_id=$2",
    )
    .bind(id.user())
    .bind(request.request)
    .fetch_optional(&mut *tx)
    .await?
    {
        if old != fingerprint {
            return Err(Error("CONFLICT"));
        }
        return Ok(Receipt {
            request: request.request,
            applied: false,
        });
    }
    let ordinary = matches!(
        &request.operation,
        Operation::Warn { .. }
            | Operation::Ban { .. }
            | Operation::Unban { .. }
            | Operation::Mute { .. }
            | Operation::Unmute { .. }
            | Operation::ClanEdit { .. }
            | Operation::Resolve { .. }
            | Operation::ReviewSignal { .. }
    );
    if role != "admin" && !ordinary {
        return Err(Error("FORBIDDEN"));
    }
    let mut before = json!({});
    if let Some(user) = target {
        let (target_role, status, verified): (String, String, bool) = sqlx::query_as(
            "SELECT role,status,email_verified_at IS NOT NULL FROM users WHERE id=$1",
        )
        .bind(user)
        .fetch_one(&mut *tx)
        .await?;
        if status == "deleted" && !matches!(&request.operation, Operation::Resolve { .. }) {
            return Err(Error("NOT_FOUND"));
        }
        before = json!({"role":target_role,"status":status,"verified":verified});
        let timing:String=sqlx::query_scalar("SELECT json_build_object('muted_until',muted_until,'banned_until',banned_until)::text FROM users WHERE id=$1").bind(user).fetch_one(&mut *tx).await?;
        before["limits"] = serde_json::from_str(&timing).map_err(|_| Error("INTERNAL"))?;
        // Administrator accounts are managed by the owner's console, preventing self-lockout
        // and races removing the last administrator. Moderators cannot sanction staff.
        if !matches!(&request.operation, Operation::Resolve { .. })
            && (target_role == "admin"
                || (role != "admin" && target_role == "moderator")
                || user == id.user())
        {
            return Err(Error("FORBIDDEN"));
        }
        if !matches!(&request.operation, Operation::Resolve { .. })
            && !matches!(status.as_str(), "active" | "banned")
            && !(status == "pending_deletion"
                && matches!(
                    &request.operation,
                    Operation::Ban { .. } | Operation::Unban { .. }
                ))
        {
            return Err(Error("FORBIDDEN"));
        }
        if matches!(&request.operation, Operation::Role { .. }) && !verified {
            return Err(Error("UNVERIFIED"));
        }
    }
    let (action, audit_target) = match &request.operation {
        Operation::Warn { user } => {
            notice(&mut tx, *user, "warning", &reason).await?;
            ("warn", Some(*user))
        }
        Operation::Ban { user, seconds } => {
            if seconds.is_some_and(|n| !(60..=365 * 86400).contains(&n))
                || (role != "admin" && seconds.is_none_or(|n| n > 7 * 86400))
            {
                return Err(Error("INVALID_MESSAGE"));
            }
            sqlx::query("UPDATE users SET deletion_previous_status=CASE WHEN status='pending_deletion' THEN 'banned' ELSE deletion_previous_status END,status=CASE WHEN status='pending_deletion' THEN status ELSE 'banned' END,banned_until=CASE WHEN $2::bigint IS NULL THEN NULL ELSE clock_timestamp()+$2*interval '1 second' END WHERE id=$1").bind(user).bind(seconds).execute(&mut *tx).await?;
            sqlx::query("DELETE FROM admin_sessions WHERE user_id=$1")
                .bind(user)
                .execute(&mut *tx)
                .await?;
            notice(&mut tx, *user, "ban", &reason).await?;
            ("ban", Some(*user))
        }
        Operation::Unban { user } => {
            sqlx::query("UPDATE users SET deletion_previous_status=CASE WHEN status='pending_deletion' THEN 'active' ELSE deletion_previous_status END,status=CASE WHEN status='pending_deletion' THEN status ELSE 'active' END,banned_until=NULL WHERE id=$1 AND status IN ('banned','pending_deletion')").bind(user).execute(&mut *tx).await?;
            notice(&mut tx, *user, "unban", &reason).await?;
            ("unban", Some(*user))
        }
        Operation::Mute { user, seconds } => {
            if !(60..=365 * 86400).contains(seconds) || (role != "admin" && *seconds > 72 * 3600) {
                return Err(Error("INVALID_MESSAGE"));
            }
            sqlx::query(
                "UPDATE users SET muted_until=clock_timestamp()+$2*interval '1 second' WHERE id=$1",
            )
            .bind(user)
            .bind(seconds)
            .execute(&mut *tx)
            .await?;
            notice(&mut tx, *user, "mute", &reason).await?;
            ("mute", Some(*user))
        }
        Operation::Unmute { user } => {
            sqlx::query("UPDATE users SET muted_until=NULL WHERE id=$1")
                .bind(user)
                .execute(&mut *tx)
                .await?;
            notice(&mut tx, *user, "unmute", &reason).await?;
            ("unmute", Some(*user))
        }
        Operation::Role { user, role } => {
            if !matches!(role.as_str(), "player" | "mentor" | "moderator") {
                return Err(Error("INVALID_MESSAGE"));
            }
            sqlx::query("UPDATE users SET role=$2 WHERE id=$1")
                .bind(user)
                .bind(role)
                .execute(&mut *tx)
                .await?;
            sqlx::query("DELETE FROM admin_sessions WHERE user_id=$1")
                .bind(user)
                .execute(&mut *tx)
                .await?;
            notice(&mut tx, *user, "role", &reason).await?;
            ("role", Some(*user))
        }
        Operation::ClanEdit {
            clan,
            description,
            motd,
        } => {
            bounded(description, 1000)?;
            bounded(motd, 300)?;
            let row:Option<(String,String)>=sqlx::query_as("SELECT description,motd FROM clans WHERE id=$1 AND status IN ('founding','active') FOR UPDATE").bind(clan).fetch_optional(&mut *tx).await?;
            before = serde_json::to_value(row.ok_or(Error("NOT_FOUND"))?)
                .map_err(|_| Error("INTERNAL"))?;
            sqlx::query("UPDATE clans SET description=$2,motd=$3,revision=revision+1 WHERE id=$1")
                .bind(clan)
                .bind(description)
                .bind(motd)
                .execute(&mut *tx)
                .await?;
            ("clan_edit", Some(*clan))
        }
        Operation::Filter {
            id: filter,
            phrase,
            enabled,
        } => {
            sqlx::query("SELECT pg_advisory_xact_lock(70612544)")
                .execute(&mut *tx)
                .await?;
            let old:Option<String>=sqlx::query_scalar("SELECT json_build_object('phrase',phrase,'enabled',enabled)::text FROM word_filters WHERE id=$1 FOR UPDATE").bind(filter).fetch_optional(&mut *tx).await?;
            before = old
                .map(|s| serde_json::from_str(&s))
                .transpose()
                .map_err(|_| Error("INTERNAL"))?
                .unwrap_or(serde_json::Value::Null);
            let count: i64 =
                sqlx::query_scalar("SELECT count(*) FROM word_filters WHERE id<>$1 AND enabled")
                    .bind(filter)
                    .fetch_one(&mut *tx)
                    .await?;
            if *enabled && count >= 2000 {
                return Err(Error("LIMIT"));
            }
            let phrase = super::moderation::normalize(phrase);
            if !(2..=80).contains(&phrase.chars().count()) {
                return Err(Error("INVALID_MESSAGE"));
            }
            sqlx::query("INSERT INTO word_filters(id,phrase,enabled) VALUES ($1,$2,$3) ON CONFLICT(id) DO UPDATE SET phrase=excluded.phrase,enabled=excluded.enabled").bind(filter).bind(phrase).bind(enabled).execute(&mut *tx).await?;
            ("filter", Some(*filter))
        }
        Operation::Announcement {
            id: announcement,
            body,
            seconds,
            enabled,
        } => {
            let body = crate::online::text(body, 1000)?;
            if !(60..=30 * 86400).contains(seconds) {
                return Err(Error("INVALID_MESSAGE"));
            }
            let old:Option<String>=sqlx::query_scalar("SELECT json_build_object('body',body,'enabled',enabled,'expires_at',expires_at)::text FROM announcements WHERE id=$1 FOR UPDATE").bind(announcement).fetch_optional(&mut *tx).await?;
            before = old
                .map(|s| serde_json::from_str(&s))
                .transpose()
                .map_err(|_| Error("INTERNAL"))?
                .unwrap_or(serde_json::Value::Null);
            sqlx::query("INSERT INTO announcements(id,author,body,expires_at,enabled) VALUES ($1,$2,$3,clock_timestamp()+$4*interval '1 second',$5) ON CONFLICT(id) DO UPDATE SET body=excluded.body,expires_at=excluded.expires_at,enabled=excluded.enabled").bind(announcement).bind(id.user()).bind(body).bind(seconds).bind(enabled).execute(&mut *tx).await?;
            ("announcement", Some(*announcement))
        }
        Operation::Maintenance { enabled, notice } => {
            bounded(notice, 1000)?;
            let old:String=sqlx::query_scalar("SELECT json_build_object('maintenance',maintenance,'notice',notice,'revision',revision)::text FROM server_controls WHERE singleton FOR UPDATE").fetch_one(&mut *tx).await?;
            before = serde_json::from_str(&old).map_err(|_| Error("INTERNAL"))?;
            sqlx::query("UPDATE server_controls SET maintenance=$1,notice=$2,revision=revision+1 WHERE singleton").bind(enabled).bind(notice).execute(&mut *tx).await?;
            ("maintenance", None)
        }
        Operation::Resolve { report, accepted } => {
            let old:Option<String>=sqlx::query_scalar("SELECT json_build_object('status',status,'resolution',resolution)::text FROM moderation_reports WHERE id=$1 FOR UPDATE").bind(report).fetch_optional(&mut *tx).await?;
            before = serde_json::from_str(&old.ok_or(Error("NOT_FOUND"))?)
                .map_err(|_| Error("INTERNAL"))?;
            let status = if *accepted { "reviewed" } else { "rejected" };
            let reporter:Option<Uuid>=sqlx::query_scalar("UPDATE moderation_reports SET status=$2,resolution=$3,resolved_by=$4,resolved_at=now() WHERE id=$1 AND status='open' RETURNING reporter").bind(report).bind(status).bind(&reason).bind(id.user()).fetch_optional(&mut *tx).await?;
            notice(
                &mut tx,
                reporter.ok_or(Error("CONFLICT"))?,
                "report",
                &format!("{status}: {reason}"),
            )
            .await?;
            ("report_resolve", Some(*report))
        }
        Operation::SystemTournament {
            id: tournament,
            name,
            terms,
            reward,
        } => {
            crate::tournaments::create_system(
                &mut tx,
                id.user(),
                *tournament,
                name,
                terms.clone(),
                reward.clone(),
            )
            .await?;
            ("system_tournament", Some(*tournament))
        }
        Operation::SystemCancel { id: tournament } => {
            crate::tournaments::cancel_system(&mut tx, *tournament, &locked_users).await?;
            ("system_cancel", Some(*tournament))
        }
        Operation::ReviewSignal { id: signal } => {
            let changed = sqlx::query("UPDATE abuse_signals SET reviewed=true WHERE id=$1")
                .bind(signal)
                .execute(&mut *tx)
                .await?;
            if changed.rows_affected() != 1 {
                return Err(Error("NOT_FOUND"));
            }
            ("signal_review", Some(*signal))
        }
    };
    access::audit(
        &mut tx,
        id,
        action,
        audit_target,
        Some(request.request),
        &reason,
        json!({"before":before,"operation":request.operation}),
    )
    .await?;
    sqlx::query("INSERT INTO admin_requests(actor,request_id,fingerprint) VALUES ($1,$2,$3)")
        .bind(id.user())
        .bind(request.request)
        .bind(fingerprint)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    if let Operation::Ban { user, .. } = request.operation {
        state.online.hub.revoke(user, None).await;
    } else if matches!(request.operation, Operation::SystemCancel { .. }) {
        state.online.hub.activity_changed(locked_users).await;
    } else if let Some(user) = target {
        state.online.hub.refresh_clans(vec![user]).await;
    }
    Ok(Receipt {
        request: request.request,
        applied: true,
    })
}
fn bounded(text: &str, max: usize) -> Result<()> {
    if text.chars().count() > max || text.chars().any(|c| c.is_control() && c != '\n') {
        Err(Error("INVALID_MESSAGE"))
    } else {
        Ok(())
    }
}
