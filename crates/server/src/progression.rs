//! Reincarnation and sacrifices. Caller holds the owner's row and has checked activity locks.
use crate::online::{Error, Result};
use engine_content::AvatarRarity;
use serde::Deserialize;
use sqlx::PgConnection;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Choice {
    Path { id: u16 },
    Specialization { name: String },
    Gift { card_id: u16 },
}
#[derive(sqlx::FromRow)]
struct Avatar {
    id: Uuid,
    slot: i16,
    template_id: i32,
    path_id: Option<i32>,
    level: i16,
    xp: i32,
    tier: i16,
    revision: i32,
    is_active: bool,
    is_shade: bool,
    shade_school: Option<String>,
    specialization: Option<String>,
    gift_id: Option<i32>,
}
async fn load(tx: &mut PgConnection, user: Uuid, id: Uuid) -> Result<Avatar> {
    sqlx::query_as("SELECT id,slot,template_id,path_id,level,xp,tier,revision,is_active,is_shade,shade_school,specialization,gift_id FROM avatars WHERE id=$1 AND user_id=$2 AND deleted_at IS NULL FOR UPDATE").bind(id).bind(user).fetch_optional(tx).await?.ok_or(Error("NOT_FOUND"))
}
fn ready(avatar: &Avatar, revision: i32) -> Result<()> {
    crate::game::ensure_playable(avatar.template_id)?;
    if avatar.revision != revision {
        return Err(Error("CONFLICT"));
    }
    if avatar.level != 20 {
        return Err(Error("NOT_READY"));
    }
    profile(avatar)?;
    Ok(())
}
fn profile(avatar: &Avatar) -> Result<&'static engine_content::AvatarDef> {
    let base = u16::try_from(avatar.template_id)
        .ok()
        .and_then(|id| engine_content::AVATARS.iter().find(|a| a.id == id))
        .ok_or(Error("CONTENT_MISMATCH"))?;
    if avatar.is_shade != (base.rarity == AvatarRarity::Shade)
        || match avatar.shade_school.as_deref() {
            Some(name) => !avatar.is_shade || base.schools.level(parse_school(name)?) != 1,
            None => avatar.is_shade,
        }
    {
        return Err(Error("CONTENT_MISMATCH"));
    }
    if avatar.tier == 0 {
        return if avatar.path_id.is_none()
            && avatar.specialization.is_none()
            && avatar.gift_id.is_none()
        {
            Ok(base)
        } else {
            Err(Error("CONTENT_MISMATCH"))
        };
    }
    let path = avatar
        .path_id
        .and_then(|id| u16::try_from(id).ok())
        .and_then(engine_content::path)
        .filter(|p| i32::from(p.base_avatar) == avatar.template_id)
        .ok_or(Error("CONTENT_MISMATCH"))?;
    let valid_spec = avatar.specialization.as_deref().is_some_and(|name| {
        path.specializations
            .iter()
            .any(|s| specialization_name(*s) == name)
    });
    let valid_gift = avatar
        .gift_id
        .and_then(|id| u16::try_from(id).ok())
        .is_some_and(|id| path.gifts.contains(&id));
    if !(1..=3).contains(&avatar.tier)
        || if avatar.tier >= 2 {
            !valid_spec
        } else {
            avatar.specialization.is_some()
        }
        || if avatar.tier == 3 {
            !valid_gift
        } else {
            avatar.gift_id.is_some()
        }
    {
        return Err(Error("CONTENT_MISMATCH"));
    }
    Ok(&path.avatar)
}
pub fn specialization_name(value: engine::Specialization) -> &'static str {
    engine_content::specialization_key(value)
}
pub fn school_name(value: engine::School) -> &'static str {
    match value {
        engine::School::Combat => "combat",
        engine::School::Divine => "divine",
        engine::School::Element => "element",
        engine::School::Arcane => "arcane",
        engine::School::Necro => "necro",
        engine::School::Tech => "tech",
    }
}
fn parse_school(value: &str) -> Result<engine::School> {
    engine::SCHOOLS
        .into_iter()
        .find(|s| school_name(*s) == value)
        .ok_or(Error("INVALID_CHOICE"))
}
async fn consume_donor(
    tx: &mut PgConnection,
    user: Uuid,
    avatar: &Avatar,
    donor: Option<Uuid>,
) -> Result<bool> {
    let id = donor.ok_or(Error("DONOR_REQUIRED"))?;
    if id == avatar.id {
        return Err(Error("INVALID_DONOR"));
    }
    let donor = load(tx, user, id).await?;
    let def = profile(&donor)?;
    let school = parse_school(
        avatar
            .shade_school
            .as_deref()
            .ok_or(Error("CONTENT_MISMATCH"))?,
    )?;
    if donor.tier != avatar.tier + 1 || def.at_level(donor.level as u8).1.level(school) > 0 {
        return Err(Error("INVALID_DONOR"));
    }
    sqlx::query(
        "UPDATE avatars SET deleted_at=now(),is_active=false,revision=revision+1 WHERE id=$1",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("INSERT INTO avatar_sacrifices(consumed_avatar,beneficiary_avatar,user_id,kind,content_version) VALUES ($1,$2,$3,'shade_reincarnate',$4)").bind(id).bind(avatar.id).bind(user).bind(&engine_content::CONTENT_VERSION[..]).execute(tx).await?;
    Ok(donor.is_active)
}
pub async fn reincarnate(
    tx: &mut PgConnection,
    user: Uuid,
    id: Uuid,
    revision: i32,
    choice: Choice,
    donor: Option<Uuid>,
) -> Result<()> {
    let avatar = load(tx, user, id).await?;
    ready(&avatar, revision)?;
    if !(0..2).contains(&avatar.tier) {
        return Err(Error("FINAL_TIER"));
    }
    crate::advancement::require(tx, user, if avatar.tier == 0 { 12 } else { 18 }).await?;
    if crate::advancement::character(tx, id).await?.trial_wins < 3 {
        return Err(Error("TRIAL_REQUIRED"));
    }
    let mut path_id = avatar.path_id;
    let mut specialization = avatar.specialization.clone();
    let mut gift = avatar.gift_id;
    match (avatar.tier, choice) {
        (0, Choice::Path { id }) => {
            let path = engine_content::path(id)
                .filter(|p| i32::from(p.base_avatar) == avatar.template_id)
                .ok_or(Error("INVALID_CHOICE"))?;
            path_id = Some(i32::from(path.avatar.id));
        }
        (1, Choice::Specialization { name }) => {
            let path = path_id
                .and_then(|id| u16::try_from(id).ok())
                .and_then(engine_content::path)
                .ok_or(Error("CONTENT_MISMATCH"))?;
            if i32::from(path.base_avatar) != avatar.template_id
                || !path
                    .specializations
                    .iter()
                    .any(|s| specialization_name(*s) == name)
            {
                return Err(Error("INVALID_CHOICE"));
            }
            specialization = Some(name);
        }
        (2, Choice::Gift { card_id }) => {
            let path = path_id
                .and_then(|id| u16::try_from(id).ok())
                .and_then(engine_content::path)
                .ok_or(Error("CONTENT_MISMATCH"))?;
            if i32::from(path.base_avatar) != avatar.template_id || !path.gifts.contains(&card_id) {
                return Err(Error("INVALID_CHOICE"));
            }
            gift = Some(i32::from(card_id));
        }
        _ => return Err(Error("INVALID_CHOICE")),
    }
    let activate = if avatar.is_shade {
        consume_donor(tx, user, &avatar, donor).await?
    } else {
        if donor.is_some() {
            return Err(Error("INVALID_DONOR"));
        }
        false
    };
    let next_tier = avatar.tier + 1;
    let bonus = next_tier * 3;
    sqlx::query("INSERT INTO avatar_advancement(avatar_id,permanent_points,permanent_unspent) VALUES($1,$2,$2) ON CONFLICT(avatar_id) DO UPDATE SET permanent_points=$2,permanent_unspent=$2").bind(id).bind(bonus).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO incarnation_decks(avatar_id,tier,cards) SELECT avatar_id,$2,cards FROM decks WHERE avatar_id=$1").bind(id).bind(next_tier).execute(&mut *tx).await?;
    sqlx::query("UPDATE avatars SET path_id=$2,specialization=$3,gift_id=$4,tier=$5,level=1,xp=0,dev_points_unspent=0,dp_phys_atk=0,dp_phys_def=0,dp_mag_atk=0,dp_mag_def=0,dp_hp=0,dp_stamina=0,is_active=(is_active OR $6),revision=revision+1 WHERE id=$1").bind(id).bind(path_id).bind(&specialization).bind(gift).bind(next_tier).bind(activate).execute(&mut *tx).await?;
    grant_path(
        tx,
        user,
        id,
        path_id.ok_or(Error("CONTENT_MISMATCH"))? as u16,
    )
    .await?;
    sqlx::query("INSERT INTO reincarnation_history(avatar_id,to_tier,previous_xp,path_id,specialization,gift_id,content_version) VALUES ($1,$2,$3,$4,$5,$6,$7)").bind(id).bind(next_tier).bind(avatar.xp).bind(path_id).bind(specialization).bind(gift).bind(&engine_content::CONTENT_VERSION[..]).execute(tx).await?;
    Ok(())
}
pub async fn create_shade(
    tx: &mut PgConnection,
    user: Uuid,
    id: Uuid,
    revision: i32,
    new_id: Uuid,
    school: &str,
) -> Result<()> {
    // Retired shades are never a valid destination, including from new two-stage gold.
    if !engine_content::AVATARS
        .iter()
        .any(|a| a.playable && a.rarity == AvatarRarity::Shade)
    {
        return Err(Error("AVATAR_LOCKED"));
    }
    let source = load(tx, user, id).await?;
    ready(&source, revision)?;
    if source.tier != 3 || source.is_shade || id == new_id {
        return Err(Error("INVALID_DONOR"));
    }
    let school = parse_school(school)?;
    let shade = engine_content::AVATARS
        .iter()
        .find(|a| a.playable && a.rarity == AvatarRarity::Shade && a.schools.level(school) == 1)
        .ok_or(Error("AVATAR_LOCKED"))?;
    sqlx::query(
        "UPDATE avatars SET deleted_at=now(),is_active=false,revision=revision+1 WHERE id=$1",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;
    if sqlx::query("INSERT INTO avatars(id,user_id,slot,template_id,is_shade,shade_school,is_active) VALUES ($1,$2,$3,$4,true,$5,$6) ON CONFLICT DO NOTHING").bind(new_id).bind(user).bind(source.slot).bind(i32::from(shade.id)).bind(school_name(school)).bind(source.is_active).execute(&mut *tx).await?.rows_affected() != 1 {
        return Err(Error("CONFLICT"));
    }
    sqlx::query("INSERT INTO decks(avatar_id) VALUES ($1)")
        .bind(new_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO avatar_sacrifices(consumed_avatar,beneficiary_avatar,user_id,kind,content_version) VALUES ($1,$2,$3,'shade_create',$4)").bind(id).bind(new_id).bind(user).bind(&engine_content::CONTENT_VERSION[..]).execute(tx).await?;
    Ok(())
}

/// Prefer the class starter; replace spells unavailable on the chosen level-one path.
pub fn path_deck(id: u16) -> Result<Vec<i16>> {
    let path = engine_content::path(id)
        .filter(|p| engine_content::is_playable(p.avatar.id))
        .ok_or(Error("AVATAR_LOCKED"))?;
    let starter = engine_content::starter(path.base_avatar).ok_or(Error("CONTENT_MISMATCH"))?;
    let content = engine_content::content();
    let mut deck = Vec::new();
    for card in starter.deck.iter().copied().chain(
        engine_content::STARTERS
            .iter()
            .flat_map(|s| s.deck.iter().copied()),
    ) {
        if deck.len() == 20 {
            break;
        }
        if content
            .get(card)
            .is_some_and(|c| path.avatar.schools.meets(&c.requires))
            && deck.iter().filter(|n| **n == card as i16).count() < 4
        {
            deck.push(card as i16);
        }
    }
    if deck.len() != 20 {
        return Err(Error("CONTENT_MISMATCH"));
    }
    Ok(deck)
}
async fn grant_path(db: &mut PgConnection, user: Uuid, avatar: Uuid, path: u16) -> Result<()> {
    let pack = path_deck(path)?;
    let claimed = sqlx::query(
        "INSERT INTO path_packs(user_id,path_id,cards) VALUES($1,$2,$3) ON CONFLICT DO NOTHING",
    )
    .bind(user)
    .bind(i32::from(path))
    .bind(&pack)
    .execute(&mut *db)
    .await?
    .rows_affected()
        == 1;
    if claimed {
        for card in &pack {
            sqlx::query("INSERT INTO collection(user_id,card_id,qty) VALUES($1,$2,1) ON CONFLICT(user_id,card_id) DO UPDATE SET qty=least(collection.qty::bigint+1,2147483647)::integer").bind(user).bind(*card).execute(&mut *db).await?;
        }
    }
    let owned: Vec<(i16, i32)> = sqlx::query_as(
        "SELECT card_id,qty FROM collection WHERE user_id=$1 AND qty>0 ORDER BY card_id",
    )
    .bind(user)
    .fetch_all(&mut *db)
    .await?;
    let mut deck = Vec::new();
    let schools = engine_content::path(path)
        .ok_or(Error("CONTENT_MISMATCH"))?
        .avatar
        .schools;
    let content = engine_content::content();
    for card in pack.iter().copied().chain(
        owned
            .iter()
            .flat_map(|(id, qty)| std::iter::repeat_n(*id, (*qty).min(4) as usize)),
    ) {
        if deck.len() == 20 {
            break;
        }
        let quantity = owned
            .iter()
            .find(|(id, _)| *id == card)
            .map_or(0, |(_, qty)| *qty);
        if deck.iter().filter(|id| **id == card).count() < (quantity.min(4) as usize)
            && content
                .get(card as u16)
                .is_some_and(|c| schools.meets(&c.requires))
        {
            deck.push(card);
        }
    }
    if deck.len() != 20 {
        return Err(Error("PATH_DECK_REQUIRED"));
    }
    sqlx::query(
        "UPDATE decks SET cards=$2,revision=revision+1,updated_at=now() WHERE avatar_id=$1",
    )
    .bind(avatar)
    .bind(deck)
    .execute(db)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_sixteen_released_paths_have_legal_level_one_decks() {
        let paths: Vec<_> = engine_content::PATHS
            .iter()
            .filter(|p| engine_content::is_playable(p.avatar.id))
            .collect();
        assert_eq!(paths.len(), 16);
        for path in paths {
            let deck = super::path_deck(path.avatar.id).unwrap();
            assert_eq!(deck.len(), 20);
            for id in &deck {
                assert!(
                    path.avatar
                        .schools
                        .meets(&engine_content::content().get(*id as u16).unwrap().requires)
                );
                assert!(deck.iter().filter(|c| *c == id).count() <= 4);
            }
        }
    }
}
