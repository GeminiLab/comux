# Documentation Coverage Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the complete comux workspace accurate English Rustdoc, focused implementation comments, a comprehensive root README, and concise crate READMEs without changing runtime behavior.

**Architecture:** Documentation remains closest to its source: Rustdoc describes every production Rust item, ordinary comments explain non-obvious control-flow decisions, and the root README is the canonical user and contributor guide. Each crate README is intentionally short and links back to the root README to prevent duplicated guidance from drifting.

**Tech Stack:** Rust 2021, rustdoc, Markdown, Cargo, rustfmt, Clippy

## Global Constraints

- Write all new documentation and comments in English.
- Document every production crate, public or private module, type, field, enum variant, constant, function, and method.
- Do not add item-by-item Rustdoc to test functions; document only complex test fixtures or helpers when their setup is non-obvious.
- Add ordinary comments only at important phase boundaries or design trade-offs; do not paraphrase obvious code.
- Do not change runtime behavior, public interfaces, configuration schemas, or dependency versions.
- Keep the root `README.md` canonical and the three crate READMEs concise.
- State neutrally in the root README that the project was created almost entirely with AI and that users remain responsible for review.
- Do not copy tokens or other values from `.local/config.toml` into documentation or examples.

---

## File Structure

- `README.md`: canonical project overview, architecture, installation, full configuration reference, Codex and CLI setup, CLI/TUI usage, Admin API, security, troubleshooting, development, and license.
- `crates/comux-core/README.md`: concise shared-library description and root README link.
- `crates/comux-proxy/README.md`: concise proxy-binary/library description and root README link.
- `crates/comux-cli/README.md`: concise CLI/TUI description and root README link.
- `crates/comux-core/src/*.rs`: crate/module/item Rustdoc for configuration, DTOs, errors, persistence, secrets, and runtime state.
- `crates/comux-proxy/src/*.rs`: crate/module/item Rustdoc for startup, admin routing, authentication, forwarding, serving, and statistics.
- `crates/comux-cli/src/*.rs`: crate/module/item Rustdoc for arguments, client calls, command dispatch, configuration, and the TUI.
- `docs/superpowers/specs/2026-10-10-documentation-coverage-design.md`: approved requirements; do not modify during implementation.

---

### Task 1: Document `comux-core`

**Files:**
- Modify: `crates/comux-core/src/lib.rs`
- Modify: `crates/comux-core/src/api.rs`
- Modify: `crates/comux-core/src/config.rs`
- Modify: `crates/comux-core/src/error.rs`
- Modify: `crates/comux-core/src/persistence.rs`
- Modify: `crates/comux-core/src/secrets.rs`
- Modify: `crates/comux-core/src/state.rs`
- Create: `crates/comux-core/README.md`

**Interfaces:**
- Consumes: the current `comux-core` public API and serialized configuration/API shapes.
- Produces: unchanged Rust interfaces with complete public/private Rustdoc and a concise crate landing page.

- [ ] **Step 1: Turn public documentation gaps into build failures**

Add the missing-docs lint and crate overview at the top of `src/lib.rs`:

```rust
//! Shared configuration, persistence, secrets, API types, and runtime state
//! for the comux proxy and management clients.
//!
//! This crate contains no HTTP server or terminal UI. It defines the data and
//! state-management contracts used by both `comux-proxy` and `comux`.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
```

Add one-sentence `//!` module overviews to `api.rs`, `config.rs`, `error.rs`,
`persistence.rs`, `secrets.rs`, and `state.rs`.

- [ ] **Step 2: Run rustdoc and confirm the lint finds undocumented public items**

Run:

```sh
RUSTDOCFLAGS="-D warnings" cargo doc -p comux-core --no-deps --document-private-items
```

Expected: failure containing `missing documentation` for currently
undocumented public types, fields, variants, functions, or methods.

- [ ] **Step 3: Document configuration and errors**

In `config.rs`, add Rustdoc with the following exact semantics:

- `NULL_UPSTREAM`: reserved sentinel selecting no upstream; it never appears
  in the `upstreams` map.
