use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{Request, StatusCode, header},
};
use http_body_util::BodyExt as _;
use protocol::{Message as _, game::v1 as pb};
use serde_json::{Value, json};
use server::{
    auth::{Auth, AuthConfig, mail::Mailer},
    gateway::{self, AppState},
};
use sqlx::PgPool;
use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use tower::ServiceExt as _;
use uuid::Uuid;
const PASSWORD: &str = "Privacy fixture password!";
struct User {
    id: Uuid,
    login: String,
    cookie: String,
}
struct Fixture {
    state: AppState,
}
impl Fixture {
    fn new(db: PgPool) -> Self {
        Self {
            state: AppState::new(
                db,
                Auth::new(
                    AuthConfig::local(),
                    Mailer::Capture(Arc::new(Mutex::new(Vec::new()))),
                ),
                server::online::Settings {
                    run_duels: false,
                    ..Default::default()
                },
            ),
        }
    }
    async fn call(
        &self,
        method: &str,
        path: &str,
        body: Value,
        cookie: Option<&str>,
    ) -> (StatusCode, axum::http::HeaderMap, Value) {
        let mut request = Request::builder()
            .method(method)
            .uri(path)
            .header("x-requested-with", "app")
            .header(header::CONTENT_TYPE, "application/json");
        if let Some(cookie) = cookie {
            request = request.header(header::COOKIE, cookie);
        }
        let mut request = request
            .body(if body.is_null() {
                Body::empty()
            } else {
                Body::from(body.to_string())
            })
            .unwrap();
        request.extensions_mut().insert(ConnectInfo(
            "127.0.0.1:51234".parse::<SocketAddr>().unwrap(),
        ));
        let response = gateway::router(self.state.clone())
            .oneshot(request)
            .await
            .unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (
            status,
            headers,
            if bytes.is_empty() {
                Value::Null
            } else {
                serde_json::from_slice(&bytes).unwrap()
            },
        )
    }
    async fn user(&self, login: &str) -> User {
        let(status,_,_)=self.call("POST","/api/auth/register",json!({"login":login,"email":format!("{login}@example.test"),"password":PASSWORD,"school":"combat","terms_version":"2026-10-16","age_confirmed":true}),None).await;
        assert_eq!(status, StatusCode::CREATED);
        let id = sqlx::query_scalar(
            "UPDATE users SET email_verified_at=now() WHERE login=$1::citext RETURNING id",
        )
        .bind(login)
        .fetch_one(&self.state.db)
        .await
        .unwrap();
        let (status, headers, _) = self
            .call(
                "POST",
                "/api/auth/login",
                json!({"identity":login,"password":PASSWORD}),
                None,
            )
            .await;
        assert_eq!(status, StatusCode::OK);
        User {
            id,
            login: login.into(),
            cookie: headers[header::SET_COOKIE]
                .to_str()
                .unwrap()
                .split(';')
                .next()
                .unwrap()
                .into(),
        }
    }
    async fn delete(&self, user: &User) -> StatusCode {
        self.call(
            "DELETE",
            "/api/me",
            json!({"password":PASSWORD,"confirm_login":user.login}),
            Some(&user.cookie),
        )
        .await
        .0
    }
    async fn status(&self, user: &User) -> String {
        sqlx::query_scalar("SELECT status FROM users WHERE id=$1")
            .bind(user.id)
            .fetch_one(&self.state.db)
            .await
            .unwrap()
    }
}

