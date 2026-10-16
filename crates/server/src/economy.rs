//! Virtual currency only. Every balance change has a balanced, append-only ledger entry.
use crate::{
    gateway::AppState,
    online::{Error, Result},
};
use axum::{Json, extract::State, http::HeaderMap};
use serde::Serialize;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Account {
    Player(Uuid),
    Tournament(Uuid),
    Faucet,
    Sink,
}
impl Account {
    fn key(self) -> String {
        match self {
            Self::Player(user) => format!("user:{user}"),
            Self::Tournament(id) => format!("tournament:{id}"),
            Self::Faucet => "system:faucet".into(),
            Self::Sink => "system:sink".into(),
        }
    }
}

/// Caller holds all involved user rows in UUID order and commits or rolls back the whole operation.
pub async fn transfer(
    connection: &mut PgConnection,
    from: Account,
    to: Account,
    amount: i64,
    reason: &str,
    reference: Uuid,
) -> Result<()> {
    if amount < 0 || from == to {
        return Err(Error("INVALID_MESSAGE"));
    }
    if amount == 0 {
        return Ok(());
    }
    if let Account::Player(user) = from {
        let changed=sqlx::query!("UPDATE wallets SET balance=balance-$2,updated_at=now() WHERE user_id=$1 AND balance>=$2",user,amount).execute(&mut *connection).await?;
        if changed.rows_affected() != 1 {
            return Err(Error("INSUFFICIENT_FUNDS"));
        }
    }
    if let Account::Player(user) = to {
        let changed=sqlx::query!("UPDATE wallets SET balance=balance+$2,updated_at=now() WHERE user_id=$1 AND balance<=$3",user,amount,i64::MAX-amount).execute(&mut *connection).await?;
        if changed.rows_affected() != 1 {
            return Err(Error("BALANCE_LIMIT"));
        }
    }
    if let Account::Tournament(id) = from {
        let changed =
            sqlx::query("UPDATE tournaments SET balance=balance-$2 WHERE id=$1 AND balance>=$2")
                .bind(id)
                .bind(amount)
                .execute(&mut *connection)
                .await?;
        if changed.rows_affected() != 1 {
            return Err(Error("INSUFFICIENT_FUNDS"));
        }
    }
    if let Account::Tournament(id) = to {
        let changed =
            sqlx::query("UPDATE tournaments SET balance=balance+$2 WHERE id=$1 AND balance<=$3")
                .bind(id)
                .bind(amount)
                .bind(i64::MAX - amount)
                .execute(&mut *connection)
                .await?;
        if changed.rows_affected() != 1 {
            return Err(Error("BALANCE_LIMIT"));
        }
    }
    let id = Uuid::now_v7();
    sqlx::query!(
        "INSERT INTO ledger_transactions(id,reason,ref_id) VALUES ($1,$2,$3)",
        id,
        reason,
        reference
    )
    .execute(&mut *connection)
    .await?;
    let from = from.key();
    let to = to.key();
    sqlx::query!("INSERT INTO ledger_entries(tx_id,account,delta) VALUES ($1,$2,-($4::bigint)),($1,$3,$4::bigint)",id,from,to,amount).execute(connection).await?;
    Ok(())
}
pub async fn grant_initial(connection: &mut PgConnection, user: Uuid) -> Result<()> {
    let created = sqlx::query!(
        "INSERT INTO wallets(user_id) VALUES ($1) ON CONFLICT DO NOTHING",
        user
    )
    .execute(&mut *connection)
    .await?;
    if created.rows_affected() == 1 {
        transfer(
            connection,
            Account::Faucet,
            Account::Player(user),
            i64::from(crate::rules::RULES.economy.initial),
            "start",
            user,
        )
        .await?;
    }
    Ok(())
}
pub async fn award_arena(connection: &mut PgConnection, user: Uuid, duel: Uuid) -> Result<u32> {
    let rules = &crate::rules::RULES.economy;
    sqlx::query!("INSERT INTO daily_counters(user_id,day) VALUES ($1,(now() AT TIME ZONE 'UTC')::date) ON CONFLICT DO NOTHING",user).execute(&mut *connection).await?;
    let day=sqlx::query!("SELECT first_win_bonus,win_ducats FROM daily_counters WHERE user_id=$1 AND day=(now() AT TIME ZONE 'UTC')::date FOR UPDATE",user).fetch_one(&mut *connection).await?;
    let bonus = if day.first_win_bonus {
        0
    } else {
        rules.arena_first_win
    };
    let win = rules.arena_win.min(
        rules
            .arena_win_daily_cap
            .saturating_sub(day.win_ducats as u32),
    );
    sqlx::query!("UPDATE daily_counters SET first_win_bonus=true,win_ducats=win_ducats+$2 WHERE user_id=$1 AND day=(now() AT TIME ZONE 'UTC')::date",user,win as i32).execute(&mut *connection).await?;
    transfer(
        connection,
        Account::Faucet,
        Account::Player(user),
        i64::from(bonus),
        "daily_win",
        duel,
    )
    .await?;
    transfer(
        connection,
        Account::Faucet,
        Account::Player(user),
        i64::from(win),
        "win",
        duel,
    )
    .await?;
    Ok(bonus + win)
}

