#![forbid(unsafe_code)]

pub mod config;
pub mod error;
pub mod persistence;
pub mod secrets;

pub use config::{
    CliConfig, ConnectionConfig, ListenerConfig, ProxyConfig, RedirectPolicy, UpstreamConfig,
};
pub use error::ConfigError;

#[cfg(test)]
mod tests {
    #[test]
    fn workspace_crates_compile() {
        assert_eq!(2 + 2, 4);
    }
}
