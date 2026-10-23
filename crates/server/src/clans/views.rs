use crate::{
    gateway::AppState,
    online::{Error, Result},
};
use axum::{
    Json,
    extract::{Query as Parameters, State},
    http::HeaderMap,
};
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use uuid::Uuid;

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Query {
    pub search: String,
    pub page: u32,
    pub clan: Option<Uuid>,
}
#[derive(Serialize, sqlx::FromRow)]
pub struct Summary {
    pub id: Uuid,
    pub name: String,
    pub status: String,
    pub revision: i32,
    pub members: i64,
    pub score: i64,
    pub rank: Option<i64>,
}
#[derive(Serialize, sqlx::FromRow)]
pub struct Member {
    pub user: Uuid,
    pub login: String,
    pub role: String,
    pub title: String,
    pub rating: i32,
}
#[derive(Serialize, sqlx::FromRow)]
pub struct Applicant {
    pub user: Uuid,
    pub login: String,
    pub at_ms: i64,
}
#[derive(Serialize, sqlx::FromRow)]
pub struct Application {
    pub clan: Uuid,
    pub name: String,
}
#[derive(Serialize)]
pub struct Profile {
    #[serde(flatten)]
    pub summary: Summary,
    pub description: String,
    pub motd: String,
    pub www: String,
    pub email: String,
    pub founded_ms: Option<i64>,
    pub ducats: String,
    pub roster: Vec<Member>,
    pub applicants: Vec<Applicant>,
}
#[derive(sqlx::FromRow)]
struct Details {
    description: String,
    motd: String,
    www: String,
    email: String,
    founded_ms: Option<i64>,
    ducats: String,
}
#[derive(Serialize)]
pub struct Limits {
    pub min_members: i64,
    pub max_members: i64,
    pub max_applications: i64,
}
#[derive(Serialize)]
pub struct Snapshot {
    pub content_version: String,
    pub account_level: u8,
    pub own: Option<Uuid>,
    pub own_role: Option<String>,
    pub clans: Vec<Summary>,
    pub has_more: bool,
    pub profile: Option<Profile>,
    pub applications: Vec<Application>,
    pub limits: Limits,
}

