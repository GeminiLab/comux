use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("invalid configuration: {0}")]
    Invalid(String),
    #[error("configuration I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("configuration serialization failed: {0}")]
    Serialization(String),
}
