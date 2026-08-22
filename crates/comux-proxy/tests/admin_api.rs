use axum::body::Body;
use comux_core::{ProxyConfig, RedirectPolicy, RuntimeState, UpstreamConfig};
use comux_proxy::server::ProxyApp;
use http_body_util::BodyExt;
use tempfile::tempdir;
use tower::ServiceExt;

fn app() -> (ProxyApp, tempfile::TempDir) {
    let directory = tempdir().unwrap();
    let mut config = ProxyConfig::default();
    config.admin.token = "admin-secret".to_owned();
    config.downstream.token = "client-secret".to_owned();
    config.upstreams.insert(
        "openai".to_owned(),
        UpstreamConfig {
            base_url: "https://api.example.test/v1".to_owned(),
            auth_token: "upstream-secret".to_owned(),
            redirect_policy: RedirectPolicy::Default,
        },
    );
    config.active_upstream = "openai".to_owned();
    let path = directory.path().join("proxy.toml");
    comux_core::persistence::save_proxy_config(&path, &config).unwrap();
    (ProxyApp::new(RuntimeState::new(config).unwrap()), directory)
}

#[tokio::test]
async fn every_admin_route_requires_admin_token() {
    let (app, directory) = app();
    let router = app.admin_router(directory.path().join("proxy.toml"));
    let request = axum::http::Request::builder()
        .uri("/api/v1/status")
        .body(Body::empty())
        .unwrap();
    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), http::StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn status_and_upstream_list_mask_tokens() {
    let (app, directory) = app();
    let router = app.admin_router(directory.path().join("proxy.toml"));
    let request = axum::http::Request::builder()
        .uri("/api/v1/upstreams")
        .header("authorization", "Bearer admin-secret")
        .body(Body::empty())
        .unwrap();
    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), http::StatusCode::OK);
    let value: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(value[0]["auth_token"], "upst*******cret");
}

#[tokio::test]
async fn client_token_reveal_requires_explicit_query() {
    let (app, directory) = app();
    let router = app.admin_router(directory.path().join("proxy.toml"));
    let request = axum::http::Request::builder()
        .uri("/api/v1/auth/client-token")
        .header("authorization", "Bearer admin-secret")
        .body(Body::empty())
        .unwrap();
    let response = router.clone().oneshot(request).await.unwrap();
    let masked: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(masked["token"], "clie*****cret");

    let request = axum::http::Request::builder()
        .uri("/api/v1/auth/client-token?reveal=full")
        .header("authorization", "Bearer admin-secret")
        .body(Body::empty())
        .unwrap();
    let response = router.oneshot(request).await.unwrap();
    let full: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(full["token"], "client-secret");
}

#[tokio::test]
async fn upstream_mutations_persist_and_active_deletion_selects_null() {
    let (app, directory) = app();
    let path = directory.path().join("proxy.toml");
    let router = app.admin_router(path.clone());
    let payload = serde_json::json!({
        "base_url": "https://internal.example.test/codex",
        "auth_token": "internal-secret",
        "redirect_policy": "never"
    });
    let request = axum::http::Request::builder()
        .method("PUT")
        .uri("/api/v1/upstreams/internal")
        .header("authorization", "Bearer admin-secret")
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();
    assert_eq!(
        router.clone().oneshot(request).await.unwrap().status(),
        http::StatusCode::NO_CONTENT
    );

    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/api/v1/active")
        .header("authorization", "Bearer admin-secret")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"internal"}"#))
        .unwrap();
    assert_eq!(
        router.clone().oneshot(request).await.unwrap().status(),
        http::StatusCode::NO_CONTENT
    );

    let request = axum::http::Request::builder()
        .method("DELETE")
        .uri("/api/v1/upstreams/internal")
        .header("authorization", "Bearer admin-secret")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        router.oneshot(request).await.unwrap().status(),
        http::StatusCode::NO_CONTENT
    );

    let persisted = comux_core::persistence::load_proxy_config_or_initialize(&path)
        .unwrap()
        .config;
    assert_eq!(persisted.active_upstream, "null");
    assert!(!persisted.upstreams.contains_key("internal"));
}
