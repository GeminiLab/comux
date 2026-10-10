# Documentation Coverage Design

## Goal

Make the comux workspace understandable without tracing every implementation
path. The finished documentation will cover the repository, every production
Rust item, and the non-obvious design decisions inside important functions
without changing runtime behavior.

All new documentation and comments will be written in English.

## Scope

The work covers all three workspace crates:

- `comux-core`, which owns shared configuration, state, persistence, secrets,
  and management API data transfer objects.
- `comux-proxy`, which owns the downstream proxy, management server,
  authentication, forwarding, and in-memory transport statistics.
- `comux`, which owns command-line configuration, the management API client,
  commands, and the terminal user interface.

Every production crate, public or private module, type, field, enum variant,
constant, function, and method will receive a Rust documentation comment.
Specific test functions are exempt. Complex test fixtures and helpers may
receive documentation or ordinary comments when their setup is not obvious.

Important functions will also receive ordinary comments at meaningful phase
boundaries. These comments will explain what a phase accomplishes and why the
implementation uses that ordering or trade-off. They will not paraphrase
obvious assignments, branches, or loops.

No runtime behavior, public interface, configuration schema, or dependency
version will be changed as part of this work.

## Repository Documentation

### Root README

`README.md` will be the complete source of project-level guidance. It will
contain:

1. The project's purpose and capability summary.
2. A neutral disclosure that the project was created almost entirely with AI,
   together with a reminder that users remain responsible for reviewing its
   security, configuration, and changes.
3. The workspace architecture, crate responsibilities, and downstream and
   management request flows.
4. Prerequisites, build and installation instructions, and a minimal quick
   start.
5. The complete proxy configuration format, including defaults, validation
   rules, `null` upstream behavior, token generation, connection settings, and
   redirect policies.
6. Codex and CLI configuration, override precedence, and sensitive token
   handling.
7. CLI command examples and TUI controls.
8. The management API routes, authentication, request and response shapes,
   token reveal behavior, and important error behavior.
9. The security model, especially listener exposure, credential boundaries,
   cross-host redirects, and local configuration file permissions.
10. Troubleshooting guidance for argument separation, authentication,
    unavailable upstreams, and connectivity.
11. Development and verification commands.
12. License information and links to crate documentation.

Descriptions will reflect the current implementation. Historical design goals
that are not implemented, such as graceful shutdown, will not be presented as
available features.

### Crate READMEs

Each crate will receive a concise README:

- `crates/comux-core/README.md`
- `crates/comux-proxy/README.md`
- `crates/comux-cli/README.md`

Each file will state the crate's responsibility, its relationship to the other
crates, and the smallest useful build, run, or library usage guidance. Details
will remain in the root README to avoid divergent copies. Every crate README
will link to the canonical repository documentation at
`https://github.com/GeminiLab/comux#readme`.

No separate architecture or contributing document will be added.

## Rustdoc Design

The three library roots will explain each crate's role and module map. Public
items will be guarded by `missing_docs` warnings so later additions do not
silently reduce public documentation coverage. Private-item coverage will be
verified by a source audit because Rust's `missing_docs` lint does not enforce
it.

Rustdoc will describe:

- Responsibilities and invariants rather than restating item names.
- Parameters, return values, state changes, and ownership or snapshot
  semantics where relevant.
- Error conditions and the meaning of error variants.
- Security-sensitive behavior, including token masking and propagation.
- Serialized forms and field meaning for configuration and management API
  types.
- Interactions between runtime state, persistent state, and in-flight
  requests.

Cross-links will be used when they make navigation clearer, but documentation
must remain readable when viewed directly in source.

## Internal Comment Design

Ordinary comments will be concentrated in the following control paths:

- Configuration validation and atomic persistence in `comux-core`.
- Candidate-state validation, persistence-before-commit, and active-upstream
  snapshot semantics.
- Downstream authentication, request routing, streamed response accounting,
  and statistics guard completion in `comux-proxy`.
- Redirect handling, replayable request bodies, method changes, hop limits,
  idle timeouts, and per-hop credential injection.
- CLI configuration source precedence and explicit full-token disclosure.
- TUI refresh scheduling, form/refresh exclusion, and selection
  initialization.

Comments will be updated only where the code makes the documented behavior
true. Ambiguous or surprising code will be described precisely rather than
being given an aspirational explanation.

## Error and Security Documentation

Configuration and state errors will document their triggering conditions.
Management API data transfer objects will document their JSON meaning, and the
root README will explain common HTTP outcomes without claiming a stability
guarantee that the project does not currently provide.

The security section will explicitly distinguish:

- The downstream token accepted from Codex.
- The management token accepted by the admin listener.
- Per-upstream credentials sent to selected upstream services.

It will also state that the `always` redirect policy can forward an upstream
credential across hosts and must only be used with trusted redirect targets.
Generated or example documentation will not contain credentials from local
configuration files.

## Verification

The implementation will run:

```sh
cargo fmt --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --document-private-items
```

A manual source audit will verify that every non-test production item has
Rustdoc and that important functions contain useful, non-redundant comments.
The root README configuration reference will be checked against
`comux-core/src/config.rs`, its route list against `comux-proxy/src/admin.rs`,
and its CLI reference against the clap declarations. Crate README links will
be checked for consistency.

Existing uncommitted changes to `Cargo.toml`, `Cargo.lock`, and `.gitignore`
are outside this design and must remain intact.
