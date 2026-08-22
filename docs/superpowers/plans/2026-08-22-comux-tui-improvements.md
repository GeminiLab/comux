# comux TUI Improvements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Improve the ratatui dashboard layout and add an interactive upstream creation form.

**Architecture:** Keep the existing `App` state machine and `ApiClient`, adding a modal form mode to `App` and key events. Render the dashboard vertically, format byte counts in a focused helper, and render a two-line footer plus a modal form when adding an upstream.

**Tech Stack:** Rust, ratatui 0.30, crossterm, tokio, existing comux admin API.

---

### Task 1: Lock down formatting and dashboard layout

**Files:**
- Modify: `crates/comux-cli/src/tui/ui.rs`
- Test: `crates/comux-cli/src/tui/ui.rs` unit tests

- [x] Add tests for binary byte formatting at B/KiB/MiB/GiB/TiB boundaries and for the rendered dashboard containing `Upstreams`, `Usage`, no inner `UPSTREAMS`/`STATS`, and the two footer lines.
- [x] Run `cargo test -p comux --lib tui` and verify the new assertions fail against the current horizontal layout.
- [x] Implement `format_bytes`, vertical body constraints, title-only panels, adaptive stats output, and a two-row footer.
- [x] Re-run the focused tests, then `cargo fmt --all`.

### Task 2: Add upstream form state and keyboard events

**Files:**
- Modify: `crates/comux-cli/src/tui/events.rs`
- Modify: `crates/comux-cli/src/tui/app.rs`
- Test: `crates/comux-cli/src/tui/app.rs` and `events.rs` unit tests

- [x] Add an `Action::Add` mapping for `a`, form field/focus state, draft values, and validation/error state.
- [x] Add tests showing `a` opens the form, Escape cancels it, Enter advances fields, and invalid drafts are rejected without an API call.
- [x] Run focused tests and verify they fail before implementation.
- [x] Implement the minimal state transitions and validation for name, HTTP(S) base URL, non-empty token, and three redirect policies.

### Task 3: Connect form submission to the admin API and render the modal

**Files:**
- Modify: `crates/comux-cli/src/tui/app.rs`
- Modify: `crates/comux-cli/src/tui/ui.rs`
- Modify: `crates/comux-cli/src/tui/mod.rs`
- Test: `crates/comux-cli/src/tui` unit tests

- [x] Add `ApiClient::put_upstream` submission from the completed form, refresh on success, retain an error message on failure, and pause automatic refresh while the form is open.
- [x] Add tests for successful form submission using a local admin HTTP test server and for preserving the form on server errors.
- [x] Render a centered modal with field labels, current value, focus indicator, policy selector, and validation/error text.
- [x] Route key events to the form before dashboard actions and restore the dashboard after submit/cancel.
- [x] Run `cargo test -p comux --lib tui` and the complete workspace suite.

### Task 4: Verify and commit

**Files:**
- Modify: `README.md` only if the TUI key list needs updating.

- [x] Run `cargo fmt --all -- --check`.
- [x] Run `cargo test --workspace`.
- [x] Run `cargo clippy --workspace --all-targets -- -D warnings`.
- [x] Run `git diff --check` and confirm the pre-existing untracked `1.config` remains untouched.
- [ ] Commit the TUI changes with a focused message.
