use crate::config::{CliConfig, ProxyConfig};
use crate::error::ConfigError;
use crate::secrets::generate_token;
use std::fs;
use std::io::Write;
use std::path::Path;
use tempfile::NamedTempFile;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedProxyConfig {
    pub config: ProxyConfig,
    pub generated_admin_token: bool,
    pub generated_downstream_token: bool,
}

pub fn load_proxy_config_or_initialize(path: &Path) -> Result<LoadedProxyConfig, ConfigError> {
    let mut config = if path.exists() {
        let source = fs::read_to_string(path)?;
        toml::from_str::<ProxyConfig>(&source)
            .map_err(|error| ConfigError::Serialization(format!("invalid TOML: {error}")))?
    } else {
        ProxyConfig::default()
    };

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

pub fn save_proxy_config(path: &Path, config: &ProxyConfig) -> Result<(), ConfigError> {
    config.validate()?;
    save_toml(path, config)
}

pub fn load_cli_config(path: &Path) -> Result<Option<CliConfig>, ConfigError> {
    if !path.exists() {
        return Ok(None);
    }
    let source = fs::read_to_string(path)?;
    let config = toml::from_str::<CliConfig>(&source)
        .map_err(|error| ConfigError::Serialization(format!("invalid CLI TOML: {error}")))?;
    config.validate()?;
    Ok(Some(config))
}

pub fn save_cli_config(path: &Path, config: &CliConfig) -> Result<(), ConfigError> {
    config.validate()?;
    save_toml(path, config)
}

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
    temporary
        .persist(path)
        .map_err(|error| ConfigError::Io(error.error))?;
    set_private_mode(path)?;
    Ok(())
}

#[cfg(unix)]
fn set_private_mode(path: &Path) -> Result<(), ConfigError> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
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
}