#[derive(Debug, Serialize)]
pub struct Reconciliation {
    pub unbalanced_transactions: i64,
    pub incorrect_wallets: i64,
    pub incorrect_escrows: i64,
}
impl Reconciliation {
    pub fn is_ok(&self) -> bool {
        self.unbalanced_transactions == 0
            && self.incorrect_wallets == 0
            && self.incorrect_escrows == 0
    }
}
pub async fn reconcile(db: &PgPool) -> Result<Reconciliation> {
    let unbalanced_transactions=sqlx::query_scalar!("SELECT count(*) AS \"count!\" FROM (SELECT t.id FROM ledger_transactions t LEFT JOIN ledger_entries e ON e.tx_id=t.id GROUP BY t.id HAVING count(e.account)<2 OR coalesce(sum(e.delta),0)<>0) bad").fetch_one(db).await?;
    let incorrect_wallets=sqlx::query_scalar!("SELECT count(*) AS \"count!\" FROM (SELECT COALESCE(w.balance,0) AS actual,COALESCE(l.recorded,0) AS recorded FROM wallets w FULL JOIN (SELECT account,sum(delta) AS recorded FROM ledger_entries WHERE account LIKE 'user:%' GROUP BY account) l ON l.account='user:'||w.user_id::text) balances WHERE actual<>recorded").fetch_one(db).await?;
    Ok(Reconciliation {
        unbalanced_transactions,
        incorrect_wallets,
        incorrect_escrows: sqlx::query_scalar("SELECT count(*) FROM (SELECT coalesce(t.balance,0) AS actual,coalesce(l.recorded,0) AS recorded FROM tournaments t FULL JOIN (SELECT account,sum(delta) AS recorded FROM ledger_entries WHERE account LIKE 'tournament:%' GROUP BY account) l ON l.account='tournament:'||t.id::text) balances WHERE actual<>recorded").fetch_one(db).await?,
    })
}

#[derive(Serialize)]
pub struct WalletView {
    pub balance: String,
    pub entries: Vec<Entry>,
}
#[derive(Serialize)]
pub struct Entry {
    pub id: Uuid,
    pub delta: String,
    pub reason: String,
    pub ref_id: Option<Uuid>,
    pub at_ms: i64,
}
pub async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> std::result::Result<Json<WalletView>, crate::game::HttpError> {
    let session = crate::auth::security::session(&state, &headers)
        .await
        .map_err(crate::game::HttpError)?;
    state
        .auth
        .limit("economy-read", &session.user.to_string(), 60, 60)
        .map_err(crate::game::HttpError)?;
    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|e| crate::game::response(e.into()))?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await
        .map_err(|e| crate::game::response(e.into()))?;
    let balance = sqlx::query_scalar!("SELECT balance FROM wallets WHERE user_id=$1", session.user)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| crate::game::response(e.into()))?
        .ok_or_else(|| crate::game::response(Error("NOT_FOUND")))?;
    let account = Account::Player(session.user).key();
    let rows=sqlx::query!("SELECT t.id,e.delta,t.reason,t.ref_id,(extract(epoch FROM t.created_at)*1000)::bigint AS \"at!\" FROM ledger_entries e JOIN ledger_transactions t ON t.id=e.tx_id WHERE e.account=$1 ORDER BY t.id DESC LIMIT 50",account).fetch_all(&mut *tx).await.map_err(|e|crate::game::response(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| crate::game::response(e.into()))?;
    Ok(Json(WalletView {
        balance: balance.to_string(),
        entries: rows
            .into_iter()
            .map(|r| Entry {
                id: r.id,
                delta: r.delta.to_string(),
                reason: r.reason,
                ref_id: r.ref_id,
                at_ms: r.at,
            })
            .collect(),
    }))
}
