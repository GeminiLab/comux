# comux Proxy and Selector Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the `comux` Rust workspace containing a token-authenticated Codex HTTP proxy, an authenticated management API, a command-line client, and an auto-refreshing ratatui TUI.

**Architecture:** A Cargo workspace contains `comux-core`, `comux-proxy`, and `comux-cli`. `comux-core` owns TOML schemas, validation, masking, API DTOs, and atomic persistence; `comux-proxy` owns two listeners, streaming forwarding, mutable runtime state, and in-memory statistics; `comux-cli` owns commands, local CLI configuration, the HTTP API client, prompts, and the no-subcommand TUI.

**Tech Stack:** Rust 2021, Tokio, Axum, Hyper/HTTP body streaming, Reqwest, Serde/TOML/JSON, Clap, Ratatui with Crossterm, `rpassword`, `tracing`, `thiserror`, `tempfile`, and local mock servers for integration tests. Do not add `veil` or token-usage parsing in the first version.

---

### Task 1: Create the Cargo workspace and crate skeleton

**Files:**
- Create: `Cargo.toml`
- Create: `crates/comux-core/Cargo.toml`
- Create: `crates/comux-core/src/lib.rs`
- Create: `crates/comux-proxy/Cargo.toml`
- Create: `crates/comux-proxy/src/main.rs`
- Create: `crates/comux-cli/Cargo.toml`
- Create: `crates/comux-cli/src/main.rs`
- Test: `crates/comux-core/src/lib.rs`

- [ ] **Step 1: Write the workspace manifests**

  Declare the three members and use package names `comux-core`, `comux-proxy`, and `comux-cli`. Add only the dependencies needed by each crate: serde/toml/url/thiserror/tempfile to core; tokio/axum/reqwest/tracing to proxy; clap/reqwest/serde_json/rpassword/ratatui to CLI. Enable Reqwest streaming and rustls features without native OpenSSL.

- [ ] **Step 2: Add smoke tests before implementation**

  Put a `#[test] fn workspace_crates_compile()` smoke test in `comux-core/src/lib.rs`, and keep both binary `main` functions returning a clear “not initialized” error until the proxy and CLI entry points are implemented in Tasks 5 and 7.

- [ ] **Step 3: Verify the empty workspace**

  Run `cargo test --workspace`. Expected: all crates compile and the core smoke test passes.

- [ ] **Step 4: Commit the workspace bootstrap**

  Run `git add Cargo.toml crates && git commit -m "build: create comux workspace"`.

### Task 2: Implement shared proxy and CLI configuration types

**Files:**
- Create: `crates/comux-core/src/config.rs`
- Create: `crates/comux-core/src/error.rs`
- Modify: `crates/comux-core/src/lib.rs`
- Test: `crates/comux-core/src/config.rs`

- [ ] **Step 1: Write schema/default tests**

  Test TOML parsing for this exact shape:

  ```toml
  active_upstream = "null"

  [admin]
  bind = "127.0.0.1"
  port = 6609
  token = "admin-secret"

  [downstream]
  bind = "127.0.0.1"
  port = 6608
  token = "client-secret"

  [connection]
  connect_timeout_secs = 10
  idle_timeout_secs = 120
  ```

  Assert defaults for omitted `active_upstream`, bind addresses, ports, and timeout values. Test `RedirectPolicy::{Always, Default, Never}` serialization as lowercase strings and reject invalid policies, zero ports, zero timeouts, non-HTTP(S) base URLs, and an active upstream name absent from the map unless it is `null`.

- [ ] **Step 2: Define the shared types**

  Add `ProxyConfig`, `ListenerConfig`, `ConnectionConfig`, `UpstreamConfig`, `RedirectPolicy`, and `CliConfig` with Serde derives. Keep proxy config tokens under `[admin].token` and `[downstream].token`; keep CLI config keys `admin_endpoint` and `admin_token`. Use `BTreeMap<String, UpstreamConfig>` for stable output.

