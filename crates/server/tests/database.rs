//! F6.1: tests run on disposable PostgreSQL databases, never by clearing the development DB.
//! Run: cargo test -p server --test database -- --ignored (DATABASE_URL needs CREATEDB).
use axum::body::Body;
use axum::http::{Request, StatusCode};
use server::{db, gateway};
use sqlx::{Acquire as _, PgConnection, PgPool};
use tower::ServiceExt as _;

const USER: &str = "0199e800-0000-7000-8000-000000000001";
const OTHER: &str = "0199e800-0000-7000-8000-000000000002";
const AVATAR: &str = "0199e800-0000-7000-8000-000000000010";

async fn user(pool: &PgPool, id: &str, login: &str, email: &str) {
    sqlx::query("INSERT INTO users(id, login, email, password_hash, start_school) VALUES ($1::text::uuid, $2, $3, '$argon2id$v=19$m=19456,t=2,p=1$fixture$fixture', 'combat')")
        .bind(id).bind(login).bind(email).execute(pool).await.unwrap();
}
async fn basic(pool: &PgPool) {
    user(pool, USER, "PlayerOne", "one@example.test").await;
    sqlx::query("INSERT INTO unlocked_templates(user_id, template_id, source) VALUES ($1::text::uuid, 1, 'start')").bind(USER).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO avatars(id, user_id, slot, template_id, is_active) VALUES ($1::text::uuid, $2::text::uuid, 0, 1, true)").bind(AVATAR).bind(USER).execute(pool).await.unwrap();
}
async fn rejects(pool: &PgPool, sql: &'static str, code: &str) {
    let error = sqlx::query(sql).execute(pool).await.unwrap_err();
    assert_eq!(
        error.as_database_error().and_then(|e| e.code()).as_deref(),
        Some(code),
        "{sql}: {error}"
    );
}
async fn ready(pool: &PgPool) -> StatusCode {
    gateway::router(gateway::AppState::local(pool.clone()))
        .oneshot(Request::get("/readyz").body(Body::empty()).unwrap())
        .await
        .unwrap()
        .status()
}

#[sqlx::test(migrations = false)]
#[ignore = "requires PostgreSQL and CREATEDB; check.bat runs this suite"]
async fn clean_database_requires_migrations_and_repeated_migration_is_safe(pool: PgPool) {
    assert_eq!(ready(&pool).await, StatusCode::SERVICE_UNAVAILABLE);
    db::migrate(&pool).await.unwrap();
    assert_eq!(ready(&pool).await, StatusCode::OK);
    let before: Vec<(i64, Vec<u8>)> =
        sqlx::query_as("SELECT version, checksum FROM _sqlx_migrations ORDER BY version")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(before.len(), db::MIGRATOR.iter().count());
    db::migrate(&pool).await.unwrap();
    let after: Vec<(i64, Vec<u8>)> =
        sqlx::query_as("SELECT version, checksum FROM _sqlx_migrations ORDER BY version")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(before, after);
    let tables: i64 = sqlx::query_scalar("SELECT count(*) FROM information_schema.tables WHERE table_schema = 'public' AND table_name IN ('users','sessions','email_tokens','unlocked_templates','avatars','collection','decks','deck_presets','ratings')").fetch_one(&pool).await.unwrap();
    assert_eq!(tables, 9);
}

