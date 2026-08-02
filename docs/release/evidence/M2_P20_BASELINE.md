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
| `cargo package --list -p roml` | 0 | 99 files — list below (clean-worktree capture, review item 4) |

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

### `cargo package --list -p roml` (clean-worktree capture)

Captured in a fresh worktree at the verification head (`680ea83`), where the
git tree is clean. Exit 0. Note: `roml`'s package root is the repository
root and its manifest has no `include`/`exclude` filter, so the package
currently contains repo-level planning, tooling, and documentation files
(`.planning/`, `tools/`, `.foundry.toml`, `badges/`, `docs/knowledge/`).
That is a packaging-hygiene baseline finding for P6/P24 (API-10.3 fresh
consumers, "no workspace path leakage") — not a P20 defect.

```text
.cargo_vcs_info.json
.foundry.toml
.planning/PROJECT.md
.planning/REQUIREMENTS.md
.planning/ROADMAP.md
.planning/STATE.md
.planning/milestones/M2-public-api-ergonomics/DECISIONS.md
.planning/milestones/M2-public-api-ergonomics/EXECUTION.md
.planning/milestones/M2-public-api-ergonomics/PROJECT.md
.planning/milestones/M2-public-api-ergonomics/README.md
.planning/milestones/M2-public-api-ergonomics/REQUIREMENTS.md
.planning/milestones/M2-public-api-ergonomics/RESEARCH.md
.planning/milestones/M2-public-api-ergonomics/RISKS.md
.planning/milestones/M2-public-api-ergonomics/ROADMAP.md
.planning/milestones/M2-public-api-ergonomics/STATE.md
.planning/milestones/M2-public-api-ergonomics/TRACEABILITY.md
.planning/phases/20-public-api-contract/20-PLAN.md
.planning/phases/20-public-api-contract/20-SUMMARY.md
.planning/phases/20-public-api-contract/20-UAT.md
.planning/phases/20-public-api-contract/20-VERIFICATION.md
.planning/phases/21-solver-facade/21-PLAN.md
.planning/phases/22-modeling-ergonomics/22-PLAN.md
.planning/phases/23-surface-curation/23-PLAN.md
.planning/phases/24-consumer-qualification/24-PLAN.md
CHANGELOG.md
CONTRIBUTING.md
Cargo.lock
Cargo.toml
Cargo.toml.orig
MODELING_API.md
README.md
SECURITY.md
assets/roml-logo-v2.png
assets/roml-logo.svg
badges/coverage.svg
deny.toml
docs/knowledge/current_context.md
docs/knowledge/sessions/ledger.md
docs/knowledge/steward_protocol.md
docs/release/ARCHITECTURE_DECISIONS.md
docs/release/CODING_AGENT_PROMPT.md
docs/release/CURRENT_MAIN_DELTA_AUDIT.md
docs/release/PACKAGING.md
docs/release/PRINCIPAL_ENGINEERING_AUDIT.md
docs/release/PUBLIC_API_M2_DISPOSITION.md
docs/release/RELEASE_CHECKLIST.md
docs/release/SUPPORT_MATRIX.md
docs/release/XPRESS_BINDING_DECISION.md
examples/parameter_update.rs
examples/simple_lp.rs
scripts/coverage_badge.py
scripts/p20-capture-drift.sh
src/delta.rs
src/expr/linear.rs
src/expr/mod.rs
src/id/arena.rs
src/id/mod.rs
src/journal.rs
src/lib.rs
src/main.rs
src/model/changelog.rs
src/model/coefficient.rs
src/model/constraint.rs
src/model/mod.rs
src/model/objective.rs
src/model/parameter.rs
src/model/transaction.rs
src/model/validation.rs
src/model/variable.rs
src/revision.rs
src/snapshot.rs
src/solution/mod.rs
src/solver/backend.rs
src/solver/callback.rs
src/solver/conformance.rs
src/solver/mod.rs
src/solver/reference.rs
src/solver/request.rs
src/solver/session.rs
src/sync.rs
src/transaction.rs
src/value_expr/mod.rs
tests/backend_contract.rs
tests/changelog_integration.rs
tests/delta_content_verification.rs
tests/differential_harness.rs
tests/end_to_end_equivalence.rs
tests/macro_api.rs
tests/model_characterization.rs
tests/public_api_compile.rs
tests/semicontinuous_recovery.rs
tests/status_negotiation_tests.rs
tests/sync_characterization.rs
tests/ui/current_readme_drift.rs
tests/ui/current_solve_model_method.rs
tests/ui/target_incremental.rs
tests/ui/target_quickstart.rs
tools/steward/steward
tools/steward/steward_freshness.py
```

## Public API inventory

Full `cargo public-api` output is stored as milestone evidence (API-07.5).
The dumps are **normalized** public-API output, not byte-for-byte verbatim:
absolute repository paths are replaced with the `$REPO` token so the evidence
contains no machine-local identity (review item 5). API item lines are
byte-identical to raw output; only header/path lines differ:

- `docs/release/evidence/M2_P20_public_api_roml.txt` (7431 lines)
- `docs/release/evidence/M2_P20_public_api_roml_highs.txt` (80 lines)

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

### `roml-highs` — full public surface (80 lines; 42 `pub` items)

