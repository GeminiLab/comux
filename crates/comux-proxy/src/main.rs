//! Binary entry point that initializes configuration and serves both proxy planes.

use clap::Parser;
use comux_core::persistence::load_proxy_config_or_initialize;
use comux_core::RuntimeState;
use comux_proxy::args::ProxyArgs;
use comux_proxy::server::ProxyApp;
use std::io;
use tracing_subscriber::EnvFilter;

#[tokio::main]
/// Runs the proxy process and exits non-zero after reporting a startup error.
async fn main() {
    if let Err(error) = run().await {
        eprintln!("comux-proxy: {error}");
        std::process::exit(1);
    }
}

/// Initializes tracing, configuration, listeners, and both HTTP servers.
async fn run() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();
    let args = ProxyArgs::parse();
    let loaded = load_proxy_config_or_initialize(&args.config)?;
    // Newly generated listener credentials are shown only on the initialization
    // run; subsequent starts load them from the private configuration file.
    if loaded.generated_admin_token {
        println!("generated admin token: {}", loaded.config.admin.token);
    }
    if loaded.generated_downstream_token {
        println!(
            "generated downstream token: {}",
            loaded.config.downstream.token
        );
    }

    let config = loaded.config;
    let downstream_address = format!("{}:{}", config.downstream.bind, config.downstream.port);
    let admin_address = format!("{}:{}", config.admin.bind, config.admin.port);
    let downstream_listener = tokio::net::TcpListener::bind(&downstream_address)
        .await
        .map_err(|error| {
            io::Error::new(
                error.kind(),
                format!("failed to bind downstream listener at {downstream_address}: {error}"),
            )
        })?;
    let admin_listener = tokio::net::TcpListener::bind(&admin_address)
        .await
        .map_err(|error| {
            io::Error::new(
                error.kind(),
                format!("failed to bind admin listener at {admin_address}: {error}"),
            )
        })?;
    let state = RuntimeState::new(config)?;
    let app = ProxyApp::new(state);
    let proxy_router = app.clone().router();
    let admin_router = app.admin_router(args.config);
    // Both planes share runtime state but use separate listeners and routers so
    // their credentials and exposed routes remain distinct.
    tokio::try_join!(
        axum::serve(downstream_listener, proxy_router),
        axum::serve(admin_listener, admin_router),
    )?;
    Ok(())
}
