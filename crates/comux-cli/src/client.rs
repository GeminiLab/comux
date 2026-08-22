use comux_core::api::{
    ActiveMutation, ClientTokenResponse, StatsResponse, StatusResponse, UpstreamMutation,
    UpstreamSummary,
};
use reqwest::{Client, Method, StatusCode, Url};
use serde::de::DeserializeOwned;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("invalid admin endpoint: {0}")]
    Endpoint(#[from] url::ParseError),
    #[error("management request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("management API returned {status}: {message}")]
    Server { status: StatusCode, message: String },
}

#[derive(Clone)]
pub struct ApiClient {
    endpoint: Url,
    admin_token: String,
    http: Client,
}

impl ApiClient {
    pub fn new(endpoint: &str, admin_token: String) -> Result<Self, ClientError> {
        let endpoint = Url::parse(endpoint)?;
        Ok(Self {
            endpoint,
            admin_token,
            http: Client::new(),
        })
    }

    pub async fn status(&self) -> Result<StatusResponse, ClientError> {
        self.get_json("api/v1/status").await
    }

    pub async fn stats(&self) -> Result<StatsResponse, ClientError> {
        self.get_json("api/v1/stats").await
    }

    pub async fn upstreams(&self, reveal: bool) -> Result<Vec<UpstreamSummary>, ClientError> {
        let path = if reveal {
            "api/v1/upstreams?reveal=full"
        } else {
            "api/v1/upstreams"
        };
        self.get_json(path).await
    }

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

    pub async fn delete_upstream(&self, name: &str) -> Result<(), ClientError> {
        self.send_empty::<()>(Method::DELETE, &format!("api/v1/upstreams/{name}"), None)
            .await
    }

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

    pub async fn client_token(&self, reveal: bool) -> Result<ClientTokenResponse, ClientError> {
        let path = if reveal {
            "api/v1/auth/client-token?reveal=full"
        } else {
            "api/v1/auth/client-token"
        };
        self.get_json(path).await
    }

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

    async fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<T, ClientError> {
        self.send_json(Method::GET, path, Option::<()>::None).await
    }

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

    async fn request<B: serde::Serialize>(
        &self,
        method: Method,
        path: &str,
        body: Option<B>,
    ) -> Result<reqwest::Response, ClientError> {
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
