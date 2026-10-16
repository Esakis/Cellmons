use anyhow::{Context as _, ensure};
use futures_util::{StreamExt as _, TryStreamExt as _};
use sha2::{Digest as _, Sha256};
use sqlx::{
    Connection as _, PgConnection, PgPool,
    postgres::{PgConnectOptions, PgPoolOptions},
};
use std::{net::IpAddr, str::FromStr};
use uuid::Uuid;

pub struct Database {
    pub pool: PgPool,
    admin: PgConnection,
    name: String,
}
impl Database {
    pub async fn create() -> anyhow::Result<Self> {
        let url = std::env::var("LOADTEST_DATABASE_URL")
            .unwrap_or_else(|_| "postgres://game:game@127.0.0.1:5433/game".into());
        let options =
            PgConnectOptions::from_str(&url).context("invalid load-test database configuration")?;
        ensure!(
            options.get_host() == "localhost"
                || options
                    .get_host()
                    .parse::<IpAddr>()
                    .is_ok_and(|ip| ip.is_loopback()),
            "load tests require loopback PostgreSQL"
        );
        let mut admin = PgConnection::connect_with(&options)
            .await
            .context("load test requires local PostgreSQL")?;
        let name = format!("game_load_{}", Uuid::now_v7().simple());
        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE \"{name}\"")))
            .execute(&mut admin)
            .await?;
        let setup = async {
            let pool = PgPoolOptions::new()
                .max_connections(32)
                .connect_with(options.database(&name))
                .await?;
            server::db::migrate(&pool).await?;
            Ok::<_, anyhow::Error>(pool)
        }
        .await;
        match setup {
            Ok(pool) => Ok(Self { pool, admin, name }),
            Err(error) => {
                sqlx::query(sqlx::AssertSqlSafe(format!(
                    "DROP DATABASE \"{name}\" WITH (FORCE)"
                )))
                .execute(&mut admin)
                .await?;
                Err(error)
            }
        }
    }
    pub async fn remove(mut self) -> anyhow::Result<()> {
        self.pool.close().await;
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "DROP DATABASE \"{}\" WITH (FORCE)",
            self.name
        )))
        .execute(&mut self.admin)
        .await?;
        Ok(())
    }
}
#[derive(Clone)]
pub struct Player {
    pub id: Uuid,
    pub avatar: Uuid,
    pub login: String,
    pub token: String,
}
pub async fn players(db: &PgPool, count: usize) -> anyhow::Result<Vec<Player>> {
    futures_util::stream::iter(0..count).map(|index|async move {
        let id=Uuid::now_v7(); let avatar=Uuid::now_v7();
        let login=format!("Load{index}");
        let token=format!("{}{}",Uuid::now_v7().simple(),Uuid::now_v7().simple());
        let hash=Sha256::digest(token.as_bytes()).to_vec();
        let mut tx=db.begin().await?;
        sqlx::query("INSERT INTO users(id,login,email,password_hash,start_school,email_verified_at) VALUES($1,$2,$3,'$argon2id$v=19$m=19456,t=2,p=1$fixture$fixture','combat',now())").bind(id).bind(&login).bind(format!("load{index}@example.test")).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO sessions(id_hash,kind,user_id,expires_at) VALUES($1,'web',$2,now()+interval '1 day')").bind(hash).bind(id).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO unlocked_templates(user_id,template_id,source) VALUES($1,1,'start')").bind(id).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO avatars(id,user_id,slot,template_id,is_active) VALUES($1,$2,0,1,true)").bind(avatar).bind(id).execute(&mut *tx).await?;
        let starter=&engine_content::STARTERS[0];
        for (card,qty) in starter.cards {sqlx::query("INSERT INTO collection(user_id,card_id,qty) VALUES($1,$2,$3)").bind(id).bind(*card as i16).bind(i32::from(*qty)).execute(&mut *tx).await?;}
        sqlx::query("INSERT INTO decks(avatar_id,cards) VALUES($1,$2)").bind(avatar).bind(starter.deck.iter().map(|id|*id as i16).collect::<Vec<_>>()).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO ratings(user_id) VALUES($1)").bind(id).execute(&mut *tx).await?;
        server::economy::grant_initial(&mut tx,id).await.map_err(|e|anyhow::anyhow!(e.0))?;
        tx.commit().await?;
        Ok::<_,anyhow::Error>(Player{id,avatar,login,token})
    }).buffered(24).try_collect().await
}
