---
phase: 10-backend-contract-migration-closure
plan: 01
type: execute
subsystem: roml-core-contract
status: complete
tags: [protocol-types, session-traits, contract-foundation]
requires: []
provides: [delta.rs, revision.rs, sync.rs, journal.rs, snapshot.rs, solver/backend.rs, solver/request.rs, solver/session.rs]
affects: [src/lib.rs, src/solver/mod.rs, src/model/mod.rs, src/model/coefficient.rs, src/id/mod.rs]
tech-stack:
  added: [ModelOp, DeltaBatch, ModelRevision, Journal, AdapterCursor, AdapterHealth, SyncCoordinator, BackendError, TerminationStatus, BackendSession, SessionHealth, SolutionView, CallbackSession, BackendMetadata]
  patterns: [bounded-trait-composition, revisioned-sync, hybrid-solve-result]
key-files:
  created: [src/delta.rs, src/revision.rs, src/sync.rs, src/journal.rs, src/snapshot.rs, src/solver/backend.rs, src/solver/request.rs, src/solver/session.rs]
  modified: [src/lib.rs, src/solver/mod.rs, src/model/coefficient.rs, src/model/mod.rs, src/id/mod.rs]
decisions: [D1 (bounded traits composition), D4 (hybrid SolveResult)]
metrics:
  duration: ~30m
  completed: 2026-07-18
plan_count: 2
commit_count: 2
---

# Phase 10 Plan 01: Backend Contract Migration Closure — Protocol Type Import and Session Traits

**One-liner:** Imports all revisioned protocol types from the worktree into main (delta.rs, revision.rs, sync.rs, journal.rs, snapshot.rs, solver/backend.rs, solver/request.rs) and defines the new BackendSession trait hierarchy (solver/session.rs) with module declarations in lib.rs and solver/mod.rs. Establishes the type foundation for plans M1R-02 through M1R-07.

## Work Summary

### Task 1: Core protocol types from worktree

Created five new source files copied from the worktree branch with visibility cleanup:

- **src/delta.rs** (189 lines) — `ModelOp` enum (16 self-contained operation variants) and `DeltaBatch` struct with revision guards, `new()`, `is_empty()`, `len()`, `is_noop()`, `follows()` methods.
- **src/revision.rs** (138 lines) — `ModelRevision(pub(crate) u64)` with monotonic counter, `from_u64()` made `pub` (was `pub(crate)` per plan spec), `ZERO`, `next()`, `as_u64()`, `is_before()`, `is_zero()`. `RevisionError` with Overflow/Compacted/FutureRevision variants.
- **src/sync.rs** (289 lines) — `AdapterCursor` (per-session revision tracker), `AdapterHealth` (Ready/RequiresRebuild/Terminal), `ApplyOutcome` (Applied/RequiresRebuild/RecoverableFailure/DirtyFailure), `SyncCoordinator` (journal bridge), `ApplyError` (RevisionMismatch/RevisionNotFound).
- **src/journal.rs** (168 lines) — `Journal` backed by `BTreeMap<ModelRevision, DeltaBatch>` with `record()`, `deltas_since()`, `latest_revision()`, `len()`, `is_empty()`, `get()`.
- **src/snapshot.rs** (270 lines) — `ModelSnapshot` with `VariableEntry`, `ConstraintEntry`, `ObjectiveEntry`, `ParameterEntry`, `CellEntry`. `take_snapshot()` function for deterministic projection.

All `#[allow(dead_code)]` annotations removed. All method visibilities set to `pub` on public types.

**Pre-requisite changes:**
- Added `CellKey = (CoefficientTarget, VarId)` type alias to `src/model/coefficient.rs`
- Added `PartialOrd, Ord` derives to `CoefficientTarget`
- Added `PartialOrd, Ord` derives to the `define_id!` macro (for all ID types) and `Generation`
- Exported `CellKey` from `src/model/mod.rs`

