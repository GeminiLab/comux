use comux_core::persistence::load_cli_config;
use comux_core::persistence::save_cli_config;
use comux_core::{CliConfig, ConfigError};
use rpassword::prompt_password;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};

pub fn default_config_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config")
        .join("comux.toml")
}

pub fn resolve_config(
    path: &Path,
    endpoint_override: Option<String>,
    token_override: Option<String>,
) -> Result<CliConfig, ConfigError> {
    let file = load_cli_config(path)?.unwrap_or(CliConfig {
        admin_endpoint: String::new(),
        admin_token: String::new(),
    });
    let config = CliConfig {
        admin_endpoint: endpoint_override.unwrap_or(file.admin_endpoint),
        admin_token: token_override.unwrap_or(file.admin_token),
    };
    config.validate()?;
    Ok(config)
}

pub fn resolve_config_interactive(
    path: &Path,
    endpoint_override: Option<String>,
    token_override: Option<String>,
) -> Result<CliConfig, ConfigError> {
    let file = load_cli_config(path)?.unwrap_or(CliConfig {
        admin_endpoint: String::new(),
        admin_token: String::new(),
    });
    let endpoint = match endpoint_override
        .clone()
        .or_else(|| non_empty(&file.admin_endpoint))
    {
        Some(value) => value,
        None => prompt_endpoint()?,
    };
    let token = match token_override
        .clone()
        .or_else(|| non_empty(&file.admin_token))
    {
        Some(value) => value,
        None => {
            if !io::stdin().is_terminal() {
                return Err(ConfigError::Invalid(
                    "admin token is missing and stdin is not interactive".to_owned(),
                ));
            }
            prompt_password("Admin token: ").map_err(ConfigError::Io)?
        }
    };
    let config = CliConfig {
        admin_endpoint: endpoint,
        admin_token: token,
    };
    config.validate()?;
    if endpoint_override.is_none() && token_override.is_none() && config != file {
        save_cli_config(path, &config)?;
    }
    Ok(config)
}

pub fn save_value(
    path: &Path,
    key: super::args::ConfigKey,
    value: String,
) -> Result<CliConfig, ConfigError> {
    let mut config = load_cli_config(path)?.unwrap_or(CliConfig {
        admin_endpoint: String::new(),
        admin_token: String::new(),
    });
    match key {
        super::args::ConfigKey::Endpoint => config.admin_endpoint = value,
        super::args::ConfigKey::AdminToken => config.admin_token = value,
    }
    config.validate()?;
    save_cli_config(path, &config)?;
    Ok(config)
}

fn non_empty(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_owned())
}

fn prompt_endpoint() -> Result<String, ConfigError> {
    if !io::stdin().is_terminal() {
        return Err(ConfigError::Invalid(
            "admin endpoint is missing and stdin is not interactive".to_owned(),
        ));
    }
    print!("Admin endpoint: ");
    io::stdout().flush()?;
    let mut endpoint = String::new();
    io::stdin().read_line(&mut endpoint)?;
    Ok(endpoint.trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::{default_config_path, resolve_config};
    use comux_core::persistence::save_cli_config;
    use comux_core::CliConfig;
    use tempfile::tempdir;

    #[test]
    fn default_path_uses_comux_under_dot_config() {
        assert!(default_config_path().ends_with(".config/comux.toml"));
    }

    #[test]
    fn command_overrides_take_precedence_over_local_config() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("comux.toml");
        save_cli_config(
            &path,
            &CliConfig {
                admin_endpoint: "http://file.example".to_owned(),
                admin_token: "file-token".to_owned(),
            },
        )
        .unwrap();
        let config = resolve_config(
            &path,
            Some("http://override.example".to_owned()),
            Some("override-token".to_owned()),
        )
        .unwrap();
        assert_eq!(config.admin_endpoint, "http://override.example");
        assert_eq!(config.admin_token, "override-token");
    }
}
