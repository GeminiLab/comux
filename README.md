# comux

comux is a local Codex API proxy and upstream selector. The proxy exposes one downstream HTTP endpoint for Codex and a separate token-authenticated admin endpoint for the `comux` CLI. Every downstream request is forwarded to the currently selected upstream with that upstream's token; the downstream token is never sent upstream.

## Build and run

```sh
cargo build --workspace
cargo run -p comux-proxy -- --config /path/to/comux.toml
```

通过 `cargo run` 启动时，`--` 是必须的：它把后面的参数交给 `comux-proxy`，否则 Cargo 会把 `--config` 当成自己的配置参数并尝试解析 TOML。直接运行已编译的代理时不需要这个分隔符：

```sh
cargo run -p comux-proxy -- --config "./1.config"
# 或
./target/debug/comux-proxy --config "./1.config"
```

The proxy configuration path is always supplied with `--config`. If `admin.token` or `downstream.token` is empty or absent, the proxy generates a token, atomically writes the file, and prints each generated token once. The default listeners are loopback only: downstream `127.0.0.1:6608` and admin `127.0.0.1:6609` (the hostname `localhost` is also valid). Set `bind` explicitly when exposing either listener on another interface.

An example configuration is in [examples/comux.proxy.toml](examples/comux.proxy.toml).

## Codex and CLI setup

Put the downstream URL and generated downstream token in the Codex configuration/auth file. The CLI stores its admin endpoint and admin token in `~/.config/comux.toml` on Linux and macOS:

```sh
comux config set endpoint http://127.0.0.1:6609
comux config set admin-token
comux status
comux upstream list
comux auth print
```

When `set` has no value, the CLI prompts. Admin tokens are hidden at the prompt. All normal output masks tokens by keeping only the first and last four characters; pass `--dangerously-print-full-token` when a full value is explicitly required. `--endpoint` and `--admin-token` are temporary overrides. Running `comux` without a subcommand opens the ratatui dashboard; `j`/`k` select an upstream, Enter activates it, `d` deletes it, `r` refreshes, and `t` regenerates the downstream client token.

## Configuration model

The reserved upstream name `null` is not stored in the `upstreams` table. Selecting it (or deleting the active upstream) makes downstream requests return a structured `503` error. Mutating admin API calls validate a prospective configuration, atomically persist it, and then replace the in-memory state. Existing requests keep their captured upstream; selection changes affect new requests.

Each upstream has `redirect_policy = "always"`, `"default"`, or `"never"`. `always` follows redirects and re-injects the upstream token on every hop, including cross-host redirects, so use it only with trusted upstreams.

Usage statistics are in-memory and exposed at `GET /api/v1/stats`; they reset when the proxy restarts. They include accepted requests, response classes, network errors, request/response bytes, duration totals/maxima, and active streams per upstream.

## Admin API

All `/api/v1` endpoints require `Authorization: Bearer <admin.token>`. The main endpoints are:

* `GET /api/v1/status`
* `GET /api/v1/stats`
* `GET /api/v1/upstreams`
* `PUT|DELETE /api/v1/upstreams/{name}`
* `POST /api/v1/active` with `{"name":"..."}`
* `GET /api/v1/auth/client-token`
* `POST /api/v1/auth/client-token/regenerate`

Client and upstream tokens are masked by default. The CLI's dangerous full-token mode requests `?reveal=full`; the proxy only returns full values for that explicit, authenticated request.
