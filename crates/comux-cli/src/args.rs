//! Clap definitions for global options, commands, and nested actions.

use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

/// Parsed arguments for the `comux` management client.
#[derive(Debug, Clone, Parser)]
#[command(name = "comux", version, about = "Manage a comux Codex proxy")]
pub struct CliArgs {
    /// Temporary management endpoint override.
    #[arg(long, global = true)]
    pub endpoint: Option<String>,
    /// Temporary management bearer-token override.
    #[arg(long, global = true)]
    pub admin_token: Option<String>,
    /// Allows commands to request and print complete sensitive tokens.
    #[arg(long, global = true)]
    pub dangerously_print_full_token: bool,
    /// The command to run, or `None` to open the TUI.
    #[command(subcommand)]
    pub command: Option<Command>,
}

/// Top-level operation selected by the user.
#[derive(Debug, Clone, Subcommand)]
pub enum Command {
    /// Displays listener, selection, and upstream status.
    Status,
    /// Manages configured upstreams.
    Upstream {
        /// The upstream operation to perform.
        #[command(subcommand)]
        action: UpstreamAction,
    },
    /// Reads or writes local CLI connection settings.
    Config(ConfigCommand),
    /// Inspects or rotates the downstream client token.
    Auth {
        /// The downstream-token operation to perform.
        #[command(subcommand)]
        action: AuthAction,
    },
}

/// Nested local-configuration command.
#[derive(Debug, Clone, Args)]
pub struct ConfigCommand {
    /// The operation, or `None` to display resolved configuration.
    #[command(subcommand)]
    pub action: Option<ConfigAction>,
}

/// Operations that inspect or mutate configured upstreams.
#[derive(Debug, Clone, Subcommand)]
pub enum UpstreamAction {
    /// Lists every configured upstream.
    List,
    /// Adds or replaces an upstream from complete options.
    Add(UpstreamOptions),
    /// Replaces an existing upstream while allowing token reuse.
    Set(UpstreamOptionsWithName),
    /// Removes a configured upstream.
    Remove {
        /// The upstream name to remove.
        name: String,
    },
    /// Selects an upstream for new proxy requests.
    Select {
        /// The configured upstream name to select.
        name: String,
    },
}

/// Complete command-line options for adding an upstream.
#[derive(Debug, Clone, Args)]
pub struct UpstreamOptions {
    /// The unique upstream name.
    pub name: String,
    /// The HTTP or HTTPS upstream base URL.
    #[arg(long)]
    pub base_url: String,
    /// The upstream bearer token, or `None` to prompt securely.
    #[arg(long)]
    pub auth_token: Option<String>,
    /// The `always`, `default`, or `never` redirect policy.
    #[arg(long, default_value = "default")]
    pub redirect_policy: String,
}

/// An upstream name paired with replacement fields.
#[derive(Debug, Clone, Args)]
pub struct UpstreamOptionsWithName {
    /// The existing upstream name to replace.
    pub name: String,
    /// The complete set of replacement fields.
    #[command(flatten)]
    pub options: UpstreamFields,
}

/// Replaceable fields for an existing upstream.
#[derive(Debug, Clone, Args)]
pub struct UpstreamFields {
    /// The replacement HTTP or HTTPS base URL.
    #[arg(long)]
    pub base_url: String,
    /// The replacement bearer token, or `None` to reuse the current token.
    #[arg(long)]
    pub auth_token: Option<String>,
    /// The replacement `always`, `default`, or `never` redirect policy.
    #[arg(long, default_value = "default")]
    pub redirect_policy: String,
}

/// Operations on the local CLI configuration file.
#[derive(Debug, Clone, Subcommand)]
pub enum ConfigAction {
    /// Stores one configuration value, prompting when `value` is omitted.
    Set {
        /// The setting to update.
        key: ConfigKey,
        /// The new value, or `None` to prompt.
        value: Option<String>,
    },
    /// Reads one configuration value, prompting if it is absent.
    Get {
        /// The setting to read.
        key: ConfigKey,
    },
}

/// Keys stored in local CLI configuration.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ConfigKey {
    /// The proxy management endpoint.
    Endpoint,
    /// The proxy management bearer token.
    #[value(name = "admin-token")]
    AdminToken,
}

/// Operations on the downstream client token.
#[derive(Debug, Clone, Subcommand)]
pub enum AuthAction {
    /// Prints the current token in masked or explicitly revealed form.
    Print,
    /// Generates and persists a replacement token.
    Regenerate {
        /// Skips the interactive confirmation prompt.
        #[arg(long)]
        yes: bool,
    },
}

/// Retains the path type in this command-definition module at compile time.
#[allow(dead_code)]
fn _path_type(_: PathBuf) {}

#[cfg(test)]
mod tests {
    use super::{CliArgs, Command, ConfigAction, ConfigKey};
    use clap::Parser;

    #[test]
    fn no_subcommand_enters_tui_and_global_flag_is_preserved() {
        let args = CliArgs::try_parse_from(["comux", "--dangerously-print-full-token"]).unwrap();
        assert!(args.command.is_none());
        assert!(args.dangerously_print_full_token);
    }

    #[test]
    fn parses_config_set_and_get() {
        let args =
            CliArgs::try_parse_from(["comux", "config", "set", "endpoint", "http://localhost"])
                .unwrap();
        assert!(matches!(
            args.command,
            Some(Command::Config(super::ConfigCommand {
                action: Some(ConfigAction::Set {
                    key: ConfigKey::Endpoint,
                    ..
                })
            }))
        ));

        let args = CliArgs::try_parse_from(["comux", "config", "get", "admin-token"]).unwrap();
        assert!(matches!(
            args.command,
            Some(Command::Config(super::ConfigCommand {
                action: Some(ConfigAction::Get {
                    key: ConfigKey::AdminToken
                })
            }))
        ));
    }

    #[test]
    fn parses_connection_overrides() {
        let args = CliArgs::try_parse_from([
            "comux",
            "--endpoint",
            "http://localhost:6609",
            "--admin-token",
            "temporary",
            "status",
        ])
        .unwrap();
        assert_eq!(args.endpoint.as_deref(), Some("http://localhost:6609"));
        assert_eq!(args.admin_token.as_deref(), Some("temporary"));
        assert!(matches!(args.command, Some(Command::Status)));
    }
}