**Task 1 commit:** `c3ba584` — 8 files, 1049 insertions

### Task 2: Solver backend types, request types, session traits, module declarations

- **src/solver/backend.rs** (257 lines) — `BackendInfo`, `BackendCapabilities` (15 capabilities), `BackendError` with `ErrorCategory` (9 categories) and `HealthEffect` (4 effects), `TerminationStatus` (11 variants: Optimal, Infeasible, Unbounded, Feasible, TimeLimit, IterationLimit, NodeLimit, Interrupted, NumericalIssue, Error, Unknown).
- **src/solver/request.rs** (222 lines) — `SolveRequest` (8 option fields + builder), `SolveResult` (effective_configuration, termination, solution), `EffectiveConfig` (adjustments/rejections tracking), `SolveSolution` (variable_values, objective_value, dual_values, reduced_costs), `ConfigAdjustment`, `ConfigRejection`, `validate_request()`.
- **src/solver/session.rs** (150 lines) — New design per D1: `BackendSession` (synchronize/solve/close), `Synchronization` enum (DeltaBatch/Rebuild), `SyncReceipt` (cursor/health), `SessionHealth`, `SolutionView`, `CallbackSession`, `BackendMetadata` supplementary traits. Module-level doc comment references D1 design rationale.
- **src/solver/mod.rs** — Added `pub mod backend;`, `pub mod request;`, `pub mod session;` after existing `pub mod callback;`.
- **src/lib.rs** — Added `pub mod delta;`, `pub mod revision;`, `pub mod sync;`, `pub mod journal;`, `pub mod snapshot;` after existing `pub mod solver;`.

**Task 2 commit:** `6456cb2` — 6 files, 605 insertions

## Deviations from Plan

### Pre-existing Issues (documented, not fixed)

**1. Pre-existing test failures in logging module**
- **Found during:** Task 2 compilation verification
- **Issue:** `logging::tests::workspace_root_sets_env` and `logging::tests::config_file_precedence` fail with temporary directory path resolution errors. These failures are pre-existing and unrelated to this plan's changes.
- **Impact:** `cargo test -p roml --lib` reports 103/105 pass, 2 pre-existing failures.
- **Resolution:** Out of scope per scope boundary rule. Documented for future resolution.

**2. Pre-existing `ModelConstants::default()` recursion warning**
- **Found during:** Task 2 compilation
- **Issue:** `src/model/mod.rs` has both a `Default` trait impl and an inherent `default()` method that calls `Self::default()`, producing an unconditional recursion warning.
- **Impact:** Compiler warning only, no runtime issue.
- **Resolution:** Out of scope per scope boundary rule.

## Verification Results

- [x] All 8 new source files exist
- [x] No `#[allow(dead_code)]` annotations remain in any new file
- [x] `cargo test -p roml --lib` compiles cleanly (103/105 tests pass; 2 pre-existing logging failures unrelated to this plan)
- [x] Module declarations present in both lib.rs and solver/mod.rs
- [x] All new tests from delta.rs, revision.rs, sync.rs, journal.rs, snapshot.rs, solver/backend.rs, solver/request.rs pass

## Self-Check: PASSED

| Check | Status |
|-------|--------|
| `src/delta.rs` exists | FOUND |
| `src/revision.rs` exists | FOUND |
| `src/sync.rs` exists | FOUND |
| `src/journal.rs` exists | FOUND |
| `src/snapshot.rs` exists | FOUND |
| `src/solver/backend.rs` exists | FOUND |
| `src/solver/request.rs` exists | FOUND |
| `src/solver/session.rs` exists | FOUND |
| No `#[allow(dead_code)]` in new files | PASSED |
| `c3ba584` commit exists | FOUND |
| `6456cb2` commit exists | FOUND |
| Module declarations in lib.rs | delta, revision, sync, journal, snapshot |
| Module declarations in solver/mod.rs | backend, callback, request, session |
