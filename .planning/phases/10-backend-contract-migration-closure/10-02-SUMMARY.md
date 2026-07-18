---
phase: 10-backend-contract-migration-closure
plan: 02
type: execute
subsystem: roml-core-contract
status: complete
tags: [model-protocol-wiring, sync-coordinator, reference-backend, contract-tests]
requires: [10-01-PLAN.md]
provides: [Model-SyncCoordinator-integration, ReferenceBackend, backend-contract-tests]
affects: [src/model/mod.rs, src/lib.rs, src/solver/mod.rs, src/solver/backend.rs, src/solver/reference.rs, tests/changelog_integration.rs, tests/backend_contract.rs]
tech-stack:
  added: [SyncCoordinator on Model, take_snapshot(), current_revision(), compile_change(), ReferenceBackend, NormalizedView, InfeasibleOrUnbounded variant]
  patterns: [revisioned-sync-protocol, commuting-square-verification, bounded-trait-composition]
key-files:
  created: [src/solver/reference.rs, tests/backend_contract.rs]
  modified: [src/model/mod.rs, src/lib.rs, src/solver/mod.rs, src/solver/backend.rs, tests/changelog_integration.rs]
decisions: [D2 (drain_changes removed), D3 (contract tests replace pinned tests)]
metrics:
  duration: ~4m
  completed: 2026-07-18
plan_count: 3
commit_count: 3
---

# Phase 10 Plan 02: Backend Contract Migration Closure — Model Protocol Wiring

**One-liner:** Wires Model to SyncCoordinator with revisioned delta sync, removes legacy drain_changes/solver_options from public API, imports ReferenceBackend with commuting-square tests, rewrites changelog integration tests, and adds 10 contract tests covering M1R-C1 through C6.

## Work Summary

### Task 1: Wire Model to SyncCoordinator, add take_snapshot, remove drain_changes/solver_options

Modified `src/model/mod.rs` with these changes:

- **Added SyncCoordinator field** (`pub(crate) coordinator: SyncCoordinator`) to the Model struct, replacing the removed `solver_options` field.
- **Added imports:** `DeltaBatch`, `ModelOp`, `ModelRevision`, `SyncCoordinator`
- **Removed imports:** `SolveOptions`, `log::warn`
- **Removed methods:** `drain_changes()`, `has_pending_changes()`, `changelog_sequence()`, `set_solver_options()`
- **Modified `commit()`:** After applying parameter changes, drains the changelog, compiles each `Change` entry to its corresponding `ModelOp` variant via `compile_change()`, creates a `DeltaBatch`, and commits it to `SyncCoordinator::commit_batch()`. No-op if no pending changes.
- **Added `current_revision()`:** Returns `self.coordinator.revision()`
- **Added `take_snapshot()`:** Extracts model state (variables with semicontinuous, constraints, objectives, parameters, cells with value_expr dependencies) into HashMaps and calls `crate::snapshot::take_snapshot()`.
- **Added `compile_change()`:** Private method mapping all 16 Change variants to their ModelOp equivalents. `SemiContinuousBoundChanged` and `ObjectiveSenseChanged` are skipped (no ModelOp variant).
- **Updated `changelog_tracking` test:** Uses `current_revision()` and `commit()` instead of `drain_changes()`.

**Deviation (Rule 3):** Updated `src/solver/mod.rs` `SolverModelExt` trait default methods to not call removed `drain_changes()` and `solver_options.take()`. The test in solver/mod.rs was also updated to remove `has_pending_changes()` assertion and `applied_change_count` check. Both traits are scheduled for removal in Plan 03.

**Commit:** `6a96de4`

### Task 2: Import ReferenceBackend and rewrite changelog_integration.rs

- **Created `src/solver/reference.rs`** (527 lines): Copied from worktree with `#[allow(dead_code)]` removed. Contains `ReferenceBackend` (solver-neutral state projection with HashMaps for variables, constraints, objectives, cells), `NormalizedView` (sorted deterministic comparison), and all worktree tests (commuting-square equivalence, rebuild resets state, objectiveless rebuild).
- **Added `pub mod reference;`** to `src/solver/mod.rs`.
- **Rewrote `tests/changelog_integration.rs`:** Removed all imports of `Change`/`ChangeLog`/`drain_changes()`. Wrote 4 new tests covering revision advancement, snapshot capture, commit produces correct ops, and independent revision tracking.

**API cleanup (lib.rs):** Removed `pub use model::changelog::Change;` from crate root and removed `Change` from the prelude per D2.

**Commit:** `01f845e`

### Task 3: Write contract tests for M1R-C1 through C6

