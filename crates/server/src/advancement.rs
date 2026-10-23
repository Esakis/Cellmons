//! Account contracts, card mastery and class trials share the exactly-once match transaction.
use crate::{
    duel::record::{Record, decode_move},
    game,
    online::{Error, Result},
};
use serde::Serialize;
use sqlx::PgConnection;
use std::collections::BTreeSet;
use uuid::Uuid;

pub const MASTERY_XP: [i16; 5] = [0, 15, 45, 100, 200];
pub fn mastery_level(xp: i16) -> usize {
    MASTERY_XP.partition_point(|n| *n <= xp).max(1)
}
pub fn level_for_xp(xp: i32, tier: i16) -> i16 {
    let scale = match tier {
        0 => 2,
        1 => 3,
        _ => 4,
    };
    crate::rules::RULES
        .progression
        .level_xp
        .partition_point(|n| i64::from(*n) * scale / 2 <= i64::from(xp))
        .clamp(1, 20) as i16
}
pub async fn require(db: &mut PgConnection, user: Uuid, required: u8) -> Result<()> {
    let xp: i64 = sqlx::query_scalar("SELECT account_xp FROM users WHERE id=$1")
        .bind(user)
        .fetch_one(db)
        .await?;
    if crate::journey::level(xp) < required {
        return Err(Error("LEVEL_REQUIRED"));
    }
    Ok(())
}
#[derive(Serialize, sqlx::FromRow)]
pub struct Contract {
    pub template_id: i32,
    pub fragments: i16,
    pub active: bool,
    pub claimed: bool,
}
#[derive(Serialize, sqlx::FromRow)]
pub struct Mastery {
    pub card_id: i16,
    pub xp: i16,
}
#[derive(Serialize)]
pub struct Snapshot {
    pub contracts: Vec<Contract>,
    pub mastery: Vec<Mastery>,
    pub daily_fights: i16,
    pub daily_rewarded: bool,
}
pub async fn load(db: &mut PgConnection, user: Uuid) -> Result<Snapshot> {
    let contracts = sqlx::query_as("SELECT template_id,fragments,active,claimed_at IS NOT NULL AS claimed FROM class_contracts WHERE user_id=$1 ORDER BY template_id").bind(user).fetch_all(&mut *db).await?;
    let mastery =
        sqlx::query_as("SELECT card_id,xp FROM card_mastery WHERE user_id=$1 ORDER BY card_id")
            .bind(user)
            .fetch_all(&mut *db)
            .await?;
    let (daily_fights,daily_rewarded) = sqlx::query_as("SELECT fights,rewarded FROM daily_quests WHERE user_id=$1 AND day=(now() AT TIME ZONE 'UTC')::date").bind(user).fetch_optional(db).await?.unwrap_or((0,false));
    Ok(Snapshot {
        contracts,
        mastery,
        daily_fights,
        daily_rewarded,
    })
}
#[derive(Default, Serialize, sqlx::FromRow)]
pub struct Character {
    pub permanent_points: i16,
    pub permanent_unspent: i16,
    pub trial_wins: i16,
}
pub async fn character(db: &mut PgConnection, id: Uuid) -> Result<Character> {
    Ok(sqlx::query_as("SELECT coalesce(p.permanent_points,0)::smallint AS permanent_points,coalesce(p.permanent_unspent,0)::smallint AS permanent_unspent,coalesce(t.wins,0)::smallint AS trial_wins FROM avatars a LEFT JOIN avatar_advancement p ON p.avatar_id=a.id LEFT JOIN class_trials t ON t.avatar_id=a.id AND t.tier=a.tier WHERE a.id=$1").bind(id).fetch_one(db).await?)
}
pub async fn contract(
    db: &mut PgConnection,
    user: Uuid,
    state: &game::Snapshot,
    template: u16,
    claim: bool,
) -> Result<()> {
    require(db, user, 4).await?;
    let existing = state
        .advancement
        .contracts
        .iter()
        .find(|c| c.template_id == i32::from(template));
    if claim && existing.is_some_and(|c| c.claimed) {
        return Ok(());
    }
    if !state.recruitment.available.contains(&template) {
        return Err(Error("RECRUIT_UNAVAILABLE"));
    }
    if claim {
        if !existing.is_some_and(|c| c.fragments == 30) {
            return Err(Error("NOT_READY"));
        }
        let id = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO unlocked_templates(user_id,template_id,source) VALUES($1,$2,'contract')",
        )
        .bind(user)
        .bind(i32::from(template))
        .execute(&mut *db)
        .await?;
        game::create_avatar(db, user, state, id, template).await?;
        sqlx::query("UPDATE class_contracts SET claimed_at=now(),active=false,avatar_id=$3 WHERE user_id=$1 AND template_id=$2").bind(user).bind(i32::from(template)).bind(id).execute(db).await?;
    } else {
        sqlx::query("UPDATE class_contracts SET active=false WHERE user_id=$1 AND active")
            .bind(user)
            .execute(&mut *db)
            .await?;
        sqlx::query("INSERT INTO class_contracts(user_id,template_id,active) VALUES($1,$2,true) ON CONFLICT(user_id,template_id) DO UPDATE SET active=true").bind(user).bind(i32::from(template)).execute(db).await?;
    }
    Ok(())
}
/// Rebuild every round: the final round's event buffer alone loses earlier casts.
pub fn casts(record: &Record) -> Result<[BTreeSet<u16>; 2]> {
    let content = engine_content::content();
    let mut state =
        engine::new_duel(&record.setup()?, &content).map_err(|_| Error("INVALID_JOURNAL"))?;
    let mut played: [BTreeSet<u16>; 2] = Default::default();
    let mut events = engine::EventBuf::new();
    for pair in &record.moves {
        events.clear();
        engine::resolve_round(
            &mut state,
            &content,
            &[decode_move(&pair[0])?, decode_move(&pair[1])?],
            &mut events,
        )
        .map_err(|_| Error("INVALID_JOURNAL"))?;
        let mut successful: [BTreeSet<u16>; 2] = Default::default();
        for event in events.as_slice() {
            if let engine::Event::SpellCast { side, card, .. } = event {
                successful[side.index()].insert(*card);
            }
        }
        for event in events.as_slice() {
            if let engine::Event::SpellFailed { side, card } = event {
                successful[side.index()].remove(card);
            }
        }
        for side in engine::Side::BOTH {
            played[side.index()].append(&mut successful[side.index()]);
        }
    }
    Ok(played)
}
/// Caller holds the owner lock and has rejected training, early exits and short fights.
pub async fn award(
    db: &mut PgConnection,
    record: &Record,
    index: usize,
    won: bool,
    played: &BTreeSet<u16>,
) -> Result<()> {
    let seat = &record.seats[index];
    let (Some(user), Some(avatar)) = (seat.user, seat.avatar) else {
        return Ok(());
    };
    let (xp,managed): (i64,bool) = sqlx::query_as("SELECT account_xp,EXISTS(SELECT 1 FROM bot_accounts WHERE user_id=$1) FROM users WHERE id=$1").bind(user).fetch_one(&mut *db).await?;
    if managed {
        return Ok(());
    }
    let level = crate::journey::level(xp);
    if level >= 4 {
        // A class acquired elsewhere cannot keep earning unusable fragments.
        sqlx::query("UPDATE class_contracts SET active=false WHERE user_id=$1 AND active AND template_id IN (SELECT template_id FROM unlocked_templates WHERE user_id=$1)").bind(user).execute(&mut *db).await?;
        sqlx::query("UPDATE class_contracts SET fragments=least(30,fragments+$2) WHERE user_id=$1 AND active AND claimed_at IS NULL").bind(user).bind(1_i16+i16::from(won)+i16::from(won && record.kind=="expedition")).execute(&mut *db).await?;
        let (fights,rewarded): (i16,bool)=sqlx::query_as("INSERT INTO daily_quests(user_id,day,fights) VALUES($1,(now() AT TIME ZONE 'UTC')::date,1) ON CONFLICT(user_id,day) DO UPDATE SET fights=least(3,daily_quests.fights+1) RETURNING fights,rewarded").bind(user).fetch_one(&mut *db).await?;
        if fights == 3 && !rewarded {
            crate::economy::transfer(
                db,
                crate::economy::Account::Faucet,
                crate::economy::Account::Player(user),
                5,
                "daily_quest",
                record.id,
            )
            .await?;
            sqlx::query("UPDATE daily_quests SET rewarded=true WHERE user_id=$1 AND day=(now() AT TIME ZONE 'UTC')::date").bind(user).execute(&mut *db).await?;
        }
    }
    if level >= 7 {
        for card in seat.deck.iter().copied().collect::<BTreeSet<_>>() {
            let gain = if played.contains(&card) { 3_i16 } else { 1 };
            let before: i16 =
                sqlx::query_scalar("SELECT xp FROM card_mastery WHERE user_id=$1 AND card_id=$2")
                    .bind(user)
                    .bind(card as i16)
                    .fetch_optional(&mut *db)
                    .await?
                    .unwrap_or(0);
            let after = (before + gain).min(200);
            sqlx::query("INSERT INTO card_mastery(user_id,card_id,xp) VALUES($1,$2,$3) ON CONFLICT(user_id,card_id) DO UPDATE SET xp=$3").bind(user).bind(card as i16).bind(after).execute(&mut *db).await?;
            let reward = (mastery_level(after) - mastery_level(before)) as i64 * 2;
            crate::economy::transfer(
                db,
                crate::economy::Account::Faucet,
                crate::economy::Account::Player(user),
                reward,
                "card_mastery",
                record.id,
            )
            .await?;
        }
    }
    let (character_level, tier): (i16, i16) =
        sqlx::query_as("SELECT level,tier FROM avatars WHERE id=$1")
            .bind(avatar)
            .fetch_one(&mut *db)
            .await?;
    // Use the starting level: the duel that reaches 20 does not count as a trial duel.
    if ["arena", "expedition"].contains(&record.kind.as_str())
        && won
        && !played.is_empty()
        && record.seats[index].public()?.level == 20
        && character_level == 20
        && ((tier == 0 && level >= 12) || (tier == 1 && level >= 18))
    {
        sqlx::query("INSERT INTO class_trials(avatar_id,tier,wins) VALUES($1,$2,1) ON CONFLICT(avatar_id,tier) DO UPDATE SET wins=least(3,class_trials.wins+1)").bind(avatar).bind(tier).execute(db).await?;
    }
    Ok(())
}
