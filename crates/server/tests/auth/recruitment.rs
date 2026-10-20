use super::*;
use uuid::Uuid;

fn recruit(level: u8, template: Option<u16>) -> server::game::Mutation {
    server::game::Mutation::Recruit {
        level,
        template_id: template,
    }
}
async fn progress(pool: &PgPool, user: Uuid, xp: i64) {
    sqlx::query("UPDATE users SET account_xp=$2,tutorial_step=3 WHERE id=$1")
        .bind(user)
        .bind(xp)
        .execute(pool)
        .await
        .unwrap();
}
async fn owner(pool: &PgPool) -> Uuid {
    sqlx::query_scalar("SELECT id FROM users")
        .fetch_one(pool)
        .await
        .unwrap()
}
fn quantities(state: &server::game::Snapshot) -> Vec<(i16, i32)> {
    state
        .collection
        .iter()
        .map(|s| (s.card_id, s.qty))
        .collect()
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL"]
async fn recruitment_http_gates_levels_and_retries_never_recreate_deleted_rewards(pool: PgPool) {
    let f = Fixture::new(pool.clone());
    f.register().await;
    let cookie = cookie(&f.login(false).await);
    let headers = [("cookie", cookie.as_str())];
    let user = owner(&pool).await;
    let body = json!({"action":"recruit","level":5,"template_id":2});
    assert_eq!(
        f.call("POST", "/api/game", body.clone(), &headers)
            .await
            .body["error"],
        "LEVEL_REQUIRED"
    );
    progress(&pool, user, 699).await;
    assert_eq!(
        f.call("POST", "/api/game", body.clone(), &headers)
            .await
            .body["error"],
        "LEVEL_REQUIRED"
    );
    progress(&pool, user, 700).await;
    for (value, error) in [
        (
            json!({"action":"recruit","level":6,"template_id":2}),
            "INVALID_MILESTONE",
        ),
        (
            json!({"action":"recruit","level":8,"template_id":2}),
            "LEVEL_REQUIRED",
        ),
        (
            json!({"action":"recruit","level":5,"template_id":1}),
            "RECRUIT_UNAVAILABLE",
        ),
        (
            json!({"action":"recruit","level":5,"template_id":6}),
            "RECRUIT_UNAVAILABLE",
        ),
        (
            json!({"action":"recruit","level":5,"template_id":null}),
            "RECRUIT_CHOICE_REQUIRED",
        ),
    ] {
        assert_eq!(
            f.call("POST", "/api/game", value, &headers).await.body["error"],
            error
        );
    }
    let state = f.call("POST", "/api/game", body.clone(), &headers).await;
    assert_eq!(state.status, StatusCode::OK, "{}", state.body);
    assert_eq!(state.body["templates"], json!([1, 2]));
    let created = &state.body["avatars"][1];
    assert_eq!(created["level"], 1);
    assert_eq!(created["active"], false);
    assert_eq!(created["deck"].as_array().unwrap().len(), 20);
    let id = created["id"].as_str().unwrap();
    let cards = state.body["collection"].clone();
    assert_eq!(
        f.call("POST", "/api/game", body.clone(), &headers)
            .await
            .body,
        state.body
    );
    let conflict = f
        .call(
            "POST",
            "/api/game",
            json!({"action":"recruit","level":5,"template_id":4}),
            &headers,
        )
        .await;
    assert_eq!(conflict.body["error"], "CONFLICT");
    f.call(
        "POST",
        "/api/game",
        json!({"action":"delete","id":id,"revision":0}),
        &headers,
    )
    .await;
    let retried = f.call("POST", "/api/game", body, &headers).await;
    assert_eq!(retried.body["avatars"].as_array().unwrap().len(), 1);
    assert_eq!(retried.body["collection"], cards);
    assert_eq!(
        retried.body["recruitment"]["milestones"][0]["claimed"],
        true
    );
    assert_eq!(
        f.call("GET", "/api/journey", Value::Null, &headers)
            .await
            .body["recruits"],
        0
    );
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL"]
async fn recruitment_serializes_concurrent_choices_and_rolls_back_when_slots_are_full(
    pool: PgPool,
) {
    let f = Fixture::new(pool.clone());
    f.register().await;
    let user = owner(&pool).await;
    progress(&pool, user, 42000).await;
    let a = server::game::mutate(&pool, user, recruit(5, Some(2)));
    let b = server::game::mutate(&pool, user, recruit(5, Some(4)));
    let (a, b) = tokio::join!(a, b);
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    let state = a.or(b).unwrap();
    let selected = state.recruitment.milestones[0].template_id.unwrap() as u16;
    let before = quantities(&state);
    let mut jobs = Vec::new();
    for _ in 0..12 {
        let pool = pool.clone();
        jobs.push(tokio::spawn(async move {
            server::game::mutate(&pool, user, recruit(5, Some(selected))).await
        }));
    }
    for job in jobs {
        let result = job.await.unwrap().unwrap();
        assert_eq!(result.avatars.len(), 2);
        assert_eq!(quantities(&result), before);
    }
    for _ in 0..8 {
        server::game::mutate(
            &pool,
            user,
            server::game::Mutation::Create {
                id: Uuid::now_v7(),
                template_id: 1,
            },
        )
        .await
        .unwrap();
    }
    let missing = if selected == 2 { 4 } else { 2 };
    let denied = server::game::mutate(&pool, user, recruit(8, Some(missing))).await;
    assert!(matches!(denied, Err(e) if e.0 == "SLOTS_FULL"));
    let state = server::game::load(&mut pool.acquire().await.unwrap(), user)
        .await
        .unwrap();
    assert_eq!(state.templates.len(), 2);
    assert_eq!(
        state
            .recruitment
            .milestones
            .iter()
            .filter(|m| m.claimed)
            .count(),
        1
    );
    assert_eq!(quantities(&state), before);
    let spare = &state.avatars[9];
    server::game::mutate(
        &pool,
        user,
        server::game::Mutation::Delete {
            id: spare.id,
            revision: spare.revision,
        },
    )
    .await
    .unwrap();
    let state = server::game::mutate(&pool, user, recruit(8, Some(missing)))
        .await
        .unwrap();
    assert_eq!(state.templates.len(), 3);
    assert!(state.recruitment.milestones[1].claimed);
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL"]
async fn recruitment_uses_missing_released_classes_and_seals_when_collection_is_complete(
    pool: PgPool,
) {
    let f = Fixture::new(pool.clone());
    f.register().await;
    let user = owner(&pool).await;
    progress(&pool, user, 42000).await;
    // A random arena unlock reduces the missing pool without consuming a level reward.
    sqlx::query("INSERT INTO unlocked_templates(user_id,template_id,source) VALUES($1,32,'arena')")
        .bind(user)
        .execute(&pool)
        .await
        .unwrap();
    let mut state = server::game::load(&mut pool.acquire().await.unwrap(), user)
        .await
        .unwrap();
    for level in [5, 8, 12, 16, 20, 24] {
        assert!(!state.recruitment.available.contains(&32));
        assert!(
            state
                .recruitment
                .available
                .iter()
                .all(|id| engine_content::is_playable(*id))
        );
        let template = state.recruitment.available[0];
        state = server::game::mutate(&pool, user, recruit(level, Some(template)))
            .await
            .unwrap();
        let receipt = state
            .recruitment
            .milestones
            .iter()
            .find(|m| m.level == level)
            .unwrap();
        let avatar = state
            .avatars
            .iter()
            .find(|a| Some(a.id) == receipt.avatar_id)
            .unwrap();
        assert_eq!(avatar.deck.len(), 20);
        assert_eq!(avatar.template_id, i32::from(template));
    }
    assert_eq!(state.templates.len(), 8);
    assert_eq!(state.avatars.len(), 7);
    assert!(state.recruitment.available.is_empty());
    let before = quantities(&state);
    for _ in 0..2 {
        state = server::game::mutate(&pool, user, recruit(28, None))
            .await
            .unwrap();
        let seal = &state.recruitment.milestones[6];
        assert!(seal.claimed && seal.template_id.is_none() && seal.avatar_id.is_none());
        assert_eq!(quantities(&state), before);
        assert_eq!(state.avatars.len(), 7);
    }
    assert_eq!(
        server::recruitment::pending(&mut pool.acquire().await.unwrap(), user, 30)
            .await
            .unwrap(),
        0
    );
}