#[sqlx::test(migrations = false)]
#[ignore = "requires PostgreSQL and CREATEDB; check.bat runs this suite"]
async fn upgrade_preserves_accounts_and_sessions(pool: PgPool) {
    db::MIGRATOR.run_to(202610150001, &pool).await.unwrap();
    user(&pool, USER, "ExistingPlayer", "existing@example.test").await;
    sqlx::query("INSERT INTO sessions(id_hash, kind, user_id, expires_at) VALUES (decode(repeat('ab',32),'hex'), 'app', $1::text::uuid, now() + interval '7 days')").bind(USER).execute(&pool).await.unwrap();
    assert_eq!(ready(&pool).await, StatusCode::SERVICE_UNAVAILABLE);
    db::MIGRATOR.run_to(202610150003, &pool).await.unwrap();
    sqlx::query("INSERT INTO unlocked_templates(user_id, template_id, source) VALUES ($1::text::uuid, 1, 'start')").bind(USER).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO avatars(id, user_id, slot, template_id, is_active) VALUES ($1::text::uuid, $2::text::uuid, 0, 1, true)").bind(AVATAR).bind(USER).execute(&pool).await.unwrap();
    assert_eq!(ready(&pool).await, StatusCode::SERVICE_UNAVAILABLE);
    db::migrate(&pool).await.unwrap();
    let owned_template: Option<i32> =
        sqlx::query_scalar("SELECT owned_template_id FROM avatars WHERE id = $1::text::uuid")
            .bind(AVATAR)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(owned_template, Some(1));
    let login: String =
        sqlx::query_scalar("SELECT login::text FROM users WHERE id = $1::text::uuid")
            .bind(USER)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(login, "ExistingPlayer");
    let sessions: i64 =
        sqlx::query_scalar("SELECT count(*) FROM sessions WHERE user_id = $1::text::uuid")
            .bind(USER)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(sessions, 1);
    assert_eq!(ready(&pool).await, StatusCode::OK);
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL and CREATEDB; check.bat runs this suite"]
async fn accounts_enforce_case_insensitive_identity_and_valid_fields(pool: PgPool) {
    user(&pool, USER, "PlayerOne", "One@Example.test").await;
    rejects(&pool, "INSERT INTO users(id,login,email,password_hash,start_school) SELECT '0199e800-0000-7000-8000-000000000002', 'playerone', 'different@example.test', password_hash, start_school FROM users", "23505").await;
    rejects(&pool, "UPDATE users SET login = 'a'", "23514").await;
    rejects(&pool, "UPDATE users SET login = 'Żółw'", "23514").await;
    rejects(&pool, "UPDATE users SET login = 'bad login'", "23514").await;
    rejects(&pool, "UPDATE users SET login = repeat('x',21)", "23514").await;
    rejects(&pool, "UPDATE users SET email = 'no-at-sign'", "23514").await;
    rejects(
        &pool,
        "UPDATE users SET email = ' one@example.test'",
        "23514",
    )
    .await;
    rejects(
        &pool,
        "UPDATE users SET password_hash = 'plaintext-password'",
        "23514",
    )
    .await;
    rejects(&pool, "UPDATE users SET role = 'owner'", "23514").await;
    rejects(&pool, "UPDATE users SET status = 'invisible'", "23514").await;
    rejects(&pool, "UPDATE users SET locale = 'de'", "23514").await;
    rejects(&pool, "UPDATE users SET start_school = 'ether'", "23514").await;
    rejects(
        &pool,
        "UPDATE users SET challenge_filter = 'friends'",
        "23514",
    )
    .await;
    rejects(
        &pool,
        "UPDATE users SET description = repeat('ą',301)",
        "23514",
    )
    .await;
    user(&pool, OTHER, "OtherPlayer", "other@example.test").await;
    rejects(
        &pool,
        "UPDATE users SET email = 'ONE@example.TEST' WHERE login = 'OtherPlayer'",
        "23505",
    )
    .await;
    let verified: bool = sqlx::query_scalar(
        "SELECT email_verified_at IS NOT NULL FROM users WHERE id = $1::text::uuid",
    )
    .bind(USER)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!verified);
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL and CREATEDB; check.bat runs this suite"]
async fn sessions_and_email_tokens_store_only_fixed_length_hashes(pool: PgPool) {
    user(&pool, USER, "PlayerOne", "one@example.test").await;
    sqlx::query("INSERT INTO sessions(id_hash, kind, user_id, expires_at) SELECT decode(repeat('ab',32),'hex'), 'web', id, now() + interval '1 day' FROM users").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO email_tokens(token_hash, kind, user_id, expires_at) SELECT decode(repeat('cd',32),'hex'), 'verify', id, now() + interval '1 hour' FROM users").execute(&pool).await.unwrap();
    for sql in [
        "UPDATE sessions SET id_hash = decode('ab','hex')",
        "UPDATE sessions SET kind = 'cookie'",
        "UPDATE sessions SET expires_at = created_at",
        "UPDATE sessions SET last_seen_at = created_at - interval '1 second'",
        "UPDATE sessions SET rotated_at = created_at - interval '1 second'",
        "UPDATE sessions SET ip_hash = decode('ab','hex')",
        "UPDATE email_tokens SET token_hash = decode('ab','hex')",
        "UPDATE email_tokens SET kind = 'login'",
        "UPDATE email_tokens SET expires_at = created_at",
        "UPDATE email_tokens SET used_at = created_at - interval '1 second'",
    ] {
        rejects(&pool, sql, "23514").await;
    }
    sqlx::query("DELETE FROM users")
        .execute(&pool)
        .await
        .unwrap();
    let remaining: i64 = sqlx::query_scalar(
        "SELECT (SELECT count(*) FROM sessions) + (SELECT count(*) FROM email_tokens)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(remaining, 0);
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL and CREATEDB; check.bat runs this suite"]
async fn r_pul_01_slots_active_avatar_and_unlocked_ownership_are_protected(pool: PgPool) {
    basic(&pool).await;
    user(&pool, OTHER, "OtherPlayer", "other@example.test").await;
    rejects(&pool, "INSERT INTO avatars(id,user_id,slot,template_id) SELECT '0199e800-0000-7000-8000-000000000011',user_id,0,1 FROM avatars", "23505").await;
    rejects(&pool, "INSERT INTO avatars(id,user_id,slot,template_id,is_active) SELECT '0199e800-0000-7000-8000-000000000011',user_id,1,1,true FROM avatars", "23505").await;
    rejects(&pool, "INSERT INTO avatars(id,user_id,slot,template_id) VALUES ('0199e800-0000-7000-8000-000000000011','0199e800-0000-7000-8000-000000000002',0,1)", "23503").await;
    for sql in [
        "UPDATE avatars SET slot = 10",
        "UPDATE avatars SET slot = -1",
        "UPDATE avatars SET level = 0",
        "UPDATE avatars SET level = 21",
        "UPDATE avatars SET xp = -1",
        "UPDATE avatars SET template_id = 0",
        "UPDATE avatars SET tier = 4",
        "UPDATE avatars SET dp_hp = -1",
        "UPDATE avatars SET dev_points_unspent = -1",
        "UPDATE avatars SET is_shade = true",
        "UPDATE avatars SET shade_school = 'combat'",
        "UPDATE avatars SET path_id = 1",
        "UPDATE avatars SET specialization = 'combat'",
        "UPDATE avatars SET gift_id = 1",
        "UPDATE avatars SET deleted_at = now()",
    ] {
        rejects(&pool, sql, "23514").await;
    }
    sqlx::query("UPDATE avatars SET is_active = false, deleted_at = now()")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO avatars(id,user_id,slot,template_id,is_active) VALUES ('0199e800-0000-7000-8000-000000000011',$1::text::uuid,0,1,true)").bind(USER).execute(&pool).await.unwrap();
    let counts: (i64, i64) =
        sqlx::query_as("SELECT count(*),count(*) FILTER (WHERE deleted_at IS NULL) FROM avatars")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(counts, (2, 1));
    // R-CIE-01/05: shades are created by sacrifice, not by unlocking a reusable template.
    sqlx::query("INSERT INTO avatars(id,user_id,slot,template_id,is_shade,shade_school) VALUES ('0199e800-0000-7000-8000-000000000012',$1::text::uuid,0,1,true,'arcane')").bind(OTHER).execute(&pool).await.unwrap();
    let owned: Option<i32> =
        sqlx::query_scalar("SELECT owned_template_id FROM avatars WHERE user_id = $1::text::uuid")
            .bind(OTHER)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(owned.is_none());
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL and CREATEDB; check.bat runs this suite"]
async fn concurrent_creates_cannot_claim_the_same_slot(pool: PgPool) {
    basic(&pool).await;
    let pool_ref = &pool;
    let insert = |id: &'static str| async move {
        sqlx::query("INSERT INTO avatars(id,user_id,slot,template_id) VALUES ($1::text::uuid,$2::text::uuid,9,1)").bind(id).bind(USER).execute(pool_ref).await
    };
    let (a, b) = tokio::join!(
        insert("0199e800-0000-7000-8000-000000000011"),
        insert("0199e800-0000-7000-8000-000000000012")
    );
    assert_ne!(a.is_ok(), b.is_ok());
    let error = a.err().or(b.err()).unwrap();
    assert_eq!(
        error.as_database_error().and_then(|e| e.code()).as_deref(),
        Some("23505")
    );
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL and CREATEDB; check.bat runs this suite"]
async fn r_tal_01_collections_and_deck_arrays_reject_invalid_storage(pool: PgPool) {
    basic(&pool).await;
    sqlx::query("INSERT INTO collection(user_id,card_id,qty) VALUES ($1::text::uuid,1,4)")
        .bind(USER)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO decks(avatar_id) VALUES ($1::text::uuid)")
        .bind(AVATAR)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO deck_presets(id,user_id,name) VALUES ('0199e800-0000-7000-8000-000000000020',$1::text::uuid,'Starter')").bind(USER).execute(&pool).await.unwrap();
    for sql in [
        "UPDATE collection SET qty = -1",
        "UPDATE collection SET card_id = 0",
        "UPDATE decks SET cards = array_fill(1::smallint,ARRAY[21])",
        "UPDATE decks SET cards = ARRAY[1,NULL]::smallint[]",
        "UPDATE decks SET cards = ARRAY[0]::smallint[]",
        "UPDATE decks SET cards = '[0:1]={1,2}'::smallint[]",
        "UPDATE decks SET cards = '{{1,2},{3,4}}'::smallint[]",
        "UPDATE deck_presets SET cards = array_fill(1::smallint,ARRAY[21])",
        "UPDATE deck_presets SET cards = ARRAY[1,NULL]::smallint[]",
        "UPDATE deck_presets SET cards = ARRAY[-1]::smallint[]",
        "UPDATE deck_presets SET cards = '{{1,2},{3,4}}'::smallint[]",
        "UPDATE deck_presets SET name = '   '",
    ] {
        rejects(&pool, sql, "23514").await;
    }
    sqlx::query("UPDATE decks SET cards = array_fill(1::smallint, ARRAY[20])")
        .execute(&pool)
        .await
        .unwrap();
    rejects(
        &pool,
        "INSERT INTO collection SELECT * FROM collection",
        "23505",
    )
    .await;
    sqlx::query("DELETE FROM avatars WHERE id = $1::text::uuid")
        .bind(AVATAR)
        .execute(&pool)
        .await
        .unwrap();
    let collection: i32 = sqlx::query_scalar(
        "SELECT qty FROM collection WHERE user_id = $1::text::uuid AND card_id=1",
    )
    .bind(USER)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(collection, 4); // R-TAL-07: removing an avatar never removes the collection.
    let decks: i64 = sqlx::query_scalar("SELECT count(*) FROM decks")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(decks, 0);
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL and CREATEDB; check.bat runs this suite"]
async fn r_rnk_01_05_rating_defaults_and_result_totals_are_consistent(pool: PgPool) {
    user(&pool, USER, "PlayerOne", "one@example.test").await;
    sqlx::query("INSERT INTO ratings(user_id) VALUES ($1::text::uuid)")
        .bind(USER)
        .execute(&pool)
        .await
        .unwrap();
    let initial: (i32, i32) = sqlx::query_as("SELECT rating,games FROM ratings")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(initial, (1000, 0));
    for sql in [
        "UPDATE ratings SET rating=0",
        "UPDATE ratings SET games=-1",
        "UPDATE ratings SET wins=1",
        "UPDATE ratings SET disconnects=-1",
    ] {
        rejects(&pool, sql, "23514").await;
    }
    sqlx::query("UPDATE ratings SET rating=1,games=3,wins=1,draws=1,losses=1")
        .execute(&pool)
        .await
        .unwrap();
    rejects(&pool, "INSERT INTO ratings SELECT * FROM ratings", "23505").await;
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL and CREATEDB; check.bat runs this suite"]
async fn modified_applied_migration_is_rejected(pool: PgPool) {
    sqlx::query(
        "UPDATE _sqlx_migrations SET checksum=decode('00','hex') WHERE version=202610150001",
    )
    .execute(&pool)
    .await
    .unwrap();
    let error = db::migrate(&pool).await.unwrap_err();
    assert!(error.to_string().contains("migration failed"));
    assert!(format!("{error:#}").contains("modified"));
}

async fn permission_denied(connection: &mut PgConnection, sql: &'static str) {
    let mut savepoint = connection.begin().await.unwrap();
    let error = sqlx::query(sql).execute(&mut *savepoint).await.unwrap_err();
    assert_eq!(
        error.as_database_error().and_then(|e| e.code()).as_deref(),
        Some("42501"),
        "{error}"
    );
    savepoint.rollback().await.unwrap();
}

#[sqlx::test(migrations = false)]
#[ignore = "requires PostgreSQL superuser and CREATEDB; check.bat runs this suite"]
async fn runtime_and_readonly_roles_cannot_change_schema_or_migration_history(pool: PgPool) {
    // Role creation and all grants are rolled back, leaving no cluster-wide test roles behind.
    let mut tx = pool.begin().await.unwrap();
    sqlx::raw_sql(include_str!("../../../deploy/postgres/roles.sql"))
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("SET LOCAL ROLE game_migrator")
        .execute(&mut *tx)
        .await
        .unwrap();
    db::MIGRATOR.run(&mut *tx).await.unwrap();
    sqlx::raw_sql(include_str!("../../../deploy/postgres/privileges.sql"))
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("SET LOCAL ROLE game_app")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("INSERT INTO users(id,login,email,password_hash,start_school) VALUES ('0199e800-0000-7000-8000-000000000001','PlayerOne','one@example.test','$argon2id$v=19$m=19456,t=2,p=1$fixture$fixture','combat')").execute(&mut *tx).await.unwrap();
    sqlx::query("SELECT version FROM _sqlx_migrations")
        .fetch_all(&mut *tx)
        .await
        .unwrap();
    permission_denied(&mut tx, "CREATE TABLE forbidden(id integer)").await;
    permission_denied(&mut tx, "ALTER TABLE users ADD COLUMN forbidden integer").await;
    permission_denied(&mut tx, "DELETE FROM _sqlx_migrations").await;
    sqlx::query("SET LOCAL ROLE game_readonly")
        .execute(&mut *tx)
        .await
        .unwrap();
    let users: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(users, 1);
    permission_denied(&mut tx, "DELETE FROM users").await;
    permission_denied(&mut tx, "CREATE TABLE forbidden(id integer)").await;
    tx.rollback().await.unwrap();
}
