use super::events::Action;
use crate::client::ApiClient;
use comux_core::api::{StatsResponse, StatusResponse, UpstreamSummary};

pub struct App {
    pub client: Option<ApiClient>,
    pub status: Option<StatusResponse>,
    pub stats: Option<StatsResponse>,
    pub upstreams: Vec<UpstreamSummary>,
    pub selected: usize,
    pub message: String,
    pub show_full_tokens: bool,
}

impl App {
    pub fn new(client: ApiClient, show_full_tokens: bool) -> Self {
        Self {
            client: Some(client),
            ..Self::placeholder_with_tokens(show_full_tokens)
        }
    }

    pub fn placeholder() -> Self {
        Self::placeholder_with_tokens(false)
    }

    fn placeholder_with_tokens(show_full_tokens: bool) -> Self {
        Self {
            client: None,
            status: None,
            stats: None,
            upstreams: Vec::new(),
            selected: 0,
            message: "connecting...".into(),
            show_full_tokens,
        }
    }

    pub async fn refresh(&mut self) {
        let Some(client) = self.client.clone() else {
            return;
        };
        match (
            client.status().await,
            client.stats().await,
            client.upstreams(false).await,
        ) {
            (Ok(status), Ok(stats), Ok(upstreams)) => {
                self.status = Some(status);
                self.stats = Some(stats);
                self.upstreams = upstreams;
                if self.selected >= self.upstreams.len() {
                    self.selected = self.upstreams.len().saturating_sub(1);
                }
                self.message = "connected".into();
            }
            (status, stats, upstreams) => {
                let error = status
                    .err()
                    .or_else(|| stats.err())
                    .or_else(|| upstreams.err())
                    .map(|error| error.to_string())
                    .unwrap_or_else(|| "management request failed".into());
                self.message = error;
            }
        }
    }

    pub async fn handle(&mut self, action: Action) -> bool {
        match action {
            Action::Quit => return true,
            Action::Refresh => self.refresh().await,
            Action::Up => self.selected = self.selected.saturating_sub(1),
            Action::Down => {
                if !self.upstreams.is_empty() {
                    self.selected = (self.selected + 1).min(self.upstreams.len() - 1);
                }
            }
            Action::Select => {
                if let (Some(client), Some(upstream)) =
                    (self.client.clone(), self.upstreams.get(self.selected))
                {
                    match client.select_upstream(&upstream.name).await {
                        Ok(()) => self.message = format!("active upstream: {}", upstream.name),
                        Err(error) => self.message = error.to_string(),
                    }
                    self.refresh().await;
                }
            }
            Action::Remove => {
                if let (Some(client), Some(upstream)) =
                    (self.client.clone(), self.upstreams.get(self.selected))
                {
                    let name = upstream.name.clone();
                    match client.delete_upstream(&name).await {
                        Ok(()) => self.message = format!("removed upstream: {name}"),
                        Err(error) => self.message = error.to_string(),
                    }
                    self.refresh().await;
                }
            }
            Action::RegenerateToken => {
                if let Some(client) = self.client.clone() {
                    match client.regenerate_client_token(false).await {
                        Ok(_) => self.message = "client token regenerated".into(),
                        Err(error) => self.message = error.to_string(),
                    }
                }
            }
        }
        false
    }
}