- [ ] **Step 3: Implement validation**

  Expose `ProxyConfig::validate(&self) -> Result<(), ConfigError>` and `CliConfig::validate(&self) -> Result<(), ConfigError>`. Parse `base_url` with `url::Url`, require `http` or `https`, require non-empty upstream names/tokens, and reject the reserved upstream name `null` in the configured map.

- [ ] **Step 4: Run focused tests**

  Run `cargo test -p comux-core config`. Expected: parsing/default/validation tests pass.

- [ ] **Step 5: Commit schema support**

  Run `git add crates/comux-core && git commit -m "feat(core): add comux configuration schemas"`.

### Task 3: Add masking, token generation, and atomic persistence

**Files:**
- Create: `crates/comux-core/src/secrets.rs`
- Create: `crates/comux-core/src/persistence.rs`
- Modify: `crates/comux-core/src/lib.rs`
- Test: `crates/comux-core/src/secrets.rs`
- Test: `crates/comux-core/src/persistence.rs`

- [ ] **Step 1: Write masking tests**

  Test `mask_token` preserves exactly the first four and last four characters and replaces the middle with `*`; test lengths below nine return all `*`; test empty input returns an empty string. Add token-generation tests that generate 32 random bytes encoded with URL-safe base64 and never return the same token twice in repeated calls.

- [ ] **Step 2: Implement the shared secret helpers**

  Expose `generate_token() -> String` and `mask_token(value: &str) -> String`. Do not use `veil` for this contract; use a small explicit function so JSON, CLI, TUI, and logs share identical behavior.

- [ ] **Step 3: Write persistence tests**

  With `tempfile::tempdir`, test loading a missing proxy file creates it with generated `[admin].token` and `[downstream].token`, loading an existing file does not replace or print tokens, and saving writes valid TOML after a simulated replacement. Test CLI config atomic writes separately.

- [ ] **Step 4: Implement atomic file operations**

  Add `load_proxy_config_or_initialize(path)` and `save_proxy_config(path, &ProxyConfig)`, plus `load_cli_config(path)` and `save_cli_config(path, &CliConfig)`. Write a sibling temporary file, serialize, flush and `sync_all`, rename over the destination, and set Unix mode `0600` on newly created files. Return errors that include the path and operation but never include token values.

- [ ] **Step 5: Run persistence tests**

  Run `cargo test -p comux-core secrets persistence`. Expected: all masking, generation, initialization, and atomic-write tests pass.

- [ ] **Step 6: Commit persistence**

  Run `git add crates/comux-core && git commit -m "feat(core): add token masking and atomic config writes"`.

### Task 4: Define management API DTOs and runtime state

**Files:**
- Create: `crates/comux-core/src/api.rs`
- Create: `crates/comux-core/src/state.rs`
- Modify: `crates/comux-core/src/lib.rs`
- Test: `crates/comux-core/src/state.rs`

- [ ] **Step 1: Write state transition tests**

  Test selecting an existing upstream, rejecting an unknown name, deleting the active upstream and persisting `null`, deleting a non-active upstream, and applying an upstream replacement without changing `active_upstream`. Test that a request snapshot contains an owned upstream value so later selection changes do not mutate it.

- [ ] **Step 2: Implement API DTOs**

  Define JSON types for status, upstream summaries, stats, mutation requests, client-token reveal, and structured errors. Summary types must contain masked tokens only. Define stable error types `no_active_upstream`, `unauthorized`, `invalid_request`, `not_found`, `conflict`, and `persistence_failed`.

- [ ] **Step 3: Implement mutable state**

  Add `RuntimeState` containing the validated `ProxyConfig`, a `StatsRegistry`, and a mutation mutex. Expose methods `snapshot_active_upstream`, `replace_upstream`, `delete_upstream`, `select_upstream`, and `regenerate_client_token`; each mutation returns the prospective config for persistence before the caller swaps it into memory.

- [ ] **Step 4: Run focused tests**

  Run `cargo test -p comux-core state`. Expected: every selection and deletion transition passes.