- `RedirectPolicy`: controls who follows upstream redirects.
  - `Always`: comux follows redirects itself and injects the configured
    upstream credential on every hop, including cross-host hops.
  - `Default`: reqwest's default redirect policy.
  - `Never`: redirects are returned to the downstream client.
- `UpstreamConfig`: one named target; document `base_url`, `auth_token`, and
  `redirect_policy`, including that the token is sensitive.
- `ListenerConfig`: bind host, TCP port, and bearer token for one listener.
- `ListenerConfig::admin_default` and `downstream_default`: default loopback
  host with ports 6609 and 6608 respectively and an empty token that startup
  initialization replaces.
- `ConnectionConfig`: connect timeout and per-chunk idle timeout, both in
  seconds; document its `Default::default` values as 10 and 120.
- `ProxyConfig`: complete persisted proxy state; document all five fields and
  the `null` active-upstream sentinel.
- `ProxyConfig::default`: creates loopback listeners, no upstreams, `null`
  active selection, and default connection limits.
- `ProxyConfig::validate`: rejects zero ports/timeouts, missing active targets,
  empty/reserved names, empty upstream tokens, invalid URLs, and non-HTTP(S)
  URLs; it does not require listener tokens because initialization may
  generate them.
- `CliConfig`: local management endpoint and admin credential.
- `CliConfig::validate`: requires a valid HTTP(S) endpoint and non-empty token.
- `CliConfig::validate_partial`: allows missing values while validating a
  present endpoint.
- `default_bind`, `default_connect_timeout`, and `default_idle_timeout`:
  serde default providers for `localhost`, 10 seconds, and 120 seconds.

In `error.rs`, document `ConfigError` and each variant:

```rust
/// An error produced while validating, reading, or writing configuration.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// A parsed configuration violates a semantic validation rule.
    #[error("invalid configuration: {0}")]
    Invalid(String),
    /// Reading, creating, syncing, or replacing a configuration file failed.
    #[error("configuration I/O failed: {0}")]
    Io(#[from] std::io::Error),
    /// TOML serialization or deserialization failed.
    #[error("configuration serialization failed: {0}")]
    Serialization(String),
}
```

Add ordinary comments in `ProxyConfig::validate` only before the active-target
check and the per-upstream validation loop, explaining the reserved sentinel
and that each persisted target must be independently usable.

- [ ] **Step 4: Document management API DTOs**

In `api.rs`, document every type and field:

- `StatusResponse`: current active name, both listener addresses, and sorted
  upstream names.
- `UpstreamSummary`: management-facing name, URL, masked or explicitly
  revealed token, and redirect policy.
- `UpstreamSummary::masked`: constructs a summary while masking the token.
- `UpstreamMutation`: complete replacement payload for an upstream.
- `ActiveMutation`: payload naming the upstream to activate.
- `ClientTokenResponse`: masked by default and full only for authenticated
  `reveal=full`.
- `TransportStats`: accepted requests, status-class counts, network failures,
  directional bytes, completed-response duration count/total/max, and current
  active streams; state that values are process-local.
- `StatsResponse`: statistics keyed by upstream name.
- `ApiErrorBody`: standard management/data-plane error envelope.
- `ApiError`: machine-readable `type` and human-readable `message`.

Use field comments such as:

```rust
/// The machine-readable error category serialized as `type`.
#[serde(rename = "type")]
pub error_type: String,
/// A human-readable explanation of the failure.
pub message: String,
```

- [ ] **Step 5: Document persistence and add atomic-write comments**

In `persistence.rs`, document:

- `LoadedProxyConfig` and all fields, including that the boolean flags control
  one-time startup token display.
- `load_proxy_config_or_initialize`: load existing TOML or defaults, generate
  absent listener tokens, validate, and persist when initialization changed
  the file.
- `save_proxy_config`: validate before atomically replacing the file.
- `load_cli_config`: return `None` for a missing file and accept partial local
  configuration.
- `save_cli_config`: validate only fields currently present.
- `save_toml`: create the parent, serialize pretty TOML, write/sync a sibling
  temporary file, persist it, and enforce private permissions.