// Integer average of the ten strongest current members; ties have a stable UUID order.
// Search filters the catalog after the global active-clan ranks have been computed.
macro_rules! summary_sql {
    ($filter:literal) => { concat!(r#"
WITH best AS (
  SELECT m.clan_id,coalesce(r.rating,1000) AS rating,row_number() OVER(PARTITION BY m.clan_id ORDER BY coalesce(r.rating,1000) DESC,m.user_id) AS place
  FROM clan_members m JOIN users u ON u.id=m.user_id LEFT JOIN ratings r ON r.user_id=m.user_id
  WHERE u.status='active'
), scores AS (
  SELECT clan_id,(sum(rating)/count(*))::bigint AS score FROM best WHERE place<=10 GROUP BY clan_id
), ranks AS (
  SELECT c.id,row_number() OVER(ORDER BY coalesce(s.score,0) DESC,c.id) AS rank
  FROM clans c LEFT JOIN scores s ON s.clan_id=c.id WHERE c.status='active'
)
SELECT c.id,c.name::text AS name,c.status,c.revision,
 (SELECT count(*) FROM clan_members m WHERE m.clan_id=c.id) AS members,
 coalesce(s.score,0)::bigint AS score,r.rank
FROM clans c LEFT JOIN scores s ON s.clan_id=c.id LEFT JOIN ranks r ON r.id=c.id
"#, $filter) };
}

pub async fn load(tx: &mut PgConnection, user: Uuid, query: &Query) -> Result<Snapshot> {
    if query.page > 1000
        || query.search.chars().count() > 40
        || query.search.chars().any(char::is_control)
    {
        return Err(Error("INVALID_MESSAGE"));
    }
    let membership: Option<(Uuid, String)> =
        sqlx::query_as("SELECT clan_id,role FROM clan_members WHERE user_id=$1")
            .bind(user)
            .fetch_optional(&mut *tx)
            .await?;
    let own = membership.as_ref().map(|(clan, _)| *clan);
    let own_role = membership.map(|(_, role)| role);
    let search = query
        .search
        .trim()
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    let sql = summary_sql!(
        " WHERE c.status IN ('active','founding') AND c.name::text ILIKE ('%'||$1||'%') ORDER BY (c.status='active') DESC,coalesce(s.score,0) DESC,c.id LIMIT 51 OFFSET $2"
    );
    let mut clans: Vec<Summary> = sqlx::query_as(sql)
        .bind(search)
        .bind(i64::from(query.page) * 50)
        .fetch_all(&mut *tx)
        .await?;
    let has_more = clans.len() > 50;
    clans.truncate(50);
    let profile = if let Some(clan) = query.clan.or(own) {
        let sql = summary_sql!(" WHERE c.id=$1 AND c.status IN ('active','founding')");
        let summary = sqlx::query_as::<_, Summary>(sql)
            .bind(clan)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(Error("NOT_FOUND"))?;
        let details=sqlx::query_as::<_,Details>(r#"SELECT c.description,
            CASE WHEN EXISTS(SELECT 1 FROM clan_members WHERE clan_id=c.id AND user_id=$2) THEN c.motd ELSE '' END AS motd,
            c.www,c.email,(extract(epoch FROM c.founded_at)*1000)::bigint AS founded_ms,
            (SELECT coalesce(sum(w.balance::numeric),0)::text FROM clan_members m JOIN wallets w ON w.user_id=m.user_id WHERE m.clan_id=c.id) AS ducats
            FROM clans c WHERE c.id=$1"#).bind(clan).bind(user).fetch_one(&mut *tx).await?;
        let roster=sqlx::query_as::<_,Member>("SELECT m.user_id AS user,u.login::text AS login,m.role,m.title,coalesce(r.rating,1000) AS rating FROM clan_members m JOIN users u ON u.id=m.user_id LEFT JOIN ratings r ON r.user_id=m.user_id WHERE m.clan_id=$1 ORDER BY CASE m.role WHEN 'master' THEN 0 WHEN 'vice' THEN 1 WHEN 'scribe' THEN 2 ELSE 3 END,u.login")
            .bind(clan).fetch_all(&mut *tx).await?;
        let applicants = if own == Some(clan)
            && own_role
                .as_deref()
                .is_some_and(|r| ["master", "vice"].contains(&r))
        {
            sqlx::query_as::<_,Applicant>("SELECT a.user_id AS user,u.login::text AS login,(extract(epoch FROM a.created_at)*1000)::bigint AS at_ms FROM clan_applications a JOIN users u ON u.id=a.user_id WHERE a.clan_id=$1 AND u.status='active' ORDER BY a.created_at,a.user_id LIMIT 100")
                .bind(clan).fetch_all(&mut *tx).await?
        } else {
            Vec::new()
        };
        Some(Profile {
            summary,
            description: details.description,
            motd: details.motd,
            www: details.www,
            email: details.email,
            founded_ms: details.founded_ms,
            ducats: details.ducats,
            roster,
            applicants,
        })
    } else {
        None
    };
    let applications=sqlx::query_as::<_,Application>("SELECT a.clan_id AS clan,c.name::text AS name FROM clan_applications a JOIN clans c ON c.id=a.clan_id WHERE a.user_id=$1 AND c.status<>'disbanded' ORDER BY a.created_at,a.clan_id")
        .bind(user).fetch_all(&mut *tx).await?;
    let rules = &crate::rules::RULES.clan;
    Ok(Snapshot {
        content_version: crate::auth::crypto::hex(&engine_content::CONTENT_VERSION),
        account_level: crate::journey::level(
            sqlx::query_scalar("SELECT account_xp FROM users WHERE id=$1")
                .bind(user)
                .fetch_one(&mut *tx)
                .await?,
        ),
        own,
        own_role,
        clans,
        has_more,
        profile,
        applications,
        limits: Limits {
            min_members: rules.min_members,
            max_members: rules.max_members,
            max_applications: rules.max_applications,
        },
    })
}

pub async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
    query: std::result::Result<Parameters<Query>, axum::extract::rejection::QueryRejection>,
) -> std::result::Result<Json<Snapshot>, crate::game::HttpError> {
    let session = crate::auth::security::session(&state, &headers)
        .await
        .map_err(crate::game::HttpError)?;
    crate::journey::require(&state.db, session.user, "clan")
        .await
        .map_err(crate::game::response)?;
    state
        .auth
        .limit("clan-read", &session.user.to_string(), 60, 60)
        .map_err(crate::game::HttpError)?;
    let Parameters(query) = query.map_err(|_| crate::game::response(Error("INVALID_MESSAGE")))?;
    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|e| crate::game::response(e.into()))?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await
        .map_err(|e| crate::game::response(e.into()))?;
    let snapshot = load(&mut tx, session.user, &query)
        .await
        .map_err(crate::game::response)?;
    tx.commit()
        .await
        .map_err(|e| crate::game::response(e.into()))?;
    Ok(Json(snapshot))
}
