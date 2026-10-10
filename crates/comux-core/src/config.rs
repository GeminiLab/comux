//! Persisted proxy and CLI configuration schemas and validation rules.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use url::Url;

use crate::ConfigError;

/// Reserved active-upstream name representing no selected target.
const NULL_UPSTREAM: &str = "null";

/// Controls how requests to an upstream handle HTTP redirects.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RedirectPolicy {
    /// Comux follows redirects and injects the upstream token on every hop.
    ///
    /// This includes cross-host redirects and must be used only with trusted
    /// redirect targets.
    Always,
    /// Uses reqwest's default redirect behavior.
    #[serde(rename = "default")]
    #[default]
    Default,
    /// Returns redirect responses to the downstream client without following them.
    Never,
}

/// Connection and authentication settings for one named upstream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpstreamConfig {
    /// The HTTP or HTTPS base URL to which downstream paths are appended.
    pub base_url: String,
    /// The sensitive bearer token sent to this upstream.
    pub auth_token: String,
    /// The policy applied when this upstream returns a redirect.
    #[serde(default)]
    pub redirect_policy: RedirectPolicy,
}

/// Bind address and bearer credential for one proxy listener.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListenerConfig {
    /// The hostname or IP address on which to listen.
    #[serde(default = "default_bind")]
    pub bind: String,
    /// The non-zero TCP port on which to listen.
    pub port: u16,
    /// The bearer token accepted by the listener.
    ///
    /// An empty proxy listener token is generated during startup initialization.
    #[serde(default)]
    pub token: String,
}

impl ListenerConfig {
    /// Returns the default management listener with an uninitialized token.
    fn admin_default() -> Self {
        Self {
            bind: default_bind(),
            port: 6609,
            token: String::new(),
        }
    }

    /// Returns the default downstream listener with an uninitialized token.
    fn downstream_default() -> Self {
        Self {
            bind: default_bind(),
            port: 6608,
            token: String::new(),
        }
    }
}

/// Timeout settings used when communicating with an upstream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionConfig {
    /// Maximum time allowed to establish an upstream connection, in seconds.
    #[serde(default = "default_connect_timeout")]
    pub connect_timeout_secs: u64,
    /// Maximum idle time between request or response body chunks, in seconds.
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout_secs: u64,
}

impl Default for ConnectionConfig {
    /// Uses a 10-second connect timeout and a 120-second body idle timeout.
    fn default() -> Self {
        Self {
            connect_timeout_secs: default_connect_timeout(),
            idle_timeout_secs: default_idle_timeout(),
        }
    }
}

/// Complete persisted configuration for a proxy process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProxyConfig {
    /// The selected upstream name, or the reserved value `null`.
    pub active_upstream: String,
    /// The separately authenticated management listener.
    pub admin: ListenerConfig,
    /// The listener that accepts Codex requests.
    pub downstream: ListenerConfig,
    /// Upstream connection and streaming timeout settings.
    pub connection: ConnectionConfig,
    /// Named upstream configurations in deterministic key order.
    pub upstreams: BTreeMap<String, UpstreamConfig>,
}

impl Default for ProxyConfig {
    /// Creates loopback listeners, default timeouts, and no selected upstream.
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
    /// Validates the semantic constraints required by a running proxy.
    ///
    /// Listener tokens may remain empty because startup initialization
    /// generates them. Ports and timeouts must be non-zero, the active target
    /// must exist unless it is `null`, and every upstream must have a valid
    /// HTTP(S) URL and non-empty credential.
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
        // The reserved sentinel deliberately selects no target; every other
        // active name must resolve to a configured upstream.
        if self.active_upstream != NULL_UPSTREAM
            && !self.upstreams.contains_key(&self.active_upstream)
        {
            return Err(ConfigError::Invalid(format!(
                "active upstream {:?} does not exist",
                self.active_upstream
            )));
        }
        // Each persisted target must be independently usable before the
        // configuration can become live state.
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

/// Local CLI connection settings for the proxy management endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CliConfig {
    /// The HTTP or HTTPS management endpoint URL.
    #[serde(default)]
    pub admin_endpoint: String,
    /// The bearer token accepted by the management endpoint.
    #[serde(default)]
    pub admin_token: String,
}

impl CliConfig {
    /// Validates a complete CLI configuration.
    ///
    /// The endpoint must use HTTP or HTTPS and the admin token must be present.
    pub fn validate(&self) -> Result<(), ConfigError> {
        self.validate_partial()?;
        if self.admin_token.is_empty() {
            return Err(ConfigError::Invalid(
                "admin token must not be empty".to_owned(),
            ));
        }
        Ok(())
    }

    /// Validates fields that are currently present in a partial CLI configuration.
    ///
    /// A missing endpoint is accepted so interactive setup can save one value
    /// at a time.
    pub fn validate_partial(&self) -> Result<(), ConfigError> {
        if self.admin_endpoint.is_empty() {
            return Ok(());
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

/// Returns the serde default listener host.
fn default_bind() -> String {
    "localhost".to_owned()
}

/// Returns the serde default upstream connection timeout in seconds.
fn default_connect_timeout() -> u64 {
    10
}

/// Returns the serde default body idle timeout in seconds.
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

        let config = ProxyConfig {
            upstreams: BTreeMap::from([(
                "bad".to_owned(),
                super::UpstreamConfig {
                    base_url: "file:///tmp/nope".to_owned(),
                    auth_token: "secret".to_owned(),
                    redirect_policy: RedirectPolicy::Default,
                },
            )]),
            ..ProxyConfig::default()
        };
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
