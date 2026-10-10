# comux-core

[![crates.io](https://img.shields.io/crates/v/comux-core.svg)](https://crates.io/crates/comux-core)
[![docs.rs](https://docs.rs/comux-core/badge.svg)](https://docs.rs/comux-core)
[![license](https://img.shields.io/crates/l/comux-core.svg)](https://crates.io/crates/comux-core)

`comux-core` contains the configuration schema, management API data types,
token helpers, atomic configuration persistence, and mutable runtime state
shared by `comux-proxy` and the `comux` CLI.

Requires Rust 1.88 or newer. After a crates.io release is available, library
consumers can add it with:

```sh
cargo add comux-core@0.1.0
```

Most users should run the proxy or CLI instead of depending on this crate
directly. See the [main comux documentation](https://github.com/GeminiLab/comux#readme)
for setup, configuration, security notes, and usage.
