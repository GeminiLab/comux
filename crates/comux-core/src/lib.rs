#![forbid(unsafe_code)]

pub mod api;
pub mod config;
pub mod error;
pub mod persistence;
pub mod secrets;
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
