# Phase 00 Plan — Release Baseline, Hygiene, and Controls

> Execute with Superpowers `using-git-worktrees`, `test-driven-development`, `systematic-debugging`, `verification-before-completion`, and `requesting-code-review`. Track progress in `.planning/STATE.md`.

**Goal:** create a solver-free, cross-platform, reproducible baseline and remove accidental release artifacts without changing model semantics.

**Requirements:** R0.*, R1.*, R7.1–R7.3, R9.5–R9.6.

**Do not:** publish, tag, deeply refactor adapters, or fix semantic behavior in this phase except defects that prevent baseline tooling from running. Record semantic findings for P1/P2.

## Task 0.1 — Isolate work and capture repository identity

**Files:** create `docs/release/evidence/P0_BASELINE.md`.

1. Create an isolated worktree/branch from the current `main` head.
2. Record:
   - base SHA and UTC timestamp;
   - `rustc -Vv`, `cargo -V`, host/target triples;
   - installed native solver environment variables with values redacted to presence/absence;
   - `git status --short`, tracked file list, workspace metadata.
3. Run and record exit status/output summaries:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps
cargo metadata --no-deps --format-version 1
cargo tree --workspace --all-features
cargo package --list -p roml
cargo package --list -p roml-highs
cargo package --list -p roml-mosek
cargo package --list -p roml-xpress
```

Commercial/backend commands may fail because native libraries are absent. Capture failures exactly; do not install around them before evidence is saved.

**Acceptance:** evidence distinguishes core compilation defects from missing native-library failures and is committed before cleanup.

## Task 0.2 — Add minimal solver-free CI first

**Files:** `.github/workflows/ci-core.yml`, `rust-toolchain.toml` only if policy chooses a pinned channel, `.cargo/config.toml` only when justified.

1. Add a matrix for `ubuntu-latest`, `macos-latest`, `windows-latest`.
2. Run only core package commands initially:

```bash
cargo fmt --all -- --check
cargo check -p roml --all-targets
cargo clippy -p roml --all-targets -- -D warnings
cargo test -p roml --all-targets
RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps
```

3. Add concurrency cancellation and least-privilege permissions.
4. Cache only Cargo registry/git/target inputs with a maintained action and keyed toolchain/lockfile.
5. Add a Linux MSRV job after `rust-version` is selected.

**TDD gate:** push the workflow in a PR and confirm that it fails on the unclean baseline where expected; fix code/tooling failures rather than weakening checks.

**Acceptance:** core lane passes without HiGHS, MOSEK, or Xpress installed on all three OS families.

## Task 0.3 — Clean release contamination

**Files to delete unless a documented use is proven:** `main.py`, `pyproject.toml`, `uv.lock`, `config.yaml`, `roml-mosek/roml.log`.

**Files to revise:** `.gitignore`, `README.md`.

1. Add ignores for `*.log`, native build outputs, local solver configuration, IDE/temp files, and platform dynamic libraries only where appropriate.
2. Remove the broken `MODELING_API.md` link or create the real document only if its intended content is known.
3. Remove “production-grade” claims until P6, replacing with precise pre-release status.
4. Verify no secrets, proprietary headers/binaries/licenses, absolute user paths, or generated native files are tracked:

```bash
git ls-files | sort
rg -n '/Users/|/Applications/FICO|MOSEK|XPRESS_DIR|HIGHS_ROOT|TOKEN|LICENSE' . \
  -g '!Cargo.lock' -g '!docs/release/evidence/**'