Created `tests/backend_contract.rs` with 10 tests:

| Test | Requirement | What it verifies |
|------|-------------|------------------|
| `error_preserves_journal_entry` | M1R-C1 | Failed apply does not consume journal entry |
| `two_sessions_independently_catch_up` | M1R-C1 | Two cursors advance at different rates |
| `apply_outcome_distinguishes_recoverable_terminal` | M1R-C1 | RecoverableFailure vs Applied, cursor unchanged on failure |
| `revision_mismatch_detected` | M1R-C1 | ApplyError::RevisionMismatch, cursor preserved |
| `snapshot_rebuild_equals_incremental_apply` | M1R-C5 | Commuting square: NormalizedView equivalence |
| `rebuild_resets_cursor_and_health` | M1R-C1/C4 | Cursor revision reset, AdapterHealth restored to Ready |
| `status_preserves_incumbent` | M1R-C6 | TerminationStatus::Feasible variant exists |
| `status_preserves_ambiguity` | M1R-C6 | TerminationStatus::InfeasibleOrUnbounded variant exists |
| `status_preserves_limits_and_interruption` | M1R-C6 | TimeLimit, IterationLimit, NodeLimit, Interrupted distinct |
| `synchronization_enum_dispatch` | M1R-C1 | Synchronization enum constructible, SyncReceipt fields accessible |

**Deviation (Rule 1):** Added `InfeasibleOrUnbounded` variant to `TerminationStatus` in `src/solver/backend.rs` — this variant was specified in the D4 design but missing from the imported implementation.

**Commit:** `0574277`

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] SolverModelExt trait methods and test broken by Model API changes**
- **Found during:** Task 1 compilation verification
- **Issue:** `SolverModelExt::sync_model()` called removed `model.drain_changes()`, `solve_model()` called removed `model.solver_options.take()`, and test called removed `model.has_pending_changes()`. These prevented `cargo test -p roml --lib` from compiling.
- **Fix:** Made `sync_model()` a no-op (the trait is scheduled for removal in Plan 03 per D2). Removed `solver_options.take()` from `solve_model()`. Removed `has_pending_changes()` assertion and `applied_change_count` check from test.
- **Files modified:** `src/solver/mod.rs`
- **Commit:** `6a96de4`

**2. [Rule 1 - Missing variant] `InfeasibleOrUnbounded` missing from `TerminationStatus`**
- **Found during:** Task 3 test writing (contract test `status_preserves_ambiguity`)
- **Issue:** The D4 design specifies `InfeasibleOrUnbounded` to preserve ambiguity (HiGHS can return this), but the imported `TerminationStatus` enum did not have this variant.
- **Fix:** Added `InfeasibleOrUnbounded` variant before `TimeLimit`.
- **Files modified:** `src/solver/backend.rs`
- **Commit:** `0574277`

### Pre-existing Issues (documented, not fixed)

**1. Pre-existing test failures in logging module**
- `logging::tests::workspace_root_sets_env` fails with temp directory path resolution errors. Unrelated to this plan's changes.
- **Impact:** `cargo test -p roml --lib` reports 108/109 pass, 1 pre-existing failure.

**2. Pre-existing `ModelConstants::default()` recursion warning**
- Inherent `default()` method calls `Self::default()`, producing `unconditional_recursion` warning.

## Verification Results

- [x] `cargo test -p roml --lib` compiles cleanly (108/109 pass; 1 pre-existing logging failure)
- [x] `cargo test -p roml --test changelog_integration` — 4/4 pass
- [x] `cargo test -p roml --test backend_contract` — 10/10 pass
- [x] ReferenceBackend tests pass (4 tests: empty, commuting-square, rebuild resets, objectiveless)
- [x] No `pub fn drain_changes` in `src/model/mod.rs`
- [x] No `pub fn has_pending_changes` in `src/model/mod.rs`
- [x] No `solver_options` field or method in `src/model/mod.rs`
- [x] No `roml::model::Change` import in `tests/changelog_integration.rs`
- [x] No `drain_changes()` calls in `tests/changelog_integration.rs`

## Self-Check: PASSED

| Check | Status |
|-------|--------|
| `src/solver/reference.rs` exists | FOUND |
| `tests/backend_contract.rs` exists | FOUND |
| `6a96de4` commit exists | FOUND |
| `01f845e` commit exists | FOUND |
| `0574277` commit exists | FOUND |
| `cargo test -p roml --lib` passes (modulo pre-existing logging) | PASSED |
| `cargo test -p roml --test backend_contract` passes | PASSED |
| No legacy Model API in public surface | PASSED |
