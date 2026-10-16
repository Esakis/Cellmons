//! Real protocol load on a disposable loopback fixture; never reads application .env.
mod fixture;
mod network;
mod scenarios;

use anyhow::{Context as _, ensure};
use hdrhistogram::Histogram;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Instant,
};

#[derive(Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
struct Config {
    scenario: String,
    connections: usize,
    duels: usize,
    seconds: u64,
    think_ms: u64,
    chat_per_second: usize,
    room_members: usize,
    tournaments: usize,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            scenario: "lobby".into(),
            connections: 200,
            duels: 50,
            seconds: 15,
            think_ms: 2000,
            chat_per_second: 5,
            room_members: 100,
            tournaments: 2,
        }
    }
}
#[derive(Default)]
struct Measurements(Mutex<BTreeMap<String, Histogram<u64>>>);
impl Measurements {
    fn record(&self, name: &str, start: Instant) {
        self.value(
            name,
            start.elapsed().as_micros().min(u128::from(u64::MAX)) as u64,
        );
    }
    fn value(&self, name: &str, micros: u64) {
        let mut values = self.0.lock().expect("histograms");
        let histogram = values
            .entry(name.into())
            .or_insert_with(|| Histogram::new(3).expect("histogram precision"));
        let _ = histogram.record(micros.max(1));
    }
    fn report(&self) -> serde_json::Value {
        serde_json::to_value(self.0.lock().expect("histograms").iter().map(|(name,h)|
            (name.clone(),serde_json::json!({"unit":if name == "transport_close_code" {"websocket_close_code"} else if name.ends_with("_bytes") {"bytes"} else if name.contains("simultaneous_duels") || name.ends_with("_sent") || name.ends_with("_cycles") {"count"} else {"microseconds"},"count":h.len(),"p50":h.value_at_quantile(0.5),"p95":h.value_at_quantile(0.95),"p99":h.value_at_quantile(0.99),"max":h.max()}))).collect::<BTreeMap<_,_>>()).expect("metrics JSON")
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_max_level(tracing_subscriber::filter::LevelFilter::WARN)
        .init();
    let mut args = std::env::args().skip(1);
    let mut config = Config::default();
    let mut output = PathBuf::from("target/loadtest.json");
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--config" => {
                config = toml::from_str(&std::fs::read_to_string(
                    args.next().context("--config path")?,
                )?)?
            }
            "--output" => output = args.next().context("--output path")?.into(),
            "--scenario" => config.scenario = args.next().context("--scenario value")?,
            "--connections" => {
                config.connections = args.next().context("--connections value")?.parse()?
            }
            "--duels" => config.duels = args.next().context("--duels value")?.parse()?,
            "--seconds" => config.seconds = args.next().context("--seconds value")?.parse()?,
            "--room-members" => {
                config.room_members = args.next().context("--room-members value")?.parse()?
            }
            "--chat-per-second" => {
                config.chat_per_second = args.next().context("--chat-per-second value")?.parse()?
            }
            "--think-ms" => config.think_ms = args.next().context("--think-ms value")?.parse()?,
            "--help" => {
                println!(
                    "loadtest --config <toml> --scenario lobby|duels|tournaments|economy|soak --output <json>\nCreates and drops its own local PostgreSQL database (LOADTEST_DATABASE_URL). Never targets player accounts. CPU/RSS include generator and server; target-VPS acceptance is separate."
                );
                return Ok(());
            }
            _ => anyhow::bail!("unknown argument"),
        }
    }
    ensure!(
        ["lobby", "duels", "tournaments", "economy", "soak"].contains(&config.scenario.as_str()),
        "unknown scenario"
    );
    ensure!(
        (2..=10000).contains(&config.connections)
            && config.duels <= config.connections / 2
            && (1..=86400).contains(&config.seconds)
            && (1..=20000).contains(&config.think_ms)
            && (1..=1000).contains(&config.room_members)
            && config.chat_per_second <= 500
            && config.tournaments <= 10,
        "invalid load bounds"
    );
    ensure!(
        config.scenario != "economy" || config.connections >= 400,
        "economy needs at least 400 connections; use --config loadtests/economy.toml"
    );
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let db = fixture::Database::create().await?;
    let measurements = Arc::new(Measurements::default());
    let start = Instant::now();
    let result = tokio::select! {
        result=scenarios::run(&db.pool,&config,measurements.clone())=>result,
        _=tokio::signal::ctrl_c()=>Err(anyhow::anyhow!("interrupted")),
    };
    let report = serde_json::json!({"schema":1,"config":config,"local_fixture":true,"combined_generator_and_server":true,"cleanup_parallelism":32,"chat_latency_sample":"1% of connections, every 100th synthetic player","elapsed_ms":start.elapsed().as_millis(),"success":result.is_ok(),"error":result.as_ref().err().map(|e|e.to_string()),"latencies":measurements.report()});
    let written = std::fs::write(output, serde_json::to_vec_pretty(&report)?);
    db.remove().await?;
    written?;
    result?;
    println!("Load scenario completed; report written (no credentials).");
    Ok(())
}
