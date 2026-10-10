//! Authenticated management routes for inspecting and mutating proxy state.

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

/// State shared by all management API handlers.
#[derive(Clone)]
struct AdminApp {
    /// The proxy application whose runtime state and statistics are managed.
    proxy: ProxyApp,
    /// The file atomically replaced after successful configuration mutations.
    config_path: Arc<PathBuf>,
}

/// Builds the authenticated management router for a proxy application.
///
/// The router exposes status, statistics, upstream mutation and selection,
/// and downstream-token inspection and regeneration under `/api/v1`.
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

/// Checks the request's Bearer token against the current admin token.
async fn authorized(app: &AdminApp, headers: &HeaderMap) -> bool {
    let token = app.proxy.state.read().await.config().admin.token.clone();
    is_valid(headers, &token)
}

/// Returns listener details, active selection, and configured upstream names.
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

/// Returns a process-local snapshot of transport statistics.
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

/// Lists configured upstreams with credentials masked unless explicitly revealed.
async fn list_upstreams(
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
    // Full credentials require both admin authentication and an explicit
    // `reveal=full`; all ordinary responses remain masked.
    let upstreams: Vec<_> = state
        .config()
        .upstreams
        .iter()
        .map(|(name, upstream)| {
            if query.reveal.as_deref() == Some("full") {
                UpstreamSummary {
                    name: name.clone(),
                    base_url: upstream.base_url.clone(),
                    auth_token: upstream.auth_token.clone(),
                    redirect_policy: upstream.redirect_policy.clone(),
                }
            } else {
                UpstreamSummary::masked(
                    name,
                    upstream.base_url.clone(),
                    &upstream.auth_token,
                    upstream.redirect_policy.clone(),
                )
            }
        })
        .collect();
    Json(upstreams).into_response()
}

/// Creates or completely replaces one upstream and persists the new state.
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
    // Mutate and validate a candidate, persist it while holding the write lock,
    // and publish it only after the durable write succeeds.
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

/// Deletes an upstream, persisting `null` selection when it was active.
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
    // Mutate and validate a candidate, persist it while holding the write lock,
    // and publish it only after the durable write succeeds.
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

/// Selects an existing upstream for new downstream requests and persists it.
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
    // Mutate and validate a candidate, persist it while holding the write lock,
    // and publish it only after the durable write succeeds.
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

/// Optional query parameters controlling whether sensitive values are revealed.
#[derive(Debug, Deserialize)]
struct RevealQuery {
    /// Reveals a full credential only when this value is exactly `full`.
    reveal: Option<String>,
}

/// Returns the downstream client token in masked or explicitly revealed form.
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
    // Full credentials require both admin authentication and an explicit
    // `reveal=full`; all ordinary responses remain masked.
    let token = if query.reveal.as_deref() == Some("full") {
        token.clone()
    } else {
        mask_token(token)
    };
    Json(ClientTokenResponse { token }).into_response()
}

/// Generates, persists, and returns a replacement downstream client token.
async fn regenerate_client_token(
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
    let mut state = app.proxy.state.write().await;
    // Mutate and validate a candidate, persist it while holding the write lock,
    // and publish it only after the durable write succeeds.
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
    let token = if query.reveal.as_deref() == Some("full") {
        token
    } else {
        mask_token(&token)
    };
    Json(ClientTokenResponse { token }).into_response()
}

/// Creates a JSON error envelope with the supplied HTTP status and details.
fn api_error(status: StatusCode, error_type: &str, message: &str) -> Response {
    (
        status,
        Json(json!({ "error": { "type": error_type, "message": message } })),
    )
        .into_response()
}
