//! Idempotent store and junkyard operations, serialized by the owner's database row.
use crate::{
    economy::{self, Account},
    gateway::AppState,
    online::{Error, Result},
};
use axum::{Json, extract::State, http::HeaderMap};
use engine_content::{AvatarRarity, CardRarity};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, PgPool};
use std::{collections::BTreeMap, sync::LazyLock};
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    max_unopened: i64,
    booster: Vec<Booster>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Booster {
    id: String,
    price: u32,
    slots: Vec<Slot>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Slot {
    rarity: String,
    count: u8,
}
static CONFIG: LazyLock<Config> = LazyLock::new(|| {
    let config: Config = toml::from_str(include_str!("../../../content/boosters.toml"))
        .expect("valid booster configuration");
    assert!((1..=1000).contains(&config.max_unopened));
    for (index, booster) in config.booster.iter().enumerate() {
        assert!(
            !booster.id.is_empty() && config.booster[..index].iter().all(|b| b.id != booster.id)
        );
        assert!((1..=1_000_000).contains(&booster.price));
        assert!(
            (1..=20).contains(
                &booster
                    .slots
                    .iter()
                    .map(|s| u32::from(s.count))
                    .sum::<u32>()
            )
        );
        assert!(
            booster
                .slots
                .iter()
                .all(|s| s.count > 0 && rarity(&s.rarity).is_some())
        );
    }
    config
});
fn rarity(value: &str) -> Option<CardRarity> {
    match value {
        "bronze" => Some(CardRarity::Bronze),
        "silver" => Some(CardRarity::Silver),
        "gold" => Some(CardRarity::Gold),
        _ => None,
    }
}
fn quote() -> String {
    let mut hash = Sha256::new();
    hash.update(include_bytes!("../../../content/boosters.toml"));
    hash.update(engine_content::CONTENT_VERSION);
    crate::auth::crypto::hex(&hash.finalize())
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrozenSlot {
    pub rarity: String,
    pub count: u8,
    pub pool: Vec<u16>,
}
#[derive(Serialize)]
pub struct Offer {
    pub id: String,
    pub price: String,
    pub quote: String,
    pub available: bool,
    pub slots: Vec<FrozenSlot>,
}
pub(crate) fn reward_slots(kind: &str) -> Result<Vec<FrozenSlot>> {
    offers()
        .into_iter()
        .find(|o| o.id == kind && o.available)
        .map(|o| o.slots)
        .ok_or(Error("INVALID_MESSAGE"))
}
fn offers() -> Vec<Offer> {
    CONFIG
        .booster
        .iter()
        .map(|booster| {
            let slots: Vec<_> = booster
                .slots
                .iter()
                .map(|slot| FrozenSlot {
                    rarity: slot.rarity.clone(),
                    count: slot.count,
                    pool: engine_content::CARD_META
                        .iter()
                        .filter(|card| {
                            card.in_boosters && Some(card.rarity) == rarity(&slot.rarity)
                        })
                        .map(|card| card.id)
                        .collect(),
                })
                .collect();
            Offer {
                id: booster.id.clone(),
                price: booster.price.to_string(),
                quote: quote(),
                available: slots.iter().all(|s| !s.pool.is_empty()),
                slots,
            }
        })
        .collect()
}
#[derive(Serialize)]
pub struct Inventory {
    pub content_version: String,
    pub offers: Vec<Offer>,
    pub boosters: Vec<OwnedBooster>,
    pub junkyard_points: String,
    pub junkyard_rewards: String,
    pub junkyard_available: bool,
}
#[derive(Serialize)]
pub struct OwnedBooster {
    pub id: Uuid,
    pub kind: String,
    pub status: String,
    pub price: String,
    pub contents: Option<Vec<i16>>,
    pub slots: Vec<FrozenSlot>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stack {
    pub card_id: u16,
    pub qty: u32,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    Buy { kind: String, quote: String },
    Open { id: Uuid },
    Discard { cards: Vec<Stack> },
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub key: Uuid,
    pub operation: Operation,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Receipt {
    pub key: Uuid,
    pub action: String,
    pub booster: Option<Uuid>,
    pub cards: Vec<u16>,
    pub discarded: Vec<Stack>,
    pub points: String,
    pub unlocked: Vec<u16>,
    pub ducats: String,
}
impl Receipt {
    fn new(key: Uuid, action: &str) -> Self {
        Self {
            key,
            action: action.into(),
            booster: None,
            cards: vec![],
            discarded: vec![],
            points: "0".into(),
            unlocked: vec![],
            ducats: "0".into(),
        }
    }
}
pub async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> std::result::Result<Json<Inventory>, crate::game::HttpError> {
    let session = crate::auth::security::session(&state, &headers)
        .await
        .map_err(crate::game::HttpError)?;
    state
        .auth
        .limit("store-read", &session.user.to_string(), 60, 60)
        .map_err(crate::game::HttpError)?;
    inventory(&state.db, session.user)
        .await
        .map(Json)
        .map_err(crate::game::response)
}
pub async fn post(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<Request>,
) -> std::result::Result<Json<Receipt>, crate::game::HttpError> {
    let session = crate::auth::security::session(&state, &headers)
        .await
        .map_err(crate::game::HttpError)?;
    state
        .auth
        .limit("store-write", &session.user.to_string(), 30, 60)
        .map_err(crate::game::HttpError)?;
    execute(&state.db, session.user, request)
        .await
        .map(Json)
        .map_err(crate::game::response)
}
pub async fn inventory(db: &PgPool, user: Uuid) -> Result<Inventory> {
    let rows=sqlx::query!("SELECT id,kind,status,price,contents,pools FROM boosters WHERE user_id=$1 ORDER BY (status='unopened') DESC,id DESC LIMIT 200",user).fetch_all(db).await?;
    let junk = sqlx::query!(
        "SELECT points,rewards_granted FROM junkyard WHERE user_id=$1",
        user
    )
    .fetch_optional(db)
    .await?;
    let boosters = rows
        .into_iter()
        .map(|r| {
            Ok(OwnedBooster {
                id: r.id,
                kind: r.kind,
                status: r.status,
                price: r.price.to_string(),
                contents: r.contents,
                slots: serde_json::from_slice(&r.pools).map_err(|_| Error("INTERNAL"))?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Inventory {
        content_version: crate::auth::crypto::hex(&engine_content::CONTENT_VERSION),
        offers: offers(),
        boosters,
        junkyard_points: junk.as_ref().map_or(0, |r| r.points).to_string(),
        junkyard_rewards: junk.as_ref().map_or(0, |r| r.rewards_granted).to_string(),
        junkyard_available: engine_content::AVATARS
            .iter()
            .filter(|a| a.rarity == AvatarRarity::Junkyard)
            .count()
            == 3,
    })
}
pub async fn execute(db: &PgPool, user: Uuid, request: Request) -> Result<Receipt> {
    let operation = match &request.operation {
        Operation::Buy { .. } => "buy",
        Operation::Open { .. } => "open",
        Operation::Discard { .. } => "discard",
    };
    let hash =
        Sha256::digest(serde_json::to_vec(&request.operation).map_err(|_| Error("INTERNAL"))?);
    let mut tx = db.begin().await?;
    sqlx::query!(
        "SELECT id FROM users WHERE id=$1 AND status='active' FOR UPDATE",
        user
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(Error("UNAUTHENTICATED"))?;
    if let Some(stored) = sqlx::query!(
        "SELECT request_hash,result FROM idempotency_keys WHERE user_id=$1 AND key=$2",
        user,
        request.key
    )
    .fetch_optional(&mut *tx)
    .await?
    {
        if stored.request_hash != hash.as_slice() {
            return Err(Error("CONFLICT"));
        }
        return serde_json::from_slice(&stored.result).map_err(|_| Error("INTERNAL"));
    }
    crate::activity::ensure_idle(&mut tx, user).await?;
    let receipt = match request.operation {
        Operation::Buy {
            kind,
            quote: expected,
        } => buy(&mut tx, user, request.key, &kind, &expected).await?,
        Operation::Open { id } => open(&mut tx, user, request.key, id).await?,
        Operation::Discard { cards } => discard(&mut tx, user, request.key, cards).await?,
    };
    let bytes = serde_json::to_vec(&receipt).map_err(|_| Error("INTERNAL"))?;
    sqlx::query!("INSERT INTO idempotency_keys(user_id,key,operation,request_hash,result) VALUES ($1,$2,$3,$4,$5)",user,request.key,operation,hash.as_slice(),bytes).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(receipt)
}
async fn buy(
    tx: &mut PgConnection,
    user: Uuid,
    key: Uuid,
    kind: &str,
    expected: &str,
) -> Result<Receipt> {
    if expected != quote() {
        return Err(Error("PRICE_CHANGED"));
    }
    let offer = offers()
        .into_iter()
        .find(|offer| offer.id == kind)
        .ok_or(Error("NOT_FOUND"))?;
    if !offer.available {
        return Err(Error("UNAVAILABLE"));
    }
    let count = sqlx::query_scalar!(
        "SELECT count(*) AS \"count!\" FROM boosters WHERE user_id=$1 AND status='unopened'",
        user
    )
    .fetch_one(&mut *tx)
    .await?;
    if count >= CONFIG.max_unopened {
        return Err(Error("INVENTORY_FULL"));
    }
    let id = Uuid::now_v7();
    let mut seed = [0; 32];
    getrandom::fill(&mut seed).map_err(|_| Error("INTERNAL"))?;
    let price = offer.price.parse::<i64>().map_err(|_| Error("INTERNAL"))?;
    let pools = serde_json::to_vec(&offer.slots).map_err(|_| Error("INTERNAL"))?;
    economy::transfer(
        tx,
        Account::Player(user),
        Account::Sink,
        price,
        "booster",
        id,
    )
    .await?;
    sqlx::query!("INSERT INTO boosters(id,user_id,kind,price,pools,seed,content_version,purchase_key) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)",id,user,kind,price,pools,&seed[..],&engine_content::CONTENT_VERSION[..],key).execute(tx).await?;
    let mut receipt = Receipt::new(key, "buy");
    receipt.booster = Some(id);
    receipt.ducats = (-price).to_string();
    Ok(receipt)
}
fn uniform(seed: &[u8; 32], stream: u32, bound: usize) -> Result<usize> {
    let bound = u64::try_from(bound).map_err(|_| Error("INTERNAL"))?;
    if bound == 0 {
        return Err(Error("INTERNAL"));
    }
    let threshold = bound.wrapping_neg() % bound;
    for attempt in 0u32..128 {
        let mut hash = Sha256::new();
        hash.update(b"spellgambit:booster-draw:v1");
        hash.update(seed);
        hash.update(stream.to_le_bytes());
        hash.update(attempt.to_le_bytes());
        let bytes: [u8; 32] = hash.finalize().into();
        let value = u64::from_le_bytes(bytes[..8].try_into().expect("digest"));
        if value >= threshold {
            return Ok((value % bound) as usize);
        }
    }
    Err(Error("INTERNAL"))
}
fn draw(seed: &[u8; 32], slots: &[FrozenSlot]) -> Result<Vec<u16>> {
    let mut cards = Vec::new();
    for slot in slots {
        if slot.count == 0 || slot.pool.is_empty() {
            return Err(Error("INTERNAL"));
        }
        for _ in 0..slot.count {
            if cards.len() >= 20 {
                return Err(Error("INTERNAL"));
            }
            cards.push(slot.pool[uniform(seed, cards.len() as u32, slot.pool.len())?]);
        }
    }
    if cards.is_empty() {
        return Err(Error("INTERNAL"));
    }
    Ok(cards)
}
async fn open(tx: &mut PgConnection, user: Uuid, key: Uuid, id: Uuid) -> Result<Receipt> {
    let booster = sqlx::query!(
        "SELECT status,pools,seed,contents FROM boosters WHERE id=$1 AND user_id=$2 FOR UPDATE",
        id,
        user
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(Error("NOT_FOUND"))?;
    let mut receipt = Receipt::new(key, "open");
    receipt.booster = Some(id);
    if booster.status == "opened" {
        receipt.cards = booster
            .contents
            .unwrap_or_default()
            .into_iter()
            .map(|id| id as u16)
            .collect();
        return Ok(receipt);
    }
    let slots: Vec<FrozenSlot> =
        serde_json::from_slice(&booster.pools).map_err(|_| Error("INTERNAL"))?;
    receipt.cards = draw(
        &booster.seed.try_into().map_err(|_| Error("INTERNAL"))?,
        &slots,
    )?;
    let mut counts = BTreeMap::<i16, i32>::new();
    for id in &receipt.cards {
        if engine_content::content().get(*id).is_none() {
            return Err(Error("CONTENT_MISMATCH"));
        }
        *counts
            .entry(i16::try_from(*id).map_err(|_| Error("CONTENT_MISMATCH"))?)
            .or_default() += 1;
    }
    for (card, qty) in counts {
        let changed=sqlx::query!("INSERT INTO collection(user_id,card_id,qty) VALUES ($1,$2,$3) ON CONFLICT(user_id,card_id) DO UPDATE SET qty=collection.qty+$3 WHERE collection.qty<=$4",user,card,qty,i32::MAX-qty).execute(&mut *tx).await?;
        if changed.rows_affected() != 1 {
            return Err(Error("CARD_LIMIT"));
        }
    }
    let contents: Vec<_> = receipt.cards.iter().map(|id| *id as i16).collect();
    sqlx::query!(
        "UPDATE boosters SET status='opened',contents=$2,opened_at=now() WHERE id=$1",
        id,
        &contents
    )
    .execute(tx)
    .await?;
    Ok(receipt)
}
async fn discard(
    tx: &mut PgConnection,
    user: Uuid,
    key: Uuid,
    cards: Vec<Stack>,
) -> Result<Receipt> {
    let special: Vec<_> = engine_content::AVATARS
        .iter()
        .filter(|a| a.rarity == AvatarRarity::Junkyard)
        .map(|a| a.id)
        .collect();
    if special.len() != 3 {
        return Err(Error("UNAVAILABLE"));
    }
    if cards.is_empty() || cards.len() > 200 {
        return Err(Error("INVALID_MESSAGE"));
    }
    let rules = &crate::rules::RULES.junkyard;
    let mut counts = BTreeMap::<u16, u64>::new();
    for stack in cards {
        if stack.qty == 0 {
            return Err(Error("INVALID_MESSAGE"));
        }
        let qty = counts.entry(stack.card_id).or_default();
        *qty += u64::from(stack.qty);
        if *qty > i32::MAX as u64 {
            return Err(Error("INVALID_MESSAGE"));
        }
    }
    let mut points = 0i64;
    let mut receipt = Receipt::new(key, "discard");
    for (card, qty) in counts {
        let metadata = engine_content::CARD_META
            .iter()
            .find(|c| c.id == card)
            .ok_or(Error("NOT_FOUND"))?;
        let index = match metadata.rarity {
            CardRarity::Bronze => 0,
            CardRarity::Silver => 1,
            CardRarity::Gold => 2,
        };
        points = points
            .checked_add((qty as i64) * i64::from(rules.rarity_points[index]))
            .ok_or(Error("BALANCE_LIMIT"))?;
        let changed = sqlx::query!(
            "UPDATE collection SET qty=qty-$3 WHERE user_id=$1 AND card_id=$2 AND qty>=$3",
            user,
            card as i16,
            qty as i32
        )
        .execute(&mut *tx)
        .await?;
        if changed.rows_affected() != 1 {
            return Err(Error("INSUFFICIENT_CARDS"));
        }
        receipt.discarded.push(Stack {
            card_id: card,
            qty: qty as u32,
        });
    }
    sqlx::query!(
        "INSERT INTO junkyard(user_id) VALUES ($1) ON CONFLICT DO NOTHING",
        user
    )
    .execute(&mut *tx)
    .await?;
    let previous = sqlx::query!(
        "SELECT points,rewards_granted FROM junkyard WHERE user_id=$1 FOR UPDATE",
        user
    )
    .fetch_one(&mut *tx)
    .await?;
    let total = previous
        .points
        .checked_add(points)
        .ok_or(Error("BALANCE_LIMIT"))?;
    let granted = total / i64::from(rules.reward_threshold);
    let mut new_rewards = granted - previous.rewards_granted;
    if new_rewards < 0 {
        return Err(Error("CONFLICT"));
    }
    let owned = sqlx::query_scalar!(
        "SELECT template_id FROM unlocked_templates WHERE user_id=$1",
        user
    )
    .fetch_all(&mut *tx)
    .await?;
    let mut missing: Vec<_> = special
        .into_iter()
        .filter(|id| !owned.contains(&i32::from(*id)))
        .collect();
    let mut seed = [0; 32];
    getrandom::fill(&mut seed).map_err(|_| Error("INTERNAL"))?;
    while new_rewards > 0 && !missing.is_empty() {
        let index = uniform(&seed, receipt.unlocked.len() as u32, missing.len())?;
        let id = missing.remove(index);
        sqlx::query!(
            "INSERT INTO unlocked_templates(user_id,template_id,source) VALUES ($1,$2,'junkyard')",
            user,
            i32::from(id)
        )
        .execute(&mut *tx)
        .await?;
        receipt.unlocked.push(id);
        new_rewards -= 1;
    }
    let ducats = new_rewards
        .checked_mul(i64::from(rules.fallback_ducats))
        .ok_or(Error("BALANCE_LIMIT"))?;
    economy::transfer(
        tx,
        Account::Faucet,
        Account::Player(user),
        ducats,
        "junkyard",
        key,
    )
    .await?;
    sqlx::query!(
        "UPDATE junkyard SET points=$2,rewards_granted=$3 WHERE user_id=$1",
        user,
        total,
        granted
    )
    .execute(&mut *tx)
    .await?;
    crate::game::prune(tx, user).await?;
    receipt.points = points.to_string();
    receipt.ducats = ducats.to_string();
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn basic_pack_has_declared_rarities_and_uniform_pools() {
        let basic = offers().into_iter().find(|o| o.id == "basic").unwrap();
        assert!(basic.available);
        let mut counts = [
            BTreeMap::<u16, u32>::new(),
            BTreeMap::new(),
            BTreeMap::new(),
        ];
        for value in 0u32..10_000 {
            let mut seed = [0; 32];
            seed[..4].copy_from_slice(&value.to_le_bytes());
            let cards = draw(&seed, &basic.slots).unwrap();
            assert_eq!(cards.len(), 12);
            let mut rarities = [0; 3];
            for id in &cards {
                let rarity = engine_content::CARD_META
                    .iter()
                    .find(|m| m.id == *id)
                    .unwrap()
                    .rarity;
                let index = match rarity {
                    CardRarity::Bronze => 0,
                    CardRarity::Silver => 1,
                    CardRarity::Gold => 2,
                };
                rarities[index] += 1;
                *counts[index].entry(*id).or_default() += 1;
            }
            assert_eq!(rarities, [8, 3, 1]);

            assert_eq!(cards, draw(&seed, &basic.slots).unwrap());
        }
        for (index, rarity) in ["bronze", "silver", "gold"].iter().enumerate() {
            let slot = basic.slots.iter().find(|s| s.rarity == *rarity).unwrap();
            let counts = &counts[index];
            assert_eq!(counts.len(), slot.pool.len());
            let expected = 10_000.0 * f64::from(slot.count) / counts.len() as f64;
            let chi_squared: f64 = counts
                .values()
                .map(|n| (f64::from(*n) - expected).powi(2) / expected)
                .sum();
            let degrees = (counts.len() - 1) as f64;
            assert!(
                chi_squared < degrees + 8.0 * (2.0 * degrees).sqrt() + 20.0,
                "{rarity}: chi2={chi_squared}, df={degrees}"
            );
        }
    }
}
