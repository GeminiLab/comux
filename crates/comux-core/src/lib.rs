//! Shared configuration, persistence, secrets, API types, and runtime state
//! for the comux proxy and management clients.
//!
//! This crate contains no HTTP server or terminal UI. It defines the data and
//! state-management contracts used by both `comux-proxy` and `comux`.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

/// Management API request and response types.
pub mod api;
/// Persisted proxy and CLI configuration types.
pub mod config;
/// Configuration validation and persistence errors.
pub mod error;
/// Atomic loading and saving of proxy and CLI configuration.
pub mod persistence;
/// Token generation and display-safe masking helpers.
pub mod secrets;
/// Validated, mutable proxy runtime state.
pub mod state;

pub use config::{
    CliConfig, ConnectionConfig, ListenerConfig, ProxyConfig, RedirectPolicy, UpstreamConfig,
};
pub use error::ConfigError;
pub use secrets::{generate_token, mask_token};
pub use state::{RuntimeState, StateError};

#[cfg(test)]
mod tests {
    #[test]
    fn workspace_crates_compile() {
        assert_eq!(2 + 2, 4);
    }
}
