//! Serializable request and response types used by the management API.

use crate::{mask_token, RedirectPolicy};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A summary of the proxy's current listeners and upstream selection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StatusResponse {
    /// The selected upstream name, or the reserved name `null`.
    pub active_upstream: String,
    /// The host or address used by the downstream listener.
    pub downstream_bind: String,
    /// The TCP port used by the downstream listener.
    pub downstream_port: u16,
    /// The host or address used by the management listener.
    pub admin_bind: String,
    /// The TCP port used by the management listener.
    pub admin_port: u16,
    /// The configured upstream names in sorted order.
    pub upstream_names: Vec<String>,
}

/// A management-facing view of one configured upstream.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpstreamSummary {
    /// The unique upstream name.
    pub name: String,
    /// The upstream base URL.
    pub base_url: String,
    /// The masked token, or the full token after an explicit authenticated reveal.
    pub auth_token: String,
    /// The redirect behavior configured for the upstream.
    pub redirect_policy: RedirectPolicy,
}

impl UpstreamSummary {
    /// Creates a summary whose authentication token is safe for ordinary display.
    pub fn masked(
        name: impl Into<String>,
        base_url: impl Into<String>,
        auth_token: &str,
        policy: RedirectPolicy,
    ) -> Self {
        Self {
            name: name.into(),
            base_url: base_url.into(),
            auth_token: mask_token(auth_token),
            redirect_policy: policy,
        }
    }
}

/// A complete replacement payload for one upstream configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpstreamMutation {
    /// The upstream base URL.
    pub base_url: String,
    /// The bearer token sent to the upstream.
    pub auth_token: String,
    /// The redirect behavior for requests to the upstream.
    pub redirect_policy: RedirectPolicy,
}

/// A request to change the selected upstream.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActiveMutation {
    /// The configured upstream name to select.
    pub name: String,
}

/// The downstream client token returned by the management API.
///
/// The token is masked by default and is returned in full only after an
/// authenticated request explicitly asks for `reveal=full`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClientTokenResponse {
    /// The masked or explicitly revealed downstream token.
    pub token: String,
}

/// Process-local transport counters for one upstream.
///
/// These values are kept in memory and reset whenever the proxy restarts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TransportStats {
    /// Requests accepted for forwarding.
    pub accepted_requests: u64,
    /// Completed responses with a 2xx status.
    pub responses_2xx: u64,
    /// Completed responses with a 3xx status.
    pub responses_3xx: u64,
    /// Completed responses with a 4xx status.
    pub responses_4xx: u64,
    /// Completed responses with any other status, normally 5xx.
    pub responses_5xx: u64,
    /// Requests or response streams that ended with a network failure.
    pub network_errors: u64,
    /// Downstream request-body bytes read by the proxy.
    pub request_bytes: u64,
    /// Upstream response-body bytes emitted by the proxy.
    pub response_bytes: u64,
    /// Responses included in the duration aggregates.
    pub duration_count: u64,
    /// Sum of completed response durations in milliseconds.
    pub duration_total_ms: u64,
    /// Longest completed response duration in milliseconds.
    pub duration_max_ms: u64,
    /// Accepted forwarding requests not yet terminally accounted or released.
    pub active_streams: u64,
}

/// A statistics snapshot keyed by upstream name.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct StatsResponse {
    /// The current process-local counters for each observed upstream.
    pub by_upstream: BTreeMap<String, TransportStats>,
}

/// The standard JSON error response envelope.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApiErrorBody {
    /// The structured error details.
    pub error: ApiError,
}

/// Machine-readable and human-readable details for an API failure.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApiError {
    /// The machine-readable error category serialized as `type`.
    #[serde(rename = "type")]
    pub error_type: String,
    /// A human-readable explanation of the failure.
    pub message: String,
}
