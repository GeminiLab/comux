//! Errors raised while validating or persisting configuration.

use thiserror::Error;

/// An error produced while validating, reading, or writing configuration.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// A parsed configuration violates a semantic validation rule.
    #[error("invalid configuration: {0}")]
    Invalid(String),
    /// Reading, creating, syncing, or replacing a configuration file failed.
    #[error("configuration I/O failed: {0}")]
    Io(#[from] std::io::Error),
    /// TOML serialization or deserialization failed.
    #[error("configuration serialization failed: {0}")]
    Serialization(String),
}