- [ ] **Step 5: Commit API/state types**

  Run `git add crates/comux-core && git commit -m "feat(core): define management API and runtime state"`.

### Task 5: Build the proxy listener and transparent streaming forwarder

**Files:**
- Create: `crates/comux-proxy/src/args.rs`
- Create: `crates/comux-proxy/src/auth.rs`
- Create: `crates/comux-proxy/src/forward.rs`
- Create: `crates/comux-proxy/src/server.rs`
- Modify: `crates/comux-proxy/src/main.rs`
- Test: `crates/comux-proxy/tests/proxy_forwarding.rs`

- [ ] **Step 1: Write mock-upstream forwarding tests**

  Start an Axum mock upstream on an ephemeral port. Assert missing/wrong downstream Bearer tokens return `401`; a valid request replaces `Authorization` with the upstream token; method/path/query/body and safe headers arrive unchanged; upstream status/headers/body stream back; and `active_upstream = null` returns the exact `503` JSON contract.

- [ ] **Step 2: Implement proxy CLI arguments and listener binding**

  Parse required `--config PATH`, load/initialize the TOML, bind the downstream listener at `[downstream].bind:[downstream].port` (defaults 127.0.0.1/6608) and admin listener at `[admin].bind:[admin].port` (defaults 127.0.0.1/6609). On bind failure exit with the listener name and resolved address.

- [ ] **Step 3: Implement downstream authentication**

  Extract exactly `Authorization: Bearer <token>`, compare with the current downstream token without logging either value, and return structured `401` JSON before forwarding. Re-read the state snapshot per request so token regeneration affects new requests immediately.

- [ ] **Step 4: Implement URL joining and request construction**

  Resolve the incoming path/query against the configured base URL while retaining its path prefix. Build a Reqwest request with the original method and streaming body, remove downstream authorization and hop-by-hop headers, set the upstream Bearer header, and let the HTTP stack rebuild `Host` and content framing.

- [ ] **Step 5: Stream responses and enforce timeouts**

  Convert the incoming HTTP body to a stream, pass it to Reqwest, and convert `bytes_stream()` back to the Axum response body. Apply the configured connect timeout and an idle byte-transfer watchdog without a total response deadline or body-size cap. Abort the upstream request when the downstream disconnects.

- [ ] **Step 6: Implement redirect policies**

  Configure `never` to return 3xx responses, `default` to use Reqwest's default policy, and `always` to follow up to ten hops while ensuring the configured upstream Authorization header is present on every followed request, including cross-authority redirects. Add tests for all three policies and the cross-host token behavior.

- [ ] **Step 7: Run proxy tests**

  Run `cargo test -p comux-proxy --test proxy_forwarding`. Expected: authentication, URL, streaming, null-upstream, timeout, and redirect tests pass without contacting an external service.

- [ ] **Step 8: Commit the data plane**

  Run `git add crates/comux-proxy && git commit -m "feat(proxy): add authenticated streaming data plane"`.

### Task 6: Add statistics and the authenticated management server

**Files:**
- Create: `crates/comux-proxy/src/stats.rs`
- Create: `crates/comux-proxy/src/admin.rs`
- Modify: `crates/comux-proxy/src/server.rs`
- Modify: `crates/comux-proxy/src/main.rs`
- Test: `crates/comux-proxy/tests/admin_api.rs`
- Test: `crates/comux-proxy/tests/stats.rs`

- [ ] **Step 1: Write stats tests**

  Assert per-upstream counters for accepted requests, 2xx/3xx/4xx/5xx responses, network errors, request/response bytes, duration count/total/max, and active streams. Verify active streams decrement on disconnect and all counters reset when a new process starts.

- [ ] **Step 2: Implement lock-free request accounting**

  Use an `Arc` registry keyed by upstream name with atomics for counters and a small mutex only for duration aggregates. Start a request guard after selecting an upstream, add byte counts as chunks flow, classify the final outcome, and decrement active streams in `Drop`.

