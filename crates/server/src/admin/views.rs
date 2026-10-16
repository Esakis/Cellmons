//! Bounded projections; passwords, session hashes, factors and private reward seeds never leave here.
use super::{access, control};
use crate::{
    auth::security::Peer,
    gateway::AppState,
    online::{Error, Result},
};
use axum::{
    Json,
    extract::{Query, State},
    http::HeaderMap,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Parameters {
    #[serde(default)]
    pub page: u32,
    #[serde(default)]
    pub q: String,
    #[serde(default)]
    pub category: String,
}
#[derive(Serialize)]
pub struct Page {
    pub rows: Vec<Value>,
    pub has_more: bool,
}
pub async fn load(state: &AppState, id: &access::Identity, params: Parameters) -> Result<Page> {
    if params.page > 2000 || params.q.chars().count() > 100 {
        return Err(Error("INVALID_MESSAGE"));
    }
    let mut tx = state.db.begin().await?;
    sqlx::query("SELECT id FROM users WHERE id=$1 FOR SHARE")
        .bind(id.user())
        .fetch_one(&mut *tx)
        .await?;
    access::require(&mut tx, id).await?;
    macro_rules! read {
        ($sql:literal) => {
            sqlx::query_scalar::<_, String>($sql)
                .bind(&params.q)
                .bind(i64::from(params.page) * 50)
                .fetch_all(&mut *tx)
                .await?
        };
    }
    let rows = match params.category.as_str() {
        "" | "players" => read!(
            "SELECT json_build_object('id',u.id,'login',u.login::text,'role',u.role,'status',u.status,'verified',u.email_verified_at IS NOT NULL,'muted_until',u.muted_until,'banned_until',u.banned_until,'rating',coalesce(r.rating,1000),'balance',coalesce(w.balance,0)::text,'created_at',u.created_at)::text FROM users u LEFT JOIN ratings r ON r.user_id=u.id LEFT JOIN wallets w ON w.user_id=u.id WHERE (position(lower($1) in lower(u.login::text))>0 OR u.id::text=$1) ORDER BY u.created_at DESC,u.id DESC LIMIT 51 OFFSET $2"
        ),
        "reports" => read!(
            "SELECT json_build_object('id',id,'reporter',reporter,'kind',target_kind,'target',target_id,'reason',reason,'context',convert_from(context,'UTF8')::json,'status',status,'resolution',resolution,'created_at',created_at)::text FROM moderation_reports WHERE $1='' OR status=$1 ORDER BY (status='open') DESC,created_at DESC,id DESC LIMIT 51 OFFSET $2"
        ),
        "chat" => read!(
            "SELECT json_build_object('id',c.id,'room',c.room,'from',u.login::text,'to',c.to_user,'clan',c.clan_id,'text',c.text,'created_at',c.created_at)::text FROM chat_log c JOIN users u ON u.id=c.from_user WHERE position(lower($1) in lower(c.text||' '||u.login::text||' '||c.room))>0 ORDER BY c.created_at DESC,c.id DESC LIMIT 51 OFFSET $2"
        ),
        "audit" => read!(
            "SELECT json_build_object('id',id,'actor',actor,'action',action,'target',target,'reason',reason,'details',convert_from(details,'UTF8')::json,'created_at',created_at)::text FROM admin_audit WHERE position(lower($1) in lower(action||' '||reason))>0 ORDER BY created_at DESC,id DESC LIMIT 51 OFFSET $2"
        ),
        "filters" => read!(
            "SELECT json_build_object('id',id,'phrase',phrase,'enabled',enabled)::text FROM word_filters WHERE position(lower($1) in lower(phrase))>0 ORDER BY phrase LIMIT 51 OFFSET $2"
        ),
        "announcements" => read!(
            "SELECT json_build_object('id',id,'body',body,'enabled',enabled,'expires_at',expires_at)::text FROM announcements WHERE position(lower($1) in lower(body))>0 ORDER BY created_at DESC,id DESC LIMIT 51 OFFSET $2"
        ),
        "signals" => read!(
            "SELECT json_build_object('id',s.id,'user',s.user_id,'login',u.login::text,'kind',s.kind,'day',s.day::text,'score',s.score,'details',convert_from(s.details,'UTF8')::json,'reviewed',s.reviewed)::text FROM abuse_signals s JOIN users u ON u.id=s.user_id WHERE position(lower($1) in lower(u.login::text||' '||s.kind))>0 ORDER BY s.day DESC,s.score DESC,s.id LIMIT 51 OFFSET $2"
        ),
        "clans" => read!(
            "SELECT json_build_object('id',id,'name',name::text,'status',status,'description',description,'motd',motd,'revision',revision)::text FROM clans WHERE position(lower($1) in lower(name::text))>0 ORDER BY created_at DESC,id DESC LIMIT 51 OFFSET $2"
        ),
        "control" => read!(
            "SELECT json_build_object('maintenance',maintenance,'notice',notice,'revision',revision)::text FROM server_controls WHERE $1='' LIMIT 51 OFFSET $2"
        ),
        _ => return Err(Error("INVALID_MESSAGE")),
    };
    let has_more = rows.len() > 50;
    let rows = rows
        .into_iter()
        .take(50)
        .map(|s| serde_json::from_str(&s).map_err(|_| Error("INTERNAL")))
        .collect::<Result<Vec<_>>>()?;
    tx.commit().await?;
    Ok(Page { rows, has_more })
}
pub async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
    peer: Peer,
    query: std::result::Result<Query<Parameters>, axum::extract::rejection::QueryRejection>,
) -> std::result::Result<Json<Page>, crate::game::HttpError> {
    let id = access::identity(&state, &headers, peer.0).await?;
    state
        .auth
        .limit("admin-read", &id.user().to_string(), 120, 60)
        .map_err(crate::game::HttpError)?;
    let Query(params) = query.map_err(|_| crate::game::response(Error("INVALID_MESSAGE")))?;
    load(&state, &id, params)
        .await
        .map(Json)
        .map_err(crate::game::response)
}
pub async fn post(
    State(state): State<AppState>,
    headers: HeaderMap,
    peer: Peer,
    input: std::result::Result<Json<control::Request>, axum::extract::rejection::JsonRejection>,
) -> std::result::Result<Json<control::Receipt>, crate::game::HttpError> {
    let id = access::identity(&state, &headers, peer.0).await?;
    state
        .auth
        .limit("admin-write", &id.user().to_string(), 30, 60)
        .map_err(crate::game::HttpError)?;
    let Json(input) = input.map_err(|_| crate::game::response(Error("INVALID_MESSAGE")))?;
    control::apply(&state, &id, input)
        .await
        .map(Json)
        .map_err(crate::game::response)
}
