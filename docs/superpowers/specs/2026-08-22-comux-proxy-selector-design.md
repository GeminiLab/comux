# comux Codex API Proxy and Selector Design

## Goal

`comux` is a Rust workspace that provides a Codex API reverse proxy and a CLI
selector. The proxy exposes a Codex-compatible HTTP base URL, stores upstream
credentials, and routes every request to the currently selected upstream. The
CLI communicates with the proxy over an independently authenticated HTTP
management interface.

The first version supports Linux and macOS. comux provides HTTP only; remote
deployments are expected to put it behind an HTTPS reverse proxy.

## Workspace and Processes

The Cargo workspace contains three crates:

- `comux-core`: TOML schemas and validation, upstream/selection models, token
  masking, management API DTOs, and atomic file persistence. It does not own an
  HTTP server or client.
- `comux-proxy`: the proxy binary, two HTTP listeners, streaming forwarding,
  management state, logging, graceful shutdown, and CLI parsing for the proxy
  configuration path.
- `comux-cli`: the `comux` CLI, local CLI configuration, and the management API
  client.

The proxy and CLI share types through `comux-core`; their runtime communication
is the versioned management HTTP API.

## Library Decisions

`veil` is suitable as an optional defense for derived `Debug` implementations:
its documented purpose is redacting selected struct or enum fields from debug
output. It does not provide secret storage, prevent cloning/serialization, or
define comux's exact masking contract. The required head-four/tail-four mask is
therefore implemented explicitly in `comux-core`; `veil` is not required for
the first version.

`ratatui` is a maintained terminal UI toolkit for immediate-mode widgets and
interactive terminal applications. Because running `comux` without a
subcommand enters an interactive dashboard/editor, `ratatui` with its default
`crossterm` backend is an initial dependency of `comux-cli`. The TUI is kept in
the CLI crate and does not leak terminal concerns into `comux-core` or the
proxy.

## Proxy Configuration

The proxy configuration path is always supplied with
`comux-proxy --config <PATH>`. There is no implicit proxy configuration path.
The file is TOML and is created when absent.

Example:

```toml
active_upstream = "openai"

[admin]
bind = "127.0.0.1"
port = 6609
token = "replace-with-generated-admin-token"

[downstream]
bind = "127.0.0.1"
port = 6608
token = "replace-with-generated-client-token"

[connection]
connect_timeout_secs = 10
idle_timeout_secs = 120

[upstreams.openai]
base_url = "https://api.openai.com/v1"
auth_token = "replace-with-upstream-token"
redirect_policy = "default"

[upstreams.internal]
base_url = "https://gateway.example.test/codex"
auth_token = "replace-with-another-upstream-token"
redirect_policy = "never"
```

`[downstream].bind` and `[admin].bind` default to loopback (`localhost`;
explicit `127.0.0.1` or `::1` is also supported), never a wildcard address.
`[downstream].port` defaults to `6608` and `[admin].port` defaults to `6609`.
Users may configure any address and port explicitly. If either bind fails (for
example because the port is already in use), startup exits with the exact
listener and address in the error.

If either `[downstream].token` or `[admin].token` is missing, the proxy
generates a cryptographically random token, atomically writes the completed
file, and prints the generated value during that startup. Existing values are
never printed merely because the proxy restarted. Configuration writes use a
temporary file, flush/sync, and atomic rename; Unix files are restricted to
the current user (`0600` where supported), with equivalent user-private ACLs
on Windows if support is later added.

`active_upstream` is a configured upstream name or the reserved value
`"null"`. Missing `active_upstream` defaults to `"null"`. Removing the active
upstream persists `"null"`; no arbitrary upstream is selected automatically.
The `null` upstream never opens an external connection.

Each upstream has a unique name, an HTTP/HTTPS `base_url` (including an
optional path prefix), an `auth_token`, and a `redirect_policy`:

- `always`: follow redirects, including cross-authority redirects, and send
  the configured upstream token on every followed request. This is explicitly
  a credential-leaking policy and is documented as such.
- `default`: use the selected HTTP client's default redirect behavior.
- `never`: do not follow; return the upstream redirect response unchanged.

`[connection].connect_timeout_secs` and `[connection].idle_timeout_secs` are
positive and configurable. The defaults are 10 seconds and 120 seconds. There
is no total response timeout and no fixed request-body size limit.

## CLI Configuration

The CLI always defaults to `~/.config/comux.toml` on Linux and macOS. It stores
only the management endpoint and the `admin_token`:

```toml
admin_endpoint = "http://127.0.0.1:6609"
admin_token = "replace-with-admin-token"
```

If this file or either key is missing, an interactive command asks for the
endpoint and reads the admin token without echoing it. Non-interactive use
fails with a clear missing-configuration error. Global `--endpoint` and
`--admin-token` flags temporarily override the local file for one command.

`comux config [set|get] <key> [<value>]` manages this local CLI configuration.
If `value` is omitted, `set` prompts for it; the admin token prompt is hidden,
while endpoint input is visible. `comux config` with no subcommand prints both
keys. `config set` performs an atomic local file replacement.

Running `comux` without a subcommand enters the ratatui TUI. It automatically
refreshes status, upstreams, and statistics every two seconds, and refreshes
immediately after a successful mutation. The TUI provides views for current
status/statistics, upstream add/edit/delete/select, client-token print or
regenerate, and local CLI endpoint/admin-token settings. It uses the same
masking rules and dangerous full-token flag as non-interactive commands.

## Token Masking and Authentication

All admin endpoints require `Authorization: Bearer <admin token>`, including
endpoints that do not return credentials. The downstream listener separately
requires `Authorization: Bearer <client token>` (`[downstream].token`).