- Both cfg-specific `set_private_mode` functions: Unix sets `0600`; non-Unix
  performs no permission change.

Insert these ordinary comments at the corresponding phase boundaries:

```rust
// Empty listener tokens are initialization requests, so generate them before
// validating and remember which values may be printed once by the caller.
```

```rust
// Persist a fully written and synced sibling temporary file so readers never
// observe partially serialized TOML.
```

- [ ] **Step 6: Document secrets and runtime state**

In `secrets.rs`, document:

- `generate_token`: 32 random bytes encoded as unpadded URL-safe Base64.
- `mask_token`: empty remains empty, values shorter than nine Unicode scalar
  values are fully masked, and longer values preserve four scalar values at
  each end.

In `state.rs`, document:

- `StateError` and all variants.
- `RuntimeState` and its private `config` field.
- `new`, `config`, `snapshot_active_upstream`, `replace_upstream`,
  `delete_upstream`, `select_upstream`, and `regenerate_client_token`.
- `snapshot_active_upstream` must explicitly state that it clones the selected
  configuration so an in-flight request is unaffected by later mutations.
- Mutation methods must explain candidate validation and the `null` fallback
  after deleting the active target.

Add this ordinary comment before clone/validate/commit sequences:

```rust
// Validate a complete candidate before replacing live state, leaving the
// current configuration untouched when the mutation is invalid.
```

- [ ] **Step 7: Add the concise crate README**

Create `crates/comux-core/README.md` with this content:

```markdown
# comux-core

`comux-core` contains the configuration schema, management API data types,
token helpers, atomic configuration persistence, and mutable runtime state
shared by `comux-proxy` and the `comux` CLI.

Most users should run the proxy or CLI instead of depending on this crate
directly. See the [main comux documentation](https://github.com/GeminiLab/comux#readme)
for setup, configuration, security notes, and usage.
```

- [ ] **Step 8: Verify and commit `comux-core` documentation**

Run:

```sh
cargo fmt --check
RUSTDOCFLAGS="-D warnings" cargo doc -p comux-core --no-deps --document-private-items
cargo test -p comux-core
cargo clippy -p comux-core --all-targets -- -D warnings
```

Expected: all commands pass with no warnings.

Commit:

```sh
git add crates/comux-core
git commit -m "docs(core): document shared configuration and state"
```

---

### Task 2: Document `comux-proxy`

**Files:**
- Modify: `crates/comux-proxy/src/lib.rs`
- Modify: `crates/comux-proxy/src/main.rs`
- Modify: `crates/comux-proxy/src/args.rs`
- Modify: `crates/comux-proxy/src/admin.rs`
- Modify: `crates/comux-proxy/src/auth.rs`
- Modify: `crates/comux-proxy/src/forward.rs`
- Modify: `crates/comux-proxy/src/server.rs`
- Modify: `crates/comux-proxy/src/stats.rs`
- Create: `crates/comux-proxy/README.md`

**Interfaces:**
- Consumes: documented `comux-core` configuration, state, persistence, and API DTOs.
- Produces: unchanged proxy library/binary interfaces with documented data-plane and management-plane behavior.

- [ ] **Step 1: Add crate and module documentation with missing-docs enforcement**

At the top of `src/lib.rs`, add:

```rust
//! HTTP data plane and management plane for the comux Codex proxy.
//!
//! The data-plane router authenticates Codex with the downstream token and
//! forwards requests to a snapshot of the selected upstream. The management
//! router uses a separate admin token to inspect and mutate persisted state.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
```

Add `//!` overviews to every other production source file. In `main.rs`,
describe configuration initialization and the two concurrently served
listeners. In module files, describe only that module's responsibility.

- [ ] **Step 2: Confirm rustdoc reports current gaps**

Run:

```sh
RUSTDOCFLAGS="-D warnings" cargo doc -p comux-proxy --no-deps --document-private-items
```

Expected: failure containing `missing documentation` for public proxy items.

- [ ] **Step 3: Document startup, arguments, and authentication**

In `main.rs`, document private `main` and `run`. Explain that `run` initializes
logging, loads or initializes secrets, binds both listeners before serving
either, and then serves both until one server exits or fails. Add ordinary
comments before token printing and before `try_join!`:

