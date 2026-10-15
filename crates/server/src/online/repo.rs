use super::{Error, Result};
use protocol::game::v1 as pb;
use sqlx::{PgConnection, PgPool};
use std::collections::HashSet;
use uuid::Uuid;

pub async fn session_valid(db: &PgPool, user: Uuid, hash: &[u8]) -> Result<bool> {
    Ok(sqlx::query_scalar!("SELECT EXISTS(SELECT 1 FROM sessions s JOIN users u ON u.id=s.user_id WHERE s.id_hash=$1 AND s.user_id=$2 AND s.expires_at>now() AND u.status='active') AS \"valid!\"", hash,user).fetch_one(db).await?)
}

pub struct Loaded {
    pub player: pb::PublicPlayer,
    pub friends: Vec<pb::SocialEntry>,
    pub ignores: Vec<pb::SocialEntry>,
}
pub async fn load(db: &PgPool, user: Uuid) -> Result<Loaded> {
    let player = player(db, user).await?;
    let social = social(db, user).await?;
    Ok(Loaded {
        player,
        friends: social.friends,
        ignores: social.ignores,
    })
}
pub async fn target(db: &PgPool, login: &str) -> Result<Uuid> {
    if login.len() > 20 {
        return Err(Error("NOT_FOUND"));
    }
    sqlx::query_scalar!(
        "SELECT id FROM users WHERE login=$1::citext AND status='active'",
        login
    )
    .fetch_optional(db)
    .await?
    .ok_or(Error("NOT_FOUND"))
}
pub async fn clan(db: &PgPool, user: Uuid) -> Result<Option<Uuid>> {
    Ok(sqlx::query_scalar!("SELECT c.id FROM clan_members m JOIN clans c ON c.id=m.clan_id WHERE m.user_id=$1 AND c.status='active'",user).fetch_optional(db).await?)
}
pub async fn clan_members(db: &PgPool, clan: Uuid) -> Result<HashSet<String>> {
    Ok(sqlx::query_scalar!("SELECT m.user_id FROM clan_members m JOIN clans c ON c.id=m.clan_id JOIN users u ON u.id=m.user_id WHERE m.clan_id=$1 AND c.status='active' AND u.status='active'",clan).fetch_all(db).await?.into_iter().map(|id|id.to_string()).collect())
}
pub async fn player(db: &PgPool, user: Uuid) -> Result<pb::PublicPlayer> {
    let u=sqlx::query!("SELECT (EXISTS(SELECT 1 FROM duel_locks l WHERE l.user_id=u.id) OR EXISTS(SELECT 1 FROM activity_locks l WHERE l.user_id=u.id)) AS \"busy!\",u.id,u.login::text AS \"login!\",u.description,u.challenge_filter,u.email_verified_at IS NOT NULL AS \"verified!\",COALESCE(r.rating,1000) AS \"rating!\",c.id AS clan_id,c.name::text AS clan_name,m.title FROM users u LEFT JOIN ratings r ON r.user_id=u.id LEFT JOIN clan_members m ON m.user_id=u.id LEFT JOIN clans c ON c.id=m.clan_id AND c.status='active' WHERE u.id=$1 AND u.status='active'",user).fetch_optional(db).await?.ok_or(Error("UNAUTHENTICATED"))?;
    let a=sqlx::query!("SELECT id,template_id,path_id,level,dp_phys_atk,dp_phys_def,dp_mag_atk,dp_mag_def,dp_hp,dp_stamina FROM avatars WHERE user_id=$1 AND is_active AND deleted_at IS NULL",user).fetch_optional(db).await?;
    let rank = if u.verified {
        sqlx::query_scalar!("SELECT count(*)+1 AS \"rank!\" FROM ratings r JOIN users u ON u.id=r.user_id WHERE u.status='active' AND u.email_verified_at IS NOT NULL AND (r.rating>$1 OR (r.rating=$1 AND r.user_id<$2))",u.rating,user).fetch_one(db).await?
    } else {
        0
    };
    let mut p = pb::PublicPlayer {
        busy: u.busy,
        id: user.to_string(),
        login: u.login,
        description: u.description,
        challenge_filter: u.challenge_filter,
        verified: u.verified,
        rating: u.rating,
        rank: rank.min(u32::MAX as i64) as u32,
        clan_id: u.clan_id.map(|id| id.to_string()).unwrap_or_default(),
        clan_name: u.clan_name.unwrap_or_default(),
        title: if u.verified {
            rank_title(u.rating).into()
        } else {
            "rank-unverified".into()
        },
        ..Default::default()
    };
    if let Some(a) = a {
        let definition = engine_content::avatar(
            u16::try_from(a.path_id.unwrap_or(a.template_id))
                .map_err(|_| Error("CONTENT_MISMATCH"))?,
        )
        .ok_or(Error("CONTENT_MISMATCH"))?;
        let (mut stats, schools) =
            definition.at_level(u8::try_from(a.level).map_err(|_| Error("CONTENT_MISMATCH"))?);
        stats.phys_atk += i32::from(a.dp_phys_atk);
        stats.phys_def += i32::from(a.dp_phys_def);
        stats.mag_atk += i32::from(a.dp_mag_atk);
        stats.mag_def += i32::from(a.dp_mag_def);
        stats.hp_max += 7 * i32::from(a.dp_hp);
        stats.stamina_max += 7 * i32::from(a.dp_stamina);
        p.avatar_id = a.id.to_string();
        p.template_id = a.template_id as u32;
        p.level = a.level as u32;
        p.stats = engine::Stat::ALL
            .iter()
            .map(|stat| stats.get(*stat))
            .collect();
        p.schools = schools.0.into_iter().map(u32::from).collect();
    }
    Ok(p)
}
fn rank_title(rating: i32) -> &'static str {
    match rating {
        ..400 => "rank-0",
        400..600 => "rank-400",
        600..800 => "rank-600",
        800..1000 => "rank-800",
        1000..1100 => "rank-1000",
        1100..1200 => "rank-1100",
        1200..1300 => "rank-1200",
        1300..1400 => "rank-1300",
        1400..1500 => "rank-1400",
        1500..1600 => "rank-1500",
        1600..1800 => "rank-1600",
        1800..2000 => "rank-1800",
        2000..2200 => "rank-2000",
        2200..2500 => "rank-2200",
        _ => "rank-2500",
    }
}
pub async fn social(db: &PgPool, user: Uuid) -> Result<pb::SocialSnapshot> {
    let friends=sqlx::query!("SELECT u.id,u.login::text AS \"login!\" FROM friends f JOIN users u ON u.id=f.friend_id WHERE f.user_id=$1 AND u.status='active' ORDER BY u.login LIMIT 100",user).fetch_all(db).await?.into_iter().map(|row|pb::SocialEntry{id:row.id.to_string(),login:row.login,online:false}).collect();
    let ignores=sqlx::query!("SELECT u.id,u.login::text AS \"login!\" FROM ignores i JOIN users u ON u.id=i.ignored_id WHERE i.user_id=$1 AND u.status='active' ORDER BY u.login LIMIT 100",user).fetch_all(db).await?.into_iter().map(|row|pb::SocialEntry{id:row.id.to_string(),login:row.login,online:false}).collect();
    Ok(pb::SocialSnapshot { friends, ignores })
}
pub async fn relationship(db: &PgPool, from: Uuid, to: Uuid) -> Result<(bool, bool)> {
    let row=sqlx::query!("SELECT EXISTS(SELECT 1 FROM ignores WHERE (user_id=$1 AND ignored_id=$2) OR (user_id=$2 AND ignored_id=$1)) AS \"ignored!\",EXISTS(SELECT 1 FROM friends WHERE user_id=$2 AND friend_id=$1) AS \"invited!\"",from,to).fetch_one(db).await?;
    Ok((row.ignored, row.invited))
}
pub async fn social_change(
    db: &PgPool,
    user: Uuid,
    other: Uuid,
    ignore: bool,
    add: bool,
) -> Result<pb::SocialSnapshot> {
    if user == other {
        return Err(Error("INVALID_MESSAGE"));
    }
    let mut tx = db.begin().await?;
    sqlx::query!("SELECT id FROM users WHERE id=$1 FOR UPDATE", user)
        .fetch_one(&mut *tx)
        .await?;
    if add {
        let count = if ignore {
            sqlx::query_scalar!(
                "SELECT count(*) AS \"count!\" FROM ignores WHERE user_id=$1 AND ignored_id<>$2",
                user,
                other
            )
            .fetch_one(&mut *tx)
            .await?
        } else {
            sqlx::query_scalar!(
                "SELECT count(*) AS \"count!\" FROM friends WHERE user_id=$1 AND friend_id<>$2",
                user,
                other
            )
            .fetch_one(&mut *tx)
            .await?
        };
        if count >= 100 {
            return Err(Error("BUSY"));
        }
    }
    match (ignore, add) {
        (true, true) => {
            sqlx::query!(
                "INSERT INTO ignores(user_id,ignored_id) VALUES ($1,$2) ON CONFLICT DO NOTHING",
                user,
                other
            )
            .execute(&mut *tx)
            .await?;
        }
        (true, false) => {
            sqlx::query!(
                "DELETE FROM ignores WHERE user_id=$1 AND ignored_id=$2",
                user,
                other
            )
            .execute(&mut *tx)
            .await?;
        }
        (false, true) => {
            sqlx::query!(
                "INSERT INTO friends(user_id,friend_id) VALUES ($1,$2) ON CONFLICT DO NOTHING",
                user,
                other
            )
            .execute(&mut *tx)
            .await?;
        }
        (false, false) => {
            sqlx::query!(
                "DELETE FROM friends WHERE user_id=$1 AND friend_id=$2",
                user,
                other
            )
            .execute(&mut *tx)
            .await?;
        }
    }
    tx.commit().await?;
    social(db, user).await
}
pub async fn challenge_filter(db: &PgPool, user: Uuid, filter: &str) -> Result<()> {
    sqlx::query!(
        "UPDATE users SET challenge_filter=$2 WHERE id=$1 AND status='active'",
        user,
        filter
    )
    .execute(db)
    .await?;
    Ok(())
}
pub async fn log_chat(db: &PgPool, message: &pb::ChatMessage, clan: Option<Uuid>) -> Result<()> {
    let id = Uuid::parse_str(&message.id).map_err(|_| Error("INTERNAL"))?;
    let from = Uuid::parse_str(&message.from_id).map_err(|_| Error("INTERNAL"))?;
    let to = Uuid::parse_str(&message.to).ok();
    sqlx::query!(
        "INSERT INTO chat_log(id,room,from_user,to_user,clan_id,text) VALUES ($1,$2,$3,$4,$5,$6)",
        id,
        message.room,
        from,
        to,
        clan,
        message.text
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn send_mail(db: &PgPool, from: Uuid, to: Uuid, body: &str) -> Result<()> {
    let mut tx = db.begin().await?;
    sqlx::query!(
        "SELECT id FROM users WHERE id=$1 AND status='active' FOR UPDATE",
        to
    )
    .fetch_one(&mut *tx)
    .await?;
    let count = sqlx::query_scalar!(
        "SELECT count(*) AS \"count!\" FROM mail WHERE to_user=$1 AND NOT deleted_by_recipient",
        to
    )
    .fetch_one(&mut *tx)
    .await?;
    if count >= 500 {
        return Err(Error("BUSY"));
    }
    let id = Uuid::now_v7();
    sqlx::query!(
        "INSERT INTO mail(id,from_user,to_user,body) VALUES ($1,$2,$3,$4)",
        id,
        from,
        to,
        body
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}
pub async fn mail_list(db: &PgPool, user: Uuid, list: &pb::MailList) -> Result<pb::MailSnapshot> {
    let before = if list.before_id.is_empty() {
        None
    } else {
        Some(Uuid::parse_str(&list.before_id).map_err(|_| Error("INVALID_MESSAGE"))?)
    };
    let limit = i64::from(if list.limit == 0 {
        20
    } else {
        list.limit.min(50)
    });
    let sent = list.sent;
    let rows=sqlx::query!("SELECT m.id,f.login::text AS \"sender!\",t.login::text AS \"recipient!\",m.body,(EXTRACT(EPOCH FROM m.created_at)*1000)::bigint AS \"at_ms!\",m.read_at IS NOT NULL AS \"read!\" FROM mail m JOIN users f ON f.id=m.from_user JOIN users t ON t.id=m.to_user WHERE (($3 AND m.from_user=$1 AND NOT m.deleted_by_sender) OR (NOT $3 AND m.to_user=$1 AND NOT m.deleted_by_recipient)) AND ($2::uuid IS NULL OR m.id<$2) ORDER BY m.id DESC LIMIT $4",user,before,sent,limit).fetch_all(db).await?;
    let next_cursor = if rows.len() == limit as usize {
        rows.last().map(|r| r.id.to_string()).unwrap_or_default()
    } else {
        String::new()
    };
    Ok(pb::MailSnapshot {
        messages: rows
            .into_iter()
            .map(|r| pb::MailEntry {
                id: r.id.to_string(),
                from_login: r.sender,
                to_login: r.recipient,
                text: r.body,
                at_ms: r.at_ms,
                read: r.read,
            })
            .collect(),
        next_cursor,
        sent,
    })
}
pub async fn mail_change(db: &PgPool, user: Uuid, id: Uuid, delete: bool) -> Result<()> {
    if delete {
        sqlx::query!("UPDATE mail SET deleted_by_sender=deleted_by_sender OR from_user=$1,deleted_by_recipient=deleted_by_recipient OR to_user=$1 WHERE id=$2 AND (from_user=$1 OR to_user=$1)",user,id).execute(db).await?;
    } else {
        sqlx::query!("UPDATE mail SET read_at=COALESCE(read_at,now()) WHERE id=$2 AND to_user=$1 AND NOT deleted_by_recipient",user,id).execute(db).await?;
    }
    Ok(())
}
pub async fn retention(db: &PgPool) -> Result<()> {
    sqlx::query!("DELETE FROM chat_log WHERE created_at<now()-interval '30 days'")
        .execute(db)
        .await?;
    sqlx::query!("DELETE FROM mail WHERE deleted_by_sender AND deleted_by_recipient")
        .execute(db)
        .await?;
    Ok(())
}

#[derive(Clone, Debug)]
pub struct PreparedPlayer {
    pub user: Uuid,
    pub avatar: Uuid,
    pub setup: engine::PlayerSetup,
}
pub async fn prepare_player(
    connection: &mut PgConnection,
    user: Uuid,
    expected: &str,
) -> Result<PreparedPlayer> {
    let a=sqlx::query!("SELECT id,template_id,path_id,level,specialization,gift_id,dp_phys_atk,dp_phys_def,dp_mag_atk,dp_mag_def,dp_hp,dp_stamina FROM avatars WHERE user_id=$1 AND is_active AND deleted_at IS NULL FOR UPDATE",user).fetch_optional(&mut *connection).await?.ok_or(Error("CONFLICT"))?;
    if a.id.to_string() != expected {
        return Err(Error("CONFLICT"));
    }
    let definition = engine_content::avatar(
        u16::try_from(a.path_id.unwrap_or(a.template_id)).map_err(|_| Error("CONTENT_MISMATCH"))?,
    )
    .ok_or(Error("CONTENT_MISMATCH"))?;
    let mut snapshot =
        definition.snapshot(u8::try_from(a.level).map_err(|_| Error("CONTENT_MISMATCH"))?);
    snapshot.stats.phys_atk += i32::from(a.dp_phys_atk);
    snapshot.stats.phys_def += i32::from(a.dp_phys_def);
    snapshot.stats.mag_atk += i32::from(a.dp_mag_atk);
    snapshot.stats.mag_def += i32::from(a.dp_mag_def);
    snapshot.stats.hp_max += 7 * i32::from(a.dp_hp);
    snapshot.stats.stamina_max += 7 * i32::from(a.dp_stamina);
    snapshot.gift = a
        .gift_id
        .map(u16::try_from)
        .transpose()
        .map_err(|_| Error("CONTENT_MISMATCH"))?;
    snapshot.specialization = match a.specialization.as_deref() {
        None => None,
        Some("essence") => Some(engine::Specialization::Essence),
        Some("concentration") => Some(engine::Specialization::Concentration),
        Some(s) => {
            let schools = ["combat", "divine", "element", "arcane", "necro", "tech"];
            let i = schools
                .iter()
                .position(|v| *v == s)
                .ok_or(Error("CONTENT_MISMATCH"))?;
            Some(engine::Specialization::School(engine::SCHOOLS[i]))
        }
    };
    let cards = sqlx::query_scalar!(
        "SELECT cards FROM decks WHERE avatar_id=$1 FOR UPDATE",
        a.id
    )
    .fetch_optional(&mut *connection)
    .await?
    .unwrap_or_default();
    let mut deck = Default::default();
    for id in cards {
        let card_id = u16::try_from(id).map_err(|_| Error("CONTENT_MISMATCH"))?;
        if engine_content::content().get(card_id).is_none() {
            return Err(Error("CONTENT_MISMATCH"));
        }
        // R-TAL-04 permits spells from schools the avatar cannot currently cast.
        let qty = sqlx::query_scalar!(
            "SELECT qty FROM collection WHERE user_id=$1 AND card_id=$2",
            user,
            id
        )
        .fetch_optional(&mut *connection)
        .await?
        .unwrap_or(0);
        // Ownership is checked again below for all copies; no client-supplied deck is trusted.
        let held = cards_count(&deck, card_id);
        if held >= qty as usize {
            return Err(Error("INVALID_DECK"));
        }
        if deck.try_push(card_id).is_err() {
            return Err(Error("INVALID_DECK"));
        }
    }
    Ok(PreparedPlayer {
        user,
        avatar: a.id,
        setup: engine::PlayerSetup {
            avatar: snapshot,
            deck,
        },
    })
}
fn cards_count(
    deck: &arrayvec::ArrayVec<engine::CardId, { engine::MAX_DECK_SIZE }>,
    id: engine::CardId,
) -> usize {
    deck.iter().filter(|card| **card == id).count()
}
