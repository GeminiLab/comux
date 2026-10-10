//! Authenticated HTTP client for the proxy management API.

use comux_core::api::{
    ActiveMutation, ClientTokenResponse, StatsResponse, StatusResponse, UpstreamMutation,
    UpstreamSummary,
};
use reqwest::{Client, Method, StatusCode, Url};
use serde::de::DeserializeOwned;
use thiserror::Error;

/// A failure to construct, send, or interpret a management request.
#[derive(Debug, Error)]
pub enum ClientError {
    /// The configured management endpoint or a joined API URL is invalid.
    #[error("invalid admin endpoint: {0}")]
    Endpoint(#[from] url::ParseError),
    /// The HTTP request failed before a usable API response was received.
    #[error("management request failed: {0}")]
    Request(#[from] reqwest::Error),
    /// The management API returned a non-success response.
    #[error("management API returned {status}: {message}")]
    Server {
        /// The HTTP status returned by the management API.
        status: StatusCode,
        /// The server-provided or fallback human-readable explanation.
        message: String,
    },
}

/// Cloneable authenticated client for one proxy management endpoint.
#[derive(Clone)]
pub struct ApiClient {
    /// Base URL used to resolve relative management API paths.
    endpoint: Url,
    /// Bearer credential sent with every management request.
    admin_token: String,
    /// Shared reqwest client used for HTTP transport.
    http: Client,
}

impl ApiClient {
    /// Creates a client for `endpoint` using the supplied admin token.
    pub fn new(endpoint: &str, admin_token: String) -> Result<Self, ClientError> {
        let endpoint = Url::parse(endpoint)?;
        Ok(Self {
            endpoint,
            admin_token,
            http: Client::new(),
        })
    }

    /// Fetches current listener and upstream-selection status.
    pub async fn status(&self) -> Result<StatusResponse, ClientError> {
        self.get_json("api/v1/status").await
    }

    /// Fetches a process-local transport statistics snapshot.
    pub async fn stats(&self) -> Result<StatsResponse, ClientError> {
        self.get_json("api/v1/stats").await
    }

    /// Lists upstreams, requesting full credentials only when `reveal` is true.
    pub async fn upstreams(&self, reveal: bool) -> Result<Vec<UpstreamSummary>, ClientError> {
        let path = if reveal {
            "api/v1/upstreams?reveal=full"
        } else {
            "api/v1/upstreams"
        };
        self.get_json(path).await
    }

    /// Creates or completely replaces the named upstream.
    pub async fn put_upstream(
        &self,
        name: &str,
        payload: UpstreamMutation,
    ) -> Result<(), ClientError> {
        self.send_empty(
            Method::PUT,
            &format!("api/v1/upstreams/{name}"),
            Some(payload),
        )
        .await
    }

    /// Deletes the named upstream.
    pub async fn delete_upstream(&self, name: &str) -> Result<(), ClientError> {
        self.send_empty::<()>(Method::DELETE, &format!("api/v1/upstreams/{name}"), None)
            .await
    }

    /// Selects the named upstream for new proxy requests.
    pub async fn select_upstream(&self, name: &str) -> Result<(), ClientError> {
        self.send_empty(
            Method::POST,
            "api/v1/active",
            Some(ActiveMutation {
                name: name.to_owned(),
            }),
        )
        .await
    }

    /// Fetches the downstream token, explicitly requesting its full value when asked.
    pub async fn client_token(&self, reveal: bool) -> Result<ClientTokenResponse, ClientError> {
        let path = if reveal {
            "api/v1/auth/client-token?reveal=full"
        } else {
            "api/v1/auth/client-token"
        };
        self.get_json(path).await
    }

    /// Rotates the downstream token and returns its masked or full new value.
    pub async fn regenerate_client_token(
        &self,
        reveal: bool,
    ) -> Result<ClientTokenResponse, ClientError> {
        let path = if reveal {
            "api/v1/auth/client-token/regenerate?reveal=full"
        } else {
            "api/v1/auth/client-token/regenerate"
        };
        self.send_json(Method::POST, path, Option::<()>::None).await
    }

    /// Sends a GET request and deserializes a successful JSON response.
    async fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<T, ClientError> {
        self.send_json(Method::GET, path, Option::<()>::None).await
    }

    /// Sends a mutation that is expected to return success without a JSON body.
    async fn send_empty<T: serde::Serialize>(
        &self,
        method: Method,
        path: &str,
        body: Option<T>,
    ) -> Result<(), ClientError> {
        let response = self.request(method, path, body).await?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(parse_server_error(response).await)
        }
    }

    /// Sends a request and deserializes its successful JSON response.
    async fn send_json<T: DeserializeOwned, B: serde::Serialize>(
        &self,
        method: Method,
        path: &str,
        body: Option<B>,
    ) -> Result<T, ClientError> {
        let response = self.request(method, path, body).await?;
        if !response.status().is_success() {
            return Err(parse_server_error(response).await);
        }
        Ok(response.json().await?)
    }

    /// Builds and sends one authenticated management request.
    async fn request<B: serde::Serialize>(
        &self,
        method: Method,
        path: &str,
        body: Option<B>,
    ) -> Result<reqwest::Response, ClientError> {
        // Join a relative API path to the configured endpoint so every method
        // shares URL resolution and authentication handling.
        let url = self.endpoint.join(path).map_err(ClientError::Endpoint)?;
        let mut request = self
            .http
            .request(method, url)
            .bearer_auth(&self.admin_token);
        if let Some(body) = body {
            request = request.json(&body);
        }
        Ok(request.send().await?)
    }
}

/// Converts a non-success response into a structured client error.
///
/// When the response is not the standard JSON error envelope, this falls back
/// to a generic management-request message.
async fn parse_server_error(response: reqwest::Response) -> ClientError {
    let status = response.status();
    let message = match response.json::<serde_json::Value>().await {
        Ok(value) => value["error"]["message"]
            .as_str()
            .unwrap_or("management request failed")
            .to_owned(),
        Err(_) => "management request failed".to_owned(),
    };
    ClientError::Server { status, message }
}