Confirmed exports: `HighsSession`, `HighsFixture`, `HighsInt`, `HighsError`
(= `roml::solver::backend::BackendError` alias). `HighsSession` implements
`BackendSession`, `SessionHealth`, `SolutionView`, `BackendMetadata`,
`CallbackSession`, and `Send` (not `Sync`). No façade type exists yet: the only
session entry point is `HighsSession::try_new()`.

**Key finding:** `cargo public-api` output contains **zero** occurrences of
`HighsAdapter` or `solve_model` in either crate.

## Documentation drift characterization (API-09 evidence)

`README.md` and `MODELING_API.md` document `roml_highs::HighsAdapter` with
`adapter.solve_model(&mut model)`. Neither exists in production code. Two
committed fixtures freeze the drift — both kept out of the default build
(files under `tests/ui/` are not auto-discovered, so API-10.1 stays green):

- `tests/ui/current_readme_drift.rs` — the documented `HighsAdapter` import
  path (E0432);
- `tests/ui/current_solve_model_method.rs` — the documented `solve_model`
  call against the real public session type `HighsSession` (E0599).

Both failures are reproduced from the committed fixtures by
`scripts/p20-capture-drift.sh` (temporarily copies each fixture into the
`roml-highs` integration-test directory, compiles it, asserts the expected
error code, and removes the copy). Captured output at the verification head:

```text
== 1/2: drift fixture (README HighsAdapter) -> expect E0432 ==
error[E0432]: unresolved import `roml_highs::HighsAdapter`
  --> roml-highs/tests/zz_p20_drift_capture.rs:31:5
   |
31 | use roml_highs::HighsAdapter;
   |     ^^^^^^^^^^^^^^^^^^^^^^^^ no `HighsAdapter` in the root
error: could not compile `roml-highs` (test "zz_p20_drift_capture") due to 1 previous error

== 2/2: solve_model method fixture (HighsSession) -> expect E0599 ==
error[E0599]: no method named `solve_model` found for struct `HighsSession` in the current scope
  --> roml-highs/tests/zz_p20_solve_model_capture.rs:53:28
   |
53 |     let solution = adapter.solve_model(&mut model)?;
   |                            ^^^^^^^^^^^ method not found in `HighsSession`
error: could not compile `roml-highs` (test "zz_p20_solve_model_capture") due to 1 previous error

OK: both documented drift failures reproduced from committed fixtures.
```

Repository search for production `HighsAdapter` / `solve_model`:

```bash
grep -rn "HighsAdapter\|solve_model" src/ roml-highs/src/ roml-mosek/src/ roml-xpress/src/
```

Result: no production type or method. The only source reference is a stale doc
comment in `roml-highs/src/ffi.rs` ("Only binding limited functions needed by
HighsAdapter.") — pre-existing, out of scope for P20. The primary user story
fails at copy/paste; P21 must provide the replacement façade.

## Repeated-solve protocol baseline (Task 5)

Source: `roml-highs/tests/repeated_session_baseline.rs` (3 tests).

```bash
cargo test -p roml-highs --test repeated_session_baseline   # exit 0, 3 passed
```

Model under test: `maximize price*x + y` subject to `x + y <= 4`, `x,y >= 0`,
with a parameterized objective coefficient. Recorded per solve: revision,
health, termination status, objective value, and solution availability.

| Step | Revision | Health | Status | Objective | Solution available |
|---|---|---|---|---|---|
| Rebuild from snapshot, solve (`price = 3.0`) | r1 | Ready | Optimal | 12.0 | yes |
| Apply parameter delta (`price 3.0 -> 5.0`), solve | r2 | Ready | Optimal | 20.0 | yes |
| Apply bound delta (`x` upper 4.0 -> 2.0), solve | r2 | Ready | Optimal | 8.0 | yes |
| Model advanced to r2 (`price 3.0 -> 5.0`); sync rejected (mismatched base) | session r1 (model r2) | RequiresRebuild | — | 12.0 (stale) | yes — r1 solution stays readable but no longer matches the model |
| Rebuild from real r2 snapshot (deterministic recovery), solve | r2 | Ready | Optimal | 20.0 | yes |

On the dirty path the canonical model is genuinely ahead of the session: the
model has advanced to r2 (expected objective 20.0) while the session still
exposes its r1 solution — the objective reads 12.0, which is **stale**, not
just "the r1 solution". It is not invalidated. The test
(`dirty_path_recovers_via_deterministic_snapshot_rebuild`) asserts exactly
this readable-but-stale state before recovering from the *real* r2 snapshot,
which then solves to 20.0 with no stale values surviving. P21's façade
(API-01.5) must never report that stale result as current — the invalidation
policy on the rejected path is a P21 decision (recorded in the phase
SUMMARY's Issues and M2 STATE.md).

These values are the expected behavior for the P21 `SolverSession<B>` / `Highs`
façade tests: the parameter delta applies incrementally without a rebuild, and
the dirty path recovers deterministically via one snapshot rebuild.

## Skipped checks

| Check | Reason |
|---|---|
| Doctests reporting 0 passed | All doctests are `ignore`-gated in current main; none are executable. Recorded, not claimed as coverage. |

`cargo package --list -p roml` no longer appears here: it was re-run from a
clean worktree (review item 4) and passes with exit 0 (99 files). The earlier
exit-101 was caused by untracked local artifacts (`.planning/config.json`,
`.planning/graphs/`, `graphify-out/`) in the primary working tree; those
paths — not the tracked M2 packet — are what `cargo package`'s dirty check
objected to.

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
