//! Persistent, server-controlled accounts. Public profiles use the normal account projection.
mod runtime;
pub use runtime::{Population, Settings};

use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::{BTreeMap, HashSet};
use uuid::Uuid;

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub key: String,
    pub login: String,
    pub profile: String,
    pub template: u16,
    pub level: u8,
}

pub fn roster() -> anyhow::Result<Vec<Identity>> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Roster {
        accounts: Vec<Identity>,
    }
    let roster: Roster = toml::from_str(include_str!("roster.toml"))?;
    ensure!(roster.accounts.len() == 40, "expected 40 managed accounts");
    let mut names = HashSet::new();
    let mut keys = HashSet::new();
    for account in &roster.accounts {
        ensure!(
            (3..=20).contains(&account.login.len())
                && account
                    .login
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
                && names.insert(account.login.to_ascii_lowercase()),
            "invalid or duplicate nickname"
        );
        ensure!(
            (3..=32).contains(&account.key.len())
                && account
                    .key
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
                && keys.insert(&account.key),
            "invalid or duplicate roster key"
        );
        ensure!((1..=8).contains(&account.level), "invalid initial level");
        ensure!(
            engine_content::is_playable(account.template),
            "locked roster template"
        );
        account.setup()?;
    }
    Ok(roster.accounts)
}

impl Identity {
    fn setup(&self) -> anyhow::Result<engine::PlayerSetup> {
        let profile = engine_ai::training_profiles()
            .bots
            .into_iter()
            .find(|p| p.id == self.profile)
            .context("unknown AI profile")?;
        let position = profile
            .avatar_pool
            .iter()
            .position(|id| *id == self.template)
            .context("template outside AI pool")?;
        profile
            .player(self.level, position)
            .context("invalid bot deck")
    }
}

#[derive(Debug, Serialize)]
pub struct SeedReport {
    pub created: usize,
    pub existing: usize,
    pub logins: Vec<String>,
}

/// A single transaction, serialized across processes. A nickname conflict never takes over a user.
pub async fn seed(db: &PgPool) -> anyhow::Result<SeedReport> {
    let roster = roster()?;
    let mut tx = db.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(736401227)")
        .execute(&mut *tx)
        .await?;
    let mut report = SeedReport {
        created: 0,
        existing: 0,
        logins: Vec::new(),
    };
    for identity in roster {
        if let Some((user, login)) = sqlx::query_as::<_, (Uuid, String)>(
            "SELECT u.id,u.login::text FROM bot_accounts b JOIN users u ON u.id=b.user_id WHERE b.roster_key=$1"
        ).bind(&identity.key).fetch_optional(&mut *tx).await? {
            repair_retired_avatar(&mut tx, user, &identity).await?;
            report.existing += 1;
            report.logins.push(login);
            continue;
        }
        let setup = identity.setup()?;
        let starter = engine_content::STARTERS
            .iter()
            .find(|s| s.avatar == identity.template)
            .context("missing starter")?;
        let school = ["combat", "divine", "element", "arcane", "necro", "tech"][engine::SCHOOLS
            .iter()
            .position(|s| *s == starter.school)
            .context("unknown school")?];
        // No usable password or bearer token is retained or printed.
        let password = tokio::task::spawn_blocking(|| {
            crate::auth::crypto::password_hash(&crate::auth::crypto::token()?)
        })
        .await??;
        let user = Uuid::now_v7();
        let avatar = Uuid::now_v7();
        sqlx::query("INSERT INTO users(id,login,email,password_hash,start_school,email_verified_at) VALUES($1,$2,$3,$4,$5,now())")
            .bind(user).bind(&identity.login).bind(format!("{}@residents.invalid", identity.key))
            .bind(password).bind(school).execute(&mut *tx).await
            .with_context(|| format!("cannot provision {}; nickname or address may already exist", identity.login))?;
        sqlx::query("INSERT INTO bot_accounts(user_id,roster_key,ai_profile) VALUES($1,$2,$3)")
            .bind(user)
            .bind(&identity.key)
            .bind(&identity.profile)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "INSERT INTO unlocked_templates(user_id,template_id,source) VALUES($1,$2,'admin')",
        )
        .bind(user)
        .bind(i32::from(identity.template))
        .execute(&mut *tx)
        .await?;
        let xp = crate::rules::RULES.progression.level_xp[usize::from(identity.level - 1)];
        let points: i16 = engine_content::avatar(identity.template)
            .context("missing avatar")?
            .dev_points
            .iter()
            .filter(|(level, _)| *level <= identity.level)
            .map(|(_, qty)| i16::from(*qty))
            .sum();
        sqlx::query("INSERT INTO avatars(id,user_id,slot,template_id,is_active,level,xp,dev_points_unspent) VALUES($1,$2,0,$3,true,$4,$5,$6)")
            .bind(avatar).bind(user).bind(i32::from(identity.template)).bind(i16::from(identity.level))
            .bind(xp).bind(points).execute(&mut *tx).await?;
        let mut cards = BTreeMap::<u16, i32>::new();
        for id in &setup.deck {
            *cards.entry(*id).or_default() += 1;
        }
        for (card, qty) in cards {
            sqlx::query("INSERT INTO collection(user_id,card_id,qty) VALUES($1,$2,$3)")
                .bind(user)
                .bind(card as i16)
                .bind(qty)
                .execute(&mut *tx)
                .await?;
        }
        let deck: Vec<i16> = setup.deck.iter().map(|id| *id as i16).collect();
        sqlx::query("INSERT INTO decks(avatar_id,cards) VALUES($1,$2)")
            .bind(avatar)
            .bind(deck)
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO ratings(user_id) VALUES($1)")
            .bind(user)
            .execute(&mut *tx)
            .await?;
        crate::economy::grant_initial(&mut tx, user)
            .await
            .map_err(|e| anyhow::anyhow!("initial balance: {}", e.0))?;
        report.created += 1;
        report.logins.push(identity.login);
    }
    tx.commit().await?;
    Ok(report)
}

