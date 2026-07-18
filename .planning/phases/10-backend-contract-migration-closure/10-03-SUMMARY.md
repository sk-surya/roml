---
phase: 10-backend-contract-migration-closure
plan: 03
type: execute
state: complete
completed_date: 2026-07-18
duration: "~10 min"
technology_added:
  - ADR process (.planning/adr/ADR-001)
files_created:
  - .planning/adr/ADR-001-backend-contract-freeze.md
files_modified:
  - src/solver/mod.rs
  - src/solver/request.rs
  - src/solver/backend.rs
  - src/solution/mod.rs
  - src/model/mod.rs
  - src/lib.rs
  - .planning/STATE.md
key_decisions:
  - D2: Remove all legacy solver APIs entirely (SolverAdapter, SolverModelExt, SolverStatus, SolveOptions, LpAlgorithm)
  - D5: Lightweight ADR-001 at contract freeze commit SHA
status: complete
---

# Phase 10 Plan 03: Backend Contract Migration Closure Summary

**One-liner:** Removed SolverAdapter/SolverModelExt/SolverStatus/SolveOptions/LpAlgorithm from public surface, migrated Solution to TerminationStatus, updated lib.rs re-exports and prelude for new protocol types, froze contract with ADR-001.

## Tasks

| # | Name | Commit | Key Files |
|---|------|--------|-----------|
| 1 | Remove legacy SolverAdapter/SolverModelExt/SolverStatus/LpAlgorithm/SolveOptions from solver/mod.rs | eb98a9a | src/solver/mod.rs, src/solver/request.rs |
| 2 | Migrate Solution to TerminationStatus, update lib.rs/prelude, verify no ignored tests, write ADR | e2d6246, bf3ba70, 036de35 | src/solution/mod.rs, src/model/mod.rs, src/lib.rs, src/solver/backend.rs, .planning/adr/ADR-001-backend-contract-freeze.md, .planning/STATE.md |

## What Was Done

### Task 1: Legacy type removal

- Removed `SolverAdapter` trait (13 methods), `SolverModelExt` trait and blanket impl, `SolverStatus` enum, `SolveOptions` struct, and the `#[cfg(test)]` test block with `MockAdapter` from `src/solver/mod.rs`.
- Preserved `SolverError` unchanged (same 3 variants, same Display/Error impls).
- Moved `LpAlgorithm` enum definition to `src/solver/request.rs` (conceptually part of solve configuration), with a `pub use request::LpAlgorithm;` re-export from `solver/mod.rs`.
- Updated module doc comment to reflect backend session contract types.

### Task 2: Solution migration and contract freeze

- **2a:** Migrated `src/solution/mod.rs` from `SolverStatus` to `TerminationStatus` throughout: `Solution::status`, `Solution::new()`, `Solution::from_values()`, `Solution::status()`, `SolutionBuilder`, all inline tests.
- **2b:** Updated `src/model/mod.rs` inline tests from `SolverStatus::Optimal` to `TerminationStatus::Optimal`.
- **2c:** Rewrote `src/lib.rs` re-exports: removed `SolveOptions`, `SolverAdapter`, `SolverModelExt`, `SolverStatus`; added `BackendCapabilities`, `BackendError`, `ErrorCategory`, `HealthEffect`, `TerminationStatus`, `ConfigAdjustment`, `ConfigRejection`, `EffectiveConfig`, `SolveRequest`, `SolveResult`, `SolveSolution`, `BackendMetadata`, `BackendSession`, `CallbackSession`, `SessionHealth`, `SolutionView`, `SyncReceipt`, `Synchronization`, `AdapterCursor`, `AdapterHealth`, `ApplyOutcome`. Updated prelude accordingly.
- **2d:** Verified zero `#[ignore]` annotations in `tests/` and `src/`.
- **2e:** Full verification: tests pass (107/108 lib test suite, all integration tests), clippy and doc build have pre-existing issues only.
- **2f:** Wrote `.planning/adr/ADR-001-backend-contract-freeze.md` freezing BackendSession, SessionHealth, SolutionView, CallbackSession, BackendMetadata traits and all protocol types at commit `bf3ba70`.
- **2g:** Updated STATE.md: M1R-01 marked Complete with freeze SHA and ADR-001 reference; performance metrics; session info.

## Verification Results

