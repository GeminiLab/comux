use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "comux-proxy", version, about = "Codex API proxy for comux")]
pub struct ProxyArgs {
    #[arg(long, value_name = "PATH")]
    pub config: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::ProxyArgs;
    use clap::Parser;

    #[test]
    fn parses_config_path_after_cargo_argument_separator() {
        let args = ProxyArgs::try_parse_from(["comux-proxy", "--config", "./1.config"])
            .expect("proxy arguments should parse");
        assert_eq!(args.config, std::path::PathBuf::from("./1.config"));
    }
}
