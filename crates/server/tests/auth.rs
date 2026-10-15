//! Auth acceptance tests use disposable SQLx databases and a captured mail transport.
use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{HeaderMap, Request, StatusCode, header},
};
use http_body_util::BodyExt as _;
use serde_json::{Value, json};
use server::{
    auth::{
        Auth, AuthConfig,
        mail::{Mail, Mailer},
    },
    gateway::{self, AppState},
};
use sqlx::PgPool;
use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use tower::ServiceExt as _;

const PASSWORD: &str = "A long test password!";
struct Fixture {
    state: AppState,
    mail: Arc<Mutex<Vec<Mail>>>,
}
struct Reply {
    status: StatusCode,
    headers: HeaderMap,
    body: Value,
}
impl Fixture {
    fn new(db: PgPool) -> Self {
        let mail = Arc::new(Mutex::new(Vec::new()));
        let auth = Auth::new(AuthConfig::local(), Mailer::Capture(mail.clone()));
        Self {
            state: AppState::new(db, auth, server::online::Settings::default()),
            mail,
        }
    }
    async fn call(&self, method: &str, path: &str, body: Value, extra: &[(&str, &str)]) -> Reply {
        let mut request = Request::builder()
            .method(method)
            .uri(path)
            .header("x-requested-with", "app")
            .header(header::CONTENT_TYPE, "application/json");
        for (key, value) in extra {
            request = request.header(*key, *value);
        }
        let mut request = request
            .body(if body.is_null() {
                Body::empty()
            } else {
                Body::from(body.to_string())
            })
            .unwrap();
        request.extensions_mut().insert(ConnectInfo(
            "127.0.0.1:50000".parse::<SocketAddr>().unwrap(),
        ));
        let response = gateway::router(self.state.clone())
            .oneshot(request)
            .await
            .unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap()
        };
        Reply {
            status,
            headers,
            body,
        }
    }
    async fn register(&self) {
        let reply = self.call("POST","/api/auth/register",json!({"login":"PlayerOne","email":"one@example.test","password":PASSWORD,"school":"combat"}), &[]).await;
        assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.body);
        assert!(reply.body.get("token").is_none());
    }
    async fn login(&self, android: bool) -> Reply {
        self.call("POST","/api/auth/login",json!({"identity":"PLAYERONE","password":PASSWORD,"client":if android {"android"} else {"web"}}), if android {&[("x-native-version","1.0.0"),("origin","https://localhost")]} else {&[]}).await
    }
    fn token(&self, index: usize) -> String {
        let mail = self.mail.lock().unwrap();
        mail[index]
            .body
            .split("token=")
            .nth(1)
            .unwrap()
            .chars()
            .take(64)
            .collect()
    }
    async fn wait_mail(&self, count: usize) {
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            while self.mail.lock().unwrap().len() < count {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
            // Mail capture precedes commit. Wait for the background transaction to release users.
            sqlx::query("SELECT id FROM users FOR UPDATE")
                .execute(&self.state.db)
                .await
                .unwrap();
        })
        .await
        .expect("background mail delivery");
    }
}
fn cookie(reply: &Reply) -> String {
    reply.headers[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .into()
}
fn bearer(reply: &Reply) -> String {
    format!("Bearer {}", reply.body["token"].as_str().unwrap())
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn r_knt_01_registration_provisions_the_compiled_starter_atomically(pool: PgPool) {
    let f = Fixture::new(pool.clone());
    f.register().await;
    let row:(String,i64,i64,i64,i32)=sqlx::query_as("SELECT password_hash,(SELECT count(*) FROM avatars WHERE is_active),(SELECT sum(qty)::bigint FROM collection),(SELECT cardinality(cards)::bigint FROM decks),(SELECT rating FROM ratings) FROM users").fetch_one(&pool).await.unwrap();
    assert!(row.0.starts_with("$argon2id$v=19$m=19456,t=2,p=1$"));
    assert_eq!((row.1, row.2, row.3, row.4), (1, 40, 20, 1000));
    assert_eq!(f.mail.lock().unwrap().len(), 1);
    let duplicate=f.call("POST","/api/auth/register",json!({"login":"playerone","email":"other@example.test","password":PASSWORD,"school":"tech"}), &[]).await;
    assert_eq!(duplicate.status, StatusCode::CONFLICT);
    let invalid=f.call("POST","/api/auth/register",json!({"login":"Second","email":"two@example.test","password":"password123","school":"tech"}), &[]).await;
    assert_eq!(invalid.status, StatusCode::BAD_REQUEST);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn web_sessions_are_httponly_rotated_and_revoked(pool: PgPool) {
    let f = Fixture::new(pool.clone());
    f.register().await;
    let login = f.login(false).await;
    assert_eq!(login.status, StatusCode::OK);
    assert_eq!(login.headers[header::CACHE_CONTROL], "no-store");
    assert!(login.body.get("token").is_none());
    let header = login.headers[header::SET_COOKIE].to_str().unwrap();
    assert!(
        header.contains("HttpOnly") && header.contains("SameSite=Lax") && header.contains("Path=/")
    );
    let first = cookie(&login);
    let me = f
        .call("GET", "/api/me", Value::Null, &[("cookie", &first)])
        .await;
    assert_eq!(me.body["login"], "PlayerOne");
    assert_eq!(me.body["email_verified"], false);
    assert_eq!(me.body["avatar"]["level"], 1);
    let second = f
        .call(
            "POST",
            "/api/auth/login",
            json!({"identity":"ONE@EXAMPLE.TEST","password":PASSWORD}),
            &[("cookie", &first)],
        )
        .await;
    assert_eq!(second.status, StatusCode::OK);
    let second = cookie(&second);
    assert_ne!(first, second);
    assert_eq!(
        f.call("GET", "/api/me", Value::Null, &[("cookie", &first)])
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        f.call(
            "POST",
            "/api/auth/logout",
            json!({}),
            &[("cookie", &second)]
        )
        .await
        .status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        f.call("GET", "/api/me", Value::Null, &[("cookie", &second)])
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM sessions")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn android_cors_rotation_and_one_time_ws_tickets(pool: PgPool) {
    let f = Fixture::new(pool.clone());
    f.register().await;
    let login = f.login(true).await;
    assert_eq!(login.status, StatusCode::OK, "{}", login.body);
    assert!(login.headers.get(header::SET_COOKIE).is_none());
    assert_eq!(
        login.headers[header::ACCESS_CONTROL_ALLOW_ORIGIN],
        "https://localhost"
    );
    assert!(
        login
            .headers
            .get(header::ACCESS_CONTROL_ALLOW_CREDENTIALS)
            .is_none()
    );
    let bearer = bearer(&login);
    let headers = [
        ("authorization", bearer.as_str()),
        ("x-native-version", "1.0.0"),
        ("origin", "https://localhost"),
    ];
    assert_eq!(
        f.call("GET", "/api/me", Value::Null, &headers).await.status,
        StatusCode::OK
    );
    let ticket = f
        .call("POST", "/api/auth/ws-ticket", json!({}), &headers)
        .await;
    let raw = ticket.body["ticket"].as_str().unwrap();
    assert!(
        f.state
            .auth
            .consume_ws_ticket(&f.state, raw, "https://evil.test")
            .await
            .is_err()
    );
    let (a, b) = tokio::join!(
        f.state
            .auth
            .consume_ws_ticket(&f.state, raw, "https://localhost"),
        f.state
            .auth
            .consume_ws_ticket(&f.state, raw, "https://localhost")
    );
    assert_ne!(a.is_ok(), b.is_ok());
    let another = f
        .call("POST", "/api/auth/ws-ticket", json!({}), &headers)
        .await;
    let (a, b) = tokio::join!(
        f.call("POST", "/api/auth/rotate", json!({}), &headers),
        f.call("POST", "/api/auth/rotate", json!({}), &headers)
    );
    let rotated = if a.status == StatusCode::OK {
        assert_eq!(b.status, StatusCode::UNAUTHORIZED);
        a
    } else {
        assert_eq!(b.status, StatusCode::OK);
        b
    };
    assert!(
        f.state
            .auth
            .consume_ws_ticket(
                &f.state,
                another.body["ticket"].as_str().unwrap(),
                "https://localhost"
            )
            .await
            .is_err()
    );
    assert_eq!(
        f.call("GET", "/api/me", Value::Null, &headers).await.status,
        StatusCode::UNAUTHORIZED
    );
    let next = crate::bearer(&rotated);
    let headers = [
        ("authorization", next.as_str()),
        ("x-native-version", "1.0.0"),
    ];
    assert_eq!(
        f.call("GET", "/api/me", Value::Null, &headers).await.status,
        StatusCode::OK
    );
    assert_eq!(
        f.call("POST", "/api/auth/logout", json!({}), &headers)
            .await
            .status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        f.call("GET", "/api/me", Value::Null, &headers).await.status,
        StatusCode::UNAUTHORIZED
    );
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn email_verification_is_single_use_under_concurrency(pool: PgPool) {
    let f = Fixture::new(pool.clone());
    f.register().await;
    let token = f.token(0);
    let (a, b) = tokio::join!(
        f.call(
            "POST",
            "/api/auth/verify-email",
            json!({"token":token}),
            &[]
        ),
        f.call(
            "POST",
            "/api/auth/verify-email",
            json!({"token":token}),
            &[]
        )
    );
    assert_ne!(a.status == StatusCode::OK, b.status == StatusCode::OK);
    assert_eq!(
        f.call(
            "POST",
            "/api/auth/verify-email",
            json!({"token":token}),
            &[]
        )
        .await
        .body["error"],
        "INVALID_TOKEN"
    );
    let verified: bool = sqlx::query_scalar("SELECT email_verified_at IS NOT NULL FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(verified);
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn reset_password_revokes_all_sessions_and_email_tokens(pool: PgPool) {
    let f = Fixture::new(pool.clone());
    f.register().await;
    let web = cookie(&f.login(false).await);
    let app = bearer(&f.login(true).await);
    let absent = f
        .call(
            "POST",
            "/api/auth/password-reset/request",
            json!({"email":"missing@example.test"}),
            &[],
        )
        .await;
    let existing = f
        .call(
            "POST",
            "/api/auth/password-reset/request",
            json!({"email":"ONE@example.test"}),
            &[],
        )
        .await;
    f.wait_mail(2).await;
    assert_eq!(absent.body, existing.body);
    assert_eq!(f.mail.lock().unwrap().len(), 2);
    let token = f.token(1);
    let reset = f
        .call(
            "POST",
            "/api/auth/password-reset/confirm",
            json!({"token":token,"password":"New long test password!"}),
            &[],
        )
        .await;
    assert_eq!(reset.status, StatusCode::OK, "{}", reset.body);
    assert_eq!(
        f.call("GET", "/api/me", Value::Null, &[("cookie", &web)])
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        f.call(
            "GET",
            "/api/me",
            Value::Null,
            &[("authorization", &app), ("x-native-version", "1.0.0")]
        )
        .await
        .status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(f.login(false).await.status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        f.call(
            "POST",
            "/api/auth/login",
            json!({"identity":"PlayerOne","password":"New long test password!"}),
            &[]
        )
        .await
        .status,
        StatusCode::OK
    );
    assert_eq!(
        f.call(
            "POST",
            "/api/auth/password-reset/confirm",
            json!({"token":token,"password":"Another long password!"}),
            &[]
        )
        .await
        .body["error"],
        "INVALID_TOKEN"
    );
    assert_eq!(
        f.call(
            "POST",
            "/api/auth/verify-email",
            json!({"token":f.token(0)}),
            &[]
        )
        .await
        .body["error"],
        "INVALID_TOKEN"
    );
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn login_limits_share_identity_between_login_and_email(pool: PgPool) {
    let f = Fixture::new(pool);
    f.register().await;
    for i in 0..5 {
        let reply=f.call("POST","/api/auth/login",json!({"identity":if i%2==0 {"PlayerOne"} else {"ONE@EXAMPLE.TEST"},"password":"incorrect password!"}),&[("x-forwarded-for","192.0.2.123")]).await;
        assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
    }
    let limited = f.login(false).await;
    assert_eq!(limited.status, StatusCode::TOO_MANY_REQUESTS);
    assert!(
        limited.headers[header::RETRY_AFTER]
            .to_str()
            .unwrap()
            .parse::<u64>()
            .unwrap()
            > 0
    );
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn csrf_cors_and_native_version_reject_invalid_requests(pool: PgPool) {
    let f = Fixture::new(pool.clone());
    let request = Request::builder()
        .method("POST")
        .uri("/api/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("{}"))
        .unwrap();
    assert_eq!(
        gateway::router(f.state.clone())
            .oneshot(request)
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        f.call(
            "GET",
            "/api/me",
            Value::Null,
            &[("origin", "https://evil.test")]
        )
        .await
        .status,
        StatusCode::FORBIDDEN
    );
    let version = f
        .call(
            "POST",
            "/api/auth/login",
            json!({"identity":"PlayerOne","password":PASSWORD,"client":"android"}),
            &[
                ("origin", "https://localhost"),
                ("x-native-version", "0.9.9"),
            ],
        )
        .await;
    assert_eq!(version.body["error"], "VERSION_MISMATCH");
    let preflight = f
        .call(
            "OPTIONS",
            "/api/auth/login",
            Value::Null,
            &[
                ("origin", "https://localhost"),
                ("access-control-request-method", "POST"),
                (
                    "access-control-request-headers",
                    "content-type,x-native-version,x-requested-with",
                ),
            ],
        )
        .await;
    assert_eq!(preflight.status, StatusCode::OK);
    assert_eq!(
        preflight.headers[header::ACCESS_CONTROL_ALLOW_ORIGIN],
        "https://localhost"
    );
    assert_eq!(
        f.call("POST", "/api/auth/register", json!({"client":"guest"}), &[])
            .await
            .status,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn expired_or_banned_sessions_cannot_authenticate(pool: PgPool) {
    let f = Fixture::new(pool.clone());
    f.register().await;
    let web = cookie(&f.login(false).await);
    sqlx::query(
        "UPDATE sessions SET created_at=now()-interval '2 days',expires_at=now()-interval '1 day'",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(
        f.call("GET", "/api/me", Value::Null, &[("cookie", &web)])
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    let web = cookie(&f.login(false).await);
    sqlx::query("UPDATE users SET status='banned'")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        f.call("GET", "/api/me", Value::Null, &[("cookie", &web)])
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(f.login(false).await.status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn session_list_and_revocation_are_account_scoped(pool: PgPool) {
    let f = Fixture::new(pool.clone());
    f.register().await;
    let first = cookie(&f.login(false).await);
    let second = cookie(&f.login(false).await);
    let list = f
        .call(
            "GET",
            "/api/auth/sessions",
            Value::Null,
            &[("cookie", &first)],
        )
        .await;
    assert_eq!(list.body["sessions"].as_array().unwrap().len(), 2);
    let other = list.body["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|session| session["current"] == false)
        .unwrap()["id"]
        .as_str()
        .unwrap();
    assert_eq!(
        f.call(
            "DELETE",
            &format!("/api/auth/sessions/{other}"),
            Value::Null,
            &[("cookie", &first)]
        )
        .await
        .status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        f.call("GET", "/api/me", Value::Null, &[("cookie", &second)])
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        f.call("GET", "/api/me", Value::Null, &[("cookie", &first)])
            .await
            .status,
        StatusCode::OK
    );
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn r_knt_01_all_six_schools_receive_their_exact_starter(pool: PgPool) {
    for (index, school) in ["combat", "divine", "element", "arcane", "necro", "tech"]
        .iter()
        .enumerate()
    {
        let f = Fixture::new(pool.clone());
        let login = format!("Player{index}");
        let reply=f.call("POST","/api/auth/register",json!({"login":login,"email":format!("player{index}@example.test"),"password":PASSWORD,"school":school}),&[]).await;
        assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.body);
        let user: uuid::Uuid = sqlx::query_scalar("SELECT id FROM users WHERE login=$1::citext")
            .bind(&login)
            .fetch_one(&pool)
            .await
            .unwrap();
        let actual: Vec<(i16, i32)> =
            sqlx::query_as("SELECT card_id,qty FROM collection WHERE user_id=$1 ORDER BY card_id")
                .bind(user)
                .fetch_all(&pool)
                .await
                .unwrap();
        let starter = &engine_content::STARTERS[index];
        let mut expected: Vec<_> = starter
            .cards
            .iter()
            .map(|(card, qty)| (*card as i16, i32::from(*qty)))
            .collect();
        expected.sort();
        assert_eq!(actual, expected);
        let (avatar,deck):(i32,Vec<i16>)=sqlx::query_as("SELECT a.template_id,d.cards FROM avatars a JOIN decks d ON d.avatar_id=a.id WHERE a.user_id=$1").bind(user).fetch_one(&pool).await.unwrap();
        assert_eq!(avatar, i32::from(starter.avatar));
        assert_eq!(
            deck,
            starter.deck.iter().map(|id| *id as i16).collect::<Vec<_>>()
        );
    }
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn failed_registration_delivery_rolls_back_the_entire_account(pool: PgPool) {
    let mut f = Fixture::new(pool.clone());
    let path = std::env::temp_dir().join(format!("game-mail-failure-{}", uuid::Uuid::now_v7()));
    std::fs::write(&path, b"fixture: not a directory").unwrap();
    f.state.auth = Auth::new(AuthConfig::local(), Mailer::File(path.clone()));
    let reply=f.call("POST","/api/auth/register",json!({"login":"PlayerOne","email":"one@example.test","password":PASSWORD,"school":"combat"}),&[]).await;
    std::fs::remove_file(path).unwrap();
    assert_eq!(reply.status, StatusCode::SERVICE_UNAVAILABLE);
    let users: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    let tokens: i64 = sqlx::query_scalar("SELECT count(*) FROM email_tokens")
        .fetch_one(&pool)
        .await
        .unwrap();
    let avatars: i64 = sqlx::query_scalar("SELECT count(*) FROM avatars")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!((users, tokens, avatars), (0, 0, 0));
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn resend_replaces_verification_and_expired_reset_is_rejected(pool: PgPool) {
    let f = Fixture::new(pool.clone());
    f.register().await;
    let old = f.token(0);
    let reply = f
        .call(
            "POST",
            "/api/auth/verify-email/request",
            json!({"email":"one@example.test"}),
            &[],
        )
        .await;
    assert_eq!(reply.status, StatusCode::OK);
    f.wait_mail(2).await;
    assert_eq!(
        f.call("POST", "/api/auth/verify-email", json!({"token":old}), &[])
            .await
            .body["error"],
        "INVALID_TOKEN"
    );
    assert_eq!(
        f.call(
            "POST",
            "/api/auth/verify-email",
            json!({"token":f.token(1)}),
            &[]
        )
        .await
        .status,
        StatusCode::OK
    );
    f.call(
        "POST",
        "/api/auth/password-reset/request",
        json!({"email":"one@example.test"}),
        &[],
    )
    .await;
    f.wait_mail(3).await;
    let reset = f.token(2);
    sqlx::query("UPDATE email_tokens SET created_at=now()-interval '2 hours',expires_at=now()-interval '1 hour' WHERE kind='reset'").execute(&pool).await.unwrap();
    assert_eq!(
        f.call(
            "POST",
            "/api/auth/password-reset/confirm",
            json!({"token":reset,"password":"A new long password!"}),
            &[]
        )
        .await
        .body["error"],
        "INVALID_TOKEN"
    );
    assert_eq!(f.login(false).await.status, StatusCode::OK);
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn ip_login_limit_cannot_be_bypassed_with_forwarded_headers(pool: PgPool) {
    let f = Fixture::new(pool);
    for index in 0..20 {
        let reply = f
            .call(
                "POST",
                "/api/auth/login",
                json!({"identity":format!("Missing{index}"),"password":"incorrect password!"}),
                &[("x-forwarded-for", &format!("192.0.2.{index}"))],
            )
            .await;
        assert_eq!(reply.status, StatusCode::UNAUTHORIZED, "{}", reply.body);
    }
    assert_eq!(
        f.call(
            "POST",
            "/api/auth/login",
            json!({"identity":"Missing21","password":"incorrect password!"}),
            &[("x-forwarded-for", "192.0.2.21")]
        )
        .await
        .status,
        StatusCode::TOO_MANY_REQUESTS
    );
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn production_cookie_is_host_scoped_and_other_accounts_cannot_revoke_sessions(pool: PgPool) {
    let mut f = Fixture::new(pool.clone());
    let mut config = AuthConfig::local();
    config.secure_cookie = true;
    config.public_origin = "https://game.example".into();
    f.state.auth = Auth::new(config, Mailer::Capture(f.mail.clone()));
    f.register().await;
    let login = f.login(false).await;
    let header = login.headers[header::SET_COOKIE].to_str().unwrap();
    assert!(
        header.starts_with("__Host-game_session=")
            && header.contains("Secure")
            && !header.contains("Domain=")
    );
    let original = cookie(&login);
    let list = f
        .call(
            "GET",
            "/api/auth/sessions",
            Value::Null,
            &[("cookie", &original)],
        )
        .await;
    let id = list.body["sessions"][0]["id"].as_str().unwrap();
    let other = Fixture::new(pool.clone());
    assert_eq!(other.call("POST","/api/auth/register",json!({"login":"PlayerTwo","email":"two@example.test","password":PASSWORD,"school":"tech"}),&[]).await.status,StatusCode::CREATED);
    let other_login = other
        .call(
            "POST",
            "/api/auth/login",
            json!({"identity":"PlayerTwo","password":PASSWORD}),
            &[],
        )
        .await;
    let other_cookie = cookie(&other_login);
    assert_eq!(
        other
            .call(
                "DELETE",
                &format!("/api/auth/sessions/{id}"),
                Value::Null,
                &[("cookie", &other_cookie)]
            )
            .await
            .status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        f.call("GET", "/api/me", Value::Null, &[("cookie", &original)])
            .await
            .status,
        StatusCode::OK
    );
}

#[path = "auth/game.rs"]
mod game;

#[path = "auth/economy.rs"]
mod economy;

#[path = "auth/store.rs"]
mod store;
