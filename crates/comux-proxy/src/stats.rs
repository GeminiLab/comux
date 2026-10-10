//! Thread-safe process-local transport statistics grouped by upstream.

use comux_core::api::{StatsResponse, TransportStats};
use http::StatusCode;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

/// Cloneable registry of in-memory counters keyed by upstream name.
#[derive(Clone, Default)]
pub struct StatsRegistry {
    /// Shared mutable counters protected from concurrent updates.
    inner: Arc<Mutex<BTreeMap<String, TransportStats>>>,
}

impl StatsRegistry {
    /// Records an accepted request and returns its terminal-accounting guard.
    pub fn begin(&self, upstream: impl Into<String>) -> RequestStatsGuard {
        let upstream = upstream.into();
        let mut stats = self.inner.lock().expect("stats lock");
        let entry = stats.entry(upstream.clone()).or_default();
        entry.accepted_requests += 1;
        entry.active_streams += 1;
        drop(stats);
        RequestStatsGuard {
            registry: self.clone(),
            upstream,
            released: false,
        }
    }

    /// Clones all current counters into a serializable snapshot.
    pub fn snapshot(&self) -> StatsResponse {
        StatsResponse {
            by_upstream: self.inner.lock().expect("stats lock").clone(),
        }
    }

    /// Adds downstream request-body bytes for an upstream.
    pub fn record_request_bytes(&self, upstream: &str, bytes: u64) {
        self.with_stats(upstream, |stats| stats.request_bytes += bytes);
    }

    /// Adds upstream response-body bytes for an upstream.
    pub fn record_response_bytes(&self, upstream: &str, bytes: u64) {
        self.with_stats(upstream, |stats| stats.response_bytes += bytes);
    }

    /// Applies one update while holding the counter map lock.
    fn with_stats(&self, upstream: &str, update: impl FnOnce(&mut TransportStats)) {
        let mut stats = self.inner.lock().expect("stats lock");
        update(stats.entry(upstream.to_owned()).or_default());
    }
}

/// Ensures an accepted request releases its active-stream count exactly once.
pub struct RequestStatsGuard {
    /// Registry receiving this request's terminal accounting.
    registry: StatsRegistry,
    /// Upstream key used by this request.
    upstream: String,
    /// Whether terminal accounting has already released the active stream.
    released: bool,
}

impl RequestStatsGuard {
    /// Adds request-body bytes to this request's upstream counters.
    pub fn add_request_bytes(&self, bytes: u64) {
        self.registry
            .with_stats(&self.upstream, |stats| stats.request_bytes += bytes);
    }

    /// Adds response-body bytes to this request's upstream counters.
    pub fn add_response_bytes(&self, bytes: u64) {
        self.registry
            .with_stats(&self.upstream, |stats| stats.response_bytes += bytes);
    }

    /// Records a completed response's status and duration and releases it.
    pub fn finish_response(&mut self, status: StatusCode, duration_ms: u64) {
        // The released flag makes terminal accounting idempotent when multiple
        // stream paths attempt cleanup.
        if self.released {
            return;
        }
        self.registry.with_stats(&self.upstream, |stats| {
            match status.as_u16() {
                200..=299 => stats.responses_2xx += 1,
                300..=399 => stats.responses_3xx += 1,
                400..=499 => stats.responses_4xx += 1,
                _ => stats.responses_5xx += 1,
            }
            stats.duration_count += 1;
            stats.duration_total_ms += duration_ms;
            stats.duration_max_ms = stats.duration_max_ms.max(duration_ms);
            stats.active_streams = stats.active_streams.saturating_sub(1);
        });
        self.released = true;
    }

    /// Records a transport failure and releases the active stream.
    pub fn network_error(&mut self) {
        if self.released {
            return;
        }
        self.registry.with_stats(&self.upstream, |stats| {
            stats.network_errors += 1;
            stats.active_streams = stats.active_streams.saturating_sub(1);
        });
        self.released = true;
    }
}

impl Drop for RequestStatsGuard {
    /// Releases an abandoned stream without inventing a response or network error.
    fn drop(&mut self) {
        if !self.released {
            self.registry.with_stats(&self.upstream, |stats| {
                stats.active_streams = stats.active_streams.saturating_sub(1);
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::StatsRegistry;
    use http::StatusCode;

    #[test]
    fn records_response_classes_bytes_duration_and_active_streams() {
        let registry = StatsRegistry::default();
        let mut request = registry.begin("openai");
        assert_eq!(registry.snapshot().by_upstream["openai"].active_streams, 1);
        request.add_request_bytes(10);
        request.add_response_bytes(20);
        request.finish_response(StatusCode::OK, 15);

        let stats = &registry.snapshot().by_upstream["openai"];
        assert_eq!(stats.accepted_requests, 1);
        assert_eq!(stats.responses_2xx, 1);
        assert_eq!(stats.request_bytes, 10);
        assert_eq!(stats.response_bytes, 20);
        assert_eq!(stats.duration_count, 1);
        assert_eq!(stats.duration_total_ms, 15);
        assert_eq!(stats.duration_max_ms, 15);
        assert_eq!(stats.active_streams, 0);
    }

    #[test]
    fn dropped_request_counts_network_error_and_releases_active_stream() {
        let registry = StatsRegistry::default();
        {
            let mut request = registry.begin("openai");
            request.add_request_bytes(3);
            request.network_error();
        }
        let stats = &registry.snapshot().by_upstream["openai"];
        assert_eq!(stats.network_errors, 1);
        assert_eq!(stats.active_streams, 0);
    }
}