```

5. Add regression check/script if package contamination is likely to recur.

**Acceptance:** repository root represents a Rust workspace, not an accidental mixed scaffold; `cargo package --list` has no placeholder/log/config debris.

## Task 0.4 — Establish workspace package metadata

**Files:** `Cargo.toml`, each member `Cargo.toml`, license files after owner confirmation.

1. Add `[workspace.package]` fields:
   - version policy;
   - edition;
   - rust-version;
   - authors;
   - repository;
   - license;
   - readme policy.
2. Add `[workspace.dependencies]` for shared Rust dependencies with intentional versions/features.
3. Make member manifests inherit metadata where Cargo supports it.
4. For workspace path dependencies use both version and path, e.g.:

```toml
roml = { version = "0.1.0", path = ".." }
```

5. Add `links` only to the package that ultimately owns each native library; do not add it prematurely if P3 will replace the package.
6. Add `publish = false` to commercial adapters until P4/P5 qualification.
7. Remove unused core dependencies after `cargo machete`/manual confirmation. Expected candidates: `rand`, `log4rs`, `serde_yaml`; logging removal may be split into Task 0.5.
8. Add explicit `include` lists or carefully reviewed `exclude` policies.
9. Confirm recommended dual license with owner, then add `LICENSE-MIT` and `LICENSE-APACHE` and use `MIT OR Apache-2.0`.

**Acceptance:** `cargo metadata` resolves package versions correctly; packed manifests no longer contain unusable path-only dependencies; commercial adapters cannot be accidentally published.

## Task 0.5 — Remove global logging configuration from core

**Files:** `src/logging.rs`, `src/lib.rs`, `Cargo.toml`, examples/tests using `init_logging`, `log4rs.yaml`.

1. Add a test or compile fixture showing core APIs work without logger initialization or config files.
2. Retain backend/core event emission through `log` for now.
3. Remove `init_logging` from stable core API and delete workspace scanning, env mutation, YAML parsing, and stdout printing.
4. Move any desired setup example into `examples/logging.rs` using a dev dependency, or document that applications choose their logger.
5. Remove `log4rs`/`serde_yaml` runtime dependencies from core and adapters unless independently needed.
6. Run tests serially once to detect prior global-state assumptions, then restore normal parallel tests.

**Acceptance:** importing/using ROML has no filesystem search, environment mutation, global logger setup, or console output side effect.

## Task 0.6 — Install lint, dependency, docs, and packaging controls

**Files:** root `Cargo.toml`, `deny.toml`, `.github/workflows/ci-policy.yml`, optional `rustfmt.toml` only for non-default policy, `docs/release/PACKAGING.md`.

1. Add workspace Rust/Clippy lints without creating a large unrelated formatting churn. Deny unsafe operations in unsafe functions where toolchain permits; warn/deny missing safety docs in native crates.
2. Configure `cargo-deny` for advisories, licenses, bans, and sources. Account for commercial solver package licensing separately.
3. Add unused-dependency checking (`cargo machete` or maintained equivalent).
4. Add `cargo audit` if not redundant with policy; document advisory exceptions with expiry/reason.
5. Add package checks:

```bash
cargo package -p roml --allow-dirty --no-verify
cargo package --list -p roml
```

Use a temporary consumer project to depend on the generated `.crate` archive once package metadata permits.
6. Add rustdoc warnings-as-errors and doctest execution.
7. Add `cargo-semver-checks` as advisory until the first release baseline exists.

**Acceptance:** policy failures are actionable and reproducible locally; no blanket allowlists.

## Task 0.7 — Document contribution and release governance

**Files:** `CONTRIBUTING.md`, `SECURITY.md`, `CHANGELOG.md`, `docs/release/RELEASE_CHECKLIST.md`, `docs/release/SUPPORT_MATRIX.md`.

Required content:

- development prerequisites and solver-free commands;
- backend-specific optional prerequisites;
- conventional commit/PR expectations if adopted;
- security reporting channel without exposing private information;
- Keep a Changelog-compatible unreleased section;
- support labels (`supported`, `tested`, `compile-only`, `experimental`);
- explicit no-publish-without-owner-approval gate;
- package order and fresh-consumer testing.

**Acceptance:** a contributor can run the core lane and understand why commercial backend tests may be unavailable.

## Task 0.8 — Final P0 verification

Run on a clean checkout/worktree:

```bash
cargo fmt --all -- --check
cargo check -p roml --all-targets
cargo clippy -p roml --all-targets -- -D warnings
cargo test -p roml --all-targets
RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps
cargo deny check
cargo machete
cargo package --list -p roml
cargo package -p roml --locked
```

Then verify all mandatory CI jobs and inspect package archive contents manually.

Update `.planning/STATE.md` with completed requirement IDs, commit SHA, evidence path, deviations, and P1 readiness.

**Phase gate:** no solver is required for core CI/package/docs; repository and package contents are clean; governance prevents accidental publication; all baseline failures are classified.