//! Exactly-once result transaction. Rating transfers conserve the sum, including at the floor.
use super::{Error, Result, record::Record};
use engine::{Outcome, Phase, Side};
use protocol::{Message, game::v1 as pb};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Clone, Default)]
pub struct Rewards {
    pub rating: [i32; 2],
    pub delta: [i32; 2],
    pub xp: [u32; 2],
    pub unlock: [Option<u16>; 2],
    pub ducats: [u32; 2],
}

pub fn rating_delta(winner: i32, loser: i32, winner_level: u32, loser_level: u32) -> i32 {
    let computed = u64::from(crate::rules::RULES.rating.k)
        * u64::try_from(loser.max(1)).unwrap_or(1)
        * u64::from(loser_level.clamp(1, 20))
        / (u64::try_from(winner.max(1)).unwrap_or(1) * u64::from(winner_level.clamp(1, 20)));
    computed
        .min(u64::try_from(loser.saturating_sub(1)).unwrap_or(0))
        .min(u64::try_from(i32::MAX.saturating_sub(winner)).unwrap_or(0)) as i32
}

pub fn xp_award(
    level: u32,
    tier: i16,
    other_level: u32,
    other_tier: i16,
    won: bool,
    draw: bool,
) -> u32 {
    xp_award_using(
        &crate::rules::RULES.xp,
        level,
        tier,
        other_level,
        other_tier,
        won,
        draw,
    )
}
/// The same policy calculation is used by settlement and the pacing audit's what-if scenarios.
pub fn xp_award_using(
    rules: &crate::rules::Experience,
    level: u32,
    tier: i16,
    other_level: u32,
    other_tier: i16,
    won: bool,
    draw: bool,
) -> u32 {
    let effective = |level: u32, tier: i16| {
        let index = usize::try_from(tier).unwrap_or(0).min(3);
        level.clamp(1, 20) * rules.tier_mult_pct[index] + rules.tier_add[index] * 100
    };
    if !won && !draw {
        return 0;
    }
    let base = (u64::from(rules.base) * u64::from(effective(other_level, other_tier))
        / u64::from(effective(level, tier)))
    .clamp(u64::from(rules.min), u64::from(rules.max)) as u32;
    if draw { base / 2 } else { base }
}
/// Shades are valued as Ascent at every reincarnation stage (R-PD-02).
pub fn xp_tier(template: u16, tier: i16) -> i16 {
    if engine_content::avatar(template)
        .is_some_and(|a| a.rarity == engine_content::AvatarRarity::Shade)
    {
        3
    } else {
        tier
    }
}
pub fn level_for_xp(xp: i32) -> i16 {
    crate::rules::RULES
        .progression
        .level_xp
        .partition_point(|threshold| *threshold <= xp)
        .clamp(1, 20) as i16
}
pub async fn settle(
    db: &PgPool,
    instance: Uuid,
    record: &Record,
    state: &engine::DuelState,
) -> Result<Rewards> {
    let Phase::Finished(outcome) = state.phase() else {
        return Err(Error("CONFLICT"));
    };
    let mut tx = db.begin().await?;
    let journal = sqlx::query!(
        r#"SELECT status,owner_instance,lease_until>clock_timestamp() AS "owned!" FROM duel_journal WHERE id=$1 FOR UPDATE"#,
        record.id
    )
    .fetch_one(&mut *tx)
    .await?;
    if journal.status == "finished" {
        let row=sqlx::query!("SELECT first_rating_delta,second_rating_delta,first_xp,second_xp,first_rating_new,second_rating_new,first_unlock,second_unlock,first_ducats,second_ducats FROM matches WHERE id=$1",record.id).fetch_one(&mut *tx).await?;
        return Ok(Rewards {
            rating: [row.first_rating_new, row.second_rating_new],
            delta: [row.first_rating_delta, row.second_rating_delta],
            xp: [row.first_xp as u32, row.second_xp as u32],
            unlock: [
                row.first_unlock.map(|id| id as u16),
                row.second_unlock.map(|id| id as u16),
            ],
            ducats: [row.first_ducats as u32, row.second_ducats as u32],
        });
    }
    if journal.owner_instance != instance || !journal.owned {
        return Err(Error("LEASE_LOST"));
    }
    let mut users: Vec<_> = record.seats.iter().filter_map(|s| s.user).collect();
    users.sort();
    for user in users {
        sqlx::query!("SELECT id FROM users WHERE id=$1 FOR UPDATE", user)
            .fetch_one(&mut *tx)
            .await?;
    }
    let mut reward = Rewards::default();
    let profiles = [record.seats[0].public()?, record.seats[1].public()?];
    let mut tiers = profiles
        .each_ref()
        .map(|p| xp_tier(p.template_id as u16, p.tier as i16));
    let mut verified = [false; 2];
    let mut mentors = profiles.each_ref().map(|p| p.mentor);
    let mut avatars = [None, None];
    let mut before = [1000; 2];
    for (i, seat) in record.seats.iter().enumerate() {
        if let (Some(user), Some(avatar)) = (seat.user, seat.avatar) {
            let row=sqlx::query!("SELECT a.tier,a.xp,a.level,a.path_id,a.template_id,u.email_verified_at IS NOT NULL AS \"verified!\" FROM avatars a JOIN users u ON u.id=a.user_id WHERE a.id=$1 AND a.user_id=$2 FOR UPDATE OF a",avatar,user).fetch_one(&mut *tx).await?;
            tiers[i] = xp_tier(
                u16::try_from(row.template_id).map_err(|_| Error("CONTENT_MISMATCH"))?,
                row.tier,
            );
            verified[i] = row.verified;
            avatars[i] = Some((
                avatar,
                row.xp,
                row.level,
                row.path_id.unwrap_or(row.template_id),
            ));
            before[i] = sqlx::query_scalar!(
                "SELECT rating FROM ratings WHERE user_id=$1 FOR UPDATE",
                user
            )
            .fetch_one(&mut *tx)
            .await?;
        }
    }
    let winner = match outcome {
        Outcome::Draw { .. } => 0,
        Outcome::Win { winner, .. } => winner.index() as i16 + 1,
    };
    for (index, seat) in record.seats.iter().enumerate() {
        if let Some(user) = seat.user {
            mentors[index] |=
                sqlx::query_scalar::<_, bool>("SELECT role='mentor' FROM users WHERE id=$1")
                    .bind(user)
                    .fetch_one(&mut *tx)
                    .await?;
        }
    }
    let mentor_won = winner != 0 && mentors[(winner - 1) as usize];
    let reason = protocol::duel::outcome(outcome, Side::A).reason;
    let rated = ["arena", "tournament"].contains(&record.kind.as_str())
        && !mentor_won
        && verified.iter().all(|v| *v)
        && record.seats.iter().all(|s| s.user.is_some());
    if rated && winner != 0 {
        let win = (winner - 1) as usize;
        let lose = 1 - win;
        let delta = rating_delta(
            before[win],
            before[lose],
            profiles[win].level,
            profiles[lose].level,
        );
        reward.delta[win] = delta;
        reward.delta[lose] = -delta;
    }
    reward.rating = [before[0] + reward.delta[0], before[1] + reward.delta[1]];
    let normal = record.kind != "training";
    for (i, seat) in record.seats.iter().enumerate() {
        if let Some(user) = seat.user {
            let won = winner == i as i16 + 1;
            let draw = winner == 0;
            let lost = !won && !draw;
            let disconnect = normal && reason == "disconnect" && lost;
            if record.kind == "arena" && won {
                reward.ducats[i] = crate::economy::award_arena(&mut tx, user, record.id).await?;
                let wins=sqlx::query_scalar!("INSERT INTO arena_progress(user_id,wins) VALUES ($1,1) ON CONFLICT(user_id) DO UPDATE SET wins=arena_progress.wins+1 RETURNING wins",user).fetch_one(&mut *tx).await?;
                let owned = sqlx::query_scalar!(
                    "SELECT template_id FROM unlocked_templates WHERE user_id=$1",
                    user
                )
                .fetch_all(&mut *tx)
                .await?;
                reward.unlock[i] = crate::unlock::choose(&record.seed, user, wins as u32, &owned);
                if let Some(template) = reward.unlock[i] {
                    sqlx::query!("INSERT INTO unlocked_templates(user_id,template_id,source) VALUES ($1,$2,'arena') ON CONFLICT DO NOTHING",user,i32::from(template)).execute(&mut *tx).await?;
                }
            }
            if normal {
                if record.kind != "expedition"
                    && !mentor_won
                    && let Some((avatar, xp, level, template)) = avatars[i]
                {
                    let gained = xp_award(
                        profiles[i].level,
                        tiers[i],
                        profiles[1 - i].level,
                        tiers[1 - i],
                        won,
                        draw,
                    );
                    let cap = if verified[i] {
                        crate::rules::RULES.xp.cap
                    } else {
                        crate::rules::RULES.xp.unverified_cap
                    };
                    let updated = xp.saturating_add(gained as i32).min(cap).max(xp);
                    reward.xp[i] = (updated - xp) as u32;
                    let new_level = level_for_xp(updated).max(level);
                    let definition = engine_content::avatar(
                        u16::try_from(template).map_err(|_| Error("CONTENT_MISMATCH"))?,
                    )
                    .ok_or(Error("CONTENT_MISMATCH"))?;
                    let points: i16 = definition
                        .dev_points
                        .iter()
                        .filter(|(at, _)| i16::from(*at) > level && i16::from(*at) <= new_level)
                        .map(|(_, qty)| i16::from(*qty))
                        .sum();
                    sqlx::query!("UPDATE avatars SET xp=$2,level=$3,dev_points_unspent=dev_points_unspent+$4,revision=revision+1 WHERE id=$1",avatar,updated,new_level,points).execute(&mut *tx).await?;
                }
                let wins = i32::from(won);
                let draws = i32::from(draw);
                let losses = i32::from(lost);
                let dc = i32::from(disconnect);
                if record.kind != "expedition" {
                    sqlx::query!("UPDATE ratings SET rating=$2,games=games+1,wins=wins+$3,draws=draws+$4,losses=losses+$5,disconnects=disconnects+$6,updated_at=now() WHERE user_id=$1",user,reward.rating[i],wins,draws,losses,dc).execute(&mut *tx).await?;
                }
                sqlx::query!("INSERT INTO player_stats(user_id,games,wins,draws,losses,disconnects) VALUES ($1,1,$2,$3,$4,$5) ON CONFLICT(user_id) DO UPDATE SET games=player_stats.games+1,wins=player_stats.wins+$2,draws=player_stats.draws+$3,losses=player_stats.losses+$4,disconnects=player_stats.disconnects+$5",user,wins,draws,losses,dc).execute(&mut *tx).await?;
            }
        }
    }
    if record.kind == "expedition" {
        let award = crate::expeditions::settle_fight(&mut tx, record, winner).await?;
        reward.xp[0] = award.xp;
        reward.ducats[0] = award.ducats;
        reward.unlock[0] = award.template;
    }
    let first = record.seats[0].user.ok_or(Error("INVALID_JOURNAL"))?;
    let first_avatar = record.seats[0].avatar.ok_or(Error("INVALID_JOURNAL"))?;
    let second = record.seats[1].user;
    let second_avatar = record.seats[1].avatar;
    let rounds = record.moves.len() as i16;
    if !["forfeit", "disconnect", "timeout"].contains(&reason.as_str()) {
        for (index, seat) in record.seats.iter().enumerate() {
            if let Some(user) = seat.user {
                crate::journey::award(
                    &mut tx,
                    user,
                    record.kind == "training",
                    winner == index as i16 + 1,
                    record.moves.len(),
                )
                .await?;
            }
        }
    }
    sqlx::query!("INSERT INTO matches(id,kind,first_user,second_user,first_avatar,second_avatar,winner,reason,rounds,first_rating_delta,second_rating_delta,first_xp,second_xp,first_rating_new,second_rating_new,first_unlock,second_unlock,first_ducats,second_ducats) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19)",record.id,record.kind,first,second,first_avatar,second_avatar,winner,reason,rounds,reward.delta[0],reward.delta[1],reward.xp[0] as i32,reward.xp[1] as i32,reward.rating[0],reward.rating[1],reward.unlock[0].map(i32::from),reward.unlock[1].map(i32::from),reward.ducats[0] as i32,reward.ducats[1] as i32).execute(&mut *tx).await?;
    if rated {
        for (i, seat) in record.seats.iter().enumerate() {
            if let Some(user) = seat.user {
                sqlx::query!("INSERT INTO rating_history(match_id,user_id,before,after) VALUES ($1,$2,$3,$4)",record.id,user,before[i],reward.rating[i]).execute(&mut *tx).await?;
            }
        }
    }
    let replay = super::actor::replay(record)?.encode_to_vec();
    let (_, events) = record.rebuild()?;
    let projections = Side::BOTH.map(|side| {
        let mut snapshot = super::actor::snapshot(record, state, &events, side)?;
        snapshot.result = Some(pb::DuelEnd {
            duel_id: record.id.to_string(),
            outcome: snapshot.view.as_ref().and_then(|v| v.outcome.clone()),
            xp_gained: reward.xp[side.index()],
            rating_delta: reward.delta[side.index()],
            rating_new: reward.rating[side.index()],
            seed: record.seed.to_vec(),
            unlocked_template: reward.unlock[side.index()].map_or(0, u32::from),
            ducats_gained: reward.ducats[side.index()],
        });
        Ok::<_, Error>(snapshot.encode_to_vec())
    });
    let [first_snapshot, second_snapshot] = projections;
    let first_snapshot = first_snapshot?;
    let second_snapshot = second_snapshot?;
    sqlx::query!("INSERT INTO match_projections(match_id,replay,first_snapshot,second_snapshot) VALUES ($1,$2,$3,$4)",record.id,replay,first_snapshot,second_snapshot).execute(&mut *tx).await?;
    sqlx::query!(
        "UPDATE duel_journal SET status='finished',updated_at=now() WHERE id=$1",
        record.id
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!("DELETE FROM duel_locks WHERE duel_id=$1", record.id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(reward)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ranking_matches_all_seven_archived_duels_and_conserves_points_at_floor() {
        for (winner, loser, wl, ll, expected) in [
            (929, 967, 14, 11, 8),
            (937, 959, 14, 11, 8),
            (951, 581, 11, 10, 5),
            (1228, 816, 7, 8, 7),
            (1062, 872, 7, 6, 7),
            (1240, 1364, 14, 15, 11),
            (1645, 1210, 14, 14, 7),
        ] {
            assert_eq!(rating_delta(winner, loser, wl, ll), expected);
        }
        assert_eq!(rating_delta(1, 2, 1, 20), 1);
        assert_eq!(rating_delta(1, 1, 1, 20), 0);
        assert_eq!(rating_delta(i32::MAX, 1000, 1, 20), 0);
    }
    #[test]
    fn experience_rewards_strength_draws_caps_and_level_thresholds() {
        let mut large = crate::rules::RULES.xp.clone();
        large.base = u32::MAX;
        assert_eq!(xp_award_using(&large, 1, 0, 20, 3, true, false), large.max);
        assert_eq!(xp_tier(1, 0), 0);
        for tier in 0..=3 {
            assert_eq!(xp_tier(29, tier), 3);
            assert_eq!(xp_tier(1058, tier), 3);
        }
        assert_eq!(xp_award(1, 0, 20, 0, true, false), 300);
        assert_eq!(xp_award(20, 0, 1, 0, true, false), 10);
        assert_eq!(xp_award(10, 0, 10, 3, true, false), 190);
        assert_eq!(xp_award(10, 0, 10, 3, false, true), 95);
        assert_eq!(xp_award(1, 0, 20, 3, false, false), 0);
        assert_eq!(level_for_xp(0), 1);
        assert_eq!(level_for_xp(49999), 19);
        assert_eq!(level_for_xp(50000), 20);
        assert_eq!(level_for_xp(65000), 20);
    }
}
