//! Staff identity, encrypted factors, replay protection, peer binding and revocation.
use argon2::password_hash::PasswordHasher as _;
use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{HeaderMap, Request, StatusCode},
};
use http_body_util::BodyExt as _;
use serde_json::Value;
use server::{
    admin::{self, access},
    gateway::{self, AppState},
};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use tower::ServiceExt as _;
use uuid::Uuid;
const PASSWORD: &str = "A private staff test password!";
#[path = "admin/control.rs"]
mod control;

struct Fixture {
    state: AppState,
    user: Uuid,
    headers: HeaderMap,
}
impl Fixture {
    async fn new(db: PgPool) -> Self {
        let user = Uuid::now_v7();
        let token = "11".repeat(32);
        let hash = Sha256::digest(token.as_bytes()).to_vec();
        let password = argon2::Argon2::default()
            .hash_password(PASSWORD.as_bytes())
            .unwrap()
            .to_string();
        sqlx::query("INSERT INTO users(id,login,email,password_hash,start_school,role,email_verified_at) VALUES ($1,'StaffOwner','staff@example.test',$2,'combat','admin',now())").bind(user).bind(password).execute(&db).await.unwrap();
        sqlx::query("INSERT INTO sessions(id_hash,kind,user_id,expires_at) VALUES ($1,'web',$2,now()+interval '1 day')").bind(hash).bind(user).execute(&db).await.unwrap();
        let mut config = server::auth::AuthConfig::local();
        config.admin = Some(
            admin::Config::new(
                [7; 32],
                vec![
                    "127.0.0.1/32".parse().unwrap(),
                    "198.51.100.2/32".parse().unwrap(),
                ],
            )
            .unwrap(),
        );
        let state = AppState::new(
            db,
            server::auth::Auth::from_config(config).unwrap(),
            server::online::Settings {
                run_duels: false,
                ..Default::default()
            },
        );
        let mut headers = HeaderMap::new();
        headers.insert("cookie", format!("game_session={token}").parse().unwrap());
        Self {
            state,
            user,
            headers,
        }
    }
    async fn identity(&self, peer: &str) -> access::Identity {
        access::identity(
            &self.state,
            &self.headers,
            Some(ConnectInfo(peer.parse().unwrap())),
        )
        .await
        .ok()
        .unwrap()
    }
    async fn call(
        &self,
        method: &str,
        body: Value,
        peer: &str,
        forwarded: Option<&str>,
    ) -> (StatusCode, Value) {
        self.call_at("/api/admin/session", method, body, peer, forwarded)
            .await
    }
    async fn call_at(
        &self,
        path: &str,
        method: &str,
        body: Value,
        peer: &str,
        forwarded: Option<&str>,
    ) -> (StatusCode, Value) {
        let mut request = Request::builder()
            .method(method)
            .uri(path)
            .header("content-type", "application/json")
            .header("x-requested-with", "app");
        request.headers_mut().unwrap().extend(self.headers.clone());
        if let Some(value) = forwarded {
            request = request.header("x-forwarded-for", value);
        }
        let mut request = request
            .body(if body.is_null() {
                Body::empty()
            } else {
                Body::from(body.to_string())
            })
            .unwrap();
        request
            .extensions_mut()
            .insert(ConnectInfo(peer.parse::<std::net::SocketAddr>().unwrap()));
        let response = gateway::router(self.state.clone())
            .oneshot(request)
            .await
            .unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (status, serde_json::from_slice(&bytes).unwrap())
    }
}
fn totp(setup: &access::Setup) -> totp_rs::Totp {
    totp_rs::Builder::new()
        .with_secret(totp_rs::Secret::try_from_base32(&setup.secret).unwrap())
        .build()
        .unwrap()
}
fn code(setup: &access::Setup) -> String {
    totp(setup)
        .generate(server::online::now_ms() as u64 / 1000)
        .to_string()
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn staff_gateway_is_disabled_without_config_and_rejects_ip_spoofing_and_non_staff(
    db: PgPool,
) {
    let f = Fixture::new(db.clone()).await;
    let (status, gate) = f.call("GET", Value::Null, "127.0.0.1:50000", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(gate["enrolled"], false);
    assert_eq!(gate["authorized"], false);
    assert_eq!(
        f.call("GET", Value::Null, "203.0.113.1:50000", Some("127.0.0.1"))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert!(access::identity(&f.state, &f.headers, None).await.is_err());
    sqlx::query("UPDATE users SET role='mentor' WHERE id=$1")
        .bind(f.user)
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(
        f.call("GET", Value::Null, "127.0.0.1:50000", None).await.0,
        StatusCode::FORBIDDEN
    );
    sqlx::query("UPDATE users SET role='admin' WHERE id=$1")
        .bind(f.user)
        .execute(&db)
        .await
        .unwrap();
    let disabled = AppState::local(db);
    assert!(
        access::identity(
            &disabled,
            &f.headers,
            Some(ConnectInfo("127.0.0.1:50000".parse().unwrap()))
        )
        .await
        .is_err()
    );
    f.state.online.hub.shutdown().await;
    disabled.online.hub.shutdown().await;
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn factor_enrollment_and_network_retry_are_idempotent_but_otp_replay_is_not(db: PgPool) {
    let f = Fixture::new(db.clone()).await;
    let id = f.identity("127.0.0.1:50000").await;
    assert_eq!(
        access::begin_setup(&f.state, &id, "incorrect".into())
            .await
            .err()
            .unwrap()
            .0,
        "INVALID_CREDENTIALS"
    );
    let setup = access::begin_setup(&f.state, &id, PASSWORD.into())
        .await
        .unwrap();
    let retry = access::begin_setup(&f.state, &id, PASSWORD.into())
        .await
        .unwrap();
    assert_eq!(setup.secret, retry.secret);
    assert_eq!(setup.uri, retry.uri);
    assert_eq!(setup.expires_ms, retry.expires_ms);
    let request = Uuid::now_v7();
    let token = code(&setup);
    let mut jobs = vec![];
    for _ in 0..100 {
        let state = f.state.clone();
        let id = id.clone();
        let token = token.clone();
        jobs.push(tokio::spawn(async move {
            access::complete_factor(&state, &id, request, true, None, token).await
        }));
    }
    for job in jobs {
        job.await.unwrap().unwrap();
    }
    let gate = access::gate(&f.state, &id).await.unwrap();
    assert!(gate.enrolled && gate.authorized);
    let expires = gate.expires_ms;
    access::complete_factor(&f.state, &id, request, true, None, token.clone())
        .await
        .unwrap();
    assert_eq!(
        access::gate(&f.state, &id).await.unwrap().expires_ms,
        expires
    );
    assert_eq!(
        access::complete_factor(
            &f.state,
            &id,
            Uuid::now_v7(),
            false,
            Some(PASSWORD.into()),
            token
        )
        .await
        .unwrap_err()
        .0,
        "INVALID_OTP"
    );
    let other_ip = f.identity("198.51.100.2:50000").await;
    assert!(!access::gate(&f.state, &other_ip).await.unwrap().authorized);
    let cipher: Vec<u8> =
        sqlx::query_scalar("SELECT secret_cipher FROM admin_totp WHERE user_id=$1")
            .bind(f.user)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(cipher.len(), 48);
    assert!(
        !cipher
            .windows(20)
            .any(|w| w == totp(&setup).secret().as_ref())
    );
    let before: i64 =
        sqlx::query_scalar("SELECT count(*) FROM admin_audit WHERE action='totp_confirm'")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(before, 1);
    assert_eq!(
        sqlx::query("DELETE FROM admin_audit")
            .execute(&db)
            .await
            .unwrap_err()
            .as_database_error()
            .unwrap()
            .code()
            .unwrap(),
        "23514"
    );
    f.state.online.hub.shutdown().await;
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn factor_grants_expire_and_do_not_survive_session_or_role_changes(db: PgPool) {
    let f = Fixture::new(db.clone()).await;
    let id = f.identity("127.0.0.1:50000").await;
    let setup = access::begin_setup(&f.state, &id, PASSWORD.into())
        .await
        .unwrap();
    let request = Uuid::now_v7();
    access::complete_factor(&f.state, &id, request, true, None, code(&setup))
        .await
        .unwrap();
    let token = "22".repeat(32);
    let hash = Sha256::digest(token.as_bytes()).to_vec();
    sqlx::query("INSERT INTO sessions(id_hash,kind,user_id,expires_at) VALUES ($1,'web',$2,now()+interval '1 day')").bind(hash).bind(f.user).execute(&db).await.unwrap();
    let mut other = f.headers.clone();
    other.insert("cookie", format!("game_session={token}").parse().unwrap());
    let other = access::identity(
        &f.state,
        &other,
        Some(ConnectInfo("127.0.0.1:50000".parse().unwrap())),
    )
    .await
    .ok()
    .unwrap();
    assert!(!access::gate(&f.state, &other).await.unwrap().authorized);
    sqlx::query("UPDATE admin_sessions SET expires_at=now()-interval '1 second'")
        .execute(&db)
        .await
        .unwrap();
    assert!(!access::gate(&f.state, &id).await.unwrap().authorized);
    assert_eq!(
        access::complete_factor(&f.state, &id, request, true, None, code(&setup))
            .await
            .unwrap_err()
            .0,
        "STEP_UP_REQUIRED"
    );
    sqlx::query("UPDATE users SET role='player' WHERE id=$1")
        .bind(f.user)
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(
        access::gate(&f.state, &id).await.err().unwrap().0,
        "FORBIDDEN"
    );
    assert_eq!(
        access::begin_setup(&f.state, &id, PASSWORD.into())
            .await
            .err()
            .unwrap()
            .0,
        "FORBIDDEN"
    );
    f.state.online.hub.shutdown().await;
}