| Check | Status | Details |
|-------|--------|---------|
| `cargo test -p roml --lib` | PASS (107/108) | One pre-existing logging test failure (temp env path assertion) |
| `cargo test -p roml --test backend_contract` | PASS (10/10) | All contract tests pass |
| `cargo test -p roml --test changelog_integration` | PASS (4/4) | All changelog tests pass |
| `cargo test -p roml --test macro_api` | PASS (4/4) | All macro tests pass |
| `cargo clippy -p roml --all-targets -- -D warnings` | FAIL | Pre-existing clippy issues (see below) |
| `cargo doc -p roml --no-deps -D warnings` | FAIL | Pre-existing doc issues (see below) |
| `cargo semver-checks check-release -p roml` | SKIP | Not installed; acceptable for v0.1 |
| `SolverAdapter` removed from solver/mod.rs | PASS | grep returns 0 matches |
| `SolverModelExt` removed from solver/mod.rs | PASS | grep returns 0 matches |
| `SolverStatus` removed from solver/mod.rs | PASS | grep returns 0 matches |
| `pub enum LpAlgorithm` removed from solver/mod.rs | PASS | grep returns 0 matches (re-export preserved) |
| `SolveOptions` removed from solver/mod.rs | PASS | grep returns 0 matches |
| No `#[ignore]` tests | PASS | grep returns exit code 1 |
| ADR-001 exists | PASS | File present with freeze SHA |

## Deviations from Plan

### Rule 1 - Bug: TerminationStatus missing Default derive

- **Found during:** Task 2a compilation
- **Issue:** `SolutionBuilder` derives `Default`, but `TerminationStatus` did not implement `Default`, causing a compile error.
- **Fix:** Added `#[derive(Default)]` to `TerminationStatus` and `#[default]` to the `Unknown` variant in `src/solver/backend.rs`.
- **Files modified:** `src/solver/backend.rs`
- **Commit:** `e2d6246`

### Rule 1 - Bug: Doc comment artifact in solution/mod.rs

- **Found during:** Clippy check
- **Issue:** Stray `//! //!` line in the module doc comment.
- **Fix:** Removed the duplicate doc comment marker.
- **Files modified:** `src/solution/mod.rs`
- **Commit:** `bf3ba70`

## Pre-existing Issues (Out of Scope)

The following issues existed before this plan and are unrelated to our changes:

1. **`src/model/mod.rs:140`** -- `ModelConstants::default()` causes unconditional recursion (clippy: `unconditional_recursion`, `should_implement_trait`)
2. **`src/model/mod.rs:252`** -- Doc comment `[0,1]` interpreted as intra-doc link (rustdoc: `broken_intra_doc_links`)
3. **`src/expr/linear.rs:128`** -- `TermCoeff` interpreted as HTML tag (rustdoc: `invalid_html_tags`)
4. **`src/logging.rs:205,228`** -- `write!()` with format string ending in single newline (clippy: `write_with_newline`)
5. **`src/model/variable.rs:222`** -- `assert!(len() == 0)` should use `is_empty()` (clippy: `len_zero`)
6. **`src/value_expr/mod.rs:496-498`** -- `eval(&get_param)` should be `eval(get_param)` (clippy: `needless_borrows_for_generic_args`)
7. **`src/expr/linear.rs:727,918`** -- needless conversion and borrow (clippy: `useless_conversion`, `needless_borrows_for_generic_args`)
8. **`src/logging.rs:220`** -- `workspace_root_sets_env` test fails due to temp path env var matching (test failure)
9. **`tests/changelog_integration.rs:13`** -- Unused import `ValueExpr` (clippy: `unused_imports`)

## Test Suite Status

| Suite | Pass | Fail | Ignored |
|-------|------|------|---------|
| lib tests | 107 | 1 (pre-existing) | 0 |
| backend_contract integration | 10 | 0 | 0 |
| changelog_integration | 4 | 0 | 0 |
| macro_api | 4 | 0 | 0 |

## Known Stubs

None. All types are fully defined and wired.

## Threat Surface

No new security-relevant surface introduced. Changes are type removals and re-exports only.

## Self-Check

| Check | Status |
|-------|--------|
| src/solver/mod.rs cleaned | PASS |
| src/solution/mod.rs uses TerminationStatus | PASS |
| src/model/mod.rs tests use TerminationStatus | PASS |
| src/lib.rs re-exports updated | PASS |
| src/lib.rs prelude updated | PASS |
| .planning/adr/ADR-001-backend-contract-freeze.md exists | PASS |
| .planning/STATE.md updated | PASS |
