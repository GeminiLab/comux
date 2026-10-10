//! Atomic loading and saving of proxy and CLI TOML configuration.

use crate::config::{CliConfig, ProxyConfig};
use crate::error::ConfigError;
use crate::secrets::generate_token;
use std::fs;
use std::io::Write;
use std::path::Path;
use tempfile::NamedTempFile;

/// A proxy configuration together with its startup initialization results.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedProxyConfig {
    /// The validated configuration ready to become runtime state.
    pub config: ProxyConfig,
    /// Whether startup generated the admin token and may display it once.
    pub generated_admin_token: bool,
    /// Whether startup generated the downstream token and may display it once.
    pub generated_downstream_token: bool,
}

/// Loads a proxy configuration, generating and persisting missing listener tokens.
///
/// A missing file starts from [`ProxyConfig::default`]. The returned flags
/// identify credentials generated during this call so the proxy can display
/// each new value once.
pub fn load_proxy_config_or_initialize(path: &Path) -> Result<LoadedProxyConfig, ConfigError> {
    let mut config = if path.exists() {
        let source = fs::read_to_string(path)?;
        toml::from_str::<ProxyConfig>(&source)
            .map_err(|error| ConfigError::Serialization(format!("invalid TOML: {error}")))?
    } else {
        ProxyConfig::default()
    };

    // Empty listener tokens are initialization requests, so generate them
    // before validating and remember which values the caller may print once.
    let generated_admin_token = config.admin.token.is_empty();
    let generated_downstream_token = config.downstream.token.is_empty();
    if generated_admin_token {
        config.admin.token = generate_token();
    }
    if generated_downstream_token {
        config.downstream.token = generate_token();
    }

    config.validate()?;
    if generated_admin_token || generated_downstream_token || !path.exists() {
        save_proxy_config(path, &config)?;
    }

    Ok(LoadedProxyConfig {
        config,
        generated_admin_token,
        generated_downstream_token,
    })
}

/// Validates and atomically saves a complete proxy configuration.
pub fn save_proxy_config(path: &Path, config: &ProxyConfig) -> Result<(), ConfigError> {
    config.validate()?;
    save_toml(path, config)
}

/// Loads optional local CLI configuration from `path`.
///
/// A missing file returns `Ok(None)`. Present files may omit either setting
/// while interactive setup is still incomplete.
pub fn load_cli_config(path: &Path) -> Result<Option<CliConfig>, ConfigError> {
    if !path.exists() {
        return Ok(None);
    }
    let source = fs::read_to_string(path)?;
    let config = toml::from_str::<CliConfig>(&source)
        .map_err(|error| ConfigError::Serialization(format!("invalid CLI TOML: {error}")))?;
    config.validate_partial()?;
    Ok(Some(config))
}

/// Validates the fields currently present and atomically saves CLI configuration.
pub fn save_cli_config(path: &Path, config: &CliConfig) -> Result<(), ConfigError> {
    config.validate_partial()?;
    save_toml(path, config)
}

/// Serializes `value` and atomically replaces the file at `path`.
fn save_toml<T: serde::Serialize>(path: &Path, value: &T) -> Result<(), ConfigError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let serialized = toml::to_string_pretty(value)
        .map_err(|error| ConfigError::Serialization(error.to_string()))?;
    let mut temporary = NamedTempFile::new_in(parent)?;
    set_private_mode(temporary.path())?;
    temporary.write_all(serialized.as_bytes())?;
    temporary.flush()?;
    temporary.as_file().sync_all()?;
    // Persist a fully written and synced sibling temporary file so readers
    // never observe partially serialized TOML.
    temporary
        .persist(path)
        .map_err(|error| ConfigError::Io(error.error))?;
    set_private_mode(path)?;
    Ok(())
}

#[cfg(unix)]
/// Restricts a configuration file to its owning user on Unix.
fn set_private_mode(path: &Path) -> Result<(), ConfigError> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
/// Leaves permissions unchanged where Unix mode bits are unavailable.
fn set_private_mode(_path: &Path) -> Result<(), ConfigError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        load_cli_config, load_proxy_config_or_initialize, save_cli_config, save_proxy_config,
    };
    use crate::{CliConfig, ProxyConfig};
    use tempfile::tempdir;

    #[test]
    fn initializes_missing_proxy_tokens_and_persists_them() {
        let directory = tempdir().expect("temporary directory");
        let path = directory.path().join("proxy.toml");

        let loaded = load_proxy_config_or_initialize(&path).expect("initialize proxy config");
        assert!(!loaded.config.admin.token.is_empty());
        assert!(!loaded.config.downstream.token.is_empty());
        assert!(loaded.generated_admin_token);
        assert!(loaded.generated_downstream_token);

        let reloaded = load_proxy_config_or_initialize(&path).expect("reload proxy config");
        assert_eq!(reloaded.config, loaded.config);
        assert!(!reloaded.generated_admin_token);
        assert!(!reloaded.generated_downstream_token);
    }

    #[test]
    fn saves_and_loads_proxy_toml_atomically() {
        let directory = tempdir().expect("temporary directory");
        let path = directory.path().join("nested").join("proxy.toml");
        let mut config = ProxyConfig::default();
        config.admin.token = "admin-secret".to_owned();
        config.downstream.token = "client-secret".to_owned();

        save_proxy_config(&path, &config).expect("save proxy config");
        let loaded = load_proxy_config_or_initialize(&path).expect("load saved config");
        assert_eq!(loaded.config, config);
        assert!(!loaded.generated_admin_token);
        assert!(!loaded.generated_downstream_token);
    }

    #[test]
    fn cli_config_is_optional_and_can_be_saved() {
        let directory = tempdir().expect("temporary directory");
        let path = directory.path().join("cli.toml");
        assert!(load_cli_config(&path)
            .expect("missing CLI config")
            .is_none());

        let config = CliConfig {
            admin_endpoint: "http://127.0.0.1:6609".to_owned(),
            admin_token: "admin-secret".to_owned(),
        };
        save_cli_config(&path, &config).expect("save CLI config");
        assert_eq!(
            load_cli_config(&path).expect("load CLI config"),
            Some(config)
        );
    }

    #[test]
    fn cli_config_can_temporarily_store_one_missing_key() {
        let directory = tempdir().expect("temporary directory");
        let path = directory.path().join("cli.toml");
        let config = CliConfig {
            admin_endpoint: "http://127.0.0.1:6609".to_owned(),
            admin_token: String::new(),
        };
        save_cli_config(&path, &config).expect("save partial CLI config");
        assert_eq!(load_cli_config(&path).unwrap(), Some(config));
    }
}