```rust
// Newly generated listener credentials are shown only on the initialization
// run; subsequent starts load them from the private configuration file.
```

```rust
// Both planes share runtime state but use separate listeners and routers so
// their credentials and exposed routes remain distinct.
```

In `args.rs`, document `ProxyArgs` and its `config` field.

In `auth.rs`, document:

- `AuthError`, `Missing`, and `Invalid`.
- `bearer_token`: accepts a case-insensitive Bearer scheme and exactly one
  non-empty token without spaces.
- `is_valid`: exact token comparison after parsing; do not claim a
  constant-time comparison.

- [ ] **Step 4: Document management routing and transactional mutations**

In `admin.rs`, document all private and public items:

- `AdminApp`, `proxy`, and `config_path`.
- `router` and every route it mounts.
- `authorized`, `status`, `stats`, `list_upstreams`, `put_upstream`,
  `delete_upstream`, `select_active`, `client_token`,
  `regenerate_client_token`, and `api_error`.
- `RevealQuery` and `reveal`, including that only the exact string `full`
  requests an unmasked value after authentication.

For each mutating handler, place an ordinary comment before the candidate
clone/persist/commit sequence:

```rust
// Mutate and validate a candidate, persist it while holding the write lock,
// and publish it only after the durable write succeeds.
```

For list/token reveal branches, add one comment shared by the branch logic:

```rust
// Full credentials require both admin authentication and an explicit
// `reveal=full`; all ordinary responses remain masked.
```

Do not refactor the repeated handler code in this documentation-only task.

- [ ] **Step 5: Document forwarding and redirect trade-offs**

In `forward.rs`, document:

- `ForwardError` and every variant.
- `forward_request`: URL joining, hop-by-hop filtering, downstream
  Authorization replacement, connect timeout, per-body-chunk idle timeout,
  and redirect-policy differences.
- `build_request`: copies end-to-end headers while replacing Authorization and
  omitting hop-by-hop headers.
- `is_hop_by_hop`: identifies headers that must not be forwarded by this
  proxy.
- `join_upstream_url`: appends the downstream path to the configured base path
  and replaces the query with the downstream query.

Add ordinary comments around the `Always` branch:

```rust
// `always` must replay bodies for 307/308 redirects, so record chunks while
// streaming the first request instead of buffering before the first hop.
```

```rust
// Disable reqwest redirect handling for this branch so comux can deliberately
// re-inject the upstream credential on every hop, including cross-host hops.
```

```rust
// RFC-style 307/308 redirects preserve method and body; other redirects
// convert non-GET/HEAD requests to GET and discard the body.
```

Before constructing the downstream response stream, add:

```rust
// The idle timeout applies between body chunks rather than to total response
// duration, allowing long-lived streaming responses that remain active.
```

- [ ] **Step 6: Document serving and streaming statistics**

In `server.rs`, document:

- `ProxyApp`, `state`, and `stats`.
- `new`, `router`, and `admin_router`.
- `handle_downstream`: auth, selected-upstream snapshot, forwarding, streamed
  byte accounting, structured 401/503/502 responses, and request snapshot
  semantics.
- `json_error`: creates the common JSON error envelope.

Add ordinary comments:

```rust
// Clone the routing decision and connection settings under a short read lock;
// later management changes affect new requests but not this in-flight request.
```

```rust
// Keep the completion guard inside the downstream body stream so duration and
// status are finalized only when the response is fully consumed.
```

In `stats.rs`, document:

- `StatsRegistry` and private `inner`.
- `begin`, `snapshot`, both byte-recording methods, and `with_stats`.
- `RequestStatsGuard` and its three private fields.
- `add_request_bytes`, `add_response_bytes`, `finish_response`, and
  `network_error`.
- `Drop::drop`: releases `active_streams` without inventing a completed
  response or network failure when a stream is simply abandoned.

Add an ordinary comment in `finish_response` that the `released` flag makes
terminal accounting idempotent.

- [ ] **Step 7: Add the concise proxy README**

Create `crates/comux-proxy/README.md`:

