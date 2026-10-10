# comux CLI

The `comux` crate provides the command-line client and ratatui dashboard for a
running `comux-proxy` management endpoint. It stores local connection settings
in `~/.config/comux.toml`; running `comux` without a subcommand opens the TUI.

```sh
cargo run -p comux -- status
cargo run -p comux -- upstream list
```

See the [main comux documentation](https://github.com/GeminiLab/comux#readme)
for initial setup, all commands, TUI keys, token-handling rules, and the
management API.
