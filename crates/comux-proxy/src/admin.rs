use crate::auth::is_valid;
use crate::server::ProxyApp;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use comux_core::api::{
    ActiveMutation, ClientTokenResponse, StatusResponse, UpstreamMutation, UpstreamSummary,
};
use comux_core::persistence::save_proxy_config;
use comux_core::secrets::mask_token;
use comux_core::UpstreamConfig;
use serde::Deserialize;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone)]
struct AdminApp {
    proxy: ProxyApp,
    config_path: Arc<PathBuf>,
}

pub fn router(proxy: ProxyApp, config_path: PathBuf) -> Router {
    let state = AdminApp {
        proxy,
        config_path: Arc::new(config_path),
    };
    Router::new()
        .route("/api/v1/status", get(status))
        .route("/api/v1/stats", get(stats))
        .route("/api/v1/upstreams", get(list_upstreams))
        .route(
            "/api/v1/upstreams/{name}",
            put(put_upstream).delete(delete_upstream),
        )
        .route("/api/v1/active", post(select_active))
        .route("/api/v1/auth/client-token", get(client_token))
        .route(
            "/api/v1/auth/client-token/regenerate",
            post(regenerate_client_token),
        )
        .with_state(state)
}

async fn authorized(app: &AdminApp, headers: &HeaderMap) -> bool {
    let token = app.proxy.state.read().await.config().admin.token.clone();
    is_valid(headers, &token)
}

async fn status(State(app): State<AdminApp>, headers: HeaderMap) -> Response {
    if !authorized(&app, &headers).await {
        return api_error(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "invalid admin token",
        );
    }
    let state = app.proxy.state.read().await;
    let config = state.config();
    Json(StatusResponse {
        active_upstream: config.active_upstream.clone(),
        downstream_bind: config.downstream.bind.clone(),
        downstream_port: config.downstream.port,
        admin_bind: config.admin.bind.clone(),
        admin_port: config.admin.port,
        upstream_names: config.upstreams.keys().cloned().collect(),
    })
    .into_response()
}

async fn stats(State(app): State<AdminApp>, headers: HeaderMap) -> Response {
    if !authorized(&app, &headers).await {
        return api_error(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "invalid admin token",
        );
    }
    Json(app.proxy.stats.snapshot()).into_response()
}

async fn list_upstreams(State(app): State<AdminApp>, headers: HeaderMap) -> Response {
    if !authorized(&app, &headers).await {
        return api_error(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "invalid admin token",
        );
    }
    let state = app.proxy.state.read().await;
    let upstreams: Vec<_> = state
        .config()
        .upstreams
        .iter()
        .map(|(name, upstream)| {
            UpstreamSummary::masked(
                name,
                upstream.base_url.clone(),
                &upstream.auth_token,
                upstream.redirect_policy.clone(),
            )
        })
        .collect();
    Json(upstreams).into_response()
}

async fn put_upstream(
    State(app): State<AdminApp>,
    headers: HeaderMap,
    Path(name): Path<String>,
    Json(payload): Json<UpstreamMutation>,
) -> Response {
    if !authorized(&app, &headers).await {
        return api_error(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "invalid admin token",
        );
    }
    let mut state = app.proxy.state.write().await;
    let mut candidate = state.clone();
    let upstream = UpstreamConfig {
        base_url: payload.base_url,
        auth_token: payload.auth_token,
        redirect_policy: payload.redirect_policy,
    };
    if let Err(error) = candidate.replace_upstream(name, upstream) {
        return api_error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            &error.to_string(),
        );
    }
    if let Err(error) = save_proxy_config(&app.config_path, candidate.config()) {
        return api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "persistence_failed",
            &error.to_string(),
        );
    }
    *state = candidate;
    StatusCode::NO_CONTENT.into_response()
}

async fn delete_upstream(
    State(app): State<AdminApp>,
    headers: HeaderMap,
    Path(name): Path<String>,
) -> Response {
    if !authorized(&app, &headers).await {
        return api_error(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "invalid admin token",
        );
    }
    let mut state = app.proxy.state.write().await;
    let mut candidate = state.clone();
    if let Err(error) = candidate.delete_upstream(&name) {
        return api_error(StatusCode::NOT_FOUND, "not_found", &error.to_string());
    }
    if let Err(error) = save_proxy_config(&app.config_path, candidate.config()) {
        return api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "persistence_failed",
            &error.to_string(),
        );
    }
    *state = candidate;
    StatusCode::NO_CONTENT.into_response()
}

async fn select_active(
    State(app): State<AdminApp>,
    headers: HeaderMap,
    Json(payload): Json<ActiveMutation>,
) -> Response {
    if !authorized(&app, &headers).await {
        return api_error(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "invalid admin token",
        );
    }
    let mut state = app.proxy.state.write().await;
    let mut candidate = state.clone();
    if let Err(error) = candidate.select_upstream(&payload.name) {
        return api_error(StatusCode::NOT_FOUND, "not_found", &error.to_string());
    }
    if let Err(error) = save_proxy_config(&app.config_path, candidate.config()) {
        return api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "persistence_failed",
            &error.to_string(),
        );
    }
    *state = candidate;
    StatusCode::NO_CONTENT.into_response()
}

#[derive(Debug, Deserialize)]
struct RevealQuery {
    reveal: Option<String>,
}

async fn client_token(
    State(app): State<AdminApp>,
    headers: HeaderMap,
    Query(query): Query<RevealQuery>,
) -> Response {
    if !authorized(&app, &headers).await {
        return api_error(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "invalid admin token",
        );
    }
    let state = app.proxy.state.read().await;
    let token = &state.config().downstream.token;
    let token = if query.reveal.as_deref() == Some("full") {
        token.clone()
    } else {
        mask_token(token)
    };
    Json(ClientTokenResponse { token }).into_response()
}

async fn regenerate_client_token(State(app): State<AdminApp>, headers: HeaderMap) -> Response {
    if !authorized(&app, &headers).await {
        return api_error(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "invalid admin token",
        );
    }
    let mut state = app.proxy.state.write().await;
    let mut candidate = state.clone();
    let token = candidate.regenerate_client_token();
    if let Err(error) = save_proxy_config(&app.config_path, candidate.config()) {
        return api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "persistence_failed",
            &error.to_string(),
        );
    }
    *state = candidate;
    Json(ClientTokenResponse {
        token: mask_token(&token),
    })
    .into_response()
}

fn api_error(status: StatusCode, error_type: &str, message: &str) -> Response {
    (
        status,
        Json(json!({ "error": { "type": error_type, "message": message } })),
    )
        .into_response()
}
