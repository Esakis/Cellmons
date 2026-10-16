//! Only the administration transaction can create system tournaments and freeze their prizes.
use super::*;
use crate::{
    economy::{self, Account},
    store::FrozenSlot,
};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Reward {
    Ducats { amount: i64 },
    Avatar { template: u16 },
    Card { card: u16 },
    Booster { booster: String },
}
#[derive(Serialize, Deserialize)]
struct Frozen {
    reward: Reward,
    slots: Option<Vec<FrozenSlot>>,
    content_version: Vec<u8>,
}
impl Reward {
    pub fn cash(&self) -> i64 {
        if let Self::Ducats { amount } = self {
            *amount
        } else {
            0
        }
    }
}
pub async fn create_system(
    db: &mut PgConnection,
    actor: Uuid,
    id: Uuid,
    name: &str,
    terms: Terms,
    reward: Reward,
) -> Result<()> {
    terms.validate()?;
    let name = crate::online::text(name, 60)?;
    if terms.fee != 0 || name.chars().count() < 3 {
        return Err(Error("INVALID_MESSAGE"));
    }
    crate::activity::ensure_available(db).await?;
    let slots = match &reward {
        Reward::Ducats { amount } => {
            if !(1..=1_000_000).contains(amount) {
                return Err(Error("INVALID_MESSAGE"));
            }
            None
        }
        Reward::Avatar { template } => {
            if !engine_content::AVATARS.iter().any(|a| {
                a.id == *template
                    && matches!(
                        a.rarity,
                        engine_content::AvatarRarity::Common
                            | engine_content::AvatarRarity::Uncommon
                            | engine_content::AvatarRarity::Rare
                    )
            }) {
                return Err(Error("INVALID_MESSAGE"));
            }
            None
        }
        Reward::Card { card } => {
            if !engine_content::CARD_META.iter().any(|c| c.id == *card) {
                return Err(Error("INVALID_MESSAGE"));
            }
            None
        }
        Reward::Booster { booster } => Some(crate::store::reward_slots(booster)?),
    };
    if terms.team_size > 1 {
        let count: i64 =
            sqlx::query_scalar("SELECT count(*) FROM clans WHERE id=ANY($1) AND status='active'")
                .bind(&terms.allowed_clans)
                .fetch_one(&mut *db)
                .await?;
        if count != terms.allowed_clans.len() as i64 {
            return Err(Error("INVALID_MESSAGE"));
        }
    }
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tournaments WHERE id=$1)")
        .bind(id)
        .fetch_one(&mut *db)
        .await?;
    if exists {
        return Err(Error("CONFLICT"));
    }
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM tournaments WHERE creator=$1 AND status IN ('registering','running')",
    )
    .bind(actor)
    .fetch_one(&mut *db)
    .await?;
    if count >= 3 {
        return Err(Error("LIMIT"));
    }
    let mut seed = [0; 32];
    getrandom::fill(&mut seed).map_err(|_| Error("INTERNAL"))?;
    let amount = reward.cash();
    let frozen = Frozen {
        reward,
        slots,
        content_version: engine_content::CONTENT_VERSION.to_vec(),
    };
    let bytes = encode(&frozen)?;
    if bytes.len() > 4096 {
        return Err(Error("INVALID_MESSAGE"));
    }
    sqlx::query("INSERT INTO tournaments(id,creator,name,kind,terms,seed,system_reward,engine_version,content_version) VALUES ($1,$2,$3,'system',$4,$5,$6,$7,$8)").bind(id).bind(actor).bind(name).bind(encode(&terms)?).bind(seed.as_slice()).bind(bytes).bind(engine::VERSION).bind(&engine_content::CONTENT_VERSION[..]).execute(&mut *db).await?;
    economy::transfer(
        db,
        Account::Faucet,
        Account::Tournament(id),
        amount,
        "admin",
        id,
    )
    .await?;
    Ok(())
}
pub(super) fn public(bytes: &[u8]) -> Result<Reward> {
    Ok(decode::<Frozen>(bytes)?.reward)
}
pub async fn cancel_system(db: &mut PgConnection, id: Uuid, locked_users: &[Uuid]) -> Result<()> {
    let tournament = locked(db, id).await?;
    if tournament.system_reward.is_none() {
        return Err(Error("FORBIDDEN"));
    }
    if tournament.status != "registering" {
        return Err(Error("CONFLICT"));
    }
    let roster = entries(db, id).await?;
    if roster.iter().any(|e| !locked_users.contains(&e.user_id)) {
        return Err(Error("CONFLICT"));
    }
    repository::refund(db, id, &roster).await
}
pub(super) async fn grant(
    db: &mut PgConnection,
    user: Uuid,
    tournament: Uuid,
    bytes: &[u8],
) -> Result<()> {
    let frozen: Frozen = decode(bytes)?;
    match frozen.reward {
        Reward::Ducats { .. } => {} // Cash was funded into the escrow, and follows the common payout path.
        Reward::Avatar { template } => {
            sqlx::query("INSERT INTO unlocked_templates(user_id,template_id,source) VALUES ($1,$2,'tournament') ON CONFLICT DO NOTHING").bind(user).bind(i32::from(template)).execute(db).await?;
        }
        Reward::Card { card } => {
            sqlx::query("INSERT INTO collection(user_id,card_id,qty) VALUES ($1,$2,1) ON CONFLICT(user_id,card_id) DO UPDATE SET qty=least(collection.qty::bigint+1,2147483647)::integer").bind(user).bind(card as i16).execute(db).await?;
        }
        Reward::Booster { booster } => {
            let slots = frozen.slots.ok_or(Error("INVALID_JOURNAL"))?;
            let mut seed = [0; 32];
            getrandom::fill(&mut seed).map_err(|_| Error("INTERNAL"))?;
            sqlx::query("INSERT INTO boosters(id,user_id,kind,price,pools,seed,content_version,purchase_key,source) VALUES ($1,$2,$3,0,$4,$5,$6,$7,'tournament')").bind(Uuid::now_v7()).bind(user).bind(booster).bind(encode(&slots)?).bind(seed.as_slice()).bind(frozen.content_version).bind(tournament).execute(db).await?;
        }
    }
    Ok(())
}