async fn advancement_fixture(db: &PgPool, user: Uuid) {
    let avatar: Uuid = sqlx::query_scalar("SELECT id FROM avatars WHERE user_id=$1 AND is_active")
        .bind(user)
        .fetch_one(db)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO class_contracts(user_id,template_id,fragments,active) VALUES($1,4,14,true)",
    )
    .bind(user)
    .execute(db)
    .await
    .unwrap();
    sqlx::query("INSERT INTO card_mastery(user_id,card_id,xp) VALUES($1,1,45)")
        .bind(user)
        .execute(db)
        .await
        .unwrap();
    sqlx::query("INSERT INTO daily_quests(user_id,day,fights) VALUES($1,CURRENT_DATE,2)")
        .bind(user)
        .execute(db)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO path_packs(user_id,path_id,cards) VALUES($1,1002,ARRAY[1]::smallint[])",
    )
    .bind(user)
    .execute(db)
    .await
    .unwrap();
    sqlx::query("INSERT INTO class_trials(avatar_id,tier,wins) VALUES($1,0,2)")
        .bind(avatar)
        .execute(db)
        .await
        .unwrap();
    sqlx::query("INSERT INTO avatar_advancement(avatar_id,permanent_points,permanent_unspent) VALUES($1,3,2)").bind(avatar).execute(db).await.unwrap();
    sqlx::query(
        "INSERT INTO incarnation_decks(avatar_id,tier,cards) VALUES($1,1,ARRAY[1]::smallint[])",
    )
    .bind(avatar)
    .execute(db)
    .await
    .unwrap();
    sqlx::query("INSERT INTO legacy_progression(avatar_id,profile) SELECT id,to_jsonb(a) FROM avatars a WHERE id=$1").bind(avatar).execute(db).await.unwrap();
}
#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL"]
async fn export_is_a_complete_snapshot_of_own_data_without_credentials_or_other_mail(db: PgPool) {
    let f = Fixture::new(db.clone());
    let a = f.user("PrivacyAlice").await;
    let b = f.user("PrivacyBob").await;
    let c = f.user("PrivacyCarol").await;
    advancement_fixture(&db, a.id).await;
    advancement_fixture(&db, b.id).await;
    sqlx::query("UPDATE users SET account_xp=700 WHERE id=$1")
        .bind(a.id)
        .execute(&db)
        .await
        .unwrap();
    server::game::mutate(
        &db,
        a.id,
        server::game::Mutation::Recruit {
            level: 5,
            template_id: Some(2),
        },
    )
    .await
    .unwrap();
    for (from, to, text) in [
        (a.id, b.id, "My own letter"),
        (b.id, a.id, "A letter sent to me"),
        (b.id, c.id, "Not my letter"),
    ] {
        sqlx::query("INSERT INTO mail(id,from_user,to_user,body) VALUES($1,$2,$3,$4)")
            .bind(Uuid::now_v7())
            .bind(from)
            .bind(to)
            .bind(text)
            .execute(&db)
            .await
            .unwrap();
    }
    sqlx::query("INSERT INTO boosters(id,user_id,kind,price,pools,seed,content_version,purchase_key) VALUES($1,$2,'fixture',1,$3,$4,$5,$6)").bind(Uuid::now_v7()).bind(a.id).bind(b"PRIVATE UNREVEALED POOL".to_vec()).bind(vec![99u8;32]).bind(vec![1u8;32]).bind(Uuid::now_v7()).execute(&db).await.unwrap();
    assert_eq!(
        f.call("GET", "/api/me/export", Value::Null, None).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        f.call(
            "GET",
            &format!("/api/me/export?account={}", b.id),
            Value::Null,
            Some(&a.cookie)
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let (status, headers, data) = f
        .call("GET", "/api/me/export", Value::Null, Some(&a.cookie))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        headers[header::CONTENT_DISPOSITION]
            .to_str()
            .unwrap()
            .starts_with("attachment")
    );
    assert!(
        headers[header::CACHE_CONTROL]
            .to_str()
            .unwrap()
            .contains("no-store")
    );
    assert_eq!(data["account"][0]["id"], a.id.to_string());
    assert_eq!(data["account_progress"][0]["xp"], 700);
    assert_eq!(data["recruitment_rewards"].as_array().unwrap().len(), 1);
    assert_eq!(data["recruitment_rewards"][0]["template_id"], 2);
    for section in [
        "class_contracts",
        "card_mastery",
        "daily_quests",
        "path_packs",
        "class_trials",
        "permanent_points",
        "incarnation_decks",
        "legacy_progression",
    ] {
        assert_eq!(data[section].as_array().unwrap().len(), 1, "{section}");
    }
    assert_eq!(data["mail"].as_array().unwrap().len(), 2);
    assert_eq!(data["wallet"][0]["balance"], "40");
    assert!(!data["collection"].as_array().unwrap().is_empty());
    let text = data.to_string();
    for secret in [
        "password_hash",
        "token_hash",
        "id_hash",
        "ip_hash",
        "PRIVATE UNREVEALED POOL",
        "Not my letter",
        "PrivacyBob@example.test",
    ] {
        assert!(!text.contains(secret));
    }
    assert!(!text.contains(a.cookie.split('=').nth(1).unwrap()));
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL"]
async fn deletion_revokes_sessions_and_restore_does_not_resurrect_them(db: PgPool) {
    let f = Fixture::new(db.clone());
    let a = f.user("RestoreAlice").await;
    assert_eq!(
        f.call(
            "DELETE",
            "/api/me",
            json!({"password":"wrong password","confirm_login":a.login}),
            Some(&a.cookie)
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(f.status(&a).await, "active");
    assert_eq!(f.delete(&a).await, StatusCode::NO_CONTENT);
    assert_eq!(f.status(&a).await, "pending_deletion");
    let (status, _, pending) = f
        .call(
            "POST",
            "/api/auth/deletion-status",
            json!({"identity":a.login,"password":PASSWORD}),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(pending["pending"], true);
    assert!(pending["erase_after"].is_string());
    assert_eq!(f.status(&a).await, "pending_deletion");
    assert_eq!(
        f.call("GET", "/api/me", Value::Null, Some(&a.cookie))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(server::auth::privacy::cleanup(&db).await.unwrap(), 0);
    assert_eq!(
        f.call(
            "POST",
            "/api/auth/cancel-deletion",
            json!({"identity":a.login,"password":"wrong password"}),
            None
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        f.call(
            "POST",
            "/api/auth/cancel-deletion",
            json!({"identity":a.login,"password":PASSWORD}),
            None
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(f.status(&a).await, "active");
    assert_eq!(
        f.call("GET", "/api/me", Value::Null, Some(&a.cookie))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        f.call(
            "POST",
            "/api/auth/login",
            json!({"identity":a.login,"password":PASSWORD}),
            None
        )
        .await
        .0,
        StatusCode::OK
    );
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL"]
async fn banned_users_can_exercise_rights_without_escaping_their_ban(db: PgPool) {
    let f = Fixture::new(db.clone());
    let mut a = f.user("BannedRights").await;
    sqlx::query(
        "UPDATE users SET status='banned',banned_until=now()+interval '2 days' WHERE id=$1",
    )
    .bind(a.id)
    .execute(&db)
    .await
    .unwrap();
    sqlx::query("DELETE FROM sessions WHERE user_id=$1")
        .bind(a.id)
        .execute(&db)
        .await
        .unwrap();
    let (status, headers, _) = f
        .call(
            "POST",
            "/api/auth/privacy-login",
            json!({"identity":a.login,"password":PASSWORD}),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    a.cookie = headers[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .into();
    assert_eq!(
        f.call("GET", "/api/me", Value::Null, Some(&a.cookie))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        f.call("GET", "/api/me/export", Value::Null, Some(&a.cookie))
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(f.delete(&a).await, StatusCode::NO_CONTENT);
    assert_eq!(
        f.call(
            "POST",
            "/api/auth/cancel-deletion",
            json!({"identity":a.login,"password":PASSWORD}),
            None
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(f.status(&a).await, "banned");
    assert_eq!(
        f.call(
            "POST",
            "/api/auth/login",
            json!({"identity":a.login,"password":PASSWORD}),
            None
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL"]
async fn activity_staff_and_clan_leadership_are_not_silently_destroyed(db: PgPool) {
    let f = Fixture::new(db.clone());
    let a = f.user("BusyRights").await;
    let b = f.user("OtherMember").await;
    sqlx::query("INSERT INTO activity_locks(user_id,kind,owner_id) VALUES($1,'expedition',$2)")
        .bind(a.id)
        .bind(Uuid::now_v7())
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(f.delete(&a).await, StatusCode::CONFLICT);
    sqlx::query("DELETE FROM activity_locks WHERE user_id=$1")
        .bind(a.id)
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("UPDATE users SET role='moderator' WHERE id=$1")
        .bind(a.id)
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(f.delete(&a).await, StatusCode::CONFLICT);
    sqlx::query("UPDATE users SET role='player' WHERE id=$1")
        .bind(a.id)
        .execute(&db)
        .await
        .unwrap();
    let clan = Uuid::now_v7();
    sqlx::query("INSERT INTO clans(id,name) VALUES($1,'Privacy Clan')")
        .bind(clan)
        .execute(&db)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO clan_members(clan_id,user_id,role) VALUES($1,$2,'master'),($1,$3,'member')",
    )
    .bind(clan)
    .bind(a.id)
    .bind(b.id)
    .execute(&db)
    .await
    .unwrap();
    assert_eq!(f.delete(&a).await, StatusCode::CONFLICT);
    sqlx::query("UPDATE clan_members SET role='member' WHERE user_id=$1")
        .bind(a.id)
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("UPDATE clan_members SET role='master' WHERE user_id=$1")
        .bind(b.id)
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(f.delete(&a).await, StatusCode::NO_CONTENT);
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM clan_audit WHERE clan_id=$1 AND actor_id=$2 AND action='leave'",
    )
    .bind(clan)
    .bind(a.id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(count, 1);
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL"]
async fn erasure_is_exactly_once_and_preserves_the_currency_ledger(db: PgPool) {
    let f = Fixture::new(db.clone());
    let a = f.user("EraseAlice").await;
    let b = f.user("UntouchedBob").await;
    advancement_fixture(&db, a.id).await;
    sqlx::query("UPDATE users SET account_xp=700 WHERE id=$1")
        .bind(a.id)
        .execute(&db)
        .await
        .unwrap();
    server::game::mutate(
        &db,
        a.id,
        server::game::Mutation::Recruit {
            level: 5,
            template_id: Some(2),
        },
    )
    .await
    .unwrap();
    assert_eq!(f.delete(&a).await, StatusCode::NO_CONTENT);
    sqlx::query("UPDATE users SET deletion_requested_at=now()-interval '31 days' WHERE id=$1")
        .bind(a.id)
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(
        f.call(
            "POST",
            "/api/auth/cancel-deletion",
            json!({"identity":a.login,"password":PASSWORD}),
            None
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    let (one, two) = tokio::join!(
        server::auth::privacy::cleanup(&db),
        server::auth::privacy::cleanup(&db)
    );
    assert_eq!(one.unwrap() + two.unwrap(), 1);
    let (login, email, status): (String, Option<String>, String) =
        sqlx::query_as("SELECT login::text,email::text,status FROM users WHERE id=$1")
            .bind(a.id)
            .fetch_one(&db)
            .await
            .unwrap();
    assert!(login.starts_with("deleted_"));
    assert!(email.is_none());
    assert_eq!(status, "deleted");
    let balances: Vec<i64> = sqlx::query_scalar("SELECT balance FROM wallets ORDER BY balance")
        .fetch_all(&db)
        .await
        .unwrap();
    assert_eq!(balances, vec![0, 40]);
    assert!(server::economy::reconcile(&db).await.unwrap().is_ok());
    assert_eq!(f.status(&b).await, "active");
    assert_eq!(
        f.call("GET", "/api/me", Value::Null, Some(&b.cookie))
            .await
            .0,
        StatusCode::OK
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM collection WHERE user_id=$1")
        .bind(a.id)
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(count, 0);
    let claims: i64 =
        sqlx::query_scalar("SELECT count(*) FROM recruitment_claims WHERE user_id=$1")
            .bind(a.id)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(claims, 0);
    let advancement:i64=sqlx::query_scalar("SELECT (SELECT count(*) FROM class_contracts WHERE user_id=$1)+(SELECT count(*) FROM card_mastery WHERE user_id=$1)+(SELECT count(*) FROM daily_quests WHERE user_id=$1)+(SELECT count(*) FROM path_packs WHERE user_id=$1)+(SELECT count(*) FROM class_trials c JOIN avatars a ON a.id=c.avatar_id WHERE a.user_id=$1)+(SELECT count(*) FROM avatar_advancement c JOIN avatars a ON a.id=c.avatar_id WHERE a.user_id=$1)+(SELECT count(*) FROM incarnation_decks c JOIN avatars a ON a.id=c.avatar_id WHERE a.user_id=$1)+(SELECT count(*) FROM legacy_progression c JOIN avatars a ON a.id=c.avatar_id WHERE a.user_id=$1)").bind(a.id).fetch_one(&db).await.unwrap();
    assert_eq!(advancement, 0);
    assert_eq!(server::auth::privacy::cleanup(&db).await.unwrap(), 0);
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL"]
async fn erasure_redacts_retained_release_profiles_without_changing_combat_history(db: PgPool) {
    let f = Fixture::new(db.clone());
    let a = f.user("HistoryAlice").await;
    let b = f.user("HistoryBob").await;
    let c = f.user("HealthyErasure").await;
    let setup = engine::PlayerSetup {
        avatar: engine_content::avatar(engine_content::STARTERS[0].avatar)
            .unwrap()
            .snapshot(1),
        deck: engine_content::STARTERS[0].deck.iter().copied().collect(),
    };
    let mut seats = Vec::new();
    for user in [&a, &b] {
        let avatar: Uuid =
            sqlx::query_scalar("SELECT id FROM avatars WHERE user_id=$1 AND is_active")
                .bind(user.id)
                .fetch_one(&db)
                .await
                .unwrap();
        let public = pb::PublicPlayer {
            id: user.id.to_string(),
            login: user.login.clone(),
            avatar_id: avatar.to_string(),
            template_id: 1,
            level: 1,
            rating: 1000,
            verified: true,
            description: "Private profile biography".into(),
            ..Default::default()
        };
        seats.push(server::duel::record::Seat::player(
            user.id, avatar, &setup, &public,
        ));
    }
    let settings = server::duel::Settings::default();
    let instance = Uuid::now_v7();
    let id = Uuid::now_v7();
    let mut record = server::duel::record::Record::new(
        id,
        "arena",
        seats
            .try_into()
            .unwrap_or_else(|_| panic!("two fixture seats")),
        [13; 32],
        &settings,
    )
    .unwrap();
    let mut tx = db.begin().await.unwrap();
    server::duel::journal::create(&mut tx, &record, instance, 30000)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    record.ended = Some(server::duel::record::ExternalEnd {
        loser: Some(0),
        cause: 0,
    });
    let (state, _) = record.rebuild().unwrap();
    sqlx::query("UPDATE duel_journal SET data=$2 WHERE id=$1")
        .bind(id)
        .bind(record.bytes().unwrap())
        .execute(&db)
        .await
        .unwrap();
    server::duel::result::settle(&db, instance, &record, &state)
        .await
        .unwrap();
    let original: Vec<u8> =
        sqlx::query_scalar("SELECT replay FROM match_projections WHERE match_id=$1")
            .bind(id)
            .fetch_one(&db)
            .await
            .unwrap();
    let original = pb::MatchReplayData::decode(original.as_slice()).unwrap();
    // Do not depend on the current engine/content decoder to remove identity from an old release.
    let bytes: Vec<u8> = sqlx::query_scalar("SELECT data FROM duel_journal WHERE id=$1")
        .bind(id)
        .fetch_one(&db)
        .await
        .unwrap();
    let mut json: Value = serde_json::from_slice(&bytes).unwrap();
    json["engine_version"] = Value::String("retained-release".into());
    sqlx::query("UPDATE duel_journal SET engine_version='retained-release',data=$2 WHERE id=$1")
        .bind(id)
        .bind(serde_json::to_vec(&json).unwrap())
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(f.delete(&a).await, StatusCode::NO_CONTENT);
    assert_eq!(f.delete(&c).await, StatusCode::NO_CONTENT);
    sqlx::query("UPDATE users SET deletion_requested_at=now()-interval '31 days' WHERE id=$1")
        .bind(a.id)
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("UPDATE users SET deletion_requested_at=now()-interval '31 days' WHERE id=$1")
        .bind(c.id)
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("UPDATE duel_journal SET data=$2 WHERE id=$1")
        .bind(id)
        .bind(b"corrupt archive".to_vec())
        .execute(&db)
        .await
        .unwrap();
    assert!(server::auth::privacy::cleanup(&db).await.is_err());
    assert_eq!(f.status(&c).await, "deleted");
    assert_eq!(f.status(&a).await, "pending_deletion");
    let unchanged: Vec<u8> =
        sqlx::query_scalar("SELECT replay FROM match_projections WHERE match_id=$1")
            .bind(id)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(
        pb::MatchReplayData::decode(unchanged.as_slice()).unwrap(),
        original
    );
    sqlx::query("UPDATE duel_journal SET data=$2 WHERE id=$1")
        .bind(id)
        .bind(serde_json::to_vec(&json).unwrap())
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(server::auth::privacy::cleanup(&db).await.unwrap(), 1);
    let bytes: Vec<u8> =
        sqlx::query_scalar("SELECT replay FROM match_projections WHERE match_id=$1")
            .bind(id)
            .fetch_one(&db)
            .await
            .unwrap();
    let redacted = pb::MatchReplayData::decode(bytes.as_slice()).unwrap();
    assert_eq!(original.rounds.len(), redacted.rounds.len());
    for (before, after) in original.rounds.iter().zip(&redacted.rounds) {
        assert_eq!(before.view, after.view);
        assert_eq!(before.state_hash, after.state_hash);
        assert_eq!(before.events, after.events);
        for p in [&after.you, &after.opponent].into_iter().flatten() {
            if p.id == a.id.to_string() {
                assert_eq!(p.login, "Former player");
                assert!(p.description.is_empty());
            } else {
                assert_eq!(p.login, b.login);
            }
        }
    }
    let bytes: Vec<u8> = sqlx::query_scalar("SELECT data FROM duel_journal WHERE id=$1")
        .bind(id)
        .fetch_one(&db)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&bytes).unwrap();
    let profile: Vec<u8> = serde_json::from_value(json["seats"][0]["profile"].clone()).unwrap();
    assert_eq!(
        pb::PublicPlayer::decode(profile.as_slice()).unwrap().login,
        "Former player"
    );
    assert_eq!(json["engine_version"], "retained-release");
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL"]
async fn privacy_credentials_never_grant_gameplay_or_socket_tickets(db: PgPool) {
    let f = Fixture::new(db.clone());
    let a = f.user("RestrictedSession").await;
    let (status, headers, _) = f
        .call(
            "POST",
            "/api/auth/privacy-login",
            json!({"identity":a.login,"password":PASSWORD}),
            Some(&a.cookie),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let cookie = headers[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap();
    assert_eq!(
        f.call("GET", "/api/me/privacy", Value::Null, Some(cookie))
            .await
            .0,
        StatusCode::OK
    );
    for (method, path, body) in [
        ("GET", "/api/me", Value::Null),
        ("POST", "/api/auth/ws-ticket", json!({})),
        ("POST", "/api/auth/rotate", json!({})),
    ] {
        assert_eq!(
            f.call(method, path, body, Some(cookie)).await.0,
            StatusCode::UNAUTHORIZED
        );
    }
    assert_eq!(
        f.call("GET", "/api/me", Value::Null, Some(&a.cookie))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        f.call("GET", "/api/me/export", Value::Null, Some(cookie))
            .await
            .0,
        StatusCode::OK
    );
    sqlx::query("UPDATE sessions SET created_at=now()-interval '1 day',rotated_at=now()-interval '1 day',expires_at=now()-interval '1 second' WHERE user_id=$1")
        .bind(a.id)
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(
        f.call("GET", "/api/me/privacy", Value::Null, Some(cookie))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL"]
async fn a_partial_export_fails_the_response_body_instead_of_looking_like_a_complete_file(
    db: PgPool,
) {
    let f = Fixture::new(db.clone());
    let a = f.user("BrokenExport").await;
    sqlx::query("ALTER TABLE mail RENAME COLUMN body TO fixture_missing_column")
        .execute(&db)
        .await
        .unwrap();
    let request = Request::builder()
        .uri("/api/me/export")
        .header("cookie", &a.cookie)
        .body(Body::empty())
        .unwrap();
    let response = gateway::router(f.state.clone())
        .oneshot(request)
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.into_body().collect().await.is_err());
}
