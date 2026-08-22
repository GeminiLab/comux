use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Router;
use comux_core::{ProxyConfig, RedirectPolicy, RuntimeState, UpstreamConfig};
use comux_proxy::server::ProxyApp;
use http_body_util::BodyExt;
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

type SeenRequestValue = (String, String, Vec<u8>);

#[derive(Clone, Default)]
struct SeenRequest(Arc<Mutex<Option<SeenRequestValue>>>);

async fn mock_upstream(State(seen): State<SeenRequest>, request: Request) -> Response {
    let authorization = request
        .headers()
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    let uri = request.uri().to_string();
    let (parts, body) = request.into_parts();
    let body = body.collect().await.unwrap().to_bytes().to_vec();
    *seen.0.lock().unwrap() = Some((
        authorization,
        format!("{} {}", parts.method, uri),
        body.clone(),
    ));
    (StatusCode::ACCEPTED, [("x-mock", "ok")], Body::from(body)).into_response()
}

fn proxy_app(base_url: String, active: &str) -> ProxyApp {
    proxy_app_with_policy(base_url, active, RedirectPolicy::Never)
}

fn proxy_app_with_policy(
    base_url: String,
    active: &str,
    redirect_policy: RedirectPolicy,
) -> ProxyApp {
    let mut config = ProxyConfig::default();
    config.admin.token = "admin-secret".to_owned();
    config.downstream.token = "client-secret".to_owned();
    config.active_upstream = active.to_owned();
    config.upstreams.insert(
        "mock".to_owned(),
        UpstreamConfig {
            base_url,
            auth_token: "upstream-secret".to_owned(),
            redirect_policy,
        },
    );
    ProxyApp::new(RuntimeState::new(config).unwrap())
}

#[tokio::test]
async fn always_redirect_reinjects_upstream_token_cross_host() {
    let seen = SeenRequest::default();
    let target = Router::new()
        .fallback(mock_upstream)
        .with_state(seen.clone());
    let target_listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let target_address = target_listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(target_listener, target).await.unwrap() });

    let redirect = Router::new().fallback(move || async move {
        (
            StatusCode::FOUND,
            [("location", format!("http://{target_address}/final"))],
        )
    });
    let redirect_listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let redirect_address = redirect_listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(redirect_listener, redirect).await.unwrap() });

    let app = proxy_app_with_policy(
        format!("http://{redirect_address}"),
        "mock",
        RedirectPolicy::Always,
    )
    .router();
    let request = axum::http::Request::builder()
        .uri("/start")
        .header("authorization", "Bearer client-secret")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let seen = seen.0.lock().unwrap().clone().unwrap();
    assert_eq!(seen.0, "Bearer upstream-secret");
}

#[tokio::test]
async fn forwards_authenticated_request_and_rewrites_authorization() {
    let seen = SeenRequest::default();
    let upstream = Router::new()
        .fallback(mock_upstream)
        .with_state(seen.clone());
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, upstream).await.unwrap() });

    let app = proxy_app(format!("http://{address}/v1"), "mock").router();
    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/responses?stream=true")
        .header("authorization", "Bearer client-secret")
        .header("x-request-id", "request-1")
        .body(Body::from("hello"))
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert_eq!(response.headers()["x-mock"], "ok");
    assert_eq!(
        response.into_body().collect().await.unwrap().to_bytes(),
        "hello"
    );

    let seen = seen.0.lock().unwrap().clone().unwrap();
    assert_eq!(seen.0, "Bearer upstream-secret");
    assert_eq!(seen.1, "POST /v1/responses?stream=true");
    assert_eq!(seen.2, b"hello");
}

#[tokio::test]
async fn rejects_invalid_downstream_token() {
    let app = proxy_app("http://127.0.0.1:1".to_owned(), "mock").router();
    let request = axum::http::Request::builder()
        .uri("/responses")
        .header("authorization", "Bearer wrong")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn returns_503_without_active_upstream() {
    let app = proxy_app("http://127.0.0.1:1".to_owned(), "null").router();
    let request = axum::http::Request::builder()
        .uri("/responses")
        .header("authorization", "Bearer client-secret")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["error"]["type"], "no_active_upstream");
    assert_eq!(body["error"]["message"], "no active upstream configured");
}
