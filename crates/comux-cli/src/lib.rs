//! Command-line and terminal user interfaces for managing a comux proxy.
//!
//! The CLI resolves local connection settings, calls the authenticated
//! management API, and opens a ratatui dashboard when no subcommand is given.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

/// Command-line argument and subcommand definitions.
pub mod args;
/// Authenticated client for the proxy management API.
pub mod client;
/// Command dispatch and terminal-oriented output.
pub mod commands;
/// Local CLI configuration resolution and persistence.
pub mod config;
/// Interactive ratatui dashboard.
pub mod tui;
