use crate::admin;
use crate::auth::is_valid;
use crate::forward::forward_request;
use axum::body::Body;
use axum::extract::State;
use axum::http::{Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::Router;
use comux_core::RuntimeState;
use futures_util::{stream, StreamExt};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct ProxyApp {
    pub state: Arc<RwLock<RuntimeState>>,
    pub stats: crate::stats::StatsRegistry,
}

impl ProxyApp {
    pub fn new(state: RuntimeState) -> Self {
        Self {
            state: Arc::new(RwLock::new(state)),
            stats: crate::stats::StatsRegistry::default(),
        }
    }

    pub fn router(self) -> Router {
        Router::new().fallback(handle_downstream).with_state(self)
    }

    pub fn admin_router(self, config_path: PathBuf) -> Router {
        admin::router(self, config_path)
    }
}

pub async fn handle_downstream(
    State(app): State<ProxyApp>,
    method: Method,
    uri: Uri,
    headers: axum::http::HeaderMap,
    body: Body,
) -> Response {
    let (token, snapshot, connection) = {
        let state = app.state.read().await;
        (
            state.config().downstream.token.clone(),
            state.snapshot_active_upstream(),
            state.config().connection.clone(),
        )
    };
    if !is_valid(&headers, &token) {
        return json_error(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "invalid downstream token",
        );
    }
    let Some((upstream_name, upstream)) = snapshot else {
        return json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "no_active_upstream",
            "no active upstream configured",
        );
    };

    let started = Instant::now();
    let mut request_stats = app.stats.begin(upstream_name.clone());
    let request_stats_registry = app.stats.clone();
    let request_upstream_name = upstream_name.clone();
    let request_body = Body::from_stream(body.into_data_stream().map(move |chunk| {
        if let Ok(ref bytes) = chunk {
            request_stats_registry.record_request_bytes(&request_upstream_name, bytes.len() as u64);
        }
        chunk
    }));

    match forward_request(method, &uri, &headers, request_body, &upstream, &connection).await {
        Ok(response) => {
            let status = response.status();
            let (parts, response_body) = response.into_parts();
            let stats = app.stats.clone();
            let stream = stream::unfold(
                (
                    response_body.into_data_stream(),
                    Some(request_stats),
                    stats,
                    upstream_name,
                    status,
                    started,
                ),
                |(mut body, mut guard, stats, upstream_name, status, started)| async move {
                    match body.next().await {
                        Some(Ok(bytes)) => {
                            stats.record_response_bytes(&upstream_name, bytes.len() as u64);
                            Some((
                                Ok(bytes),
                                (body, guard, stats, upstream_name, status, started),
                            ))
                        }
                        Some(Err(error)) => {
                            if let Some(mut guard) = guard.take() {
                                guard.network_error();
                            }
                            Some((
                                Err(error),
                                (body, guard, stats, upstream_name, status, started),
                            ))
                        }
                        None => {
                            if let Some(mut guard) = guard.take() {
                                guard.finish_response(status, started.elapsed().as_millis() as u64);
                            }
                            None
                        }
                    }
                },
            );
            Response::from_parts(parts, Body::from_stream(stream))
        }
        Err(error) => {
            request_stats.network_error();
            tracing::warn!(error = %error, "upstream request failed");
            json_error(
                StatusCode::BAD_GATEWAY,
                "upstream_error",
                "upstream request failed",
            )
        }
    }
}

fn json_error(status: StatusCode, error_type: &str, message: &str) -> Response {
    (
        status,
        axum::Json(json!({
            "error": { "type": error_type, "message": message }
        })),
    )
        .into_response()
}
