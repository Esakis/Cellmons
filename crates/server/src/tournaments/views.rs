// HTTP projections are separate from the private seed and durable bracket representation.
use super::*;
use crate::gateway::AppState;
use axum::{
    Json,
    extract::{Query, State},
    http::HeaderMap,
};

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Parameters {
    id: Option<Uuid>,
    #[serde(default)]
    page: u32,
}
#[derive(Serialize, sqlx::FromRow)]
pub struct Summary {
    id: Uuid,
    name: String,
    status: String,
    creator: Uuid,
    enrolled: i64,
    revision: i64,
    balance: String,
}
#[derive(Serialize)]
pub struct PublicEntry {
    user: Uuid,
    avatar: Uuid,
    team: Uuid,
    login: String,
    template_id: u32,
    level: u32,
    tier: u32,
    status: String,
    departed: bool,
    ready: bool,
}
#[derive(Serialize)]
pub struct Profile {
    summary: Summary,
    terms: Terms,
    entries: Vec<PublicEntry>,
    rounds: Vec<bracket::Round>,
    champion: Option<Uuid>,
    reward: Option<Reward>,
}
#[derive(Serialize, sqlx::FromRow)]
pub struct Clan {
    id: Uuid,
    name: String,
}
#[derive(Serialize)]
pub struct Snapshot {
    own_clan: Option<Uuid>,
    content_version: String,
    own: Option<Uuid>,
    catalog: Vec<Summary>,
    profile: Option<Profile>,
    has_more: bool,
    clans: Vec<Clan>,
    first_fight_s: i64,
    next_fight_s: i64,
    ready_grace_s: i64,
}
macro_rules! summary { () => {"SELECT t.id,t.name,t.status,t.creator,(SELECT count(*) FROM tournament_entries e WHERE e.tournament_id=t.id AND e.status='enrolled') AS enrolled,t.revision,t.balance::text AS balance FROM tournaments t "}; }
pub async fn load(
    connection: &mut PgConnection,
    user: Uuid,
    query: &Parameters,
) -> Result<Snapshot> {
    if query.page > 1000 {
        return Err(Error("INVALID_MESSAGE"));
    }
    let own = sqlx::query_scalar(
        "SELECT owner_id FROM activity_locks WHERE user_id=$1 AND kind='tournament'",
    )
    .bind(user)
    .fetch_optional(&mut *connection)
    .await?;
    let mut catalog: Vec<Summary>=sqlx::query_as(concat!(summary!(),"WHERE t.status IN ('registering','running') OR EXISTS(SELECT 1 FROM tournament_entries e WHERE e.tournament_id=t.id AND e.user_id=$1) OR t.creator=$1 ORDER BY t.created_at DESC LIMIT 51 OFFSET $2")).bind(user).bind(i64::from(query.page)*50).fetch_all(&mut *connection).await?;
    let has_more = catalog.len() > 50;
    catalog.truncate(50);
    let profile = if let Some(id) = query.id.or(own) {
        let summary: Summary = sqlx::query_as(concat!(summary!(), "WHERE t.id=$1"))
            .bind(id)
            .fetch_optional(&mut *connection)
            .await?
            .ok_or(Error("NOT_FOUND"))?;
        let (terms, raw): (Vec<u8>, Option<Vec<u8>>) =
            sqlx::query_as("SELECT terms,bracket FROM tournaments WHERE id=$1")
                .bind(id)
                .fetch_one(&mut *connection)
                .await?;
        let bracket: Option<bracket::Bracket> = raw.as_deref().map(decode).transpose()?;
        let roster = entries(connection, id).await?;
        let now = crate::online::now_ms();
        let mut entries = vec![];
        for entry in roster {
            let seat: crate::duel::record::Seat = decode(&entry.seat)?;
            let public = seat.public()?;
            entries.push(PublicEntry {
                user: entry.user_id,
                avatar: entry.avatar_id,
                team: entry.team_id,
                login: public.login,
                template_id: public.template_id,
                level: public.level,
                tier: public.tier,
                status: entry.status,
                departed: bracket
                    .as_ref()
                    .is_some_and(|b| b.departed.contains(&entry.user_id)),
                ready: entry.ready_until_ms > now,
            });
        }
        Some(Profile {
            summary,
            terms: decode(&terms)?,
            entries,
            rounds: bracket.as_ref().map_or(vec![], |b| b.rounds.clone()),
            champion: bracket.and_then(|b| b.champion),
            reward: sqlx::query_scalar::<_, Option<Vec<u8>>>(
                "SELECT system_reward FROM tournaments WHERE id=$1",
            )
            .bind(id)
            .fetch_one(&mut *connection)
            .await?
            .as_deref()
            .map(rewards::public)
            .transpose()?,
        })
    } else {
        None
    };
    let clans = sqlx::query_as(
        "SELECT id,name::text AS name FROM clans WHERE status='active' ORDER BY name LIMIT 256",
    )
    .fetch_all(&mut *connection)
    .await?;
    let rules = &crate::rules::RULES.tournament;
    Ok(Snapshot {
        own_clan: crate::online::repo::clan_on(connection, user).await?,
        content_version: crate::auth::crypto::hex(&engine_content::CONTENT_VERSION),
        own,
        catalog,
        profile,
        has_more,
        clans,
        first_fight_s: rules.first_fight_s,
        next_fight_s: rules.next_fight_s,
        ready_grace_s: rules.ready_grace_s,
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
    crate::journey::require(&state.db, session.user, "castle")
        .await
        .map_err(crate::game::response)?;
    state
        .auth
        .limit("tournament-read", &session.user.to_string(), 60, 60)
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
    let snapshot = load(&mut tx, session.user, &query)
        .await
        .map_err(crate::game::response)?;
    tx.commit()
        .await
        .map_err(|e| crate::game::response(e.into()))?;
    Ok(Json(snapshot))
}
pub async fn post(
    State(state): State<AppState>,
    headers: HeaderMap,
    mutation: std::result::Result<Json<Mutation>, axum::extract::rejection::JsonRejection>,
) -> std::result::Result<Json<Uuid>, crate::game::HttpError> {
    let session = crate::auth::security::session(&state, &headers)
        .await
        .map_err(crate::game::HttpError)?;
    crate::journey::require(&state.db, session.user, "castle")
        .await
        .map_err(crate::game::response)?;
    state
        .auth
        .limit("tournament-write", &session.user.to_string(), 30, 60)
        .map_err(crate::game::HttpError)?;
    let Json(mutation) = mutation.map_err(|_| crate::game::response(Error("INVALID_MESSAGE")))?;
    let id = match mutation {
        Mutation::Create { id, name, terms } => {
            state
                .auth
                .limit("tournament-create", &session.user.to_string(), 10, 86400)
                .map_err(crate::game::HttpError)?;
            repository::create(&state.db, session.user, id, &name, terms).await
        }
        Mutation::Join {
            id,
            request,
            avatar,
        } => repository::join(&state.db, session.user, id, request, avatar)
            .await
            .map(|()| id),
        Mutation::Leave { id, request } => repository::leave(&state.db, session.user, id, request)
            .await
            .map(|()| id),
        Mutation::Cancel { id, request } => {
            repository::cancel(&state.db, session.user, id, request)
                .await
                .map(|()| id)
        }
    }
    .map_err(crate::game::response)?;
    state.online.hub.activity_changed(vec![session.user]).await;
    Ok(Json(id))
}
