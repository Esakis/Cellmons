use engine_content::AvatarRarity;
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// A committed duel seed determines the roll, so transaction retries never reroll rewards.
pub fn choose(seed: &[u8; 32], user: Uuid, wins: u32, owned: &[i32]) -> Option<u16> {
    let rules = &crate::rules::RULES.unlock;
    let mut hash = Sha256::new();
    hash.update(b"spellgambit:arena-unlock:v1");
    hash.update(seed);
    hash.update(user.as_bytes());
    let bytes: [u8; 32] = hash.finalize().into();
    let number = |at: usize| u64::from_le_bytes(bytes[at..at + 8].try_into().expect("digest"));
    if !rules.newbie_wins.contains(&wins)
        && number(0) % 1_000_000 >= u64::from(rules.arena_chance_ppm)
    {
        return None;
    }
    let available: Vec<_> = engine_content::AVATARS
        .iter()
        .filter(|a| {
            !owned.contains(&i32::from(a.id))
                && matches!(
                    a.rarity,
                    AvatarRarity::Common | AvatarRarity::Uncommon | AvatarRarity::Rare
                )
        })
        .collect();
    if available.is_empty() {
        return None;
    }
    let roll = (number(8) % 100) as u32;
    let rarity = if roll < rules.rarity_weights[0] {
        AvatarRarity::Common
    } else if roll < rules.rarity_weights[0] + rules.rarity_weights[1] {
        AvatarRarity::Uncommon
    } else {
        AvatarRarity::Rare
    };
    let category: Vec<_> = available
        .iter()
        .filter(|a| a.rarity == rarity)
        .copied()
        .collect();
    // During partial content releases a rolled category may have no missing templates.
    let pool = if category.is_empty() {
        &available
    } else {
        &category
    };
    Some(pool[number(16) as usize % pool.len()].id)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn arena_probability_guarantees_and_missing_template_selection() {
        let user = Uuid::nil();
        let mut successes = 0;
        for value in 0u32..50_000 {
            let mut seed = [0; 32];
            seed[..4].copy_from_slice(&value.to_le_bytes());
            successes += u32::from(choose(&seed, user, 1, &[1]).is_some());
        }
        assert!((200..=300).contains(&successes), "{successes}/50000");
        let mut owned = vec![1];
        for wins in [5, 15, 30] {
            let reward = choose(&[7; 32], user, wins, &owned).unwrap();
            assert!(!owned.contains(&i32::from(reward)));
            owned.push(i32::from(reward));
        }
        let all = engine_content::AVATARS
            .iter()
            .map(|a| i32::from(a.id))
            .collect::<Vec<_>>();
        assert!(choose(&[7; 32], user, 5, &all).is_none());
    }
}
