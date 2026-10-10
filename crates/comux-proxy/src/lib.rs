//! HTTP data plane and management plane for the comux Codex proxy.
//!
//! The data-plane router authenticates Codex with the downstream token and
//! forwards requests to a snapshot of the selected upstream. The management
//! router uses a separate admin token to inspect and mutate persisted state.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

/// Authenticated management API routing and handlers.
pub mod admin;
/// Command-line arguments for the proxy binary.
pub mod args;
/// Bearer credential parsing and validation.
pub mod auth;
/// Upstream request forwarding and redirect handling.
pub mod forward;
/// Shared application state and downstream routing.
pub mod server;
/// Process-local request and transport statistics.
pub mod stats;
