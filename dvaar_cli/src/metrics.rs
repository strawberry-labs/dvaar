//! Metrics tracking for tunnel requests
//!
//! Tracks request counts, rates (sliding windows), and duration percentiles.

use serde::Serialize;
use std::collections::VecDeque;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Metrics tracker with sliding window support
pub struct Metrics {
    inner: RwLock<MetricsInner>,
}

struct MetricsInner {
    total_requests: u64,
    open_connections: u32,

    /// Request timestamps for rate calculation (keep last 15 minutes)
    request_times: VecDeque<Instant>,

    /// Durations for percentile calculation (keep last 1000)
    durations: VecDeque<u64>,
}

/// Snapshot of current metrics
#[derive(Debug, Clone, Serialize)]
pub struct MetricsSnapshot {
    pub total_requests: u64,
    pub open_connections: u32,
    pub requests_per_minute_1m: f64,
    pub requests_per_minute_5m: f64,
    pub requests_per_minute_15m: f64,
    pub p50_duration_ms: u64,
    pub p90_duration_ms: u64,
    pub p95_duration_ms: u64,
    pub p99_duration_ms: u64,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(MetricsInner {
                total_requests: 0,
                open_connections: 0,
                request_times: VecDeque::with_capacity(10000),
                durations: VecDeque::with_capacity(1000),
            }),
        }
    }

    /// Record a completed request with its duration
    pub async fn record_request(&self, duration_ms: u64) {
        let mut inner = self.inner.write().await;
        inner.total_requests += 1;
        inner.request_times.push_back(Instant::now());

        // Keep only last 15 minutes of request times
        let cutoff = Instant::now() - Duration::from_secs(15 * 60);
        while let Some(front) = inner.request_times.front() {
            if *front < cutoff {
                inner.request_times.pop_front();
            } else {
                break;
            }
        }

        // Store duration for percentiles (keep last 1000)
        inner.durations.push_back(duration_ms);
        if inner.durations.len() > 1000 {
            inner.durations.pop_front();
        }
    }

    /// Increment open connection count
    pub async fn increment_connections(&self) {
        self.inner.write().await.open_connections += 1;
    }

    /// Decrement open connection count
    pub async fn decrement_connections(&self) {
        let mut inner = self.inner.write().await;
        inner.open_connections = inner.open_connections.saturating_sub(1);
    }

    /// Get current metrics snapshot
    pub async fn snapshot(&self) -> MetricsSnapshot {
        let inner = self.inner.read().await;
        let now = Instant::now();

        // Calculate request rates for different windows
        let count_in_window = |minutes: u64| -> usize {
            let cutoff = now - Duration::from_secs(minutes * 60);
            inner.request_times.iter().filter(|t| **t >= cutoff).count()
        };

        let rate_1m = count_in_window(1) as f64;
        let rate_5m = count_in_window(5) as f64 / 5.0;
        let rate_15m = count_in_window(15) as f64 / 15.0;

        // Calculate percentiles from duration history
        let mut sorted_durations: Vec<u64> = inner.durations.iter().cloned().collect();
        sorted_durations.sort_unstable();

        let percentile = |p: f64| -> u64 {
            if sorted_durations.is_empty() {
                return 0;
            }
            let idx = ((p / 100.0) * sorted_durations.len() as f64) as usize;
            let idx = idx.min(sorted_durations.len() - 1);
            sorted_durations[idx]
        };

        MetricsSnapshot {
            total_requests: inner.total_requests,
            open_connections: inner.open_connections,
            requests_per_minute_1m: rate_1m,
            requests_per_minute_5m: rate_5m,
            requests_per_minute_15m: rate_15m,
            p50_duration_ms: percentile(50.0),
            p90_duration_ms: percentile(90.0),
            p95_duration_ms: percentile(95.0),
            p99_duration_ms: percentile(99.0),
        }
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}
