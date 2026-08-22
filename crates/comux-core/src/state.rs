use crate::config::{ProxyConfig, UpstreamConfig};
use crate::error::ConfigError;
use crate::secrets::generate_token;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StateError {
    #[error("upstream {0:?} was not found")]
    UnknownUpstream(String),
    #[error("upstream {0:?} already exists")]
    DuplicateUpstream(String),
    #[error(transparent)]
    InvalidConfig(#[from] ConfigError),
}

#[derive(Debug, Clone)]
pub struct RuntimeState {
    config: ProxyConfig,
}

impl RuntimeState {
    pub fn new(config: ProxyConfig) -> Result<Self, ConfigError> {
        config.validate()?;
        Ok(Self { config })
    }

    pub fn config(&self) -> &ProxyConfig {
        &self.config
    }

    pub fn snapshot_active_upstream(&self) -> Option<(String, UpstreamConfig)> {
        let name = &self.config.active_upstream;
        self.config
            .upstreams
            .get(name)
            .cloned()
            .map(|upstream| (name.clone(), upstream))
    }

    pub fn replace_upstream(
        &mut self,
        name: impl Into<String>,
        upstream: UpstreamConfig,
    ) -> Result<(), StateError> {
        let name = name.into();
        if name.is_empty() || name == "null" {
            return Err(StateError::InvalidConfig(ConfigError::Invalid(
                "upstream name is reserved or empty".to_owned(),
            )));
        }
        let mut candidate = self.config.clone();
        candidate.upstreams.insert(name, upstream);
        candidate.validate()?;
        self.config = candidate;
        Ok(())
    }

    pub fn delete_upstream(&mut self, name: &str) -> Result<(), StateError> {
        let mut candidate = self.config.clone();
        if candidate.upstreams.remove(name).is_none() {
            return Err(StateError::UnknownUpstream(name.to_owned()));
        }
        if candidate.active_upstream == name {
            candidate.active_upstream = "null".to_owned();
        }
        candidate.validate()?;
        self.config = candidate;
        Ok(())
    }

    pub fn select_upstream(&mut self, name: &str) -> Result<(), StateError> {
        if !self.config.upstreams.contains_key(name) {
            return Err(StateError::UnknownUpstream(name.to_owned()));
        }
        let mut candidate = self.config.clone();
        candidate.active_upstream = name.to_owned();
        candidate.validate()?;
        self.config = candidate;
        Ok(())
    }

    pub fn regenerate_client_token(&mut self) -> String {
        let token = generate_token();
        self.config.downstream.token = token.clone();
        token
    }
}

#[cfg(test)]
mod tests {
    use super::RuntimeState;
    use crate::{ProxyConfig, RedirectPolicy, UpstreamConfig};

    fn state_with_upstream() -> RuntimeState {
        let mut config = ProxyConfig::default();
        config.admin.token = "admin".to_owned();
        config.downstream.token = "client".to_owned();
        config.upstreams.insert(
            "one".to_owned(),
            UpstreamConfig {
                base_url: "https://one.example.test/v1".to_owned(),
                auth_token: "one-secret".to_owned(),
                redirect_policy: RedirectPolicy::Default,
            },
        );
        RuntimeState::new(config).expect("valid state")
    }

    #[test]
    fn selects_existing_upstream_and_rejects_unknown_names() {
        let mut state = state_with_upstream();
        state.select_upstream("one").expect("select existing");
        assert_eq!(state.config().active_upstream, "one");
        assert!(state.select_upstream("missing").is_err());
    }

    #[test]
    fn deleting_active_upstream_selects_null() {
        let mut state = state_with_upstream();
        state.select_upstream("one").expect("select existing");
        state.delete_upstream("one").expect("delete existing");
        assert_eq!(state.config().active_upstream, "null");
        assert!(state.config().upstreams.is_empty());
    }

    #[test]
    fn request_snapshot_owns_upstream_configuration() {
        let mut state = state_with_upstream();
        state.select_upstream("one").expect("select existing");
        let snapshot = state.snapshot_active_upstream().expect("active snapshot");
        state
            .replace_upstream(
                "one",
                UpstreamConfig {
                    base_url: "https://new.example.test".to_owned(),
                    auth_token: "new-secret".to_owned(),
                    redirect_policy: RedirectPolicy::Never,
                },
            )
            .expect("replace existing");
        assert_eq!(snapshot.1.base_url, "https://one.example.test/v1");
        assert_eq!(
            state.snapshot_active_upstream().unwrap().1.base_url,
            "https://new.example.test"
        );
    }
}