- [ ] **Step 3: Write admin authentication and route tests**

  Test that every `/api/v1` route rejects missing/wrong admin Bearer tokens, status/upstream/stats responses mask tokens, and the reveal request is the only successful path returning the full client token.

- [ ] **Step 4: Implement admin routes**

  Add `GET /api/v1/status`, `GET /api/v1/stats`, `GET/PUT/DELETE /api/v1/upstreams/{name}`, `POST /api/v1/active`, `GET /api/v1/auth/client-token`, and `POST /api/v1/auth/client-token/regenerate`. Validate a prospective config, persist it atomically, then replace runtime state; leave memory unchanged on persistence failure.

- [ ] **Step 5: Add structured error mapping**

  Map authentication to 401, malformed payloads to 400, missing names to 404, conflicts to 409, persistence errors to 500, and null-upstream forwarding to 503. Ensure error serialization never includes credentials.

- [ ] **Step 6: Run management tests**

  Run `cargo test -p comux-proxy --test admin_api --test stats`. Expected: all auth, CRUD, persistence-ordering, reveal, regeneration, and metrics tests pass.

- [ ] **Step 7: Commit the control plane and metrics**

  Run `git add crates/comux-proxy && git commit -m "feat(proxy): add admin API and usage statistics"`.

### Task 7: Implement the non-interactive CLI and local configuration

**Files:**
- Create: `crates/comux-cli/src/args.rs`
- Create: `crates/comux-cli/src/config.rs`
- Create: `crates/comux-cli/src/client.rs`
- Create: `crates/comux-cli/src/commands.rs`
- Create: `crates/comux-cli/src/output.rs`
- Modify: `crates/comux-cli/src/main.rs`
- Test: `crates/comux-cli/tests/commands.rs`
- Test: `crates/comux-cli/tests/config.rs`

- [ ] **Step 1: Write command parsing and masking tests**

  Test global `--endpoint`, `--admin-token`, and `--dangerously-print-full-token`; `status`, `upstream list/add/set/remove/select`, `config`, `config get/set`, `auth print`, and `auth regenerate`; default masking; and full output only when the dangerous flag is present.

- [ ] **Step 2: Implement Clap command types**

  Model the command tree with no subcommand as `Tui`, and explicit subcommands for status, upstream, config, and auth. Keep the dangerous flag global and reject full-token output unless that flag is set.

- [ ] **Step 3: Implement CLI config resolution**

  Resolve `~/.config/comux.toml`, load endpoint/admin token, apply one-command flag overrides, prompt for missing values only on an interactive terminal, and save `config set` atomically. Use hidden input for admin-token values and visible input for endpoint values.

- [ ] **Step 4: Implement the management client**

  Add a Reqwest client that sends `Authorization: Bearer <admin_token>` to every admin request, requests reveal only when explicitly asked, decodes the shared DTOs, and turns structured server errors into stable CLI exit codes.

- [ ] **Step 5: Implement command output and mutations**

  Render status, upstreams, stats, config, and auth output with the shared mask helper. Send CRUD/select/regenerate requests; prompt for hidden upstream tokens and confirmation before client-token regeneration.

- [ ] **Step 6: Run CLI tests**

  Run `cargo test -p comux-cli --test commands --test config`. Expected: command parsing, local config, masking, reveal, prompts, and API-client tests pass.

- [ ] **Step 7: Commit the scriptable CLI**

  Run `git add crates/comux-cli && git commit -m "feat(cli): add comux management commands"`.

### Task 8: Add the auto-refreshing ratatui TUI

**Files:**
- Create: `crates/comux-cli/src/tui/mod.rs`
- Create: `crates/comux-cli/src/tui/app.rs`
- Create: `crates/comux-cli/src/tui/ui.rs`
- Create: `crates/comux-cli/src/tui/events.rs`
- Modify: `crates/comux-cli/src/main.rs`
- Test: `crates/comux-cli/tests/tui.rs`

- [ ] **Step 1: Write headless TUI state tests**

  With Ratatui's `TestBackend`, test rendering of active upstream, masked tokens, stats, empty/null upstream state, and error banners. Test keyboard actions map to list selection, add/edit/delete/select, auth print/regenerate, config endpoint/token editing, refresh, and quit.