````markdown
# comux-proxy

`comux-proxy` is the comux HTTP proxy. It exposes a downstream endpoint for
Codex and a separately authenticated management endpoint for the `comux` CLI.
The library modules are also used by the integration tests.

Run it with an explicit configuration path:

```sh
cargo run -p comux-proxy -- --config /path/to/comux.toml
```

See the [main comux documentation](https://github.com/GeminiLab/comux#readme)
for the complete configuration format, Codex setup, API reference, redirect
security notes, and troubleshooting.
````

- [ ] **Step 8: Verify and commit proxy documentation**

Run:

```sh
cargo fmt --check
RUSTDOCFLAGS="-D warnings" cargo doc -p comux-proxy --no-deps --document-private-items
cargo test -p comux-proxy
cargo clippy -p comux-proxy --all-targets -- -D warnings
```

Expected: all commands pass with no warnings.

Commit:

```sh
git add crates/comux-proxy
git commit -m "docs(proxy): document request and management flows"
```

---

### Task 3: Document the `comux` CLI and TUI

**Files:**
- Modify: `crates/comux-cli/src/lib.rs`
- Modify: `crates/comux-cli/src/main.rs`
- Modify: `crates/comux-cli/src/args.rs`
- Modify: `crates/comux-cli/src/client.rs`
- Modify: `crates/comux-cli/src/commands.rs`
- Modify: `crates/comux-cli/src/config.rs`
- Modify: `crates/comux-cli/src/tui/mod.rs`
- Modify: `crates/comux-cli/src/tui/app.rs`
- Modify: `crates/comux-cli/src/tui/events.rs`
- Modify: `crates/comux-cli/src/tui/ui.rs`
- Create: `crates/comux-cli/README.md`

**Interfaces:**
- Consumes: documented management API DTOs and the proxy's authenticated admin routes.
- Produces: unchanged CLI/TUI interfaces with complete argument, client, configuration, and UI documentation.

- [ ] **Step 1: Add crate/module overviews and missing-docs enforcement**

At the top of `src/lib.rs`, add:

```rust
//! Command-line and terminal user interfaces for managing a comux proxy.
//!
//! The CLI resolves local connection settings, calls the authenticated
//! management API, and opens a ratatui dashboard when no subcommand is given.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
```

Add focused `//!` comments to `args.rs`, `client.rs`, `commands.rs`,
`config.rs`, `tui/mod.rs`, `tui/app.rs`, `tui/events.rs`, and `tui/ui.rs`.
Add a binary-crate `//!` overview to `main.rs`.

- [ ] **Step 2: Confirm rustdoc reports current public gaps**

Run:

```sh
RUSTDOCFLAGS="-D warnings" cargo doc -p comux --no-deps --document-private-items
```

Expected: failure containing `missing documentation` for public CLI/TUI items.

- [ ] **Step 3: Document command-line arguments and startup**

In `main.rs`, document private `main`, including runtime creation, async
dispatch, stderr reporting, and non-zero exit on failure.

In `args.rs`, document every type, field, and variant:

- Global `CliArgs` connection overrides, dangerous full-token flag, and
  optional command/TUI fallback.
- `Command::{Status, Upstream, Config, Auth}` and nested action fields.
- `ConfigCommand` and its optional action, where no action displays resolved
  values.
- Every `UpstreamAction` variant and payload.
- `UpstreamOptions`, `UpstreamOptionsWithName`, and `UpstreamFields`, including
  prompt behavior for omitted tokens.
- `ConfigAction::{Set, Get}`, `ConfigKey::{Endpoint, AdminToken}`, and
  `AuthAction::{Print, Regenerate}`.
- `_path_type`: dead-code-only compile-time retention of the `PathBuf` import;
  do not remove it in this task.

- [ ] **Step 4: Document the management API client**

In `client.rs`, document:

- `ClientError` and all variants/fields.
- `ApiClient` and private `endpoint`, `admin_token`, and `http` fields.
- `new` and every public endpoint method.
- Private `get_json`, `send_empty`, `send_json`, `request`, and
  `parse_server_error`.

State that `reveal=true` appends `reveal=full`, mutation methods expect success
without a response body, and server-error parsing falls back to a generic
message when the body is not the standard JSON envelope.

Add one ordinary comment in `request`:

```rust
// Join a relative API path to the configured endpoint so deployments may use
// an endpoint with an origin while all methods share authentication handling.
```

- [ ] **Step 5: Document command dispatch and sensitive output choices**

In `commands.rs`, document every function:

- `run`, `client_from_args`, `run_config`, `get_local_value`,
  `prompt_endpoint_value`, `run_upstream`, `run_auth`,
  `parse_redirect_policy`, `display_token`, `prompt_value`,
  `config_key_name`, and `confirm`.

Add ordinary comments at these decisions:

```rust
// Configuration commands operate locally and therefore do not require a
// reachable management endpoint; all other subcommands resolve a client.
```

```rust
// Updating an upstream without a replacement token explicitly fetches the
// full existing credential so PUT can send a complete replacement object.
```

```rust
// Full token output requires the global dangerous flag; ordinary output masks
// credentials even when an API response happens to contain a full value.
```

- [ ] **Step 6: Document local configuration resolution**

In `config.rs`, document:

- `default_config_path`: `$HOME/.config/comux.toml`, with `.` fallback when a
  home directory cannot be determined.
- `resolve_config`: explicit overrides win over file values and all required
  fields must be present.
- `resolve_config_interactive`: override, then non-empty file value, then
  prompt; only a fully prompted no-override resolution is persisted.
- `save_value`: update one local key, validate the complete result, and save.
- `non_empty` and `prompt_endpoint`.

Add this ordinary comment before endpoint/token resolution:

```rust
// Resolve each value independently in precedence order: command-line
// override, non-empty file value, then an interactive prompt.
```

- [ ] **Step 7: Document TUI lifecycle, state, events, and rendering**

In `tui/mod.rs`, document:

- The `app`, `events`, and `ui` modules.
- `run`: initial refresh, raw/alternate-screen setup, cleanup, and propagated
  errors.
- `run_loop`: redraws, handles keyboard input, and refreshes every two seconds.

Add an ordinary comment around terminal cleanup stating that cleanup runs
after the loop result is captured so terminal state is restored before the
error is returned.

In `tui/app.rs`, document:

- `App` and every public/private field.
- `AddField` and all variants.
- `AddUpstreamForm` and every field.
- `Default::default`.
- Every method in `impl App`, including private helpers.
- `selected_index_for_active`, `current_value_mut`, `next_field`,
  `cycle_policy`, and `validate_field`.
- The test-only `Received` alias is exempt because it is inside the test
  module.

Add ordinary comments:

```rust
// Freeze background refresh while editing so network results cannot replace
// state that the modal is using between keystrokes.
```

```rust
// Align selection with the active upstream only on the first successful
// refresh; later refreshes preserve the user's navigation position.
```

```rust
// Take the form out of `self` while awaiting submission, then restore it only
// when editing or validation must continue.
```

In `tui/events.rs`, document `Action`, every variant, and `action`.

In `tui/ui.rs`, document `render`, every private render helper, field/policy
label helpers, and `format_bytes`. Document `UNITS` as a local binary-unit
lookup and explain that values use one decimal only when needed.

- [ ] **Step 8: Add the concise CLI README**

Create `crates/comux-cli/README.md`:

````markdown
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
````

- [ ] **Step 9: Verify and commit CLI/TUI documentation**

Run:

```sh
cargo fmt --check
RUSTDOCFLAGS="-D warnings" cargo doc -p comux --no-deps --document-private-items
cargo test -p comux
cargo clippy -p comux --all-targets -- -D warnings
```

Expected: all commands pass with no warnings.

Commit:

```sh
git add crates/comux-cli
git commit -m "docs(cli): document commands and terminal interface"
```

---

### Task 4: Replace the Root README with the Canonical Project Guide

**Files:**
- Modify: `README.md`
- Reference without modification: `examples/comux.proxy.toml`
- Reference without modification: `crates/comux-core/src/config.rs`
- Reference without modification: `crates/comux-proxy/src/admin.rs`
- Reference without modification: `crates/comux-cli/src/args.rs`

**Interfaces:**
- Consumes: the current configuration schema, routes, commands, and behavior documented in Tasks 1-3.
- Produces: one canonical user/contributor guide linked from all crate READMEs.

- [ ] **Step 1: Write the project overview, AI disclosure, and architecture**

Start `README.md` with:

```markdown
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
```

Follow it with a workspace section describing `comux-core`, `comux-proxy`, and
`comux`, plus this text request flow:

```text
Codex -- downstream token --> comux-proxy :6608 -- upstream token --> selected API
comux CLI/TUI -- admin token --> comux-proxy :6609 --> state + persisted config
```

State that in-flight requests retain the upstream snapshot captured at
acceptance and that statistics are process-local.

- [ ] **Step 2: Write prerequisites, build, install, and quick start**

Document:

- A stable Rust toolchain with Cargo.
- `cargo build --workspace`.
- `cargo install --path crates/comux-proxy` and
  `cargo install --path crates/comux-cli`.
- `cargo run -p comux-proxy -- --config /path/to/comux.toml`, explicitly
  explaining Cargo's required `--`.
- Direct binary startup without the separator.
- First-start listener token generation, atomic persistence, and one-time
  display.
- Loopback defaults and the risk of non-loopback binds.
- Link to `examples/comux.proxy.toml`.

- [ ] **Step 3: Add the complete proxy configuration reference**

Include a sanitized complete TOML example with:

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

Explain every field and validation rule exactly:

- `active_upstream`: a configured name or reserved `null`.
- `bind`: defaults to `localhost`.
- Admin/downstream ports: defaults 6609/6608 and must be non-zero.
- Empty/missing listener token: generated and persisted at proxy startup.
- `connect_timeout_secs`: default 10, positive, connection establishment only.
- `idle_timeout_secs`: default 120, positive, request/response body chunk idle
  timeout rather than total request lifetime.
- Upstream names: non-empty and cannot equal `null`.
- `base_url`: valid HTTP(S), with downstream paths appended after its base
  path.
- `auth_token`: required and sensitive.
- Redirect policies: precise `always`, `default`, and `never` behavior,
  including cross-host credential propagation for `always`.

- [ ] **Step 4: Document Codex, CLI, commands, and TUI**

Describe how to point Codex at downstream port 6608 and use the downstream
token without prescribing a version-specific Codex file path.

Document local CLI setup:

```sh
# Prompt for both values and persist the completed configuration.
comux config
comux status
```

Also document the non-interactive two-command order required by the current
complete-config validation:

```sh
comux config set admin-token
comux config set endpoint http://127.0.0.1:6609
comux status
```

State that global `--endpoint` and `--admin-token` override the local file and
that missing values prompt only on an interactive terminal. Explain that on a
new file `admin-token` must be stored before `endpoint` when using separate
`config set` commands, because each completed save is fully validated.

List every command represented in `args.rs`:

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

Explain `--dangerously-print-full-token`, token prompts, regeneration
confirmation, no-subcommand TUI startup, and every TUI key including add-form
controls.

- [ ] **Step 5: Document the Admin API and errors**

State that all routes require `Authorization: Bearer <admin.token>`. Document
method, path, purpose, request body, success body/status, and reveal behavior
for:

- `GET /api/v1/status`
- `GET /api/v1/stats`
- `GET /api/v1/upstreams`
- `PUT /api/v1/upstreams/{name}`
- `DELETE /api/v1/upstreams/{name}`
- `POST /api/v1/active`
- `GET /api/v1/auth/client-token`
- `POST /api/v1/auth/client-token/regenerate`

Include these request payload examples:

```json
{"base_url":"https://api.example.com/v1","auth_token":"secret","redirect_policy":"default"}
```

```json
{"name":"openai"}
```

Include the common error envelope:

```json
{"error":{"type":"invalid_request","message":"..."}}
```

Document the implemented categories and common statuses:
`unauthorized`/401, `invalid_request`/400, `not_found`/404,
`persistence_failed`/500, `no_active_upstream`/503, and
`upstream_error`/502. Do not call this list a versioned stability guarantee.

- [ ] **Step 6: Add security, troubleshooting, development, and license**

The security section must distinguish admin, downstream, and upstream tokens;
recommend loopback listeners; explain `0600` on Unix and the lack of an
equivalent permission change on non-Unix; warn about `reveal=full`, dangerous
CLI output, configuration backups, and `always` redirects.

Troubleshooting must cover:

- Missing Cargo `--` before proxy arguments.
- 401 on the wrong listener/token.
- 503 when `active_upstream = "null"` or the active target was deleted.
- 502 for upstream URL/connectivity/redirect/idle failures.
- Non-interactive CLI failure when configuration is incomplete.
- Listener bind failures and occupied ports.

Development must include:

```sh
cargo fmt --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --document-private-items
```

End with the MIT license and links to each crate README.

- [ ] **Step 7: Cross-check and commit the root README**

Manually compare the configuration section with
`crates/comux-core/src/config.rs`, routes with
`crates/comux-proxy/src/admin.rs`, and commands with
`crates/comux-cli/src/args.rs`.

Check that no literal from `.local/config.toml` appears:

```sh
git diff -- README.md crates/*/README.md
```

Expected: only sanitized placeholders such as `secret`,
`replace-with-your-upstream-token`, or `<token>`.

Commit:

```sh
git add README.md
git commit -m "docs: expand the canonical project guide"
```

---

### Task 5: Audit Coverage and Run Workspace Verification

**Files:**
- Review: `README.md`
- Review: `crates/comux-core/README.md`
- Review: `crates/comux-proxy/README.md`
- Review: `crates/comux-cli/README.md`
- Review: `crates/comux-core/src/*.rs`
- Review: `crates/comux-proxy/src/*.rs`
- Review: `crates/comux-cli/src/*.rs`

**Interfaces:**
- Consumes: all documentation changes from Tasks 1-4.
- Produces: a clean, behavior-preserving, fully verified documentation change set.

- [ ] **Step 1: Audit every production source file**

Use the following source inventory and check off every item:

```text
crates/comux-core/src/{lib,api,config,error,persistence,secrets,state}.rs
crates/comux-proxy/src/{lib,main,args,admin,auth,forward,server,stats}.rs
crates/comux-cli/src/{lib,main,args,client,commands,config}.rs
crates/comux-cli/src/tui/{mod,app,events,ui}.rs
```

For each file verify:

1. The enclosing crate or module has `//!` documentation.
2. Every production type, field, variant, constant, function, and method has
   `///` documentation, regardless of visibility.
3. Test functions remain free of boilerplate item-by-item Rustdoc.
4. Ordinary comments explain only important phases and design choices.
5. No comment describes unimplemented behavior.

- [ ] **Step 2: Format and inspect the final diff**

Run:

```sh
cargo fmt
git diff --check
git status --short
git diff --stat
```

Expected: no whitespace errors; only planned documentation/source files are
modified, plus no unrelated uncommitted files.

- [ ] **Step 3: Run complete workspace verification**

Run:

```sh
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --document-private-items
cargo fmt --check
```

Expected: every command exits successfully with no warnings and all existing
tests pass.

- [ ] **Step 4: Perform final documentation consistency checks**

Verify:

- All three crate READMEs link to
  `https://github.com/GeminiLab/comux#readme`.
- Root README links to all three crate READMEs.
- README defaults match `ProxyConfig::default`.
- README validation rules match `ProxyConfig::validate` and
  `CliConfig::{validate,validate_partial}`.
- README routes match `admin::router`.
- README commands and flags match clap declarations.
- AI disclosure is present and neutrally worded.
- No documentation claims graceful shutdown or other unimplemented behavior.
- No local token or machine-specific `.local` path/value was copied.

- [ ] **Step 5: Commit any audit-only corrections**

If the audit required corrections, commit only those corrections:

```sh
git add README.md crates/comux-core crates/comux-proxy crates/comux-cli
git commit -m "docs: complete documentation coverage audit"
```

If no files changed after verification, do not create an empty commit.
