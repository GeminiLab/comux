use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Clone, Parser)]
#[command(name = "comux", version, about = "Manage a comux Codex proxy")]
pub struct CliArgs {
    #[arg(long, global = true)]
    pub endpoint: Option<String>,
    #[arg(long, global = true)]
    pub admin_token: Option<String>,
    #[arg(long, global = true)]
    pub dangerously_print_full_token: bool,
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum Command {
    Status,
    Upstream {
        #[command(subcommand)]
        action: UpstreamAction,
    },
    Config(ConfigCommand),
    Auth {
        #[command(subcommand)]
        action: AuthAction,
    },
}

#[derive(Debug, Clone, Args)]
pub struct ConfigCommand {
    #[command(subcommand)]
    pub action: Option<ConfigAction>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum UpstreamAction {
    List,
    Add(UpstreamOptions),
    Set(UpstreamOptionsWithName),
    Remove { name: String },
    Select { name: String },
}

#[derive(Debug, Clone, Args)]
pub struct UpstreamOptions {
    pub name: String,
    #[arg(long)]
    pub base_url: String,
    #[arg(long)]
    pub auth_token: Option<String>,
    #[arg(long, default_value = "default")]
    pub redirect_policy: String,
}

#[derive(Debug, Clone, Args)]
pub struct UpstreamOptionsWithName {
    pub name: String,
    #[command(flatten)]
    pub options: UpstreamFields,
}

#[derive(Debug, Clone, Args)]
pub struct UpstreamFields {
    #[arg(long)]
    pub base_url: String,
    #[arg(long)]
    pub auth_token: Option<String>,
    #[arg(long, default_value = "default")]
    pub redirect_policy: String,
}

#[derive(Debug, Clone, Subcommand)]
pub enum ConfigAction {
    Set {
        key: ConfigKey,
        value: Option<String>,
    },
    Get {
        key: ConfigKey,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ConfigKey {
    Endpoint,
    #[value(name = "admin-token")]
    AdminToken,
}

#[derive(Debug, Clone, Subcommand)]
pub enum AuthAction {
    Print,
    Regenerate {
        #[arg(long)]
        yes: bool,
    },
}

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
