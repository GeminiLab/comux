# comux

comux is a local Codex API proxy and upstream selector. Codex sends requests to
one downstream endpoint, while the `comux` CLI and TUI use a separately
authenticated management endpoint. Each downstream request is forwarded to a
snapshot of the currently selected upstream with that upstream's credential;
the downstream credential is never sent upstream.

> [!IMPORTANT]
> This project was created almost entirely with AI. Treat its implementation,
> configuration, and security properties as code that still requires your own
> review before use, especially before exposing listeners or supplying
> credentials.

## Architecture

The Rust workspace contains three crates:

- [`comux-core`](crates/comux-core/README.md) defines configuration, management
  API data types, token helpers, atomic persistence, and validated runtime
  state.
- [`comux-proxy`](crates/comux-proxy/README.md) serves the downstream data
  plane and the separate management plane.
- [`comux`](crates/comux-cli/README.md) provides the command-line client and
  ratatui dashboard.

```text
Codex -- downstream token --> comux-proxy :6608 -- upstream token --> selected API
comux CLI/TUI -- admin token --> comux-proxy :6609 --> state + persisted config
```

The two HTTP planes share runtime state but have separate listeners, routes,
and credentials. A downstream request takes an owned snapshot of the selected
upstream before forwarding, so selection changes affect new requests but do
not reroute requests already in flight. Management mutations validate and
persist a candidate configuration before publishing it to shared state.

Transport statistics are held in memory, grouped by upstream, and reset when
the proxy process restarts.

## Requirements and installation

Install a stable Rust toolchain with Cargo, then build the complete workspace:

```sh
cargo build --workspace
```

You can install both binaries from the checkout:

```sh
cargo install --path crates/comux-proxy
cargo install --path crates/comux-cli
```

To run without installing:

```sh
cargo run -p comux-proxy -- --config /path/to/comux.toml
```

The `--` is required when using `cargo run`: it separates Cargo's arguments
from the `comux-proxy` arguments. A directly executed binary does not need that
separator:

```sh
./target/debug/comux-proxy --config /path/to/comux.toml
```

## Quick start

1. Copy [the example proxy configuration](examples/comux.proxy.toml) and
   replace the upstream URL and credential. Listener tokens may remain empty
   for the first start.
2. Start `comux-proxy` with the explicit `--config` path.
3. Record the generated admin and downstream tokens. Each generated value is
   printed only by the startup that created it and is persisted to the proxy
   configuration.
4. Point Codex at `http://127.0.0.1:6608` and configure the generated
   downstream token as its Bearer credential.
5. Run `comux config`, enter `http://127.0.0.1:6609` and the generated admin
   token, then verify the connection with `comux status`.

