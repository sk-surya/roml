# M2 — Phase 20 Public API Baseline

**Phase:** 20-public-api-contract
**Plan:** `20-PLAN.md`
**Requirements:** API-04, API-07, API-08, API-10
**Branch:** `phase-roml-P20-api-contract`

This document records the exact current-main public API surface and behavior of
`roml` and `roml-highs` at the P20 implementation base. It is the reference for
the target-contract fixtures (`tests/ui/target_*.rs`), the disposition table
(`docs/release/PUBLIC_API_M2_DISPOSITION.md`), and the repeated-solve protocol
baseline (`roml-highs/tests/repeated_session_baseline.rs`).

## Base commit and environment

| Item | Value |
|---|---|
| Base commit (`git rev-parse HEAD`) | `d1391fb3a58a61d24b5c597aab78e4be7683f894` |
| Branch | `phase-roml-P20-api-contract` |
| `rustc --version` | `rustc 1.97.1 (8bab26f4f 2026-07-14)` |
| `cargo --version` | `cargo 1.97.1 (c980f4866 2026-06-30)` |
| `rustc -vV` host | `aarch64-apple-darwin` (LLVM 22.1.6) |
| OS | macOS `Darwin forge 25.4.0` Darwin Kernel 25.4.0 arm64 |
| `cargo public-api --version` | `cargo-public-api 0.52.0` |
| HiGHS build | bundled via `highs-sys 1.15.0` (cmake); no system HiGHS |

All commands below ran on the platform above with the toolchain above at the base
commit. Exit status and test counts are recorded per command per EXECUTION.md.

## Core baseline matrix (`-p roml`)

| Command | Exit | Result |
|---|---|---|
| `cargo fmt --all -- --check` | 0 | formatting clean |
| `cargo check -p roml --all-targets` | 0 | clean |
| `cargo clippy -p roml --all-targets -- -D warnings` | 0 | clean, warnings denied |
| `cargo test -p roml --all-targets` | 0 | **399 passed; 0 failed; 2 ignored** |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps` | 0 | docs generated, no warnings |
| `cargo test --doc -p roml` | 0 | 0 passed; 8 ignored (all `ignore`-gated) |
| `cargo package --list -p roml` | 101 | **skipped** — see "Skipped checks" |

## HiGHS baseline matrix (`-p roml-highs`)

| Command | Exit | Result |
|---|---|---|
| `cargo check -p roml-highs --all-targets` | 0 | clean |
| `cargo clippy -p roml-highs --all-targets -- -D warnings` | 0 | clean, warnings denied |
| `cargo test -p roml-highs --all-targets` | 0 | **73 passed; 0 failed; 0 ignored** |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml-highs --no-deps` | 0 | docs generated, no warnings |
| `cargo test --doc -p roml-highs` | 0 | 0 passed; 1 ignored |
| `cargo package --list -p roml-highs` | 0 | 19 files (list below) |

### `cargo package --list -p roml-highs`

```text
.cargo_vcs_info.json
Cargo.lock
Cargo.toml
Cargo.toml.orig
build.rs
src/bindings.rs
src/callback.rs
src/error.rs
src/ffi.rs
src/index_map.rs
src/lib.rs
src/lifecycle.rs
src/projection.rs
src/session.rs
src/solution.rs
tests/behavior_tests.rs
tests/conformance.rs
tests/contract_tests.rs
tests/solve_observables_tests.rs
```

## Public API inventory

Full `cargo public-api` output is stored verbatim as milestone evidence
(API-07.5):

- `docs/release/evidence/M2_P20_public_api_roml.txt` (7431 lines)
- `docs/release/evidence/M2_P20_public_api_roml_highs.txt` (110 lines)

### `roml` — public item counts (from public-api output)

| Kind | Count |
|---|---|
| `pub fn` | 2501 |
| `pub struct` | 90 |
| `pub trait` | 18 |
| `pub enum` | 42 |
| `pub type` | 613 |
| `pub mod` | 26 |
| total `pub` lines (non-indented) | 4290 |