The proxy replaces the downstream authorization header with
`Authorization: Bearer <upstream.auth_token>` before forwarding. The
`[downstream].token` is never sent upstream. `Host`, `Content-Length`,
`Connection`, and other hop-by-hop headers are rebuilt by the HTTP stack; other
safe headers are forwarded.

Default token masking keeps the first and last four characters and replaces
the middle with `*` (for example `abcd********wxyz`). Tokens shorter than nine
characters are rendered entirely as `*`. The global CLI flag
`--dangerously-print-full-token` disables masking only for an explicitly
requested output operation.

The client-token endpoint returns a masked value by default. The CLI sends an
explicit reveal request only for `auth print --dangerously-print-full-token`;
the server then returns the full value after normal admin authentication.
The local CLI `admin_token` is masked locally by default because it never goes
through the server for printing.

`comux auth print` reads the current client token through the management API.
`comux auth regenerate` generates, atomically persists, and immediately
activates a new client token; the old token stops working at once. Regeneration
requires interactive confirmation or an explicit non-interactive confirmation
flag. `[admin].token` rotation is deliberately outside this command and
requires editing the proxy configuration and restarting the proxy.

## Proxy Data Plane

The proxy listener accepts every HTTP method and path. It preserves the
downstream path and query while joining it to the upstream `base_url` using
RFC 3986 URL rules, retaining any upstream path prefix. Request and response
bodies are streamed end-to-end; SSE (`text/event-stream`) is not buffered.

At request start, the handler captures the current upstream. A later selection
change affects only new requests; in-flight requests continue against their
captured upstream until completion or disconnect.

When `active_upstream = "null"`, requests receive:

```http
HTTP/1.1 503 Service Unavailable
Content-Type: application/json
```

```json
{"error":{"type":"no_active_upstream","message":"no active upstream configured"}}
```

Upstream redirects obey the per-upstream policy. Redirect processing preserves
streaming for the final response, does not expose stored credentials in logs,
and enforces the HTTP client's redirect limit.

## Management API

The admin listener is separate from the proxy listener and uses the same
loopback defaults. Its versioned prefix is `/api/v1`:

- `GET /api/v1/status`: active upstream, listener information, and names;
  tokens are masked.
- `GET /api/v1/stats`: in-memory transport statistics grouped by upstream.
- `GET /api/v1/upstreams`: names, base URLs, and masked tokens/policies.
- `PUT /api/v1/upstreams/{name}`: add or replace a complete upstream.
- `DELETE /api/v1/upstreams/{name}`: delete an upstream; deleting the active
  one atomically selects `null`.
- `POST /api/v1/active` with `{"name":"..."}`: select an existing upstream.
- `GET /api/v1/auth/client-token`: return the masked client token by default;
  an explicit reveal parameter, used only by the dangerous CLI flag, returns
  the full value.
- `POST /api/v1/auth/client-token/regenerate`: rotate and activate the client
  token.

Management changes validate the complete prospective configuration, write it
atomically, and only then replace in-memory state. A failed write leaves the
running state unchanged. Management mutations are serialized inside the
proxy. Listener addresses and ports are not mutable through this API and
require a restart.

All management errors are structured JSON: `401` for authentication failure,
`400` for invalid input, `404` for missing resources, `409` for name/state
conflicts, and `500` for persistence failures.

## CLI Commands

```text
comux status
comux upstream list
comux upstream add <name> --base-url <url> --auth-token <token> [--redirect-policy <always|default|never>]
comux upstream set <name> ...
comux upstream remove <name>
comux upstream select <name>
comux config [set|get] <key> [<value>]
comux auth print
comux auth regenerate
```

CRUD and selection are performed through the admin API; the CLI never edits
the proxy configuration directly. Token-bearing arguments are supported for
automation, while interactive commands prompt without echoing secrets. Status,
config, upstream listing, and auth print are masked by default. The dangerous
global flag is required to print full tokens.

## Usage Statistics

The proxy keeps in-memory transport counters per upstream and resets them on
restart. It counts accepted requests, completed responses by status class,
network/connection errors, request and response bytes, request duration
aggregate (count, total, and maximum), and active streams. It does not parse
JSON or SSE bodies for token usage and never persists request bodies or
statistics. Streaming counters update as bytes flow and finalize when the
request ends or is disconnected. The admin stats endpoint and TUI expose only
these aggregate values.

## Redirect, Error, and Privacy Behavior

The proxy does not automatically follow redirects unless the configured policy
allows it. For `always`, cross-authority requests intentionally carry the
upstream token because that policy explicitly opts into the risk. For
`never`, the upstream 3xx response is returned unchanged, including its
`Location` header.

Connection, DNS, TLS, and timeout failures before an upstream response map to
`502` JSON responses and detailed but credential-free internal logs. If a
stream fails after response bytes have been sent, the connection is closed;
the proxy cannot replace already-sent bytes with a new JSON response.

Logs include method, path, status, duration, upstream name, and error class.
They do not include authorization headers, upstream tokens, request bodies, or
sensitive query values.

## Testing

`comux-core` unit tests cover TOML validation/defaults, token generation and
masking, atomic writes, and null selection behavior. Proxy integration tests
use local mock upstreams to cover authentication replacement, URL joining,
streaming/SSE, redirect policies, timeouts, and null-upstream responses.
Management tests cover admin authentication, CRUD, serialized writes, token
reveal and regeneration, and all error status mappings. CLI tests cover the
default path, interactive missing values, temporary overrides, `config`, auth
commands, masking, the dangerous flag, and headless TUI rendering/actions. No
test contacts a real external API.
