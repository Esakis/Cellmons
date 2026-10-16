use super::*;
use axum::body::{Body, Bytes};
use futures_util::TryStreamExt as _;
use tokio::sync::mpsc;

type Chunk = std::result::Result<Bytes, std::io::Error>;
async fn send(sender: &mpsc::Sender<Chunk>, value: impl Into<Bytes>) -> anyhow::Result<()> {
    sender
        .send(Ok(value.into()))
        .await
        .map_err(|_| anyhow::anyhow!("export client disconnected"))
}
#[derive(Deserialize)]
pub struct Expected {
    account: Option<Uuid>,
}
pub async fn export(
    State(state): State<AppState>,
    axum::extract::Query(expected): axum::extract::Query<Expected>,
    headers: HeaderMap,
) -> Result<Response> {
    let current = session(&state, &headers).await?;
    if expected.account.is_some_and(|user| user != current.user) {
        return Err(Error(StatusCode::CONFLICT, "SESSION_CHANGED", 0));
    }
    state
        .auth
        .limit("privacy-export", &current.user.to_string(), 2, 3600)?;
    let permit = state
        .auth
        .0
        .exports
        .clone()
        .try_acquire_owned()
        .map_err(|_| Error::busy())?;
    let mut transaction = state.db.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *transaction)
        .await?;
    sqlx::query("SET LOCAL statement_timeout='30s'")
        .execute(&mut *transaction)
        .await?;
    let (sender, receiver) = mpsc::channel::<Chunk>(8);
    let failed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let terminal = failed.clone();
    tokio::spawn(async move {
        let _permit = permit;
        let user = current.user;
        let work = async {
            send(&sender, "{\"schema\":1").await?;
            macro_rules! section {
                ($name:literal,$sql:literal) => {{
                    send(&sender, concat!(",\"", $name, "\":[")).await?;
                    let mut rows = sqlx::query_scalar::<_, String>($sql)
                        .bind(user)
                        .fetch(&mut *transaction);
                    let mut first = true;
                    while let Some(row) = rows.try_next().await? {
                        if !first {
                            send(&sender, ",").await?;
                        }
                        first = false;
                        send(&sender, row).await?;
                    }
                    send(&sender, "]").await?;
                }};
            }
            // Explicit fields exclude credentials, moderator-only context and unrevealed RNG.
            section!(
                "agreements",
                "SELECT to_jsonb(a)::text FROM account_agreements a WHERE user_id=$1 ORDER BY accepted_at"
            );
            section!(
                "account",
                "SELECT json_build_object('id',id,'login',login::text,'email',email::text,'verified_at',email_verified_at,'role',role,'status',status,'locale',locale,'start_school',start_school,'challenge_filter',challenge_filter,'description',description,'created_at',created_at,'last_login_at',last_login_at,'deletion_requested_at',deletion_requested_at)::text FROM users WHERE id=$1"
            );
            section!(
                "characters",
                "SELECT to_jsonb(a)::text FROM avatars a WHERE user_id=$1 ORDER BY id"
            );
            section!(
                "templates",
                "SELECT to_jsonb(t)::text FROM unlocked_templates t WHERE user_id=$1 ORDER BY template_id"
            );
            section!(
                "collection",
                "SELECT to_jsonb(c)::text FROM collection c WHERE user_id=$1 ORDER BY card_id"
            );
            section!(
                "decks",
                "SELECT to_jsonb(d)::text FROM decks d JOIN avatars a ON a.id=d.avatar_id WHERE a.user_id=$1 ORDER BY d.avatar_id"
            );
            section!(
                "presets",
                "SELECT to_jsonb(p)::text FROM deck_presets p WHERE user_id=$1 ORDER BY id"
            );
            section!(
                "wallet",
                "SELECT json_build_object('balance',balance::text,'updated_at',updated_at)::text FROM wallets WHERE user_id=$1"
            );
            section!(
                "ledger",
                "SELECT json_build_object('transaction',t.id,'reason',t.reason,'reference',t.ref_id,'created_at',t.created_at,'delta',e.delta::text)::text FROM ledger_entries e JOIN ledger_transactions t ON t.id=e.tx_id WHERE e.account='user:'||$1::uuid::text ORDER BY t.created_at,t.id"
            );
            section!(
                "boosters",
                "SELECT json_build_object('id',id,'kind',kind,'status',status,'price',price::text,'contents',contents,'purchased_at',purchased_at,'opened_at',opened_at,'source',source)::text FROM boosters WHERE user_id=$1 ORDER BY id"
            );
            section!(
                "junkyard",
                "SELECT to_jsonb(j)::text FROM junkyard j WHERE user_id=$1"
            );
            section!(
                "friends",
                "SELECT json_build_object('id',u.id,'login',u.login::text)::text FROM friends f JOIN users u ON u.id=f.friend_id WHERE f.user_id=$1 ORDER BY u.id"
            );
            section!(
                "ignores",
                "SELECT json_build_object('id',u.id,'login',u.login::text)::text FROM ignores i JOIN users u ON u.id=i.ignored_id WHERE i.user_id=$1 ORDER BY u.id"
            );
            section!(
                "mail",
                "SELECT json_build_object('id',id,'from_user',from_user,'to_user',to_user,'body',body,'created_at',created_at,'read_at',CASE WHEN to_user=$1 THEN read_at ELSE NULL END,'deleted_by_you',CASE WHEN from_user=$1 THEN deleted_by_sender ELSE deleted_by_recipient END)::text FROM mail WHERE from_user=$1 OR to_user=$1 ORDER BY id"
            );
            section!(
                "chat",
                "SELECT to_jsonb(c)::text FROM chat_log c WHERE from_user=$1 OR to_user=$1 ORDER BY id"
            );
            section!(
                "clan_membership",
                "SELECT to_jsonb(m)::text FROM clan_members m WHERE user_id=$1"
            );
            section!(
                "clan_applications",
                "SELECT to_jsonb(a)::text FROM clan_applications a WHERE user_id=$1 ORDER BY clan_id"
            );
            section!(
                "matches",
                "SELECT json_build_object('id',id,'kind',kind,'opponent',CASE WHEN first_user=$1 THEN second_user ELSE first_user END,'winner',winner,'reason',reason,'rounds',rounds,'created_at',created_at,'your_avatar',CASE WHEN first_user=$1 THEN first_avatar ELSE second_avatar END,'your_xp',CASE WHEN first_user=$1 THEN first_xp ELSE second_xp END,'your_ducats',CASE WHEN first_user=$1 THEN first_ducats ELSE second_ducats END,'your_rating_delta',CASE WHEN first_user=$1 THEN first_rating_delta ELSE second_rating_delta END)::text FROM matches WHERE first_user=$1 OR second_user=$1 ORDER BY id"
            );
            section!(
                "rating_history",
                "SELECT to_jsonb(r)::text FROM rating_history r WHERE user_id=$1 ORDER BY match_id"
            );
            section!(
                "tournaments",
                "SELECT json_build_object('id',t.id,'name',t.name,'status',t.status,'entry',to_jsonb(e)-'seat')::text FROM tournament_entries e JOIN tournaments t ON t.id=e.tournament_id WHERE e.user_id=$1 ORDER BY t.id"
            );
            section!(
                "expeditions",
                "SELECT (to_jsonb(e)-'plan'-'award'-'content_version')::text FROM expeditions e WHERE user_id=$1 ORDER BY id"
            );
            section!(
                "reports",
                "SELECT json_build_object('id',id,'kind',target_kind,'target',target_id,'reason',reason,'status',status,'created_at',created_at,'resolved_at',resolved_at)::text FROM moderation_reports WHERE reporter=$1 ORDER BY id"
            );
            section!(
                "notices",
                "SELECT to_jsonb(n)::text FROM user_notices n WHERE user_id=$1 ORDER BY id"
            );
            section!(
                "sessions",
                "SELECT json_build_object('kind',kind,'created_at',created_at,'expires_at',expires_at,'last_seen_at',last_seen_at,'device',device_label,'user_agent',user_agent)::text FROM sessions WHERE user_id=$1 ORDER BY created_at"
            );
            section!(
                "privacy_events",
                "SELECT to_jsonb(e)::text FROM privacy_events e WHERE user_id=$1 ORDER BY created_at,id"
            );
            send(&sender, "}").await?;
            transaction.commit().await?;
            Ok::<(), anyhow::Error>(())
        };
        if !matches!(
            tokio::time::timeout(std::time::Duration::from_secs(120), work).await,
            Ok(Ok(()))
        ) {
            terminal.store(true, std::sync::atomic::Ordering::Release);
            tracing::warn!("account export interrupted");
        }
    });
    let stream = futures_util::stream::unfold(
        (receiver, failed, false),
        |(mut receiver, failed, done)| async move {
            if done {
                return None;
            }
            match receiver.recv().await {
                Some(chunk) => Some((chunk, (receiver, failed, false))),
                None if failed.load(std::sync::atomic::Ordering::Acquire) => Some((
                    Err(std::io::Error::other("export interrupted")),
                    (receiver, failed, true),
                )),
                None => None,
            }
        },
    );
    let mut response = Body::from_stream(stream).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        "application/json; charset=utf-8".parse().unwrap(),
    );
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        "attachment; filename=\"spellgambit-data.json\""
            .parse()
            .unwrap(),
    );
    Ok(response)
}