/// Move only managed accounts off retired classes. Keep their old character and account history.
async fn repair_retired_avatar(
    tx: &mut sqlx::PgConnection,
    user: Uuid,
    identity: &Identity,
) -> anyhow::Result<()> {
    let (old, template, level, xp): (Uuid, i32, i16, i32) = sqlx::query_as(
        "SELECT id,template_id,level,xp FROM avatars WHERE user_id=$1 AND is_active AND deleted_at IS NULL"
    ).bind(user).fetch_one(&mut *tx).await?;
    if u16::try_from(template).is_ok_and(engine_content::is_playable) {
        return Ok(());
    }
    crate::activity::ensure_idle(tx, user)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e.0))?;
    let mut replacement = identity.clone();
    replacement.level = level as u8;
    let setup = replacement.setup()?;
    let slot: i16 = sqlx::query_scalar("SELECT s::smallint FROM generate_series(0,9) s WHERE NOT EXISTS(SELECT 1 FROM avatars WHERE user_id=$1 AND slot=s AND deleted_at IS NULL) ORDER BY s LIMIT 1")
        .bind(user).fetch_one(&mut *tx).await?;
    sqlx::query("INSERT INTO unlocked_templates(user_id,template_id,source) VALUES($1,$2,'admin') ON CONFLICT DO NOTHING")
        .bind(user).bind(i32::from(identity.template)).execute(&mut *tx).await?;
    sqlx::query("UPDATE avatars SET is_active=false,revision=revision+1 WHERE id=$1")
        .bind(old)
        .execute(&mut *tx)
        .await?;
    let id = Uuid::now_v7();
    let points: i16 = engine_content::avatar(identity.template)
        .context("missing template")?
        .dev_points
        .iter()
        .filter(|(at, _)| i16::from(*at) <= level)
        .map(|(_, qty)| i16::from(*qty))
        .sum();
    sqlx::query("INSERT INTO avatars(id,user_id,slot,template_id,is_active,level,xp,dev_points_unspent) VALUES($1,$2,$3,$4,true,$5,$6,$7)")
        .bind(id).bind(user).bind(slot).bind(i32::from(identity.template)).bind(level).bind(xp).bind(points).execute(&mut *tx).await?;
    let mut counts = BTreeMap::<u16, i32>::new();
    for card in &setup.deck {
        *counts.entry(*card).or_default() += 1;
    }
    for (card, qty) in counts {
        sqlx::query("INSERT INTO collection(user_id,card_id,qty) VALUES($1,$2,$3) ON CONFLICT(user_id,card_id) DO UPDATE SET qty=greatest(collection.qty,EXCLUDED.qty)")
            .bind(user).bind(card as i16).bind(qty).execute(&mut *tx).await?;
    }
    let deck: Vec<i16> = setup.deck.iter().map(|id| *id as i16).collect();
    sqlx::query("INSERT INTO decks(avatar_id,cards) VALUES($1,$2)")
        .bind(id)
        .bind(deck)
        .execute(tx)
        .await?;
    Ok(())
}

/// Freeze the controller along with each duel so recovery does not depend on live roster changes.
pub(crate) async fn configure_seats(
    connection: &mut sqlx::PgConnection,
    seats: &mut [crate::duel::record::Seat; 2],
) -> crate::online::Result<bool> {
    for seat in seats.iter_mut() {
        let Some(user) = seat.user else { continue };
        if let Some(id) =
            sqlx::query_scalar::<_, String>("SELECT ai_profile FROM bot_accounts WHERE user_id=$1")
                .bind(user)
                .fetch_optional(&mut *connection)
                .await?
        {
            let profile = engine_ai::training_profiles()
                .bots
                .into_iter()
                .find(|p| p.id == id)
                .ok_or(crate::online::Error("CONTENT_MISMATCH"))?;
            seat.bot = Some(id);
            seat.bot_profile = Some(profile);
        }
    }
    Ok(seats.iter().all(|s| s.bot.is_some()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn forty_distinct_accounts_have_legal_decks_and_beginner_opponents() {
        let accounts = roster().unwrap();
        assert!(accounts.iter().filter(|a| a.level == 1).count() >= 12);
        assert_eq!(
            accounts
                .iter()
                .map(|a| a.template)
                .collect::<HashSet<_>>()
                .len(),
            8
        );
        for identity in accounts {
            let setup = identity.setup().unwrap();
            engine::new_duel(
                &engine::DuelSetup {
                    rules: engine_content::rules(),
                    seed: [0; 32],
                    players: [setup.clone(), setup],
                },
                &engine_content::content(),
            )
            .unwrap();
        }
    }
}
