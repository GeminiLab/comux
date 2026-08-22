use comux_core::{ProxyConfig, RuntimeState};
use comux_proxy::server::ProxyApp;
use reqwest::Client;
use tempfile::tempdir;

#[tokio::test]
async fn bound_listeners_enforce_separate_authentication() {
    let directory = tempdir().unwrap();
    let mut config = ProxyConfig::default();
    config.admin.token = "admin-secret".into();
    config.downstream.token = "client-secret".into();
    let config_path = directory.path().join("proxy.toml");
    comux_core::persistence::save_proxy_config(&config_path, &config).unwrap();
    let app = ProxyApp::new(RuntimeState::new(config).unwrap());

    let downstream_listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let admin_listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let downstream_addr = downstream_listener.local_addr().unwrap();
    let admin_addr = admin_listener.local_addr().unwrap();
    let downstream_app = app.clone();
    let downstream_task = tokio::spawn(async move {
        axum::serve(downstream_listener, downstream_app.router())
            .await
            .unwrap();
    });
    let admin_task = tokio::spawn(async move {
        axum::serve(admin_listener, app.admin_router(config_path))
            .await
            .unwrap();
    });
    tokio::task::yield_now().await;

    let client = Client::new();
    let response = client
        .get(format!("http://{downstream_addr}/v1/models"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);

    let response = client
        .get(format!("http://{downstream_addr}/v1/models"))
        .bearer_auth("client-secret")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::SERVICE_UNAVAILABLE);

    let response = client
        .get(format!("http://{admin_addr}/api/v1/status"))
        .bearer_auth("admin-secret")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);

    downstream_task.abort();
    admin_task.abort();
}