Root re-exports (`src/lib.rs`) and the `prelude` module currently expose a mix of
audiences: model types, expression traits, macro entry points (`constraint!`,
`objective!`, `constrain!`, `set_objective!`), protocol types (`Change`,
`CoeffId`, `DeltaBatch`, `ModelOp`, `ModelRevision`, `ModelSnapshot`), and
backend/session types (`BackendSession`, `Synchronization`, `SyncReceipt`,
`AdapterCursor`, `AdapterHealth`, `SessionHealth`, `SolutionView`). This mixed
surface is classified per-item in `docs/release/PUBLIC_API_M2_DISPOSITION.md`.

### `roml-highs` — full public surface (110 lines; 42 `pub` items)

Confirmed exports: `HighsSession`, `HighsFixture`, `HighsInt`, `HighsError`
(= `roml::solver::backend::BackendError` alias). `HighsSession` implements
`BackendSession`, `SessionHealth`, `SolutionView`, `BackendMetadata`,
`CallbackSession`, and `Send` (not `Sync`). No façade type exists yet: the only
session entry point is `HighsSession::try_new()`.

**Key finding:** `cargo public-api` output contains **zero** occurrences of
`HighsAdapter` or `solve_model` in either crate.

## Documentation drift characterization (API-09 evidence)

`README.md` and `MODELING_API.md` document `roml_highs::HighsAdapter` with
`adapter.solve_model(&mut model)`. Neither exists in production code. The frozen
fixture is `tests/ui/current_readme_drift.rs` (kept out of the default build —
files under `tests/ui/` are not auto-discovered — so API-10.1 stays green).

Compile of the README fixture body from a `roml-highs` integration-test context:

```text
error[E0432]: unresolved import `roml_highs::HighsAdapter`
  --> roml-highs/tests/zz_current_readme_drift_check.rs:31:5
   |
31 | use roml_highs::HighsAdapter;
   |     ^^^^^^^^^^^^^^^^^^^^^^^^ no `HighsAdapter` in the root
error: could not compile `roml-highs` (test "zz_current_readme_drift_check")
```

Compile of the `solve_model` call against the real session type:

```text
error[E0599]: no method named `solve_model` found for struct `HighsSession` in the current scope
  --> roml-highs/tests/zz_solve_model_method_check.rs:13:28
   |
13 |     let solution = adapter.solve_model(&mut model)?;
   |                            ^^^^^^^^^^^ method not found in `HighsSession`
error: could not compile `roml-highs` (test "zz_solve_model_method_check")
```

Repository search for production `HighsAdapter` / `solve_model`:

```bash
grep -rn "HighsAdapter\|solve_model" src/ roml-highs/src/ roml-mosek/src/ roml-xpress/src/
```

Result: no production type or method. The only source reference is a stale doc
comment in `roml-highs/src/ffi.rs` ("Only binding limited functions needed by
HighsAdapter.") — pre-existing, out of scope for P20. The primary user story
fails at copy/paste; P21 must provide the replacement façade.

## Skipped checks

| Check | Reason |
|---|---|
| `cargo package --list -p roml` (exit 101) | Working tree contains untracked local artifacts (`.planning/`, `graphify-out/`) that are not part of this phase; `cargo package` requires a clean tree. The roml-highs package list succeeds (its crate directory is clean). Expected to pass in a clean checkout. |
| Doctests reporting 0 passed | All doctests are `ignore`-gated in current main; none are executable. Recorded, not claimed as coverage. |

## Companion P20 artifacts

- `tests/ui/current_readme_drift.rs` — frozen README drift fixture (Task 2).
- `tests/ui/target_quickstart.rs`, `tests/ui/target_incremental.rs` — frozen
  target API contracts (Task 3); expected to become compile-pass tests in P21/P22.
- `tests/public_api_compile.rs` — compile-pass characterization of the current
  `roml` public surface that does compile today (Task 3).
- `docs/release/PUBLIC_API_M2_DISPOSITION.md` — per-item disposition of the
  public surface (Task 4).
- `roml-highs/tests/repeated_session_baseline.rs` — repeated-solve protocol
  behavior baseline (Task 5); results are appended to this document there.
