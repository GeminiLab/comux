# Publishing comux

The workspace publishes three crates at the shared version in the root
`Cargo.toml`: `comux-core`, `comux-proxy`, and `comux`. Both application crates
depend on `comux-core`. The workspace dependency specifies both `path` and
`version`: local builds use the path, and packaged manifests use the registry
version.

## Prepare a release

1. Check that you can publish each crate name on crates.io. For existing
   crates, your crates.io account must be an owner. A name lookup does not
   reserve the name.
2. Update `workspace.package.version` and the version requirement for
   `workspace.dependencies.comux-core` together. Update version examples in
   the documentation, and review `Cargo.lock` after dependency changes.
3. Keep the MIT license at the repository root. Each crate's `LICENSE` links
   to that file; Cargo includes its contents as a regular file in the archive.
   The release checkout must preserve these symlinks. If Git uses
   `core.symlinks=false` (common on Windows), create a checkout with symlink
   support before packaging; otherwise the archive can contain the link's
   path text instead of the license. Inspect each archived `LICENSE` to confirm
   it contains the full MIT text.
4. Run the checks below and review the package contents. Commit the intended
   release changes before the final dry run, so the package records the source
   revision and Cargo's clean-working-tree check remains enabled.

## Verify the workspace

The code requires Rust 1.88 or newer. Use the current stable toolchain for
release operations; workspace packaging and publishing require Cargo 1.90 or
newer.

```sh
cargo +stable fmt --all --check
cargo +stable check --workspace --all-targets --locked
cargo +stable test --workspace --locked
cargo +stable clippy --workspace --all-targets --locked -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo +stable doc --workspace --no-deps --document-private-items --locked
cargo +1.88.0 check --workspace --all-targets --locked
```

The last command requires the Rust 1.88.0 toolchain to be installed. Run it
after dependency changes to check the declared minimum version against the
lockfile.

## Inspect packages and rehearse publication

From a clean checkout:

```sh
cargo +stable package --workspace --list
cargo +stable publish --workspace --dry-run --locked
```

The workspace dry run packages and builds all three crates, including their
local dependencies, without uploading them. Using `--workspace` lets Cargo
stage the new `comux-core` package for verifying the two dependent crates
before the first release exists on crates.io. The verified archives are
written to `target/package/`.

Each archive should contain the normalized manifest, lockfile, README, MIT
license, and source files. `comux-proxy` also includes its integration tests.
The shared `include` allowlist in `workspace.package` keeps package contents
explicit. If you add build scripts, assets, examples, or other files needed
outside the current source and test directories, update that allowlist.

When inspecting uncommitted changes locally, add `--allow-dirty` to the package
or dry-run command. Use a clean checkout for the final release verification.

## Publish

Authenticate with crates.io using `cargo login` or a configured Cargo
credential provider. After reviewing the dry-run results, publish from the
same clean source revision:

```sh
cargo +stable publish --workspace --locked
```

Cargo publishes `comux-core` before its dependents and waits for registry
availability. Publication is not atomic: if a later upload fails, check which
versions were accepted before retrying the remaining crates.

Alternatively, publish one crate at a time in dependency order:

```sh
cargo +stable publish -p comux-core --locked
# Wait until comux-core is available in the crates.io index.
cargo +stable publish -p comux-proxy --dry-run --locked
cargo +stable publish -p comux-proxy --locked
cargo +stable publish -p comux --dry-run --locked
cargo +stable publish -p comux --locked
```

A published version cannot be overwritten. Confirm its manifests and contents
before uploading. Once all three packages are available, verify installation
from crates.io with their release version:

```sh
cargo install --locked --version 0.1.0 comux-proxy
cargo install --locked --version 0.1.0 comux
comux-proxy --version
comux --version
```

`comux-core` is a library dependency and has no binary to install.