- [ ] **Step 2: Implement the TUI application state**

  Define `AppState` with current status/upstreams/stats, selected row, active modal, input buffer, pending request, error message, and last refresh time. Keep all HTTP calls in the CLI client layer and communicate results back to the UI state.

- [ ] **Step 3: Implement the two-second refresh loop**

  Run a Tokio event loop with a two-second interval, terminal key events through Crossterm, and a cancellation channel. Refresh status/upstreams/stats at startup, every interval, and immediately after each successful mutation; never block drawing while waiting for the network.

- [ ] **Step 4: Implement views and forms**

  Render a status/stats dashboard and upstream table, with modal forms for upstream fields, hidden token input, active selection, client-token regeneration confirmation, and local CLI endpoint/admin-token settings. Keep all token output masked unless the global dangerous flag explicitly enables a reveal action.

- [ ] **Step 5: Restore the terminal on all exits**

  Use Ratatui's Crossterm initialization/restoration helpers and a panic hook so raw mode and alternate-screen state are restored on errors, Ctrl-C, normal quit, and failed HTTP operations.

- [ ] **Step 6: Run TUI tests**

  Run `cargo test -p comux-cli --test tui`. Expected: headless rendering and action tests pass without requiring an interactive terminal.

- [ ] **Step 7: Commit the TUI**

  Run `git add crates/comux-cli && git commit -m "feat(cli): add auto-refreshing ratatui interface"`.

### Task 9: Complete cross-platform integration tests and documentation

**Files:**
- Create: `crates/comux-proxy/tests/end_to_end.rs`
- Modify: `crates/comux-core/src/config.rs`
- Modify: `README.md`
- Create: `docs/configuration.md`
- Create: `docs/management-api.md`

- [ ] **Step 1: Write an end-to-end local test**

  Start a mock upstream, proxy with a temporary TOML file and ephemeral listener ports, then use the CLI client to add/select an upstream, send an authenticated streaming request, switch upstream while a stream is active, query stats, regenerate the client token, and verify the old token fails.

- [ ] **Step 2: Add platform path and permission tests**

  Verify Linux/macOS CLI path resolution is `~/.config/comux.toml`, Unix config files are mode `0600`, loopback defaults never use wildcard binds, and all configured addresses/ports can be overridden.

- [ ] **Step 3: Document operation and security**

  Document the proxy sample TOML, default ports 6608/6609, client/admin token distinction, HTTP-only remote deployment requirement, redirect policy risk, dangerous token flag, TUI controls, and in-memory statistics reset behavior.

- [ ] **Step 4: Run the full verification suite**

  Run:

  ```bash
  cargo fmt --all -- --check
  cargo test --workspace
  cargo clippy --workspace --all-targets -- -D warnings
  ```

  Expected: formatting is clean, all unit/integration/TUI tests pass, and Clippy reports no warnings.

- [ ] **Step 5: Commit the completed feature**

  Run `git add Cargo.toml crates README.md docs && git commit -m "feat: add comux codex proxy and selector"`.

## Spec Coverage Checklist

- Proxy/admin split listeners, loopback defaults, and 6608/6609 ports: Tasks 2, 5, and 9.
- Generated proxy tokens, separate admin/client authentication, masking, reveal, and regeneration: Tasks 3, 6, and 7.
- TOML config grouping and atomic writes: Tasks 2 and 3.
- Upstream CRUD, null selection, active request snapshots: Tasks 4, 6, and 7.
- Transparent all-path HTTP forwarding, auth replacement, SSE, URL joining, timeouts, and redirects: Task 5.
- In-memory transport statistics and `/api/v1/stats`: Task 6.
- CLI commands, local config, dangerous flag, and hidden prompts: Task 7.
- No-subcommand auto-refreshing TUI: Task 8.
- Linux/macOS behavior, privacy, docs, and full verification: Task 9.
