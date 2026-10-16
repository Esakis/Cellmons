use super::*;
use crate::{
    duel::record::Seat,
    economy::{self, Account},
    online::repo,
};
use sha2::{Digest, Sha256};
use sqlx::PgPool;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Terms {
    /// Individual players, or clan count when team_size > 1.
    pub teams: usize,
    pub team_size: usize,
    pub fee: i64,
    pub max_games: u8,
    pub min_level: u8,
    pub max_level: u8,
    pub tiers: Vec<u8>,
    pub only_clan: Option<Uuid>,
    pub allowed_clans: Vec<Uuid>,
}
impl Terms {
    pub fn validate(&self) -> Result<()> {
        let rules = &crate::rules::RULES.tournament;
        let clans: std::collections::BTreeSet<_> = self.allowed_clans.iter().collect();
        let tiers: std::collections::BTreeSet<_> = self.tiers.iter().collect();
        if !(2..=rules.max_teams).contains(&self.teams)
            || !(1..=16).contains(&self.team_size)
            || self.teams * self.team_size > rules.max_players
            || !(0..=rules.max_fee).contains(&self.fee)
            || !(1..=9).contains(&self.max_games)
            || self.min_level == 0
            || self.max_level > 20
            || self.min_level > self.max_level
            || self.tiers.is_empty()
            || tiers.len() != self.tiers.len()
            || self.tiers.iter().any(|t| *t > 3)
            || clans.len() != self.allowed_clans.len()
            || clans.len() > rules.max_teams
            || (self.team_size > 1 && (clans.len() != self.teams || self.only_clan.is_some()))
            || (self.team_size == 1 && !clans.is_empty())
        {
            return Err(Error("INVALID_MESSAGE"));
        }
        Ok(())
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum Mutation {
    Create {
        id: Uuid,
        name: String,
        terms: Terms,
    },
    Join {
        id: Uuid,
        request: Uuid,
        avatar: Uuid,
    },
    Leave {
        id: Uuid,
        request: Uuid,
    },
    Cancel {
        id: Uuid,
        request: Uuid,
    },
}
pub async fn create(db: &PgPool, user: Uuid, id: Uuid, name: &str, terms: Terms) -> Result<Uuid> {
    terms.validate()?;
    let name = name.trim();
    if !(3..=60).contains(&name.chars().count()) || name.chars().any(char::is_control) {
        return Err(Error("INVALID_MESSAGE"));
    }
    let mut tx = db.begin().await?;
    lock_users(&mut tx, vec![user]).await?;
    verified(&mut tx, user).await?;
    crate::activity::ensure_available(&mut tx).await?;
    if let Some((owner, old_name, old_terms)) = sqlx::query_as::<_, (Uuid, String, Vec<u8>)>(
        "SELECT creator,name,terms FROM tournaments WHERE id=$1",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?
    {
        if owner == user && old_name == name && decode::<Terms>(&old_terms)? == terms {
            return Ok(id);
        }
        return Err(Error("CONFLICT"));
    }
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM tournaments WHERE creator=$1 AND status IN ('registering','running')",
    )
    .bind(user)
    .fetch_one(&mut *tx)
    .await?;
    if count >= 3 {
        return Err(Error("LIMIT"));
    }
    if let Some(clan) = terms.only_clan
        && repo::clan_on(&mut tx, user).await? != Some(clan)
    {
        return Err(Error("FORBIDDEN"));
    }
    if terms.team_size > 1 {
        let count: i64 =
            sqlx::query_scalar("SELECT count(*) FROM clans WHERE id=ANY($1) AND status='active'")
                .bind(&terms.allowed_clans)
                .fetch_one(&mut *tx)
                .await?;
        if count != terms.allowed_clans.len() as i64 {
            return Err(Error("INVALID_MESSAGE"));
        }
    }
    let mut seed = [0_u8; 32];
    getrandom::fill(&mut seed).map_err(|_| Error("INTERNAL"))?;
    sqlx::query("INSERT INTO tournaments(id,creator,name,terms,seed,engine_version,content_version) VALUES ($1,$2,$3,$4,$5,$6,$7)")
        .bind(id)
        .bind(user)
        .bind(name)
        .bind(encode(&terms)?)
        .bind(seed.as_slice())
        .bind(engine::VERSION)
        .bind(&engine_content::CONTENT_VERSION[..])
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(id)
}
async fn verified(connection: &mut PgConnection, user: Uuid) -> Result<()> {
    let allowed: bool = sqlx::query_scalar(
        "SELECT status='active' AND email_verified_at IS NOT NULL FROM users WHERE id=$1",
    )
    .bind(user)
    .fetch_optional(connection)
    .await?
    .unwrap_or(false);
    if allowed {
        Ok(())
    } else {
        Err(Error("FORBIDDEN"))
    }
}
async fn repeated(
    connection: &mut PgConnection,
    user: Uuid,
    key: Uuid,
    id: Uuid,
    action: &str,
    avatar: Option<Uuid>,
) -> Result<bool> {
    let fingerprint = Sha256::digest(encode(&(id, action, avatar))?).to_vec();
    if let Some(old) = sqlx::query_scalar::<_, Vec<u8>>(
        "SELECT fingerprint FROM tournament_requests WHERE user_id=$1 AND request_id=$2",
    )
    .bind(user)
    .bind(key)
    .fetch_optional(&mut *connection)
    .await?
    {
        if old != fingerprint {
            return Err(Error("CONFLICT"));
        }
        return Ok(true);
    }
    sqlx::query("INSERT INTO tournament_requests(user_id,request_id,tournament_id,fingerprint) VALUES ($1,$2,$3,$4)").bind(user).bind(key).bind(id).bind(fingerprint).execute(connection).await?;
    Ok(false)
}
pub async fn join(db: &PgPool, user: Uuid, id: Uuid, key: Uuid, avatar: Uuid) -> Result<()> {
    let mut tx = db.begin().await?;
    lock_users(&mut tx, vec![user]).await?;
    let tournament = locked(&mut tx, id).await?;
    tournament.compatible()?;
    if repeated(&mut tx, user, key, id, "join", Some(avatar)).await? {
        return Ok(());
    }
    if tournament.status != "registering" {
        return Err(Error("CONFLICT"));
    }
    verified(&mut tx, user).await?;
    crate::activity::ensure_idle(&mut tx, user).await?;
    let terms: Terms = decode(&tournament.terms)?;
    let cash = tournament
        .system_reward
        .as_deref()
        .map(super::rewards::public)
        .transpose()?
        .map_or(0, |r| r.cash());
    let maximum_prize = terms.fee * terms.teams as i64 * terms.team_size as i64 + cash;
    let balance: i64 = sqlx::query_scalar("SELECT balance FROM wallets WHERE user_id=$1")
        .bind(user)
        .fetch_one(&mut *tx)
        .await?;
    if balance > i64::MAX - maximum_prize {
        return Err(Error("BALANCE_LIMIT"));
    }
    let profile = repo::player_on(&mut tx, user).await?;
    if profile.avatar_id != avatar.to_string()
        || profile.level < u32::from(terms.min_level)
        || profile.level > u32::from(terms.max_level)
        || !terms.tiers.contains(&(profile.tier as u8))
    {
        return Err(Error("INELIGIBLE"));
    }
    let clan = repo::clan_on(&mut tx, user).await?;
    if terms.only_clan.is_some() && terms.only_clan != clan {
        return Err(Error("FORBIDDEN"));
    }
    let team = if terms.team_size > 1 {
        clan.filter(|c| terms.allowed_clans.contains(c))
            .ok_or(Error("FORBIDDEN"))?
    } else {
        user
    };
    let current = entries(&mut tx, id).await?;
    let joined = current
        .iter()
        .filter(|e| e.status == "enrolled")
        .collect::<Vec<_>>();
    if joined.len() >= terms.teams * terms.team_size
        || joined.iter().filter(|e| e.team_id == team).count() >= terms.team_size
    {
        return Err(Error("FULL"));
    }
    let player = repo::prepare_player(&mut tx, user, &avatar.to_string()).await?;
    let seat = Seat::player(user, avatar, &player.setup, &profile);
    economy::transfer(
        &mut tx,
        Account::Player(user),
        Account::Tournament(id),
        terms.fee,
        "tournament_fee",
        id,
    )
    .await?;
    sqlx::query("INSERT INTO tournament_entries(tournament_id,user_id,avatar_id,team_id,seat,fee) VALUES ($1,$2,$3,$4,$5,$6) ON CONFLICT(tournament_id,user_id) DO UPDATE SET avatar_id=excluded.avatar_id,team_id=excluded.team_id,seat=excluded.seat,fee=excluded.fee,status='enrolled',ready_since_ms=NULL,ready_until_ms=0,joined_at=now()")
        .bind(id).bind(user).bind(avatar).bind(team).bind(encode(&seat)?).bind(terms.fee).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO activity_locks(user_id,kind,owner_id) VALUES ($1,'tournament',$2)")
        .bind(user)
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE tournaments SET revision=revision+1 WHERE id=$1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}
pub async fn leave(db: &PgPool, user: Uuid, id: Uuid, key: Uuid) -> Result<()> {
    let mut tx = db.begin().await?;
    lock_users(&mut tx, vec![user]).await?;
    let tournament = locked(&mut tx, id).await?;
    if repeated(&mut tx, user, key, id, "leave", None).await? {
        return Ok(());
    }
    let entry = entries(&mut tx, id)
        .await?
        .into_iter()
        .find(|e| e.user_id == user)
        .ok_or(Error("NOT_FOUND"))?;
    if entry.status != "enrolled" {
        tx.commit().await?;
        return Ok(());
    }
    if tournament.status == "registering" {
        economy::transfer(
            &mut tx,
            Account::Tournament(id),
            Account::Player(user),
            entry.fee,
            "tournament_fee",
            id,
        )
        .await?;
        unlock(&mut tx, id, user).await?;
        sqlx::query("UPDATE tournament_entries SET status='left',ready_since_ms=NULL,ready_until_ms=0 WHERE tournament_id=$1 AND user_id=$2").bind(id).bind(user).execute(&mut *tx).await?;
    } else if tournament.status == "running" {
        let mut bracket: bracket::Bracket = decode(
            tournament
                .bracket
                .as_deref()
                .ok_or(Error("INVALID_JOURNAL"))?,
        )?;
        bracket.withdraw(user)?;
        // The activity stays reserved until any real duel has settled and this team is eliminated.
        sqlx::query("UPDATE tournaments SET bracket=$2 WHERE id=$1")
            .bind(id)
            .bind(encode(&bracket)?)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE tournament_entries SET ready_since_ms=NULL,ready_until_ms=0 WHERE tournament_id=$1 AND user_id=$2").bind(id).bind(user).execute(&mut *tx).await?;
    } else {
        return Err(Error("CONFLICT"));
    }
    sqlx::query("UPDATE tournaments SET revision=revision+1 WHERE id=$1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}
pub(super) async fn unlock(connection: &mut PgConnection, id: Uuid, user: Uuid) -> Result<()> {
    sqlx::query(
        "DELETE FROM activity_locks WHERE user_id=$1 AND kind='tournament' AND owner_id=$2",
    )
    .bind(user)
    .bind(id)
    .execute(connection)
    .await?;
    Ok(())
}
pub async fn ready(db: &PgPool, user: Uuid, value: bool) -> Result<()> {
    let mut tx = db.begin().await?;
    lock_users(&mut tx, vec![user]).await?;
    let id: Option<Uuid> = sqlx::query_scalar(
        "SELECT owner_id FROM activity_locks WHERE user_id=$1 AND kind='tournament'",
    )
    .bind(user)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(id) = id else {
        return Ok(());
    };
    locked(&mut tx, id).await?;
    if value {
        verified(&mut tx, user).await?;
        let busy: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM duel_locks WHERE user_id=$1)")
                .bind(user)
                .fetch_one(&mut *tx)
                .await?;
        if busy {
            return Err(Error("BUSY"));
        }
    }
    let now = crate::online::now_ms();
    sqlx::query("UPDATE tournament_entries SET ready_since_ms=CASE WHEN $3 THEN CASE WHEN ready_until_ms>$4 THEN ready_since_ms ELSE $4 END ELSE NULL END,ready_until_ms=CASE WHEN $3 THEN $4+15000 ELSE 0 END WHERE tournament_id=$1 AND user_id=$2 AND status='enrolled'")
        .bind(id).bind(user).bind(value).bind(now).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}
pub async fn cancel(db: &PgPool, user: Uuid, id: Uuid, key: Uuid) -> Result<()> {
    let mut tx = db.begin().await?;
    let mut users: Vec<Uuid> =
        sqlx::query_scalar("SELECT user_id FROM tournament_entries WHERE tournament_id=$1")
            .bind(id)
            .fetch_all(&mut *tx)
            .await?;
    users.push(user);
    let locked_users = users.clone();
    lock_users(&mut tx, users).await?;
    let tournament = locked(&mut tx, id).await?;
    if tournament.creator != user {
        return Err(Error("FORBIDDEN"));
    }
    if tournament.system_reward.is_some() {
        return Err(Error("FORBIDDEN"));
    }
    if repeated(&mut tx, user, key, id, "cancel", None).await? {
        return Ok(());
    }
    if tournament.status != "registering" {
        return Err(Error("CONFLICT"));
    }
    let entries = entries(&mut tx, id).await?;
    if entries.iter().any(|e| !locked_users.contains(&e.user_id)) {
        return Err(Error("CONFLICT"));
    }
    refund(&mut tx, id, &entries).await?;
    tx.commit().await?;
    Ok(())
}
pub(super) async fn refund(
    connection: &mut PgConnection,
    id: Uuid,
    entries: &[Entry],
) -> Result<()> {
    for e in entries.iter().filter(|e| e.status == "enrolled") {
        economy::transfer(
            connection,
            Account::Tournament(id),
            Account::Player(e.user_id),
            e.fee,
            "tournament_fee",
            id,
        )
        .await?;
        unlock(connection, id, e.user_id).await?;
    }
    sqlx::query("UPDATE tournament_entries SET status='left',ready_since_ms=NULL,ready_until_ms=0 WHERE tournament_id=$1 AND status='enrolled'").bind(id).execute(&mut *connection).await?;
    let remainder: i64 = sqlx::query_scalar("SELECT balance FROM tournaments WHERE id=$1")
        .bind(id)
        .fetch_one(&mut *connection)
        .await?;
    economy::transfer(
        connection,
        Account::Tournament(id),
        Account::Sink,
        remainder,
        "tournament_prize",
        id,
    )
    .await?;
    sqlx::query("UPDATE tournaments SET status='cancelled',revision=revision+1,finished_at=now() WHERE id=$1").bind(id).execute(connection).await?;
    Ok(())
}
