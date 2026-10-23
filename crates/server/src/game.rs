//! Character and deck operations. Call mutations through Hub to serialize pending challenges.
use crate::{
    gateway::AppState,
    online::{Error, Result},
};
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, PgPool};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Serialize)]
pub struct Snapshot {
    pub content_version: String,
    pub recruitment: crate::recruitment::Snapshot,
    pub advancement: crate::advancement::Snapshot,
    pub templates: Vec<i32>,
    pub avatars: Vec<Avatar>,
    pub collection: Vec<Stack>,
    pub presets: Vec<Preset>,
}
#[derive(Serialize)]
pub struct Avatar {
    pub id: Uuid,
    pub slot: i16,
    pub template_id: i32,
    pub level: i16,
    pub xp: i32,
    pub tier: i16,
    pub advancement: crate::advancement::Character,
    pub archived_decks: Vec<Vec<i16>>,
    pub path_id: Option<i32>,
    pub specialization: Option<String>,
    pub gift_id: Option<i32>,
    pub is_shade: bool,
    pub shade_school: Option<String>,
    pub active: bool,
    pub revision: i32,
    pub unspent: i16,
    pub spent: [i16; 6],
    pub stats: [i32; 6],
    pub schools: [u8; 6],
    pub deck: Vec<i16>,
    pub deck_revision: i32,
}
#[derive(Serialize)]
pub struct Stack {
    pub card_id: i16,
    pub qty: i32,
}
#[derive(Serialize)]
pub struct Preset {
    pub id: Uuid,
    pub name: String,
    pub cards: Vec<i16>,
    pub revision: i32,
}

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum Mutation {
    Contract {
        template_id: u16,
        claim: bool,
    },
    Recruit {
        level: u8,
        template_id: Option<u16>,
    },
    Reincarnate {
        id: Uuid,
        revision: i32,
        choice: crate::progression::Choice,
        sacrifice: Option<Uuid>,
    },
    Shade {
        id: Uuid,
        revision: i32,
        new_id: Uuid,
        school: String,
    },
    Create {
        id: Uuid,
        template_id: u16,
    },
    Delete {
        id: Uuid,
        revision: i32,
    },
    Activate {
        id: Uuid,
    },
    Spend {
        id: Uuid,
        revision: i32,
        points: [u16; 6],
    },
    Deck {
        id: Uuid,
        revision: i32,
        cards: Vec<u16>,
    },
    PresetSave {
        id: Uuid,
        revision: Option<i32>,
        name: String,
        cards: Vec<u16>,
    },
    PresetDelete {
        id: Uuid,
        revision: i32,
    },
}
pub struct HttpError(pub(crate) crate::auth::Error);
impl IntoResponse for HttpError {
    fn into_response(self) -> Response {
        self.0.into_response()
    }
}
pub(crate) fn response(error: Error) -> HttpError {
    let status = match error.0 {
        "UNAUTHENTICATED" | "STEP_UP_REQUIRED" | "INVALID_OTP" | "INVALID_CREDENTIALS" => {
            StatusCode::UNAUTHORIZED
        }
        "FORBIDDEN" | "LEVEL_REQUIRED" | "TOTP_SETUP_REQUIRED" => StatusCode::FORBIDDEN,
        "NOT_FOUND" => StatusCode::NOT_FOUND,
        "BUSY"
        | "CONFLICT"
        | "PRICE_CHANGED"
        | "SLOTS_FULL"
        | "ACTIVE_AVATAR"
        | "PRESETS_FULL"
        | "ALREADY_MEMBER"
        | "MASTER_REQUIRED"
        | "NAME_TAKEN"
        | "CLAN_FULL"
        | "APPLICATION_LIMIT"
        | "INELIGIBLE"
        | "FULL"
        | "LIMIT"
        | "RECRUIT_UNAVAILABLE" => StatusCode::CONFLICT,
        "INTERNAL" | "TIMEOUT" | "UNAVAILABLE" => StatusCode::SERVICE_UNAVAILABLE,
        _ => StatusCode::BAD_REQUEST,
    };
    HttpError(crate::auth::Error(status, error.0, 0))
}
pub async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> std::result::Result<Json<Snapshot>, HttpError> {
    let session = crate::auth::security::session(&state, &headers)
        .await
        .map_err(HttpError)?;
    state
        .auth
        .limit("game-read", &session.user.to_string(), 60, 60)
        .map_err(HttpError)?;
    let mut tx = state.db.begin().await.map_err(|e| response(e.into()))?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await
        .map_err(|e| response(e.into()))?;
    let data = load(&mut tx, session.user).await.map_err(response)?;
    tx.commit().await.map_err(|e| response(e.into()))?;
    Ok(Json(data))
}
pub async fn post(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(mutation): Json<Mutation>,
) -> std::result::Result<Json<Snapshot>, HttpError> {
    let session = crate::auth::security::session(&state, &headers)
        .await
        .map_err(HttpError)?;
    state
        .auth
        .limit("game-write", &session.user.to_string(), 60, 60)
        .map_err(HttpError)?;
    state
        .online
        .hub
        .game_mutation(session.user, mutation)
        .await
        .map(Json)
        .map_err(response)
}
pub async fn load(connection: &mut PgConnection, user: Uuid) -> Result<Snapshot> {
    let mut templates = sqlx::query_scalar!(
        "SELECT template_id FROM unlocked_templates WHERE user_id=$1 ORDER BY template_id",
        user
    )
    .fetch_all(&mut *connection)
    .await?;
    templates.retain(|id| u16::try_from(*id).is_ok_and(engine_content::is_playable));
    let rows=sqlx::query!("SELECT a.id,a.slot,a.template_id,a.path_id,a.specialization,a.gift_id,a.is_shade,a.shade_school,a.level,a.xp,a.tier,a.is_active,a.revision,a.dev_points_unspent,a.dp_phys_atk,a.dp_phys_def,a.dp_mag_atk,a.dp_mag_def,a.dp_hp,a.dp_stamina,COALESCE(d.cards,'{}'::smallint[]) AS \"cards!\",COALESCE(d.revision,0) AS \"deck_revision!\" FROM avatars a LEFT JOIN decks d ON d.avatar_id=a.id WHERE a.user_id=$1 AND a.deleted_at IS NULL ORDER BY a.slot",user).fetch_all(&mut *connection).await?;
    let mut avatars = Vec::new();
    for a in rows {
        let def = engine_content::avatar(
            u16::try_from(a.path_id.unwrap_or(a.template_id))
                .map_err(|_| Error("CONTENT_MISMATCH"))?,
        )
        .ok_or(Error("CONTENT_MISMATCH"))?;
        let (mut stats, schools) = def.at_level(a.level as u8);
        let spent = [
            a.dp_phys_atk,
            a.dp_phys_def,
            a.dp_mag_atk,
            a.dp_mag_def,
            a.dp_hp,
            a.dp_stamina,
        ];
        stats.phys_atk += i32::from(spent[0]);
        stats.phys_def += i32::from(spent[1]);
        stats.mag_atk += i32::from(spent[2]);
        stats.mag_def += i32::from(spent[3]);
        stats.hp_max += 7 * i32::from(spent[4]);
        stats.stamina_max += 7 * i32::from(spent[5]);
        let advancement = crate::advancement::character(connection, a.id).await?;
        let archived_decks = sqlx::query_scalar(
            "SELECT cards FROM incarnation_decks WHERE avatar_id=$1 ORDER BY tier",
        )
        .bind(a.id)
        .fetch_all(&mut *connection)
        .await?;
        avatars.push(Avatar {
            id: a.id,
            slot: a.slot,
            template_id: a.template_id,
            level: a.level,
            xp: a.xp,
            tier: a.tier,
            path_id: a.path_id,
            specialization: a.specialization,
            gift_id: a.gift_id,
            is_shade: a.is_shade,
            shade_school: a.shade_school,
            active: a.is_active,
            revision: a.revision,
            unspent: a.dev_points_unspent + advancement.permanent_unspent,
            advancement,
            archived_decks,
            spent,
            stats: engine::Stat::ALL.map(|s| stats.get(s)),
            schools: schools.0,
            deck: a.cards,
            deck_revision: a.deck_revision,
        });
    }
    let collection = sqlx::query_as!(
        Stack,
        "SELECT card_id,qty FROM collection WHERE user_id=$1 AND qty>0 ORDER BY card_id",
        user
    )
    .fetch_all(&mut *connection)
    .await?;
    let presets = sqlx::query_as!(
        Preset,
        "SELECT id,name,cards,revision FROM deck_presets WHERE user_id=$1 ORDER BY name,id",
        user
    )
    .fetch_all(&mut *connection)
    .await?;
    let recruitment = crate::recruitment::load(connection, user, &templates).await?;
    Ok(Snapshot {
        content_version: crate::auth::crypto::hex(&engine_content::CONTENT_VERSION),
        recruitment,
        advancement: crate::advancement::load(connection, user).await?,
        templates,
        avatars,
        collection,
        presets,
    })
}
pub async fn mutate(db: &PgPool, user: Uuid, mutation: Mutation) -> Result<Snapshot> {
    let mut tx = db.begin().await?;
    sqlx::query!(
        "SELECT id FROM users WHERE id=$1 AND status='active' FOR UPDATE",
        user
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(Error("UNAUTHENTICATED"))?;
    crate::activity::ensure_idle(&mut tx, user).await?;
    let mut snapshot = load(&mut tx, user).await?;
    match mutation {
        Mutation::Contract { template_id, claim } => {
            crate::advancement::contract(&mut tx, user, &snapshot, template_id, claim).await?
        }
        Mutation::Recruit { level, template_id } => {
            crate::recruitment::claim(&mut tx, user, &snapshot, level, template_id).await?
        }
        Mutation::Reincarnate {
            id,
            revision,
            choice,
            sacrifice,
        } => {
            crate::progression::reincarnate(&mut tx, user, id, revision, choice, sacrifice).await?
        }
        Mutation::Shade {
            id,
            revision,
            new_id,
            school,
        } => crate::progression::create_shade(&mut tx, user, id, revision, new_id, &school).await?,
        Mutation::Create { id, template_id } => {
            if !engine_content::AVATARS.iter().any(|a| a.id == template_id) {
                return Err(Error("NOT_FOUND"));
            }
            if !engine_content::is_playable(template_id) {
                return Err(Error("AVATAR_LOCKED"));
            }
            if !snapshot.templates.contains(&i32::from(template_id))
                || !engine_content::AVATARS
                    .iter()
                    .any(|a| a.id == template_id && a.rarity != engine_content::AvatarRarity::Shade)
            {
                return Err(Error("NOT_FOUND"));
            }
            if let Some(avatar) = snapshot.avatars.iter().find(|a| a.id == id) {
                if avatar.template_id != i32::from(template_id) {
                    return Err(Error("CONFLICT"));
                }
            } else {
                create_avatar(&mut tx, user, &snapshot, id, template_id).await?;
            }
        }
        Mutation::Activate { id } => {
            let avatar = find(&snapshot, id)?;
            ensure_playable(avatar.template_id)?;
            if !avatar.active {
                sqlx::query!("UPDATE avatars SET is_active=false,revision=revision+1 WHERE user_id=$1 AND is_active",user).execute(&mut *tx).await?;
                sqlx::query!("UPDATE avatars SET is_active=true,revision=revision+1 WHERE id=$1 AND user_id=$2 AND deleted_at IS NULL",id,user).execute(&mut *tx).await?;
            }
        }
        Mutation::Delete { id, revision } => {
            let avatar = find(&snapshot, id)?;
            version(avatar.revision, revision)?;
            if avatar.active {
                return Err(Error("ACTIVE_AVATAR"));
            }
            sqlx::query!("UPDATE avatars SET deleted_at=now(),revision=revision+1 WHERE id=$1 AND user_id=$2",id,user).execute(&mut *tx).await?;
        }
        Mutation::Spend {
            id,
            revision,
            points,
        } => {
            let avatar = find(&snapshot, id)?;
            ensure_playable(avatar.template_id)?;
            version(avatar.revision, revision)?;
            let sum: u32 = points.map(u32::from).iter().sum();
            if sum == 0 || sum > avatar.unspent as u32 {
                return Err(Error("INVALID_POINTS"));
            }
            let mut added = [0i16; 6];
            for i in 0..6 {
                added[i] = avatar.spent[i]
                    .checked_add(i16::try_from(points[i]).map_err(|_| Error("INVALID_POINTS"))?)
                    .ok_or(Error("INVALID_POINTS"))?;
            }
            let permanent = (sum as i16).min(avatar.advancement.permanent_unspent);
            sqlx::query("UPDATE avatar_advancement SET permanent_unspent=permanent_unspent-$2 WHERE avatar_id=$1").bind(id).bind(permanent).execute(&mut *tx).await?;
            sqlx::query!("UPDATE avatars SET dev_points_unspent=dev_points_unspent-$3,dp_phys_atk=$4,dp_phys_def=$5,dp_mag_atk=$6,dp_mag_def=$7,dp_hp=$8,dp_stamina=$9,revision=revision+1 WHERE id=$1 AND user_id=$2",id,user,sum as i16-permanent,added[0],added[1],added[2],added[3],added[4],added[5]).execute(&mut *tx).await?;
        }
        Mutation::Deck {
            id,
            revision,
            cards,
        } => {
            crate::advancement::require(&mut tx, user, 2).await?;
            let avatar = find(&snapshot, id)?;
            ensure_playable(avatar.template_id)?;
            version(avatar.deck_revision, revision)?;
            let cards = validate_deck(&cards, &snapshot.collection)?;
            sqlx::query!("INSERT INTO decks(avatar_id,cards,revision) VALUES ($1,$2,1) ON CONFLICT(avatar_id) DO UPDATE SET cards=$2,revision=decks.revision+1,updated_at=now()",id,&cards).execute(&mut *tx).await?;
        }
        Mutation::PresetSave {
            id,
            revision,
            name,
            cards,
        } => {
            crate::advancement::require(&mut tx, user, 7).await?;
            let name = crate::online::text(&name, 40)?;
            let cards = validate_deck(&cards, &snapshot.collection)?;
            if let Some(preset) = snapshot.presets.iter().find(|p| p.id == id) {
                version(preset.revision, revision.ok_or(Error("CONFLICT"))?)?;
                if snapshot
                    .presets
                    .iter()
                    .any(|p| p.id != id && p.name == name)
                {
                    return Err(Error("CONFLICT"));
                }
                sqlx::query!("UPDATE deck_presets SET name=$3,cards=$4,revision=revision+1,updated_at=now() WHERE id=$1 AND user_id=$2",id,user,name,&cards).execute(&mut *tx).await?;
            } else {
                if revision.is_some() || snapshot.presets.iter().any(|p| p.name == name) {
                    return Err(Error("CONFLICT"));
                }
                if snapshot.presets.len()
                    >= usize::from(engine_content::rules().deck.saved_presets_max)
                {
                    return Err(Error("PRESETS_FULL"));
                }
                sqlx::query!(
                    "INSERT INTO deck_presets(id,user_id,name,cards) VALUES ($1,$2,$3,$4)",
                    id,
                    user,
                    name,
                    &cards
                )
                .execute(&mut *tx)
                .await?;
            }
        }
        Mutation::PresetDelete { id, revision } => {
            version(
                snapshot
                    .presets
                    .iter()
                    .find(|p| p.id == id)
                    .ok_or(Error("NOT_FOUND"))?
                    .revision,
                revision,
            )?;
            sqlx::query!(
                "DELETE FROM deck_presets WHERE id=$1 AND user_id=$2",
                id,
                user
            )
            .execute(&mut *tx)
            .await?;
        }
    }
    snapshot = load(&mut tx, user).await?;
    tx.commit().await?;
    Ok(snapshot)
}

pub(crate) async fn create_avatar(
    tx: &mut PgConnection,
    user: Uuid,
    snapshot: &Snapshot,
    id: Uuid,
    template: u16,
) -> Result<()> {
    let slot = (0..10)
        .find(|slot| snapshot.avatars.iter().all(|a| a.slot != *slot))
        .ok_or(Error("SLOTS_FULL"))?;
    sqlx::query!(
        "INSERT INTO avatars(id,user_id,slot,template_id,is_active) VALUES ($1,$2,$3,$4,$5)",
        id,
        user,
        slot,
        i32::from(template),
        snapshot.avatars.is_empty()
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!("INSERT INTO decks(avatar_id) VALUES ($1)", id)
        .execute(&mut *tx)
        .await?;
    grant_starter(tx, user, id, template).await
}

pub(crate) fn ensure_playable(template: i32) -> Result<()> {
    if u16::try_from(template).is_ok_and(engine_content::is_playable) {
        Ok(())
    } else {
        Err(Error("AVATAR_LOCKED"))
    }
}

/// The owner row is locked by the caller; receipts prevent delete/create pack farming.
async fn grant_starter(
    tx: &mut PgConnection,
    user: Uuid,
    avatar: Uuid,
    template: u16,
) -> Result<()> {
    let starter = engine_content::starter(template).ok_or(Error("CONTENT_MISMATCH"))?;
    let claimed = sqlx::query(
        "INSERT INTO starter_claims(user_id,template_id) VALUES($1,$2) ON CONFLICT DO NOTHING",
    )
    .bind(user)
    .bind(i32::from(template))
    .execute(&mut *tx)
    .await?
    .rows_affected()
        == 1;
    if claimed {
        for &(id, count) in starter.cards {
            sqlx::query("INSERT INTO collection(user_id,card_id,qty) VALUES($1,$2,$3) ON CONFLICT(user_id,card_id) DO UPDATE SET qty=least(collection.qty::bigint+EXCLUDED.qty,2147483647)::integer")
                .bind(user).bind(id as i16).bind(i32::from(count)).execute(&mut *tx).await?;
        }
    }
    let mut deck = Vec::new();
    for id in starter.deck {
        let qty: i32 =
            sqlx::query_scalar("SELECT qty FROM collection WHERE user_id=$1 AND card_id=$2")
                .bind(user)
                .bind(*id as i16)
                .fetch_optional(&mut *tx)
                .await?
                .unwrap_or(0);
        if deck.iter().filter(|card| **card == *id as i16).count() < qty as usize {
            deck.push(*id as i16);
        }
    }
    sqlx::query("UPDATE decks SET cards=$2,revision=revision+1 WHERE avatar_id=$1")
        .bind(avatar)
        .bind(deck)
        .execute(tx)
        .await?;
    Ok(())
}
fn find(snapshot: &Snapshot, id: Uuid) -> Result<&Avatar> {
    snapshot
        .avatars
        .iter()
        .find(|a| a.id == id)
        .ok_or(Error("NOT_FOUND"))
}
fn version(actual: i32, expected: i32) -> Result<()> {
    if actual == expected {
        Ok(())
    } else {
        Err(Error("CONFLICT"))
    }
}
pub fn validate_deck(cards: &[u16], collection: &[Stack]) -> Result<Vec<i16>> {
    let rules = engine_content::rules().deck;
    if cards.len() > usize::from(rules.size) {
        return Err(Error("INVALID_DECK"));
    }
    let mut counts = BTreeMap::<u16, usize>::new();
    for id in cards {
        if engine_content::content().get(*id).is_none() {
            return Err(Error("INVALID_DECK"));
        }
        let copies = counts.entry(*id).or_default();
        *copies += 1;
        let qty = collection
            .iter()
            .find(|s| i32::from(s.card_id) == i32::from(*id))
            .map_or(0, |s| s.qty);
        if *copies > usize::from(rules.max_copies) || *copies > qty.max(0) as usize {
            return Err(Error("INVALID_DECK"));
        }
    }
    cards
        .iter()
        .map(|id| i16::try_from(*id).map_err(|_| Error("INVALID_DECK")))
        .collect()
}
pub fn trim_deck(cards: &[i16], collection: &[Stack]) -> Vec<i16> {
    let mut held = BTreeMap::<i16, usize>::new();
    cards
        .iter()
        .copied()
        .filter(|id| {
            let qty = collection
                .iter()
                .find(|s| s.card_id == *id)
                .map_or(0, |s| s.qty)
                .max(0) as usize;
            let count = held.entry(*id).or_default();
            *count += 1;
            *count <= qty.min(usize::from(engine_content::rules().deck.max_copies))
                && u16::try_from(*id)
                    .ok()
                    .is_some_and(|id| engine_content::content().get(id).is_some())
        })
        .take(usize::from(engine_content::rules().deck.size))
        .collect()
}
/// Future economy operations call this inside their existing user-locked transaction.
pub async fn prune(connection: &mut PgConnection, user: Uuid) -> Result<()> {
    let snapshot = load(connection, user).await?;
    for avatar in snapshot.avatars {
        let cards = trim_deck(&avatar.deck, &snapshot.collection);
        if cards != avatar.deck {
            sqlx::query!(
                "UPDATE decks SET cards=$2,revision=revision+1,updated_at=now() WHERE avatar_id=$1",
                avatar.id,
                &cards
            )
            .execute(&mut *connection)
            .await?;
        }
    }
    for preset in snapshot.presets {
        let cards = trim_deck(&preset.cards, &snapshot.collection);
        if cards != preset.cards {
            sqlx::query!("UPDATE deck_presets SET cards=$2,revision=revision+1,updated_at=now() WHERE id=$1 AND user_id=$3",preset.id,&cards,user).execute(&mut *connection).await?;
        }
    }
    Ok(())
}
