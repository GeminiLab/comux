# comux CLI

The `comux` crate provides the command-line client and ratatui dashboard for a
running `comux-proxy` management endpoint. It stores local connection settings
in `~/.config/comux.toml`; running `comux` without a subcommand opens the TUI.

Requires Rust 1.88 or newer. After a crates.io release is available, install
both the management client and the proxy:

```sh
cargo install --locked comux
cargo install --locked comux-proxy
```

Start `comux-proxy` with an explicit configuration path, then connect the
client to its management listener:

```sh
comux config
comux status
```

From a source checkout:

```sh
cargo run -p comux -- status
cargo run -p comux -- upstream list
```

See the [main comux documentation](https://github.com/GeminiLab/comux#readme)
for initial setup, all commands, TUI keys, token-handling rules, and the
management API.
