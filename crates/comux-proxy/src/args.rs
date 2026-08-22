use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "comux-proxy", version, about = "Codex API proxy for comux")]
pub struct ProxyArgs {
    #[arg(long, value_name = "PATH")]
    pub config: PathBuf,
}
