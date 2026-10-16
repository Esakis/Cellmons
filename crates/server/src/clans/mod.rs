//! R-KLA: persistent membership and role changes, serialized by user rows then the clan row.
mod views;
pub use views::{Query, Snapshot, get, load};

use crate::{
    gateway::AppState,
    online::{Error, Result},
};
use axum::{Json, extract::State, http::HeaderMap};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Vice,
    Scribe,
    Member,
}
impl Role {
    fn key(self) -> &'static str {
        match self {
            Self::Vice => "vice",
            Self::Scribe => "scribe",
            Self::Member => "member",
        }
    }
}
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum Mutation {
    Create {
        id: Uuid,
        name: String,
    },
    Apply {
        clan: Uuid,
    },
    Withdraw {
        clan: Uuid,
    },
    Accept {
        clan: Uuid,
        user: Uuid,
        revision: i32,
    },
    Reject {
        clan: Uuid,
        user: Uuid,
        revision: i32,
    },
    Role {
        clan: Uuid,
        user: Uuid,
        role: Role,
        revision: i32,
    },
    Title {
        clan: Uuid,
        user: Uuid,
        title: String,
        revision: i32,
    },
    Edit {
        clan: Uuid,
        description: String,
        motd: String,
        www: String,
        email: String,
        revision: i32,
    },
    Leave {
        clan: Uuid,
        revision: i32,
    },
    Kick {
        clan: Uuid,
        user: Uuid,
        revision: i32,
    },
    Transfer {
        clan: Uuid,
        user: Uuid,
        revision: i32,
    },
    Disband {
        clan: Uuid,
        revision: i32,
    },
}
impl Mutation {
    fn clan(&self) -> Uuid {
        match self {
            Self::Create { id, .. } => *id,
            Self::Apply { clan }
            | Self::Withdraw { clan }
            | Self::Accept { clan, .. }
            | Self::Reject { clan, .. }
            | Self::Role { clan, .. }
            | Self::Title { clan, .. }
            | Self::Edit { clan, .. }
            | Self::Leave { clan, .. }
            | Self::Kick { clan, .. }
            | Self::Transfer { clan, .. }
            | Self::Disband { clan, .. } => *clan,
        }
    }
    fn target(&self) -> Option<Uuid> {
        match self {
            Self::Accept { user, .. }
            | Self::Reject { user, .. }
            | Self::Role { user, .. }
            | Self::Title { user, .. }
            | Self::Kick { user, .. }
            | Self::Transfer { user, .. } => Some(*user),
            _ => None,
        }
    }
    fn revision(&self) -> Option<i32> {
        match self {
            Self::Accept { revision, .. }
            | Self::Reject { revision, .. }
            | Self::Role { revision, .. }
            | Self::Title { revision, .. }
            | Self::Edit { revision, .. }
            | Self::Leave { revision, .. }
            | Self::Kick { revision, .. }
            | Self::Transfer { revision, .. }
            | Self::Disband { revision, .. } => Some(*revision),
            _ => None,
        }
    }
}
#[derive(Debug, Serialize)]
pub struct Changed {
    pub clan: Uuid,
    pub revision: i32,
}
pub struct Change {
    pub result: Changed,
    pub affected: Vec<Uuid>,
}
#[derive(sqlx::FromRow)]
struct Clan {
    id: Uuid,
    name: String,
    status: String,
    revision: i32,
    www: String,
    email: String,
}
fn text(value: &str, max: usize, multiline: bool) -> Result<String> {
    let value = value.trim();
    if value.chars().count() > max
        || value
            .chars()
            .any(|c| c.is_control() && !(multiline && matches!(c, '\n' | '\t')))
    {
        return Err(Error("INVALID_MESSAGE"));
    }
    Ok(value.to_owned())
}
fn name(value: &str) -> Result<String> {
    let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if !(3..=40).contains(&value.chars().count())
        || !value
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, ' ' | '_' | '-' | '\''))
    {
        return Err(Error("INVALID_MESSAGE"));
    }
    Ok(value)
}
fn contacts(www: &str, email: &str) -> Result<(String, String)> {
    let www = text(www, 200, false)?;
    let email = text(email, 254, false)?;
    if !www.is_empty() {
        let url = reqwest::Url::parse(&www).map_err(|_| Error("INVALID_MESSAGE"))?;
        if url.scheme() != "https"
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err(Error("INVALID_MESSAGE"));
        }
    }
    if !email.is_empty() && email.parse::<lettre::Address>().is_err() {
        return Err(Error("INVALID_MESSAGE"));
    }
    Ok((www, email))
}
async fn role(tx: &mut PgConnection, clan: Uuid, user: Uuid) -> Result<Option<String>> {
    Ok(
        sqlx::query_scalar("SELECT role FROM clan_members WHERE clan_id=$1 AND user_id=$2")
            .bind(clan)
            .bind(user)
            .fetch_optional(tx)
            .await?,
    )
}
async fn members(tx: &mut PgConnection, clan: Uuid) -> Result<Vec<Uuid>> {
    Ok(
        sqlx::query_scalar("SELECT user_id FROM clan_members WHERE clan_id=$1 ORDER BY user_id")
            .bind(clan)
            .fetch_all(tx)
            .await?,
    )
}
async fn audit(
    tx: &mut PgConnection,
    clan: Uuid,
    actor: Uuid,
    target: Option<Uuid>,
    action: &str,
    details: Value,
) -> Result<()> {
    sqlx::query("INSERT INTO clan_audit(id,clan_id,actor_id,target_id,action,details) VALUES ($1,$2,$3,$4,$5,$6)")
        .bind(Uuid::now_v7()).bind(clan).bind(actor).bind(target).bind(action).bind(details).execute(tx).await?;
    Ok(())
}
fn officer(role: &str) -> Result<()> {
    if ["master", "vice"].contains(&role) {
        Ok(())
    } else {
        Err(Error("FORBIDDEN"))
    }
}
fn master(role: &str) -> Result<()> {
    if role == "master" {
        Ok(())
    } else {
        Err(Error("FORBIDDEN"))
    }
}

