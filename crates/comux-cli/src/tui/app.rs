use super::events::Action;
use crate::client::ApiClient;
use comux_core::api::{StatsResponse, StatusResponse, UpstreamMutation, UpstreamSummary};
use comux_core::RedirectPolicy;
use crossterm::event::{KeyCode, KeyEvent};
use url::Url;

pub struct App {
    pub client: Option<ApiClient>,
    pub status: Option<StatusResponse>,
    pub stats: Option<StatsResponse>,
    pub upstreams: Vec<UpstreamSummary>,
    pub selected: usize,
    pub message: String,
    pub show_full_tokens: bool,
    pub add_form: Option<AddUpstreamForm>,
    selection_initialized: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddField {
    Name,
    BaseUrl,
    AuthToken,
    RedirectPolicy,
}

#[derive(Debug, Clone)]
pub struct AddUpstreamForm {
    pub field: AddField,
    pub name: String,
    pub base_url: String,
    pub auth_token: String,
    pub redirect_policy: comux_core::RedirectPolicy,
    pub error: Option<String>,
}

impl Default for AddUpstreamForm {
    fn default() -> Self {
        Self {
            field: AddField::Name,
            name: String::new(),
            base_url: String::new(),
            auth_token: String::new(),
            redirect_policy: RedirectPolicy::Default,
            error: None,
        }
    }
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
            add_form: None,
            selection_initialized: false,
        }
    }

    pub async fn refresh(&mut self) {
        if self.add_form.is_some() {
            return;
        }
        let Some(client) = self.client.clone() else {
            return;
        };
        match (
            client.status().await,
            client.stats().await,
            client.upstreams(false).await,
        ) {
            (Ok(status), Ok(stats), Ok(upstreams)) => {
                if !self.selection_initialized {
                    self.selected = selected_index_for_active(&status.active_upstream, &upstreams);
                    self.selection_initialized = true;
                }
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
            Action::Add => self.add_form = Some(AddUpstreamForm::default()),
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

    pub async fn handle_key(&mut self, key: KeyEvent) -> bool {
        if self.add_form.is_some() {
            self.handle_add_form_key(key).await;
            return false;
        }
        match super::events::action(key) {
            Some(action) => self.handle(action).await,
            None => false,
        }
    }

    async fn handle_add_form_key(&mut self, key: KeyEvent) {
        let Some(mut form) = self.add_form.take() else {
            return;
        };
        match key.code {
            KeyCode::Esc => {
                self.message = "add upstream cancelled".into();
                return;
            }
            KeyCode::Char(c) if !matches!(form.field, AddField::RedirectPolicy) => {
                current_value_mut(&mut form).push(c);
                form.error = None;
            }
            KeyCode::Backspace if !matches!(form.field, AddField::RedirectPolicy) => {
                current_value_mut(&mut form).pop();
                form.error = None;
            }
            KeyCode::Left | KeyCode::Right | KeyCode::Char(' ')
                if matches!(form.field, AddField::RedirectPolicy) =>
            {
                let direction = matches!(key.code, KeyCode::Right | KeyCode::Char(' '));
                form.redirect_policy = cycle_policy(form.redirect_policy, direction);
                form.error = None;
            }
            KeyCode::Enter | KeyCode::Tab => {
                if let Err(error) = validate_field(&form) {
                    form.error = Some(error);
                } else if form.field == AddField::RedirectPolicy && key.code == KeyCode::Enter {
                    match self.submit_add_form(&form).await {
                        Ok(()) => return,
                        Err(error) => form.error = Some(error),
                    }
                } else {
                    form.field = next_field(form.field);
                    form.error = None;
                }
            }
            _ => {}
        }
        self.add_form = Some(form);
    }

    async fn submit_add_form(&mut self, form: &AddUpstreamForm) -> Result<(), String> {
        let Some(client) = self.client.clone() else {
            return Err("management client is unavailable".into());
        };
        let payload = UpstreamMutation {
            base_url: form.base_url.clone(),
            auth_token: form.auth_token.clone(),
            redirect_policy: form.redirect_policy.clone(),
        };
        client
            .put_upstream(&form.name, payload)
            .await
            .map_err(|error| error.to_string())?;
        let message = format!("added upstream: {}", form.name);
        self.refresh().await;
        self.message = message;
        Ok(())
    }
}

fn selected_index_for_active(active: &str, upstreams: &[UpstreamSummary]) -> usize {
    upstreams
        .iter()
        .position(|upstream| upstream.name == active)
        .unwrap_or(0)
}

fn current_value_mut(form: &mut AddUpstreamForm) -> &mut String {
    match form.field {
        AddField::Name => &mut form.name,
        AddField::BaseUrl => &mut form.base_url,
        AddField::AuthToken => &mut form.auth_token,
        AddField::RedirectPolicy => unreachable!("policy is not text input"),
    }
}

fn next_field(field: AddField) -> AddField {
    match field {
        AddField::Name => AddField::BaseUrl,
        AddField::BaseUrl => AddField::AuthToken,
        AddField::AuthToken => AddField::RedirectPolicy,
        AddField::RedirectPolicy => AddField::Name,
    }
}

fn cycle_policy(policy: RedirectPolicy, forward: bool) -> RedirectPolicy {
    use RedirectPolicy::{Always, Default, Never};
    match (policy, forward) {
        (Always, true) => Default,
        (Default, true) => Never,
        (Never, true) => Always,
        (Always, false) => Never,
        (Default, false) => Always,
        (Never, false) => Default,
    }
}

fn validate_field(form: &AddUpstreamForm) -> Result<(), String> {
    match form.field {
        AddField::Name if form.name.trim().is_empty() => Err("name is required".into()),
        AddField::Name if form.name.trim() == "null" => Err("name null is reserved".into()),
        AddField::BaseUrl => {
            let url = Url::parse(form.base_url.trim()).map_err(|error| error.to_string())?;
            if matches!(url.scheme(), "http" | "https") {
                Ok(())
            } else {
                Err("base URL must use http or https".into())
            }
        }
        AddField::AuthToken if form.auth_token.is_empty() => Err("auth token is required".into()),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::{selected_index_for_active, AddField, App};
    use crate::client::ApiClient;
    use crate::tui::events::Action;
    use axum::extract::State;
    use axum::http::StatusCode;
    use axum::routing::{get, put};
    use axum::{Json, Router};
    use comux_core::api::{StatsResponse, StatusResponse, UpstreamMutation};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::sync::{Arc, Mutex};

    type Received = Arc<Mutex<Option<(String, UpstreamMutation)>>>;

    #[test]
    fn initial_selection_follows_active_upstream() {
        let upstreams = vec![
            comux_core::api::UpstreamSummary {
                name: "first".into(),
                base_url: "https://first.example".into(),
                auth_token: "masked".into(),
                redirect_policy: comux_core::RedirectPolicy::Default,
            },
            comux_core::api::UpstreamSummary {
                name: "active".into(),
                base_url: "https://active.example".into(),
                auth_token: "masked".into(),
                redirect_policy: comux_core::RedirectPolicy::Default,
            },
        ];
        assert_eq!(selected_index_for_active("active", &upstreams), 1);
        assert_eq!(selected_index_for_active("null", &upstreams), 0);
    }

    #[tokio::test]
    async fn add_action_opens_form_and_escape_cancels_it() {
        let mut app = App::placeholder();
        assert!(!app.handle(Action::Add).await);
        assert_eq!(app.add_form.as_ref().unwrap().field, AddField::Name);
        app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
            .await;
        assert!(app.add_form.is_none());
    }

    #[tokio::test]
    async fn empty_name_is_rejected_without_leaving_form() {
        let mut app = App::placeholder();
        app.handle(Action::Add).await;
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
            .await;
        let form = app.add_form.as_ref().unwrap();
        assert_eq!(form.field, AddField::Name);
        assert_eq!(form.error.as_deref(), Some("name is required"));
    }

    #[tokio::test]
    async fn completed_form_submits_to_admin_api_and_closes() {
        let received: Received = Arc::new(Mutex::new(None));
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let address = listener.local_addr().unwrap();
        let route_state = received.clone();
        let router =
            Router::new()
                .route(
                    "/api/v1/status",
                    get(|| async {
                        Json(StatusResponse {
                            active_upstream: "null".into(),
                            downstream_bind: "127.0.0.1".into(),
                            downstream_port: 6608,
                            admin_bind: "127.0.0.1".into(),
                            admin_port: 6609,
                            upstream_names: Vec::new(),
                        })
                    }),
                )
                .route(
                    "/api/v1/stats",
                    get(|| async { Json(StatsResponse::default()) }),
                )
                .route(
                    "/api/v1/upstreams",
                    get(|| async { Json(Vec::<comux_core::api::UpstreamSummary>::new()) }),
                )
                .route(
                    "/api/v1/upstreams/demo",
                    put(
                        |State(received): State<Received>,
                         Json(payload): Json<UpstreamMutation>| async move {
                            *received.lock().unwrap() = Some(("demo".into(), payload));
                            StatusCode::NO_CONTENT
                        },
                    ),
                )
                .with_state(route_state);
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });

        let client = ApiClient::new(&format!("http://{address}"), "admin".into()).unwrap();
        let mut app = App::new(client, false);
        app.handle(Action::Add).await;
        let form = app.add_form.as_mut().unwrap();
        form.name = "demo".into();
        form.base_url = "https://example.test/v1".into();
        form.auth_token = "upstream-secret".into();
        form.field = AddField::RedirectPolicy;
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
            .await;

        assert!(app.add_form.is_none());
        assert_eq!(app.message, "added upstream: demo");
        let received = received.lock().unwrap().clone().unwrap();
        assert_eq!(received.0, "demo");
        assert_eq!(received.1.base_url, "https://example.test/v1");
    }
}
