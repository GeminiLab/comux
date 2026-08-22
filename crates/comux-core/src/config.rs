use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use url::Url;

use crate::ConfigError;

const NULL_UPSTREAM: &str = "null";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RedirectPolicy {
    Always,
    #[serde(rename = "default")]
    Default,
    Never,
}

impl Default for RedirectPolicy {
    fn default() -> Self {
        Self::Default
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpstreamConfig {
    pub base_url: String,
    pub auth_token: String,
    #[serde(default)]
    pub redirect_policy: RedirectPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListenerConfig {
    #[serde(default = "default_bind")]
    pub bind: String,
    pub port: u16,
    #[serde(default)]
    pub token: String,
}

impl ListenerConfig {
    fn admin_default() -> Self {
        Self {
            bind: default_bind(),
            port: 6609,
            token: String::new(),
        }
    }

    fn downstream_default() -> Self {
        Self {
            bind: default_bind(),
            port: 6608,
            token: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionConfig {
    #[serde(default = "default_connect_timeout")]
    pub connect_timeout_secs: u64,
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout_secs: u64,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            connect_timeout_secs: default_connect_timeout(),
            idle_timeout_secs: default_idle_timeout(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProxyConfig {
    pub active_upstream: String,
    pub admin: ListenerConfig,
    pub downstream: ListenerConfig,
    pub connection: ConnectionConfig,
    pub upstreams: BTreeMap<String, UpstreamConfig>,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            active_upstream: NULL_UPSTREAM.to_owned(),
            admin: ListenerConfig::admin_default(),
            downstream: ListenerConfig::downstream_default(),
            connection: ConnectionConfig::default(),
            upstreams: BTreeMap::new(),
        }
    }
}

impl ProxyConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.admin.port == 0 {
            return Err(ConfigError::Invalid(
                "admin port must be non-zero".to_owned(),
            ));
        }
        if self.downstream.port == 0 {
            return Err(ConfigError::Invalid(
                "downstream port must be non-zero".to_owned(),
            ));
        }
        if self.connection.connect_timeout_secs == 0 {
            return Err(ConfigError::Invalid(
                "connect timeout must be positive".to_owned(),
            ));
        }
        if self.connection.idle_timeout_secs == 0 {
            return Err(ConfigError::Invalid(
                "idle timeout must be positive".to_owned(),
            ));
        }
        if self.active_upstream != NULL_UPSTREAM
            && !self.upstreams.contains_key(&self.active_upstream)
        {
            return Err(ConfigError::Invalid(format!(
                "active upstream {:?} does not exist",
                self.active_upstream
            )));
        }
        for (name, upstream) in &self.upstreams {
            if name.is_empty() || name == NULL_UPSTREAM {
                return Err(ConfigError::Invalid(format!(
                    "invalid upstream name {:?}",
                    name
                )));
            }
            if upstream.auth_token.is_empty() {
                return Err(ConfigError::Invalid(format!(
                    "upstream {:?} has an empty auth token",
                    name
                )));
            }
            let url = Url::parse(&upstream.base_url).map_err(|error| {
                ConfigError::Invalid(format!(
                    "upstream {:?} has an invalid base URL: {error}",
                    name
                ))
            })?;
            if !matches!(url.scheme(), "http" | "https") {
                return Err(ConfigError::Invalid(format!(
                    "upstream {:?} base URL must use http or https",
                    name
                )));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CliConfig {
    pub admin_endpoint: String,
    pub admin_token: String,
}

impl CliConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.admin_token.is_empty() {
            return Err(ConfigError::Invalid(
                "admin token must not be empty".to_owned(),
            ));
        }
        let endpoint = Url::parse(&self.admin_endpoint)
            .map_err(|error| ConfigError::Invalid(format!("invalid admin endpoint: {error}")))?;
        if !matches!(endpoint.scheme(), "http" | "https") {
            return Err(ConfigError::Invalid(
                "admin endpoint must use http or https".to_owned(),
            ));
        }
        Ok(())
    }
}

fn default_bind() -> String {
    "localhost".to_owned()
}

fn default_connect_timeout() -> u64 {
    10
}

fn default_idle_timeout() -> u64 {
    120
}

#[cfg(test)]
mod tests {
    use super::{CliConfig, ProxyConfig, RedirectPolicy};
    use std::collections::BTreeMap;

    #[test]
    fn parses_grouped_proxy_configuration() {
        let source = r#"
            active_upstream = "openai"

            [admin]
            bind = "127.0.0.1"
            port = 6609
            token = "admin-secret"

            [downstream]
            bind = "127.0.0.1"
            port = 6608
            token = "client-secret"

            [connection]
            connect_timeout_secs = 10
            idle_timeout_secs = 120

            [upstreams.openai]
            base_url = "https://api.openai.com/v1"
            auth_token = "upstream-secret"
            redirect_policy = "default"
        "#;

        let config: ProxyConfig = toml::from_str(source).expect("valid proxy TOML");
        assert_eq!(config.active_upstream, "openai");
        assert_eq!(config.admin.bind, "127.0.0.1");
        assert_eq!(config.admin.port, 6609);
        assert_eq!(config.downstream.port, 6608);
        assert_eq!(config.connection.idle_timeout_secs, 120);
        assert_eq!(
            config.upstreams["openai"].redirect_policy,
            RedirectPolicy::Default
        );
    }

    #[test]
    fn applies_proxy_defaults() {
        let config: ProxyConfig = toml::from_str("").expect("empty config uses defaults");

        assert_eq!(config.active_upstream, "null");
        assert_eq!(config.admin.bind, "localhost");
        assert_eq!(config.admin.port, 6609);
        assert_eq!(config.downstream.bind, "localhost");
        assert_eq!(config.downstream.port, 6608);
        assert_eq!(config.connection.connect_timeout_secs, 10);
        assert_eq!(config.connection.idle_timeout_secs, 120);
        assert!(config.upstreams.is_empty());
    }

    #[test]
    fn parses_all_redirect_policies() {
        for (text, expected) in [
            ("always", RedirectPolicy::Always),
            ("default", RedirectPolicy::Default),
            ("never", RedirectPolicy::Never),
        ] {
            let source = format!(
                "[upstreams.test]\nbase_url = \"https://example.test\"\nauth_token = \"x\"\nredirect_policy = \"{text}\"\n"
            );
            let config: ProxyConfig = toml::from_str(&source).expect("valid redirect policy");
            assert_eq!(config.upstreams["test"].redirect_policy, expected);
        }
    }

    #[test]
    fn rejects_invalid_proxy_configuration() {
        let mut config = ProxyConfig::default();
        config.admin.port = 0;
        assert!(config.validate().is_err());

        let mut config = ProxyConfig::default();
        config.upstreams = BTreeMap::from([(
            "bad".to_owned(),
            super::UpstreamConfig {
                base_url: "file:///tmp/nope".to_owned(),
                auth_token: "secret".to_owned(),
                redirect_policy: RedirectPolicy::Default,
            },
        )]);
        assert!(config.validate().is_err());
    }

    #[test]
    fn validates_cli_configuration() {
        let config = CliConfig {
            admin_endpoint: "http://127.0.0.1:6609".to_owned(),
            admin_token: "secret".to_owned(),
        };
        assert!(config.validate().is_ok());
    }
}
