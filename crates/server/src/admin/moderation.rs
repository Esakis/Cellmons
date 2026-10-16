//! Player reports capture server records, never client-supplied chat transcripts.
use crate::{
    gateway::AppState,
    online::{Error, Result},
};
use axum::{Json, extract::State, http::HeaderMap};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;
use unicode_normalization::UnicodeNormalization as _;
use uuid::Uuid;

pub(super) fn normalize(value: &str) -> String {
    value.nfkc().filter(|c| !matches!(c,'\u{00ad}'|'\u{034f}'|'\u{061c}'|'\u{180e}'|'\u{200b}'..='\u{200f}'|'\u{202a}'..='\u{202e}'|'\u{2060}'..='\u{206f}'|'\u{feff}'))
        .flat_map(char::to_lowercase).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ")
}
fn contains_phrase(text: &str, phrase: &str) -> bool {
    text.match_indices(phrase).any(|(index, _)| {
        let before = text[..index].chars().next_back();
        let after = text[index + phrase.len()..].chars().next();
        before.is_none_or(|c| !c.is_alphanumeric() && c != '_')
            && after.is_none_or(|c| !c.is_alphanumeric() && c != '_')
    })
}
pub async fn can_speak(db: &PgPool, user: Uuid, text: &str) -> Result<()> {
    let mut connection = db.acquire().await?;
    can_speak_on(&mut connection, user, text).await
}
pub(crate) async fn can_speak_on(
    db: &mut sqlx::PgConnection,
    user: Uuid,
    text: &str,
) -> Result<()> {
    let allowed:bool=sqlx::query_scalar("SELECT status='active' AND (muted_until IS NULL OR muted_until<=clock_timestamp()) FROM users WHERE id=$1").bind(user).fetch_optional(&mut *db).await?.unwrap_or(false);
    if !allowed {
        return Err(Error("MUTED"));
    }
    let text = normalize(text);
    let phrases: Vec<String> =
        sqlx::query_scalar("SELECT phrase FROM word_filters WHERE enabled ORDER BY id LIMIT 2001")
            .fetch_all(&mut *db)
            .await?;
    if phrases.iter().any(|phrase| contains_phrase(&text, phrase)) {
        return Err(Error("FILTERED"));
    }
    Ok(())
}
/// Expiry is checked at authentication boundaries, so a timed ban ends without a cron delay.
pub async fn expire_ban(db: &PgPool, user: Option<Uuid>, login: Option<&str>) -> Result<()> {
    sqlx::query("UPDATE users SET status='active',banned_until=NULL WHERE status='banned' AND banned_until<=clock_timestamp() AND (id=$1 OR login=$2::citext OR email=$2::citext)").bind(user).bind(login).execute(db).await?;
    Ok(())
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub id: Uuid,
    pub kind: String,
    pub target: Uuid,
    pub reason: String,
}
pub async fn report(db: &PgPool, user: Uuid, input: Report) -> Result<Uuid> {
    let reason = crate::online::text(&input.reason, 2000)?;
    if reason.chars().count() < 5 {
        return Err(Error("INVALID_MESSAGE"));
    }
    let mut tx = db.begin().await?;
    let active: bool =
        sqlx::query_scalar("SELECT status='active' FROM users WHERE id=$1 FOR UPDATE")
            .bind(user)
            .fetch_optional(&mut *tx)
            .await?
            .unwrap_or(false);
    if !active {
        return Err(Error("FORBIDDEN"));
    }
    if let Some(old) = sqlx::query_as::<_, (Uuid, String, Uuid, String)>(
        "SELECT reporter,target_kind,target_id,reason FROM moderation_reports WHERE id=$1",
    )
    .bind(input.id)
    .fetch_optional(&mut *tx)
    .await?
    {
        return if old == (user, input.kind, input.target, reason) {
            Ok(input.id)
        } else {
            Err(Error("CONFLICT"))
        };
    }
    let recent:i64=sqlx::query_scalar("SELECT count(*) FROM moderation_reports WHERE reporter=$1 AND created_at>now()-interval '1 day'").bind(user).fetch_one(&mut *tx).await?;
    if recent >= 20 {
        return Err(Error("RATE_LIMITED"));
    }
    let context:Option<String>=match input.kind.as_str() {
        "player"=>sqlx::query_scalar("SELECT json_build_object('id',id,'login',login::text,'description',description)::text FROM users WHERE id=$1 AND status IN ('active','banned')").bind(input.target).fetch_optional(&mut *tx).await?,
        "clan"=>sqlx::query_scalar("SELECT json_build_object('id',id,'name',name::text,'description',description)::text FROM clans WHERE id=$1 AND status IN ('founding','active')").bind(input.target).fetch_optional(&mut *tx).await?,
        "chat"=>sqlx::query_scalar("SELECT json_build_object('id',c.id,'from',c.from_user,'to',c.to_user,'room',c.room,'text',c.text,'created_at',c.created_at)::text FROM chat_log c WHERE c.id=$1 AND (c.from_user=$2 OR c.to_user=$2 OR (c.to_user IS NULL AND ((c.clan_id IS NOT NULL AND EXISTS(SELECT 1 FROM clan_members m WHERE m.clan_id=c.clan_id AND m.user_id=$2)) OR (c.clan_id IS NULL AND c.room<>'expedition' AND c.room NOT LIKE 'clan:%') OR (c.room='expedition' AND EXISTS(SELECT 1 FROM expeditions e WHERE e.user_id=$2 AND e.created_at<=c.created_at AND (e.ended_at IS NULL OR e.ended_at>=c.created_at))))))").bind(input.target).bind(user).fetch_optional(&mut *tx).await?,
        _=>return Err(Error("INVALID_MESSAGE")),
    };
    let context = context.ok_or(Error("NOT_FOUND"))?;
    sqlx::query("INSERT INTO moderation_reports(id,reporter,target_kind,target_id,reason,context) VALUES ($1,$2,$3,$4,$5,$6)").bind(input.id).bind(user).bind(input.kind).bind(input.target).bind(reason).bind(context.as_bytes()).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(input.id)
}
pub async fn post(
    State(state): State<AppState>,
    headers: HeaderMap,
    input: std::result::Result<Json<Report>, axum::extract::rejection::JsonRejection>,
) -> std::result::Result<Json<Value>, crate::game::HttpError> {
    let session = crate::auth::security::session(&state, &headers)
        .await
        .map_err(crate::game::HttpError)?;
    state
        .auth
        .limit("report", &session.user.to_string(), 5, 300)
        .map_err(crate::game::HttpError)?;
    let Json(input) = input.map_err(|_| crate::game::response(Error("INVALID_MESSAGE")))?;
    let id = report(&state.db, session.user, input)
        .await
        .map_err(crate::game::response)?;
    Ok(Json(serde_json::json!({"id":id})))
}
/// Restricted read-only endpoint also works with a banned user's still-valid credential.
pub async fn notices(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> std::result::Result<Json<Value>, crate::game::HttpError> {
    let (token, kind) =
        crate::auth::security::credential(&state.auth, &headers).map_err(crate::game::HttpError)?;
    let hash = crate::auth::crypto::hash(&token);
    let user:Uuid=sqlx::query_scalar("SELECT s.user_id FROM sessions s JOIN users u ON u.id=s.user_id WHERE s.id_hash=$1 AND s.kind=$2 AND s.expires_at>clock_timestamp() AND u.status IN ('active','banned')").bind(hash).bind(kind).fetch_optional(&state.db).await.map_err(|e|crate::game::response(e.into()))?.ok_or_else(||crate::game::response(Error("UNAUTHENTICATED")))?;
    state
        .auth
        .limit("notices", &user.to_string(), 30, 60)
        .map_err(crate::game::HttpError)?;
    let notices:Vec<String>=sqlx::query_scalar("SELECT json_build_object('id',id,'kind',kind,'body',body,'created_at',created_at)::text FROM user_notices WHERE user_id=$1 ORDER BY created_at DESC,id DESC LIMIT 50").bind(user).fetch_all(&state.db).await.map_err(|e|crate::game::response(e.into()))?;
    let rows = notices
        .into_iter()
        .map(|s| serde_json::from_str::<Value>(&s))
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|_| crate::game::response(Error("INTERNAL")))?;
    Ok(Json(serde_json::json!({"notices":rows})))
}
pub async fn public_notices(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> std::result::Result<Json<Value>, crate::game::HttpError> {
    let session = crate::auth::security::session(&state, &headers)
        .await
        .map_err(crate::game::HttpError)?;
    state
        .auth
        .limit("bulletin", &session.user.to_string(), 30, 60)
        .map_err(crate::game::HttpError)?;
    let control:String=sqlx::query_scalar("SELECT json_build_object('maintenance',maintenance,'notice',notice,'revision',revision)::text FROM server_controls WHERE singleton").fetch_one(&state.db).await.map_err(|e|crate::game::response(e.into()))?;
    let notices:Vec<String>=sqlx::query_scalar("SELECT json_build_object('id',id,'body',body,'expires_at',expires_at)::text FROM announcements WHERE enabled AND expires_at>clock_timestamp() ORDER BY created_at DESC LIMIT 20").fetch_all(&state.db).await.map_err(|e|crate::game::response(e.into()))?;
    let control: Value =
        serde_json::from_str(&control).map_err(|_| crate::game::response(Error("INTERNAL")))?;
    let announcements = notices
        .into_iter()
        .map(|s| serde_json::from_str::<Value>(&s))
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|_| crate::game::response(Error("INTERNAL")))?;
    Ok(Json(
        serde_json::json!({"control":control,"announcements":announcements}),
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normalization_and_word_boundaries_avoid_substring_false_positives() {
        assert_eq!(normalize("  ＦＯＯ\u{200b}bar\tZAŻÓŁĆ  "), "foobar zażółć");
        assert!(contains_phrase(&normalize("[BAd Word!]"), "bad word"));
        assert!(!contains_phrase("classical", "ass"));
        assert!(!contains_phrase("my_bad", "bad"));
        assert!(contains_phrase("bad!", "bad"));
    }
}
