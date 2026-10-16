//! Read-only weekly aggregates from online play. Guests never enter these tables.
use anyhow::Context as _;
use futures_util::TryStreamExt as _;
use protocol::{Message as _, game::v1 as pb};
use serde::Serialize;
use sqlx::PgPool;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default, Serialize)]
struct Performance {
    games: u64,
    wins: u64,
    draws: u64,
}
impl Performance {
    fn add(&mut self, winner: i16, side: usize) {
        self.games += 1;
        self.wins += u64::from(winner == side as i16 + 1);
        self.draws += u64::from(winner == 0);
    }
}
pub async fn weekly(db: &PgPool) -> anyhow::Result<serde_json::Value> {
    let mut tx = db.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL statement_timeout='60s'")
        .execute(&mut *tx)
        .await?;
    let overview: String = sqlx::query_scalar("SELECT json_build_object('window_days',7,'registered_accounts',(SELECT count(*) FROM users WHERE created_at>=now()-interval '7 days'),'accounts_with_session_activity',(SELECT count(DISTINCT user_id) FROM sessions WHERE last_seen_at>=now()-interval '7 days'),'matches',(SELECT count(*) FROM matches WHERE created_at>=now()-interval '7 days'),'xp_awarded',(SELECT coalesce(sum(first_xp+second_xp),0)::text FROM matches WHERE created_at>=now()-interval '7 days'),'ducats_faucet',(SELECT coalesce(-sum(e.delta),0)::text FROM ledger_entries e JOIN ledger_transactions t ON t.id=e.tx_id WHERE e.account='system:faucet' AND t.created_at>=now()-interval '7 days'),'ducats_sink',(SELECT coalesce(sum(e.delta),0)::text FROM ledger_entries e JOIN ledger_transactions t ON t.id=e.tx_id WHERE e.account='system:sink' AND t.created_at>=now()-interval '7 days'))::text").fetch_one(&mut *tx).await?;
    let mut templates = BTreeMap::<String, Performance>::new();
    let mut cards = BTreeMap::<String, Performance>::new();
    let mut sampled = 0;
    let mut invalid = 0;
    let mut truncated = false;
    {
        let mut rows = sqlx::query_as::<_, (Vec<u8>,i16,String,String)>("SELECT p.replay,m.winner,d.engine_version,encode(d.content_version,'hex') FROM matches m JOIN match_projections p ON p.match_id=m.id JOIN duel_journal d ON d.id=m.id WHERE m.created_at>=now()-interval '7 days' ORDER BY m.id DESC LIMIT 20001").fetch(&mut *tx);
        while let Some((raw, winner, engine, content)) = rows.try_next().await? {
            if sampled == 20000 {
                truncated = true;
                break;
            }
            sampled += 1;
            let replay = match pb::MatchReplayData::decode(raw.as_slice()) {
                Ok(replay) => replay,
                Err(_) => {
                    invalid += 1;
                    continue;
                }
            };
            let Some(opening) = replay.rounds.first() else {
                invalid += 1;
                continue;
            };
            for (side, profile) in [opening.you.as_ref(), opening.opponent.as_ref()]
                .into_iter()
                .enumerate()
            {
                if let Some(profile) = profile {
                    templates
                        .entry(format!(
                            "{engine}/{content}/{}/{}@{}",
                            opening.kind, profile.template_id, profile.level
                        ))
                        .or_default()
                        .add(winner, side);
                }
            }
            // One count per side/card/match; duplicated final snapshots never add usage.
            let mut used = BTreeSet::new();
            for round in &replay.rounds {
                for event in &round.events {
                    if matches!(event.kind.as_str(), "cast" | "countered" | "failed")
                        && event.side < 2
                        && event.card_id > 0
                    {
                        used.insert((event.side, event.card_id));
                    }
                }
            }
            for (side, card) in used {
                cards
                    .entry(format!("{engine}/{content}/{}/{card}", opening.kind))
                    .or_default()
                    .add(winner, side as usize);
            }
        }
    }
    tx.commit().await?;
    templates.retain(|_, value| value.games >= 20);
    cards.retain(|_, value| value.games >= 20);
    Ok(
        serde_json::json!({"overview":serde_json::from_str::<serde_json::Value>(&overview).context("aggregate JSON")?,"sampled_latest_matches":sampled,"sample_truncated":truncated,"invalid_replays":invalid,"minimum_group_games":20,"templates":templates,"cards_attempted_in_matches":cards,"interpretation":"Descriptive counts split by engine/content; no causal balance claim or automatic tuning. Guest activity is not collected."}),
    )
}
