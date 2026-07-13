# Phase 05 Plan — Public API, Documentation, and Package Engineering

**Goal:** curate a coherent pre-1.0 API, complete user/maintainer documentation, and prove that packed crates work outside the workspace.

**Requirements:** R0.*, R1.*, R9.*.

## Task 5.1 — Inventory and classify the public API

Generate rustdoc JSON or use `cargo public-api`/equivalent to inventory all public items. Classify each as:

- stable user concept;
- intentionally experimental;
- backend extension point;
- internal implementation accidentally public.

Review especially:

- entity data/store types and public fields;
- raw slot index/generation methods;
- changelog/delta internals;
- callback types;
- solution map ownership;
- prelude contents;
- macros and operator overloads.

Create `docs/release/PUBLIC_API_REVIEW.md` with disposition and semver rationale.

## Task 5.2 — Narrow and regularize API surface

1. Make internal stores/fields private and expose focused read-only views/accessors.
2. Keep IDs opaque; provide formatting/debug names without promising persistent raw indices.
3. Standardize constructor/mutator naming and `Result` behavior.
4. Expose transaction/bulk APIs explicitly.
5. Make backend/session/capability interfaces discoverable but not coupled to raw bindings.
6. Use sealed traits where downstream implementation would violate invariants.
7. Mark genuinely experimental features and document stability.
8. Add compile tests for intended ergonomics and negative compile tests where misuse must fail.

## Task 5.3 — Complete rustdoc

**Files:** crate/module docs throughout; enable missing-doc lint after debt closure.

Crate root must explain:

- why ROML exists;
- canonical parameter-to-cell tracking;
- revision/snapshot/delta synchronization;
- transactions and error semantics;
- solver-neutral core vs adapter crates;
- thread/callback safety;
- performance model and non-goals.

Every public item needs:

- semantics/invariants;
- errors/panics (ideally none on normal input);
- complexity where material;
- examples for high-value APIs;
- safety section for unsafe public APIs (prefer none).

Run doctests on all examples and deny rustdoc warnings/broken intra-doc links.

## Task 5.4 — Build user guides and examples

**Files:** `docs/guide/`, `examples/`, backend examples.

Mandatory guides/examples:

1. solver-free model construction and inspection;
2. HiGHS LP and MIP solve;
3. parameterized coefficient update and incremental re-solve;
4. transactions/bulk changes;
5. multiple adapter sessions/cursors;
6. failure/rebuild recovery;
7. capabilities/status/error handling;
8. native installation/troubleshooting;
9. performance/batching advice;
10. commercial adapter setup only for qualified support tiers.

Examples must compile in CI and avoid absolute paths/licenses.

## Task 5.5 — Rewrite README and project claims

README structure:

- concise differentiator;
- support/status badge table;
- minimal code example;
- incremental example;
- crate topology;
- supported platforms/backends with precise labels;
- installation links;
- safety/correctness philosophy;
- benchmark methodology link, not unsupported headline claims;
- contribution/license links.

Do not restore “production-grade” until P6 evidence justifies it. Prefer “pre-1.0” and exact capabilities.

## Task 5.6 — Finalize manifests and package contents

For each publishable crate:

- package metadata inherited or explicit;
- SPDX license and license files included;
- readme path valid relative to package;
- repository/documentation links;
- rust-version/MSRV;
- max five valid keywords/categories;
- path+version workspace dependencies;
- `links` only at raw native owner;
- docs.rs metadata/features;
- explicit include list;
- no tests/fixtures requiring unavailable proprietary assets in package verification;
- `publish = false` on unqualified crates.

Commands:

```bash
cargo package --list -p <crate>
cargo package -p <crate> --locked
```

Inspect normalized packaged `Cargo.toml` and archive contents.

## Task 5.7 — Fresh consumer verification

Create temporary projects outside the workspace and test dependencies from generated `.crate` archives or local registry tooling:

- core-only app;
- HiGHS app using default features;
- HiGHS system-discovery feature if supported;
- docs/examples compilation;
- missing commercial library diagnostics for any publishable commercial adapter.

Do not allow workspace path resolution to mask missing versioned dependencies/files.

## Task 5.8 — Establish semver and deprecation process

**Files:** `docs/release/SEMVER.md`, `CHANGELOG.md`.

Define:

- pre-1.0 compatibility promises;
- public vs experimental modules/features;
- MSRV change policy;
- solver-version support policy;
- deprecation window;
- feature additive/removal rules;
- semver check baseline after first release.

Run `cargo-semver-checks` against a generated baseline and include output in evidence.

## Task 5.9 — Maintainer and security documentation

Complete/review:

- CONTRIBUTING;
- SECURITY and unsafe/native threat model;
- support matrix;
- native troubleshooting;
- release checklist;
- dependency/license policy;
- architecture decisions;
- commercial solver handling;
- benchmark reproduction.

## Verification

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --all-features
cargo test --doc --workspace
cargo package --list -p roml
cargo package --list -p roml-highs
cargo package -p roml --locked
cargo package -p roml-highs --locked
cargo deny check
```

Commercial crates run only according to their publication/support state.

**Phase gate:** intended public API is reviewed and documented; packed core/HiGHS crates work in fresh consumers; package contents and claims are accurate; unqualified adapters cannot be published accidentally.