Both listeners default to loopback-oriented `localhost` binds. Review the
[security guidance](#security) before binding either listener to another
interface.

## Proxy configuration

The proxy configuration path is always supplied through `--config`. A complete
configuration looks like this:

```toml
active_upstream = "openai"

[admin]
bind = "127.0.0.1"
port = 6609
token = ""

[downstream]
bind = "127.0.0.1"
port = 6608
token = ""

[connection]
connect_timeout_secs = 10
idle_timeout_secs = 120

[upstreams.openai]
base_url = "https://api.openai.com/v1"
auth_token = "replace-with-your-upstream-token"
redirect_policy = "default"
```

If the file is missing, comux creates it from defaults. If `admin.token` or
`downstream.token` is empty or absent, startup generates a token, atomically
rewrites the file, and prints the generated value once.

### Top-level fields

- `active_upstream` selects the target used by new downstream requests. It
  must name an entry under `upstreams` or use the reserved value `null`.
- `[admin]` configures the management listener used by the CLI and TUI.
- `[downstream]` configures the listener used by Codex.
- `[connection]` configures upstream connection and body-stream timeouts.
- `[upstreams.<name>]` defines one selectable upstream. Names must be non-empty
  and cannot be `null`.

The reserved name `null` is never stored in the upstream map. When it is
selected, authenticated downstream requests receive a structured `503
Service Unavailable` response. Deleting the active upstream automatically
selects `null`.

### Listener fields

Both `[admin]` and `[downstream]` use the same fields:

- `bind` is a hostname or IP address and defaults to `localhost`.
- `port` must be non-zero. The admin default is `6609`; the downstream default
  is `6608`.
- `token` is the Bearer credential accepted by that listener. An empty or
  missing value is generated during proxy initialization.

### Connection fields

- `connect_timeout_secs` defaults to `10`, must be positive, and limits
  upstream connection establishment.
- `idle_timeout_secs` defaults to `120`, must be positive, and limits idle time
  between request or response body chunks. It is not a total request-duration
  limit, so active long-lived streams can remain open.

### Upstream fields

- `base_url` must be a valid HTTP or HTTPS URL. The downstream request path is
  appended to its existing base path, and the downstream query is retained.
- `auth_token` is required and is sent upstream as a Bearer credential. The
  downstream Authorization header is not forwarded.
- `redirect_policy` is `always`, `default`, or `never` and defaults to
  `default`.

Redirect behavior:

- `always` makes comux follow up to ten redirect hops itself and inject the
  upstream token at every hop. `307` and `308` preserve the method and replay
  the recorded body; other redirects convert non-GET/HEAD requests to GET.
  This policy sends the credential across hosts when directed to do so and
  must be used only with trusted redirect targets.
- `default` uses reqwest's default redirect policy.
- `never` does not follow redirects and returns the upstream redirect response
  to Codex.

Hop-by-hop transport headers are removed in both directions. Other downstream
headers are forwarded except Authorization, which is replaced by the selected
upstream token.

### Persistence

Configuration writes use a temporary sibling file, flush and sync it, then
atomically replace the destination. On Unix, comux applies mode `0600` to
configuration files. On non-Unix platforms it does not currently apply an
equivalent access-control change.

## Codex setup

Configure Codex to use the downstream listener as its API base URL and the
downstream token as its Bearer credential. For the default listener:

```text
Base URL: http://127.0.0.1:6608
Token:    <downstream.token>
```

Codex configuration locations and field names can vary by release, so consult
the documentation for the installed Codex version. Do not use the admin token
for Codex: it authenticates only management requests on the separate admin
listener.

## CLI configuration

The CLI stores its management endpoint and admin token in
`~/.config/comux.toml` when a home directory is available. The simplest
initial setup prompts for both values and saves the completed configuration:

```sh
comux config
comux status
```

For separate `config set` calls on a new file, store the admin token first.
The current command validates the complete configuration after every update,
so setting the endpoint first would leave a missing token and fail validation:

```sh
comux config set admin-token
comux config set endpoint http://127.0.0.1:6609
```

Omitting a `set` value prompts for it; admin-token input is hidden. The global
`--endpoint` and `--admin-token` options temporarily override file values.
Resolution order is command-line override, non-empty file value, then an
interactive prompt. Missing values fail when stdin is not interactive.

Normal output masks tokens. The global `--dangerously-print-full-token` flag
allows commands to request and print complete values. Use it only when the
value is explicitly needed, and avoid capturing its output in shell history or
logs.

## CLI commands

```text
comux status
comux upstream list
comux upstream add <name> --base-url <url> [--auth-token <token>] [--redirect-policy <policy>]
comux upstream set <name> --base-url <url> [--auth-token <token>] [--redirect-policy <policy>]
comux upstream remove <name>
comux upstream select <name>
comux config
comux config get <endpoint|admin-token>
comux config set <endpoint|admin-token> [value]
comux auth print
comux auth regenerate [--yes]
```

- `status` displays listeners, the active upstream, and configured names.
- `upstream list` masks credentials unless dangerous full-token output is
  enabled.
- `upstream add` prompts securely when `--auth-token` is omitted.
- `upstream set` fetches and reuses the existing full credential when
  `--auth-token` is omitted because the management API accepts complete
  replacements.
- `upstream remove` selects `null` if the removed target was active.
- `auth regenerate` asks for confirmation unless `--yes` is supplied.

## Terminal dashboard

Running `comux` without a subcommand opens the ratatui dashboard. It refreshes
status, statistics, and upstreams every two seconds.

Dashboard keys:

- `j`/`k` or Down/Up: move the upstream selection.
- Enter: activate the selected upstream.
- `a`: open the add-upstream form.
- `d`: delete the selected upstream.
- `r`: refresh immediately.
- `t`: regenerate the downstream client token.
- `q` or Esc: exit.

Add-form keys:

- Type and Backspace edit the current text field.
- Enter or Tab advances to the next field.
- Left, Right, or Space changes redirect policy.
- Enter on redirect policy validates and submits the form.
- Esc cancels.

Automatic refresh pauses while the add form is open so remote state cannot
replace data used between keystrokes.

## Management API

All routes below are served by the admin listener and require:

```http
Authorization: Bearer <admin.token>
```

Upstream and downstream tokens are masked by default. An authenticated request
must include `?reveal=full` to receive a complete supported credential.

### `GET /api/v1/status`

Returns `200 OK` with listener details and configured names:

```json
{
  "active_upstream": "openai",
  "downstream_bind": "127.0.0.1",
  "downstream_port": 6608,
  "admin_bind": "127.0.0.1",
  "admin_port": 6609,
  "upstream_names": ["openai"]
}
```

### `GET /api/v1/stats`

Returns `200 OK` with process-local counters grouped by upstream:

```json
{
  "by_upstream": {
    "openai": {
      "accepted_requests": 1,
      "responses_2xx": 1,
      "responses_3xx": 0,
      "responses_4xx": 0,
      "responses_5xx": 0,
      "network_errors": 0,
      "request_bytes": 128,
      "response_bytes": 1024,
      "duration_count": 1,
      "duration_total_ms": 250,
      "duration_max_ms": 250,
      "active_streams": 0
    }
  }
}
```

### `GET /api/v1/upstreams`

Returns `200 OK` with all upstream summaries. `auth_token` is masked unless the
request uses `?reveal=full`.

```json
[
  {
    "name": "openai",
    "base_url": "https://api.openai.com/v1",
    "auth_token": "repl************************oken",
    "redirect_policy": "default"
  }
]
```

### `PUT /api/v1/upstreams/{name}`

Creates or completely replaces an upstream and returns `204 No Content`:

```json
{
  "base_url": "https://api.example.com/v1",
  "auth_token": "secret",
  "redirect_policy": "default"
}
```

The prospective complete configuration is validated and persisted before it
becomes live.

### `DELETE /api/v1/upstreams/{name}`

Deletes the upstream and returns `204 No Content`. Deleting the active upstream
selects `null`.

### `POST /api/v1/active`

Selects an existing upstream and returns `204 No Content`:

```json
{"name":"openai"}
```

### `GET /api/v1/auth/client-token`

Returns `200 OK` with the masked downstream token. Use `?reveal=full` for its
complete value:

```json
{"token":"clie************************oken"}
```

### `POST /api/v1/auth/client-token/regenerate`

Generates and persists a replacement downstream token, then returns `200 OK`
with the new masked value. Use `?reveal=full` when the caller must receive the
complete replacement.

### Errors

API failures use this JSON envelope:

```json
{"error":{"type":"invalid_request","message":"..."}}
```

Implemented categories include:

- `unauthorized` with `401` for a missing, malformed, or incorrect listener
  token.
- `invalid_request` with `400` for an invalid upstream replacement.
- `not_found` with `404` for unknown upstream deletion or selection.
- `persistence_failed` with `500` when a validated management mutation cannot
  be saved.
- `no_active_upstream` with `503` on the downstream plane when `null` is
  selected.
- `upstream_error` with `502` when forwarding fails.

These categories describe the current implementation and are not a versioned
API-stability guarantee.

## Security

comux handles three different credential classes:

1. The **admin token** authenticates CLI/TUI management requests on the admin
   listener.
2. The **downstream token** authenticates Codex requests on the downstream
   listener.
3. Each **upstream token** is sent only when forwarding to its configured
   target, except that `redirect_policy = "always"` deliberately re-injects it
   at every redirect hop.

Keep both listeners on loopback unless remote access is intentionally secured
by additional network controls. The built-in listeners use plain HTTP and do
not terminate TLS.

Proxy configuration contains all listener and upstream credentials. Restrict
access to the live file and its backups. Unix writes use mode `0600`; verify
permissions yourself on other platforms.

Treat `?reveal=full` and `--dangerously-print-full-token` as secret-export
operations. Avoid terminal recording, logs, shell expansion, and copied output
when using them.

Use `redirect_policy = "always"` only for trusted redirect destinations. A
cross-host redirect receives the same upstream credential by design.

## Troubleshooting

### Cargo treats `--config` as its own option

Include the argument separator:

```sh
cargo run -p comux-proxy -- --config /path/to/comux.toml
```

### `401 Unauthorized`

Check that the request targets the correct listener. Codex uses the downstream
token on port 6608 by default; the CLI and management API use the admin token
on port 6609.

### `503 no_active_upstream`

Set `active_upstream` to a configured name or run:

```sh
comux upstream select <name>
```

The same response is expected after the active upstream is deleted, because
selection falls back to `null`.

### `502 upstream_error`

Check the upstream URL, network reachability, credential, redirect target,
connect timeout, and body idle timeout. Proxy logs include the underlying
forwarding error while the downstream response stays generic.

### CLI configuration fails without prompting

Interactive prompts require a terminal. Provide both `--endpoint` and
`--admin-token`, or create `~/.config/comux.toml` before running from a script.

On a new local file, separate `config set` calls must store `admin-token`
before `endpoint` because each save validates the resulting complete
configuration.

### Listener bind failure

Verify the configured host exists locally, the port is not already in use, and
the process has permission to bind it.

## Development and verification

Run the full verification suite from the repository root:

```sh
cargo fmt --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --document-private-items
```

The crate-specific documentation is available in:

- [`comux-core`](crates/comux-core/README.md)
- [`comux-proxy`](crates/comux-proxy/README.md)
- [`comux` CLI/TUI](crates/comux-cli/README.md)

## License

comux is licensed under the MIT License.
