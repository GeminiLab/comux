//! Command dispatch, prompting, management calls, and terminal output.

use crate::args::{AuthAction, CliArgs, Command, ConfigAction, ConfigKey, UpstreamAction};
use crate::client::ApiClient;
use crate::config::{default_config_path, resolve_config_interactive, save_value};
use crate::tui;
use comux_core::api::UpstreamMutation;
use comux_core::persistence::load_cli_config;
use comux_core::{mask_token, ConfigError, RedirectPolicy};
use rpassword::prompt_password;
use std::io::{self, IsTerminal, Write};
use std::path::Path;

/// Dispatches a parsed command or starts the TUI when no command was supplied.
pub async fn run(args: CliArgs) -> Result<(), Box<dyn std::error::Error>> {
    let path = default_config_path();
    match args.command.clone() {
        // Configuration commands operate locally and therefore do not require
        // a reachable management endpoint; other commands resolve a client.
        Some(Command::Config(command)) => {
            run_config(&path, command.action, args.dangerously_print_full_token)?
        }
        Some(Command::Status) => {
            let client = client_from_args(&path, &args)?;
            let status = client.status().await?;
            println!("active upstream: {}", status.active_upstream);
            println!(
                "downstream: {}:{}",
                status.downstream_bind, status.downstream_port
            );
            println!("admin: {}:{}", status.admin_bind, status.admin_port);
            println!("upstreams: {}", status.upstream_names.join(", "));
        }
        Some(Command::Upstream { action }) => run_upstream(&path, &args, action).await?,
        Some(Command::Auth { action }) => run_auth(&path, &args, action).await?,
        None => tui::run(args).await?,
    }
    Ok(())
}

/// Resolves CLI settings and constructs a management API client.
fn client_from_args(path: &Path, args: &CliArgs) -> Result<ApiClient, ConfigError> {
    let config = resolve_config_interactive(path, args.endpoint.clone(), args.admin_token.clone())?;
    ApiClient::new(&config.admin_endpoint, config.admin_token)
        .map_err(|error| ConfigError::Invalid(error.to_string()))
}

/// Displays, reads, or updates local CLI configuration.
fn run_config(
    path: &Path,
    action: Option<ConfigAction>,
    dangerous: bool,
) -> Result<(), ConfigError> {
    match action {
        None => {
            let config = resolve_config_interactive(path, None, None)?;
            println!("endpoint = {}", config.admin_endpoint);
            println!(
                "admin-token = {}",
                display_token(&config.admin_token, dangerous)
            );
        }
        Some(ConfigAction::Get { key }) => {
            let value = get_local_value(path, key)?;
            println!(
                "{}",
                if matches!(key, ConfigKey::AdminToken) {
                    display_token(&value, dangerous)
                } else {
                    value
                }
            );
        }
        Some(ConfigAction::Set { key, value }) => {
            let value = match value {
                Some(value) => value,
                None => prompt_value(key)?,
            };
            save_value(path, key, value)?;
            println!("{} updated", config_key_name(key));
        }
    }
    Ok(())
}

/// Reads one local setting, interactively creating it when absent.
fn get_local_value(path: &Path, key: ConfigKey) -> Result<String, ConfigError> {
    let config = load_cli_config(path)?.unwrap_or(comux_core::CliConfig {
        admin_endpoint: String::new(),
        admin_token: String::new(),
    });
    match key {
        ConfigKey::Endpoint if config.admin_endpoint.is_empty() => {
            let value = prompt_endpoint_value()?;
            save_value(path, key, value.clone())?;
            Ok(value)
        }
        ConfigKey::AdminToken if config.admin_token.is_empty() => {
            let value = prompt_password("Admin token: ").map_err(ConfigError::Io)?;
            save_value(path, key, value.clone())?;
            Ok(value)
        }
        ConfigKey::Endpoint => Ok(config.admin_endpoint),
        ConfigKey::AdminToken => Ok(config.admin_token),
    }
}

/// Reads a management endpoint from an interactive terminal.
fn prompt_endpoint_value() -> Result<String, ConfigError> {
    if !io::stdin().is_terminal() {
        return Err(ConfigError::Invalid(
            "admin endpoint is missing and stdin is not interactive".to_owned(),
        ));
    }
    print!("Admin endpoint: ");
    io::stdout().flush()?;
    let mut endpoint = String::new();
    io::stdin().read_line(&mut endpoint)?;
    Ok(endpoint.trim().to_owned())
}

