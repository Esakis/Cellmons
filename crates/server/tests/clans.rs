//! R-KLA integration and concurrency checks, always on disposable databases.
use server::clans::{self, Mutation as M, Query, Role};
use sqlx::PgPool;
use uuid::Uuid;

async fn user(db: &PgPool, index: u128) -> Uuid {
    let id = Uuid::from_u128(index + 1000);
    let mut tx = db.begin().await.unwrap();
    sqlx::query("INSERT INTO users(id,login,email,password_hash,start_school) VALUES ($1,$2::citext,$3::citext,'$argon2id$v=19$m=19456,t=2,p=1$fixture$fixture','combat')")
        .bind(id).bind(format!("ClanUser{index}")).bind(format!("clan{index}@example.test")).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO ratings(user_id) VALUES ($1)")
        .bind(id)
        .execute(&mut *tx)
        .await
        .unwrap();
    server::economy::grant_initial(&mut tx, id).await.unwrap();
    tx.commit().await.unwrap();
    id
}
async fn create(db: &PgPool, owner: Uuid, name: &str) -> Uuid {
    let id = Uuid::now_v7();
    clans::mutate(
        db,
        owner,
        M::Create {
            id,
            name: name.into(),
        },
    )
    .await
    .unwrap();
    id
}
async fn view(db: &PgPool, user: Uuid, clan: Option<Uuid>) -> clans::Snapshot {
    clans::load(
        &mut db.acquire().await.unwrap(),
        user,
        &Query {
            clan,
            ..Default::default()
        },
    )
    .await
    .unwrap()
}
async fn apply(db: &PgPool, user: Uuid, clan: Uuid) {
    clans::mutate(db, user, M::Apply { clan }).await.unwrap();
}
async fn accept(db: &PgPool, owner: Uuid, user: Uuid, clan: Uuid, revision: i32) {
    clans::mutate(
        db,
        owner,
        M::Accept {
            clan,
            user,
            revision,
        },
    )
    .await
    .unwrap();
}
fn error(result: server::online::Result<clans::Change>, expected: &str) {
    match result {
        Ok(_) => panic!("expected {expected}"),
        Err(error) => assert_eq!(error.0, expected),
    }
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn clan_lifecycle_forms_at_five_and_preserves_one_master(db: PgPool) {
    let mut users = Vec::new();
    for i in 0..5 {
        users.push(user(&db, i).await);
    }
    let clan = create(&db, users[0], "Zakon Światła").await;
    let first = view(&db, users[0], None).await.profile.unwrap();
    assert_eq!(
        (
            first.summary.status.as_str(),
            first.summary.members,
            first.summary.rank
        ),
        ("founding", 1, None)
    );
    for i in 1..5 {
        apply(&db, users[i], clan).await;
        accept(&db, users[0], users[i], clan, i as i32 - 1).await;
    }
    let full = view(&db, users[0], None).await.profile.unwrap();
    assert_eq!(
        (
            full.summary.status.as_str(),
            full.summary.revision,
            full.summary.members,
            full.summary.rank
        ),
        ("active", 4, 5, Some(1))
    );
    assert_eq!(full.summary.score, 1000);
    assert_eq!(full.ducats, "200");
    assert!(full.founded_ms.is_some());
    error(
        clans::mutate(&db, users[0], M::Leave { clan, revision: 4 }).await,
        "MASTER_REQUIRED",
    );
    clans::mutate(
        &db,
        users[0],
        M::Transfer {
            clan,
            user: users[1],
            revision: 4,
        },
    )
    .await
    .unwrap();
    let roles:Vec<(Uuid,String)>=sqlx::query_as("SELECT user_id,role FROM clan_members WHERE clan_id=$1 AND role IN ('master','vice') ORDER BY role").bind(clan).fetch_all(&db).await.unwrap();
    assert_eq!(
        roles,
        vec![(users[1], "master".into()), (users[0], "vice".into())]
    );
    clans::mutate(
        &db,
        users[1],
        M::Kick {
            clan,
            user: users[0],
            revision: 5,
        },
    )
    .await
    .unwrap();
    // Founding is a one-time milestone; leaving does not make an established clan unformed.
    assert_eq!(
        view(&db, users[1], None)
            .await
            .profile
            .unwrap()
            .summary
            .status,
        "active"
    );
    clans::mutate(&db, users[1], M::Disband { clan, revision: 6 })
        .await
        .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM clan_members WHERE clan_id=$1")
        .bind(clan)
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(count, 0);
    assert_eq!(view(&db, users[1], None).await.own, None);
    error(
        clans::mutate(
            &db,
            users[0],
            M::Create {
                id: Uuid::now_v7(),
                name: "zakon światła".into(),
            },
        )
        .await,
        "NAME_TAKEN",
    );
    error(
        clans::mutate(&db, users[0], M::Apply { clan }).await,
        "NOT_FOUND",
    );
    let audit_error = sqlx::query("UPDATE clan_audit SET details='{}' WHERE clan_id=$1")
        .bind(clan)
        .execute(&db)
        .await
        .unwrap_err();
    assert_eq!(
        audit_error.as_database_error().unwrap().code().as_deref(),
        Some("23514")
    );
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn clan_role_permissions_motd_privacy_and_contacts_are_enforced(db: PgPool) {
    let mut users = Vec::new();
    for i in 0..6 {
        users.push(user(&db, i).await);
    }
    let clan = create(&db, users[0], "Strażnicy Arkany").await;
    for i in 1..5 {
        apply(&db, users[i], clan).await;
        accept(&db, users[0], users[i], clan, i as i32 - 1).await;
    }
    clans::mutate(
        &db,
        users[0],
        M::Role {
            clan,
            user: users[1],
            role: Role::Vice,
            revision: 4,
        },
    )
    .await
    .unwrap();
    clans::mutate(
        &db,
        users[0],
        M::Role {
            clan,
            user: users[2],
            role: Role::Scribe,
            revision: 5,
        },
    )
    .await
    .unwrap();
    let edit = |revision, description: &str, motd: &str, www: &str| M::Edit {
        clan,
        revision,
        description: description.into(),
        motd: motd.into(),
        www: www.into(),
        email: "public@example.test".into(),
    };
    clans::mutate(
        &db,
        users[0],
        edit(
            6,
            "Publiczny opis",
            "Prywatne hasło zebrania",
            "https://example.test/clan",
        ),
    )
    .await
    .unwrap();
    apply(&db, users[5], clan).await;
    let outside = view(&db, users[5], Some(clan)).await.profile.unwrap();
    assert_eq!(outside.description, "Publiczny opis");
    assert_eq!(outside.motd, "");
    assert_eq!(outside.email, "public@example.test");
    assert!(outside.applicants.is_empty());
    assert_eq!(
        view(&db, users[1], None)
            .await
            .profile
            .unwrap()
            .applicants
            .len(),
        1
    );
    assert!(
        view(&db, users[2], None)
            .await
            .profile
            .unwrap()
            .applicants
            .is_empty()
    );
    clans::mutate(
        &db,
        users[2],
        edit(
            7,
            "Opis skryby",
            "Nowa wiadomość",
            "https://example.test/clan",
        ),
    )
    .await
    .unwrap();
    error(
        clans::mutate(
            &db,
            users[2],
            edit(8, "Nie wolno", "", "https://other.test"),
        )
        .await,
        "FORBIDDEN",
    );
    error(
        clans::mutate(
            &db,
            users[3],
            edit(8, "Nie wolno", "", "https://example.test/clan"),
        )
        .await,
        "FORBIDDEN",
    );
    error(
        clans::mutate(
            &db,
            users[1],
            M::Role {
                clan,
                user: users[0],
                role: Role::Member,
                revision: 8,
            },
        )
        .await,
        "FORBIDDEN",
    );
    error(
        clans::mutate(
            &db,
            users[1],
            M::Title {
                clan,
                user: users[0],
                title: "Tytuł".into(),
                revision: 8,
            },
        )
        .await,
        "FORBIDDEN",
    );
    error(
        clans::mutate(
            &db,
            users[1],
            M::Kick {
                clan,
                user: users[0],
                revision: 8,
            },
        )
        .await,
        "FORBIDDEN",
    );
    error(
        clans::mutate(
            &db,
            users[1],
            M::Transfer {
                clan,
                user: users[3],
                revision: 8,
            },
        )
        .await,
        "FORBIDDEN",
    );
    error(
        clans::mutate(&db, users[1], M::Disband { clan, revision: 8 }).await,
        "FORBIDDEN",
    );
    error(
        clans::mutate(&db, users[0], edit(8, "", "", "javascript:alert(1)")).await,
        "INVALID_MESSAGE",
    );
    error(
        clans::mutate(
            &db,
            users[0],
            edit(8, "", "", "https://name:password@example.test"),
        )
        .await,
        "INVALID_MESSAGE",
    );
    error(
        clans::mutate(
            &db,
            users[0],
            M::Title {
                clan,
                user: users[3],
                title: "zły\ntytuł".into(),
                revision: 8,
            },
        )
        .await,
        "INVALID_MESSAGE",
    );
    assert_eq!(
        view(&db, users[0], None)
            .await
            .profile
            .unwrap()
            .summary
            .revision,
        8
    );
    clans::mutate(
        &db,
        users[1],
        M::Role {
            clan,
            user: users[3],
            role: Role::Scribe,
            revision: 8,
        },
    )
    .await
    .unwrap();
    sqlx::query("UPDATE users SET status='banned' WHERE id=$1")
        .bind(users[4])
        .execute(&db)
        .await
        .unwrap();
    error(
        clans::mutate(&db, users[4], M::Leave { clan, revision: 9 }).await,
        "UNAUTHENTICATED",
    );
    clans::mutate(
        &db,
        users[1],
        M::Kick {
            clan,
            user: users[4],
            revision: 9,
        },
    )
    .await
    .unwrap();
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn concurrent_acceptance_and_master_transfer_are_atomic(db: PgPool) {
    let a = user(&db, 1).await;
    let b = user(&db, 2).await;
    let target = user(&db, 3).await;
    let ca = create(&db, a, "First Clan").await;
    let cb = create(&db, b, "Second Clan").await;
    apply(&db, target, ca).await;
    apply(&db, target, cb).await;
    let mut tasks = tokio::task::JoinSet::new();
    for n in 0..100 {
        let db = db.clone();
        let (clan, actor) = if n % 2 == 0 { (ca, a) } else { (cb, b) };
        tasks.spawn(async move {
            clans::mutate(
                &db,
                actor,
                M::Accept {
                    clan,
                    user: target,
                    revision: 0,
                },
            )
            .await
            .is_ok()
        });
    }
    let mut successes = 0;
    while let Some(row) =
        tokio::time::timeout(std::time::Duration::from_secs(10), tasks.join_next())
            .await
            .unwrap()
    {
        successes += u32::from(row.unwrap());
    }
    assert_eq!(successes, 1);
    let winner: Uuid = sqlx::query_scalar("SELECT clan_id FROM clan_members WHERE user_id=$1")
        .bind(target)
        .fetch_one(&db)
        .await
        .unwrap();
    let pending: i64 =
        sqlx::query_scalar("SELECT count(*) FROM clan_applications WHERE user_id=$1")
            .bind(target)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(pending, 0);
    let owner = if winner == ca { a } else { b };
    for _ in 0..100 {
        let db = db.clone();
        tasks.spawn(async move {
            clans::mutate(
                &db,
                owner,
                M::Transfer {
                    clan: winner,
                    user: target,
                    revision: 1,
                },
            )
            .await
            .is_ok()
        });
    }
    let mut transfers = 0;
    while let Some(row) = tasks.join_next().await {
        transfers += u32::from(row.unwrap());
    }
    assert_eq!(transfers, 1);
    let masters: Vec<Uuid> =
        sqlx::query_scalar("SELECT user_id FROM clan_members WHERE clan_id=$1 AND role='master'")
            .bind(winner)
            .fetch_all(&db)
            .await
            .unwrap();
    assert_eq!(masters, vec![target]);
    let accepts: i64 = sqlx::query_scalar("SELECT count(*) FROM clan_audit WHERE action='accept'")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(accepts, 1);
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn clan_creation_is_idempotent_and_application_limits_serialize(db: PgPool) {
    let actor = user(&db, 0).await;
    let clan = Uuid::now_v7();
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..100 {
        let db = db.clone();
        tasks.spawn(async move {
            clans::mutate(
                &db,
                actor,
                M::Create {
                    id: clan,
                    name: "Stable Name".into(),
                },
            )
            .await
            .is_ok()
        });
    }
    while let Some(result) = tasks.join_next().await {
        assert!(result.unwrap());
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM clan_audit WHERE action='create'")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(count, 1);
    error(
        clans::mutate(
            &db,
            actor,
            M::Create {
                id: Uuid::now_v7(),
                name: "Another Name".into(),
            },
        )
        .await,
        "ALREADY_MEMBER",
    );
    let applicant = user(&db, 90).await;
    let mut clans = Vec::new();
    for i in 1..=11 {
        let owner = user(&db, i).await;
        clans.push(create(&db, owner, &format!("Clan {i}")).await);
    }
    for clan in clans.iter().take(10) {
        apply(&db, applicant, *clan).await;
    }
    // A lost response can be retried even while the account is at its application limit.
    apply(&db, applicant, clans[0]).await;
    error(
        clans::mutate(&db, applicant, M::Apply { clan: clans[10] }).await,
        "APPLICATION_LIMIT",
    );
    clans::mutate(&db, applicant, M::Withdraw { clan: clans[0] })
        .await
        .unwrap();
    apply(&db, applicant, clans[10]).await;
    assert_eq!(view(&db, applicant, None).await.applications.len(), 10);
}

#[sqlx::test(migrator = "server::db::MIGRATOR")]
#[ignore = "requires PostgreSQL; check.bat runs this suite"]
async fn clan_capacity_and_global_top_ten_ranking_are_checked(db: PgPool) {
    let owner = user(&db, 0).await;
    let clan = create(&db, owner, "Alpha Guild").await;
    let mut ids = Vec::new();
    for i in 1..=11 {
        let member = user(&db, i).await;
        ids.push(member);
        sqlx::query("INSERT INTO clan_members(clan_id,user_id) VALUES ($1,$2)")
            .bind(clan)
            .bind(member)
            .execute(&db)
            .await
            .unwrap();
    }
    sqlx::query("UPDATE clans SET status='active',founded_at=now() WHERE id=$1")
        .bind(clan)
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("UPDATE ratings SET rating=1009 WHERE user_id=$1")
        .bind(owner)
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("UPDATE ratings SET rating=1 WHERE user_id=ANY($1)")
        .bind(&ids[9..])
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(
        view(&db, owner, None).await.profile.unwrap().summary.score,
        1000
    );
    let rival = user(&db, 50).await;
    let second = create(&db, rival, "Beta Guild").await;
    sqlx::query("UPDATE clans SET status='active',founded_at=now() WHERE id=$1")
        .bind(second)
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("UPDATE ratings SET rating=1100 WHERE user_id=$1")
        .bind(rival)
        .execute(&db)
        .await
        .unwrap();
    let snapshot = clans::load(
        &mut db.acquire().await.unwrap(),
        owner,
        &Query {
            search: "Alpha".into(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(snapshot.clans.len(), 1);
    assert_eq!(snapshot.clans[0].rank, Some(2));
    let literal = clans::load(
        &mut db.acquire().await.unwrap(),
        owner,
        &Query {
            search: "_".into(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert!(literal.clans.is_empty());
    // The storage fixture fills the limit without 256 expensive account registrations.
    let more: Vec<_> = (2000..2244).map(Uuid::from_u128).collect();
    sqlx::query("INSERT INTO users(id,login,email,password_hash,start_school) SELECT id,('Bulk'||n)::citext,('bulk'||n||'@example.test')::citext,'$argon2id$v=19$m=19456,t=2,p=1$fixture$fixture','combat' FROM unnest($1::uuid[]) WITH ORDINALITY AS x(id,n)")
        .bind(&more).execute(&db).await.unwrap();
    sqlx::query(
        "INSERT INTO clan_members(clan_id,user_id) SELECT $1,id FROM unnest($2::uuid[]) AS id",
    )
    .bind(clan)
    .bind(&more)
    .execute(&db)
    .await
    .unwrap();
    let applicant = user(&db, 9000).await;
    apply(&db, applicant, clan).await;
    error(
        clans::mutate(
            &db,
            owner,
            M::Accept {
                clan,
                user: applicant,
                revision: 0,
            },
        )
        .await,
        "CLAN_FULL",
    );
    assert!(view(&db, applicant, None).await.own.is_none());
    assert_eq!(
        view(&db, owner, None)
            .await
            .profile
            .unwrap()
            .summary
            .members,
        256
    );
}
