# comux-proxy

`comux-proxy` is the comux HTTP proxy. It exposes a downstream endpoint for
Codex and a separately authenticated management endpoint for the `comux` CLI.
The library modules are also used by the integration tests.

Requires Rust 1.88 or newer. After a crates.io release is available:

```sh
cargo install --locked comux-proxy
cargo install --locked comux
```

Run it with an explicit configuration path:

```sh
comux-proxy --config /path/to/comux.toml
```

Or run from a source checkout:

```sh
cargo run -p comux-proxy -- --config /path/to/comux.toml
```

See the [main comux documentation](https://github.com/GeminiLab/comux#readme)
for the complete configuration format, Codex setup, API reference, redirect
security notes, and troubleshooting.
