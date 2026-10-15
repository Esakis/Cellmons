use std::{
    collections::{HashMap, VecDeque},
    sync::Mutex,
    time::{Duration, Instant},
};

#[derive(Default)]
pub struct Limits(Mutex<HashMap<Vec<u8>, VecDeque<Instant>>>);

impl Limits {
    pub fn take(&self, key: Vec<u8>, limit: usize, window: Duration) -> Result<usize, u64> {
        let now = Instant::now();
        let mut entries = self.0.lock().expect("rate limiter lock");
        // All configured windows are at most one hour. Bound attacker-controlled key cardinality.
        entries.retain(|_, attempts| {
            attempts
                .back()
                .is_some_and(|at| now.duration_since(*at) < Duration::from_secs(3600))
        });
        if entries.len() >= 10000 && !entries.contains_key(&key) {
            return Err(60);
        }
        let attempts = entries.entry(key).or_default();
        while attempts
            .front()
            .is_some_and(|at| now.duration_since(*at) >= window)
        {
            attempts.pop_front();
        }
        if attempts.len() >= limit {
            return Err(window
                .saturating_sub(now.duration_since(attempts[0]))
                .as_secs()
                .max(1));
        }
        attempts.push_back(now);
        Ok(attempts.len())
    }

    pub fn clear(&self, key: &[u8]) {
        self.0.lock().expect("rate limiter lock").remove(key);
    }
}
