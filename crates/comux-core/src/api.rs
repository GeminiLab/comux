use crate::{mask_token, RedirectPolicy};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StatusResponse {
    pub active_upstream: String,
    pub downstream_bind: String,
    pub downstream_port: u16,
    pub admin_bind: String,
    pub admin_port: u16,
    pub upstream_names: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpstreamSummary {
    pub name: String,
    pub base_url: String,
    pub auth_token: String,
    pub redirect_policy: RedirectPolicy,
}

impl UpstreamSummary {
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpstreamMutation {
    pub base_url: String,
    pub auth_token: String,
    pub redirect_policy: RedirectPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActiveMutation {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClientTokenResponse {
    pub token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransportStats {
    pub accepted_requests: u64,
    pub responses_2xx: u64,
    pub responses_3xx: u64,
    pub responses_4xx: u64,
    pub responses_5xx: u64,
    pub network_errors: u64,
    pub request_bytes: u64,
    pub response_bytes: u64,
    pub duration_count: u64,
    pub duration_total_ms: u64,
    pub duration_max_ms: u64,
    pub active_streams: u64,
}

impl Default for TransportStats {
    fn default() -> Self {
        Self {
            accepted_requests: 0,
            responses_2xx: 0,
            responses_3xx: 0,
            responses_4xx: 0,
            responses_5xx: 0,
            network_errors: 0,
            request_bytes: 0,
            response_bytes: 0,
            duration_count: 0,
            duration_total_ms: 0,
            duration_max_ms: 0,
            active_streams: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct StatsResponse {
    pub by_upstream: BTreeMap<String, TransportStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApiErrorBody {
    pub error: ApiError,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApiError {
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
}
