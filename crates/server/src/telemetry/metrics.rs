//! Bounded labels only. No paths, player IDs, login, IP, cookie or token labels.
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

const BOUNDS: [u64; 15] = [
    100, 250, 500, 1000, 2500, 5000, 10000, 25000, 50000, 100000, 250000, 500000, 1000000, 2500000,
    5000000,
];
pub struct Histogram {
    buckets: [AtomicU64; 15],
    count: AtomicU64,
    sum: AtomicU64,
}
impl Histogram {
    const fn new() -> Self {
        Self {
            buckets: [const { AtomicU64::new(0) }; 15],
            count: AtomicU64::new(0),
            sum: AtomicU64::new(0),
        }
    }
    pub fn observe(&self, started: Instant) {
        let micros = started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64;
        self.count.fetch_add(1, Ordering::Relaxed);
        self.sum.fetch_add(micros, Ordering::Relaxed);
        for (bound, bucket) in BOUNDS.iter().zip(&self.buckets) {
            if micros <= *bound {
                bucket.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
    fn render(&self, name: &str, output: &mut String) {
        use std::fmt::Write as _;
        let _ = writeln!(output, "# TYPE {name} histogram");
        for (bound, bucket) in BOUNDS.iter().zip(&self.buckets) {
            let _ = writeln!(
                output,
                "{name}_bucket{{le=\"{}\"}} {}",
                *bound as f64 / 1_000_000.0,
                bucket.load(Ordering::Relaxed)
            );
        }
        let count = self.count.load(Ordering::Relaxed);
        let _ = writeln!(
            output,
            "{name}_bucket{{le=\"+Inf\"}} {count}\n{name}_count {count}\n{name}_sum {}",
            self.sum.load(Ordering::Relaxed) as f64 / 1_000_000.0
        );
    }
}
pub static ROUNDS: Histogram = Histogram::new();
static HTTP: Histogram = Histogram::new();
static STATUSES: [AtomicU64; 6] = [const { AtomicU64::new(0) }; 6];
pub async fn track(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let started = Instant::now();
    use tracing::Instrument as _;
    let trace = uuid::Uuid::now_v7();
    let mut response = next
        .run(request)
        .instrument(tracing::info_span!("http",trace_id=%trace))
        .await;
    response.headers_mut().insert(
        "x-trace-id",
        trace.to_string().parse().expect("UUID header"),
    );
    let index = usize::from(response.status().as_u16() / 100).min(5);
    STATUSES[index].fetch_add(1, Ordering::Relaxed);
    HTTP.observe(started);
    response
}
pub async fn export(
    axum::extract::State(state): axum::extract::State<crate::gateway::AppState>,
) -> axum::response::Response {
    use axum::response::IntoResponse as _;
    use std::fmt::Write as _;
    let Ok(status) = crate::operations::status(&state.db).await else {
        return (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "Metrics database unavailable\n",
        )
            .into_response();
    };
    let mut output = String::new();
    let archive = tokio::time::timeout(std::time::Duration::from_secs(3), sqlx::query_scalar::<_,bool>("SELECT coalesce(last_failed_time>last_archived_time,last_failed_time IS NOT NULL) FROM pg_stat_archiver").fetch_one(&state.db)).await;
    if let Ok(Ok(failed)) = archive {
        let _ = writeln!(
            output,
            "# TYPE wal_archive_failed gauge\nwal_archive_failed {}",
            u8::from(failed)
        );
    } else {
        return (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "Metrics archive state unavailable\n",
        )
            .into_response();
    }
    for (name, value) in [
        ("ws_connections", state.online.connections() as i64),
        ("duels_active", status.duels),
        ("tournaments_active", status.tournaments),
        ("expeditions_active", status.expeditions),
        ("trades_active", status.trades),
        ("deployment_draining", i64::from(status.draining)),
        (
            "db_pool_in_use",
            i64::from(state.db.size()) - state.db.num_idle() as i64,
        ),
        ("db_pool_size", i64::from(state.db.size())),
    ] {
        let _ = writeln!(output, "# TYPE {name} gauge\n{name} {value}");
    }
    let jobs = tokio::time::timeout(std::time::Duration::from_secs(3),sqlx::query_as::<_, (String,bool,i64)>("SELECT name,success,coalesce(extract(epoch FROM last_success_at)::bigint,0) FROM operation_jobs").fetch_all(&state.db)).await;
    if let Ok(Ok(jobs)) = jobs {
        for (name, success, timestamp) in jobs {
            // Database constraint fixes this label's cardinality at three job names.
            let _ = writeln!(
                output,
                "operation_success{{operation=\"{name}\"}} {}\noperation_last_success_seconds{{operation=\"{name}\"}} {timestamp}",
                u8::from(success)
            );
        }
    } else {
        return (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "Metrics job state unavailable\n",
        )
            .into_response();
    }
    for (class, counter) in STATUSES.iter().enumerate().skip(1) {
        let _ = writeln!(
            output,
            "http_responses_total{{class=\"{class}xx\"}} {}",
            counter.load(Ordering::Relaxed)
        );
    }
    ROUNDS.render("duel_round_latency_seconds", &mut output);
    HTTP.render("http_request_seconds", &mut output);
    (
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        output,
    )
        .into_response()
}