/// Executes an upstream list, add, replacement, removal, or selection action.
async fn run_upstream(
    path: &Path,
    args: &CliArgs,
    action: UpstreamAction,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = client_from_args(path, args)?;
    match action {
        UpstreamAction::List => {
            for upstream in client.upstreams(args.dangerously_print_full_token).await? {
                println!(
                    "{}\t{}\t{}\t{:?}",
                    upstream.name,
                    upstream.base_url,
                    display_token(&upstream.auth_token, args.dangerously_print_full_token),
                    upstream.redirect_policy
                );
            }
        }
        UpstreamAction::Add(options) => {
            let token = options
                .auth_token
                .unwrap_or(prompt_password("Upstream token: ")?);
            let payload = UpstreamMutation {
                base_url: options.base_url,
                auth_token: token,
                redirect_policy: parse_redirect_policy(&options.redirect_policy)?,
            };
            client.put_upstream(&options.name, payload).await?;
        }
        UpstreamAction::Set(options) => {
            // Updating without a replacement token explicitly fetches the full
            // current credential because PUT sends a complete replacement.
            let token = match options.options.auth_token {
                Some(token) => token,
                None => {
                    client
                        .upstreams(true)
                        .await?
                        .into_iter()
                        .find(|upstream| upstream.name == options.name)
                        .ok_or_else(|| format!("upstream {:?} not found", options.name))?
                        .auth_token
                }
            };
            let payload = UpstreamMutation {
                base_url: options.options.base_url,
                auth_token: token,
                redirect_policy: parse_redirect_policy(&options.options.redirect_policy)?,
            };
            client.put_upstream(&options.name, payload).await?;
        }
        UpstreamAction::Remove { name } => client.delete_upstream(&name).await?,
        UpstreamAction::Select { name } => client.select_upstream(&name).await?,
    }
    Ok(())
}

/// Prints or regenerates the downstream client token.
async fn run_auth(
    path: &Path,
    args: &CliArgs,
    action: AuthAction,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = client_from_args(path, args)?;
    match action {
        AuthAction::Print => {
            let token = client
                .client_token(args.dangerously_print_full_token)
                .await?;
            println!(
                "{}",
                display_token(&token.token, args.dangerously_print_full_token)
            );
        }
        AuthAction::Regenerate { yes } => {
            if !yes && (!io::stdin().is_terminal() || !confirm("Regenerate client token? [y/N] ")?)
            {
                return Err("client token regeneration was not confirmed".into());
            }
            let token = client
                .regenerate_client_token(args.dangerously_print_full_token)
                .await?;
            println!(
                "new client token: {}",
                display_token(&token.token, args.dangerously_print_full_token)
            );
        }
    }
    Ok(())
}

/// Parses the user-facing redirect policy spelling.
fn parse_redirect_policy(value: &str) -> Result<RedirectPolicy, ConfigError> {
    match value {
        "always" => Ok(RedirectPolicy::Always),
        "default" => Ok(RedirectPolicy::Default),
        "never" => Ok(RedirectPolicy::Never),
        other => Err(ConfigError::Invalid(format!(
            "invalid redirect policy {other:?}"
        ))),
    }
}

/// Returns a full token only when dangerous output was explicitly enabled.
fn display_token(token: &str, dangerous: bool) -> String {
    // Full output requires the global dangerous flag; ordinary output masks
    // credentials even when an API response contains a full value.
    if dangerous {
        token.to_owned()
    } else {
        mask_token(token)
    }
}

/// Prompts for a local configuration value, hiding admin-token input.
fn prompt_value(key: ConfigKey) -> Result<String, ConfigError> {
    match key {
        ConfigKey::Endpoint => {
            print!("Endpoint: ");
            io::stdout().flush()?;
            let mut value = String::new();
            io::stdin().read_line(&mut value)?;
            Ok(value.trim().to_owned())
        }
        ConfigKey::AdminToken => prompt_password("Admin token: ").map_err(ConfigError::Io),
    }
}

/// Returns the CLI spelling of a configuration key.
fn config_key_name(key: ConfigKey) -> &'static str {
    match key {
        ConfigKey::Endpoint => "endpoint",
        ConfigKey::AdminToken => "admin-token",
    }
}

/// Prompts for a yes/no confirmation and accepts `y` or `yes`.
fn confirm(prompt: &str) -> Result<bool, io::Error> {
    print!("{prompt}");
    io::stdout().flush()?;
    let mut value = String::new();
    io::stdin().read_line(&mut value)?;
    Ok(matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}
