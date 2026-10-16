//! R-WYP: private frozen plans, stage choices and exactly-once rewards.
pub mod model;
mod repository;
mod rewards;
mod views;
use crate::online::{Error, Result};
pub use repository::{Choice, Mutation, choose, retreat, start};
pub(crate) use rewards::settle_fight;
use serde::Deserialize;
use serde::Serialize;
use std::sync::LazyLock;
pub use views::{get, post};

pub static CONFIG: LazyLock<model::Config> = LazyLock::new(|| {
    #[derive(Deserialize)]
    struct Root {
        expedition: model::Config,
    }
    let config = toml::from_str::<Root>(include_str!("../../../../content/rules.toml"))
        .expect("expedition configuration")
        .expedition;
    config.validate().expect("valid expedition configuration");
    config
});
fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    serde_json::to_vec(value).map_err(|_| Error("INTERNAL"))
}
fn decode<T: for<'de> Deserialize<'de>>(bytes: &[u8]) -> Result<T> {
    serde_json::from_slice(bytes).map_err(|_| Error("INVALID_JOURNAL"))
}