pub async fn mutate(db: &PgPool, actor: Uuid, mutation: Mutation) -> Result<Change> {
    let clan = mutation.clan();
    let target = mutation.target();
    let mut users = vec![actor];
    users.extend(target);
    users.sort();
    users.dedup();
    let mut tx = db.begin().await?;
    let locked: Vec<(Uuid, String)> =
        sqlx::query_as("SELECT id,status FROM users WHERE id=ANY($1) ORDER BY id FOR UPDATE")
            .bind(&users)
            .fetch_all(&mut *tx)
            .await?;
    if !locked
        .iter()
        .any(|(id, status)| *id == actor && status == "active")
    {
        return Err(Error("UNAUTHENTICATED"));
    }
    if let Some(target) = target
        && !locked.iter().any(|(id, _)| *id == target)
    {
        return Err(Error("NOT_FOUND"));
    }
    if let Mutation::Create {
        name: requested, ..
    } = &mutation
    {
        let name = name(requested)?;
        if let Some(existing) = sqlx::query_as::<_, Clan>(
            "SELECT id,name::text,status,revision,www,email FROM clans WHERE id=$1 FOR UPDATE",
        )
        .bind(clan)
        .fetch_optional(&mut *tx)
        .await?
        {
            if existing.name == name
                && existing.status != "disbanded"
                && role(&mut tx, clan, actor).await?.as_deref() == Some("master")
            {
                return Ok(Change {
                    result: Changed {
                        clan,
                        revision: existing.revision,
                    },
                    affected: vec![actor],
                });
            }
            return Err(Error("CONFLICT"));
        }
        if sqlx::query_scalar::<_, Uuid>("SELECT clan_id FROM clan_members WHERE user_id=$1")
            .bind(actor)
            .fetch_optional(&mut *tx)
            .await?
            .is_some()
        {
            return Err(Error("ALREADY_MEMBER"));
        }
        let inserted = sqlx::query("INSERT INTO clans(id,name) VALUES ($1,$2::citext)")
            .bind(clan)
            .bind(&name)
            .execute(&mut *tx)
            .await;
        if let Err(error) = inserted {
            if error
                .as_database_error()
                .is_some_and(|e| e.code().as_deref() == Some("23505"))
            {
                return Err(Error("NAME_TAKEN"));
            }
            return Err(error.into());
        }
        sqlx::query("INSERT INTO clan_members(clan_id,user_id,role) VALUES ($1,$2,'master')")
            .bind(clan)
            .bind(actor)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM clan_applications WHERE user_id=$1")
            .bind(actor)
            .execute(&mut *tx)
            .await?;
        audit(&mut tx, clan, actor, None, "create", json!({})).await?;
        tx.commit().await?;
        return Ok(Change {
            result: Changed { clan, revision: 0 },
            affected: vec![actor],
        });
    }
    let current = sqlx::query_as::<_, Clan>(
        "SELECT id,name::text,status,revision,www,email FROM clans WHERE id=$1 FOR UPDATE",
    )
    .bind(clan)
    .fetch_optional(&mut *tx)
    .await?
    .filter(|c| c.status != "disbanded")
    .ok_or(Error("NOT_FOUND"))?;
    debug_assert_eq!(current.id, clan);
    if matches!(mutation, Mutation::Apply { .. } | Mutation::Withdraw { .. }) {
        let applying = matches!(mutation, Mutation::Apply { .. });
        let changed = if applying {
            if sqlx::query_scalar::<_, Uuid>("SELECT clan_id FROM clan_members WHERE user_id=$1")
                .bind(actor)
                .fetch_optional(&mut *tx)
                .await?
                .is_some()
            {
                return Err(Error("ALREADY_MEMBER"));
            }
            let exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM clan_applications WHERE clan_id=$1 AND user_id=$2)",
            )
            .bind(clan)
            .bind(actor)
            .fetch_one(&mut *tx)
            .await?;
            if exists {
                false
            } else {
                let count: i64 =
                    sqlx::query_scalar("SELECT count(*) FROM clan_applications WHERE user_id=$1")
                        .bind(actor)
                        .fetch_one(&mut *tx)
                        .await?;
                if count >= crate::rules::RULES.clan.max_applications {
                    return Err(Error("APPLICATION_LIMIT"));
                }
                sqlx::query("INSERT INTO clan_applications(clan_id,user_id) VALUES ($1,$2)")
                    .bind(clan)
                    .bind(actor)
                    .execute(&mut *tx)
                    .await?;
                true
            }
        } else {
            sqlx::query("DELETE FROM clan_applications WHERE clan_id=$1 AND user_id=$2")
                .bind(clan)
                .bind(actor)
                .execute(&mut *tx)
                .await?
                .rows_affected()
                > 0
        };
        if changed {
            audit(
                &mut tx,
                clan,
                actor,
                None,
                if applying { "apply" } else { "withdraw" },
                json!({}),
            )
            .await?;
        }
        tx.commit().await?;
        return Ok(Change {
            result: Changed {
                clan,
                revision: current.revision,
            },
            affected: vec![actor],
        });
    }
    let own_role = role(&mut tx, clan, actor)
        .await?
        .ok_or(Error("FORBIDDEN"))?;
    if mutation.revision() != Some(current.revision) {
        return Err(Error("CONFLICT"));
    }
    let mut affected = vec![actor];
    affected.extend(target);
    let (action, details) = match mutation {
        Mutation::Accept { user, .. } => {
            officer(&own_role)?;
            if !locked
                .iter()
                .any(|(id, status)| *id == user && status == "active")
            {
                return Err(Error("NOT_FOUND"));
            }
            let applied: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM clan_applications WHERE clan_id=$1 AND user_id=$2)",
            )
            .bind(clan)
            .bind(user)
            .fetch_one(&mut *tx)
            .await?;
            if !applied {
                return Err(Error("NOT_FOUND"));
            }
            if sqlx::query_scalar::<_, Uuid>("SELECT clan_id FROM clan_members WHERE user_id=$1")
                .bind(user)
                .fetch_optional(&mut *tx)
                .await?
                .is_some()
            {
                return Err(Error("ALREADY_MEMBER"));
            }
            let count: i64 =
                sqlx::query_scalar("SELECT count(*) FROM clan_members WHERE clan_id=$1")
                    .bind(clan)
                    .fetch_one(&mut *tx)
                    .await?;
            if count >= crate::rules::RULES.clan.max_members {
                return Err(Error("CLAN_FULL"));
            }
            sqlx::query("INSERT INTO clan_members(clan_id,user_id) VALUES ($1,$2)")
                .bind(clan)
                .bind(user)
                .execute(&mut *tx)
                .await?;
            sqlx::query("DELETE FROM clan_applications WHERE user_id=$1")
                .bind(user)
                .execute(&mut *tx)
                .await?;
            let activate =
                current.status == "founding" && count + 1 >= crate::rules::RULES.clan.min_members;
            if activate {
                sqlx::query("UPDATE clans SET status='active',founded_at=now() WHERE id=$1")
                    .bind(clan)
                    .execute(&mut *tx)
                    .await?;
                affected = members(&mut tx, clan).await?;
            }
            ("accept", json!({"activated":activate}))
        }
        Mutation::Reject { user, .. } => {
            officer(&own_role)?;
            if sqlx::query("DELETE FROM clan_applications WHERE clan_id=$1 AND user_id=$2")
                .bind(clan)
                .bind(user)
                .execute(&mut *tx)
                .await?
                .rows_affected()
                == 0
            {
                return Err(Error("NOT_FOUND"));
            }
            ("reject", json!({}))
        }
        Mutation::Role {
            user,
            role: new_role,
            ..
        } => {
            officer(&own_role)?;
            let old = role(&mut tx, clan, user).await?.ok_or(Error("NOT_FOUND"))?;
            if old == "master" {
                return Err(Error(if own_role == "master" {
                    "MASTER_REQUIRED"
                } else {
                    "FORBIDDEN"
                }));
            }
            sqlx::query("UPDATE clan_members SET role=$3 WHERE clan_id=$1 AND user_id=$2")
                .bind(clan)
                .bind(user)
                .bind(new_role.key())
                .execute(&mut *tx)
                .await?;
            ("role", json!({"role":new_role.key()}))
        }
        Mutation::Title { user, title, .. } => {
            officer(&own_role)?;
            let old = role(&mut tx, clan, user).await?.ok_or(Error("NOT_FOUND"))?;
            if old == "master" && own_role != "master" {
                return Err(Error("FORBIDDEN"));
            }
            let title = text(&title, 64, false)?;
            sqlx::query("UPDATE clan_members SET title=$3 WHERE clan_id=$1 AND user_id=$2")
                .bind(clan)
                .bind(user)
                .bind(&title)
                .execute(&mut *tx)
                .await?;
            ("title", json!({"title":title}))
        }
        Mutation::Edit {
            description,
            motd,
            www,
            email,
            ..
        } => {
            if !["master", "vice", "scribe"].contains(&own_role.as_str()) {
                return Err(Error("FORBIDDEN"));
            }
            let description = text(&description, 1000, true)?;
            let motd = text(&motd, 300, true)?;
            let (www, email) = contacts(&www, &email)?;
            if own_role == "scribe" && (www != current.www || email != current.email) {
                return Err(Error("FORBIDDEN"));
            }
            sqlx::query("UPDATE clans SET description=$2,motd=$3,www=$4,email=$5 WHERE id=$1")
                .bind(clan)
                .bind(description)
                .bind(motd)
                .bind(www)
                .bind(email)
                .execute(&mut *tx)
                .await?;
            ("edit", json!({}))
        }
        Mutation::Transfer { user, .. } => {
            master(&own_role)?;
            if user == actor {
                return Err(Error("CONFLICT"));
            }
            role(&mut tx, clan, user).await?.ok_or(Error("NOT_FOUND"))?;
            sqlx::query("UPDATE clan_members SET role='vice' WHERE clan_id=$1 AND user_id=$2")
                .bind(clan)
                .bind(actor)
                .execute(&mut *tx)
                .await?;
            sqlx::query("UPDATE clan_members SET role='master' WHERE clan_id=$1 AND user_id=$2")
                .bind(clan)
                .bind(user)
                .execute(&mut *tx)
                .await?;
            ("transfer", json!({}))
        }
        Mutation::Kick { user, .. } => {
            officer(&own_role)?;
            let old = role(&mut tx, clan, user).await?.ok_or(Error("NOT_FOUND"))?;
            if old == "master" {
                return Err(Error(if own_role == "master" {
                    "MASTER_REQUIRED"
                } else {
                    "FORBIDDEN"
                }));
            }
            if user == actor {
                return Err(Error("MASTER_REQUIRED"));
            }
            sqlx::query("DELETE FROM clan_members WHERE clan_id=$1 AND user_id=$2")
                .bind(clan)
                .bind(user)
                .execute(&mut *tx)
                .await?;
            ("kick", json!({}))
        }
        Mutation::Leave { .. } => {
            if own_role == "master" {
                let count: i64 =
                    sqlx::query_scalar("SELECT count(*) FROM clan_members WHERE clan_id=$1")
                        .bind(clan)
                        .fetch_one(&mut *tx)
                        .await?;
                if count != 1 {
                    return Err(Error("MASTER_REQUIRED"));
                }
                sqlx::query("UPDATE clans SET status='disbanded' WHERE id=$1")
                    .bind(clan)
                    .execute(&mut *tx)
                    .await?;
                sqlx::query("DELETE FROM clan_applications WHERE clan_id=$1")
                    .bind(clan)
                    .execute(&mut *tx)
                    .await?;
            }
            sqlx::query("DELETE FROM clan_members WHERE clan_id=$1 AND user_id=$2")
                .bind(clan)
                .bind(actor)
                .execute(&mut *tx)
                .await?;
            ("leave", json!({}))
        }
        Mutation::Disband { .. } => {
            master(&own_role)?;
            affected = members(&mut tx, clan).await?;
            sqlx::query("UPDATE clans SET status='disbanded' WHERE id=$1")
                .bind(clan)
                .execute(&mut *tx)
                .await?;
            sqlx::query("DELETE FROM clan_applications WHERE clan_id=$1")
                .bind(clan)
                .execute(&mut *tx)
                .await?;
            sqlx::query("DELETE FROM clan_members WHERE clan_id=$1")
                .bind(clan)
                .execute(&mut *tx)
                .await?;
            ("disband", json!({}))
        }
        Mutation::Create { .. } | Mutation::Apply { .. } | Mutation::Withdraw { .. } => {
            unreachable!()
        }
    };
    let revision = current.revision.checked_add(1).ok_or(Error("CONFLICT"))?;
    sqlx::query("UPDATE clans SET revision=$2 WHERE id=$1")
        .bind(clan)
        .bind(revision)
        .execute(&mut *tx)
        .await?;
    audit(&mut tx, clan, actor, target, action, details).await?;
    tx.commit().await?;
    affected.sort();
    affected.dedup();
    Ok(Change {
        result: Changed { clan, revision },
        affected,
    })
}

pub async fn post(
    State(state): State<AppState>,
    headers: HeaderMap,
    mutation: std::result::Result<Json<Mutation>, axum::extract::rejection::JsonRejection>,
) -> std::result::Result<Json<Changed>, crate::game::HttpError> {
    let session = crate::auth::security::session(&state, &headers)
        .await
        .map_err(crate::game::HttpError)?;
    state
        .auth
        .limit("clan-write", &session.user.to_string(), 30, 60)
        .map_err(crate::game::HttpError)?;
    let Json(mutation) = mutation.map_err(|_| crate::game::response(Error("INVALID_MESSAGE")))?;
    if matches!(mutation, Mutation::Create { .. }) {
        state
            .auth
            .limit("clan-create", &session.user.to_string(), 3, 86400)
            .map_err(crate::game::HttpError)?;
    }
    let change = mutate(&state.db, session.user, mutation)
        .await
        .map_err(crate::game::response)?;
    // The transaction is already committed. A slow/offline socket must not turn success into
    // a failed HTTP mutation; private chat rechecks membership independently on every send.
    state.online.hub.refresh_clans(change.affected).await;
    Ok(Json(change.result))
}
