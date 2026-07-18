# Phase 09 Plan 03: Test Classification and Branch Contamination Analysis

**Date:** 2026-07-18
**Plan:** 09-truth-reset-and-candidate-admission / 03
**Requirement:** M1R-G2, M1R-G3, M1R-G4
**Evidence for:** D2 (Ignored test disposition), D3 (Branch contamination and replay strategy)

---

## Ignored Test Existence Confirmation

All 11 ignored test annotations confirmed on candidate branch `planning/roml-M1-native-backends-release` via:

```
$ git grep -n '#\[ignore' planning/roml-M1-native-backends-release -- tests/
planning/roml-M1-native-backends-release:tests/model_characterization.rs:4://! semantic refactoring (Phase 1). Tests marked with `#[ignore]` document
planning/roml-M1-native-backends-release:tests/model_characterization.rs:545:#[ignore = "P1: last-write-wins coefficient semantics"]
planning/roml-M1-native-backends-release:tests/model_characterization.rs:566:#[ignore = "P1: last-write-wins coefficient semantics"]
planning/roml-M1-native-backends-release:tests/model_characterization.rs:818:#[ignore = "P1: semicontinuous partial apply"]
planning/roml-M1-native-backends-release:tests/model_characterization.rs:842:#[ignore = "P1: solve options should move to solve request"]
planning/roml-M1-native-backends-release:tests/sync_characterization.rs:18://! All tests are marked `#[ignore = "P2: destructive changelog — fixed by revisioned sync"]`
planning/roml-M1-native-backends-release:tests/sync_characterization.rs:185:#[ignore = "P2: destructive changelog — fixed by revisioned sync"]
planning/roml-M1-native-backends-release:tests/sync_characterization.rs:223:#[ignore = "P2: destructive changelog — fixed by revisioned sync"]
planning/roml-M1-native-backends-release:tests/sync_characterization.rs:281:#[ignore = "P2: destructive changelog — fixed by revisioned sync"]
planning/roml-M1-native-backends-release:tests/sync_characterization.rs:313:#[ignore = "P2: destructive changelog — fixed by revisioned sync"]
planning/roml-M1-native-backends-release:tests/sync_characterization.rs:356:#[ignore = "P2: destructive changelog — fixed by revisioned sync"]
planning/roml-M1-native-backends-release:tests/sync_characterization.rs:411:#[ignore = "P2: destructive changelog — fixed by revisioned sync"]
planning/roml-M1-native-backends-release:tests/sync_characterization.rs:459:#[ignore = "P2: destructive changelog — fixed by revisioned sync"]
```

**Count:** 11 test `#[ignore]` annotations (excluding 2 doc-comment references at lines 4 and 18).

---

## Ignored Test Disposition Table

| ID | Test Name | File | Line | Current Behavior | Ignore Reason | Fix Category | Fix Action | Requirement |
|----|-----------|------|------|-----------------|---------------|--------------|------------|-------------|
| P1-1 | `duplicate_coefficient_for_same_cell` | `tests/model_characterization.rs` | 545 | Last-write-wins: two coefficients for same (con, var) cell produce 2 entries in model index and expression. Assertions expect `num_coefficients() == 2` and `num_terms() == 2`. | `P1: last-write-wins coefficient semantics` | **fix-now** | Remove `#[ignore]`. Update assertions to expect 1 combined coefficient. The canonical cell implementation (commit c1fe456) combines duplicate coefficients algebraically. | M1R-C5 |
| P1-2 | `duplicate_coefficient_in_objective` | `tests/model_characterization.rs` | 566 | Same as P1-1 but for objective coefficients: two coefficients for same (objective, var) pair produce 2 entries. | `P1: last-write-wins coefficient semantics` | **fix-now** | Remove `#[ignore]`. Update assertions to expect 1 combined coefficient. Same canonical fix as P1-1. | M1R-C5 |
| P1-3 | `set_semicontinuous_low_lower_emits_change_without_bounds_update` | `tests/model_characterization.rs` | 818 | Semi-continuous lower (3.0) <= current lower (5.0), so bounds unchanged, but a `SemiContinuousBoundChanged` change IS emitted via `drain_changes()`. | `P1: semicontinuous partial apply` | **pin-m1r1** | Update expectation to assert desired post-M1R-01 behavior. Re-mark with `#[ignore = "resolved in M1R-01 — drain_changes removal"]`. The Change-based emission path is being removed by M1R-01; DeltaBatch path will handle correctly. | M1R-H7 |
| P1-4 | `solve_options_stored_on_model_and_consumed_during_solve` | `tests/model_characterization.rs` | 842 | SolveOptions set on Model via `set_solver_options()`. No public getter. Options consumed during solve via `SolverAdapter::apply_options`. | `P1: solve options should move to solve request` | **pin-m1r1** | Update assertion to document expected behavior via SolveRequest path. Re-mark with `#[ignore = "resolved in M1R-01 — solve policy removal from Model"]`. The SolveRequest type exists but `Model.solver_options` field persists — M1R-01 removes it. | M1R-C2 |
| P2-1 | `drained_changes_are_lost_on_adapter_error` | `tests/sync_characterization.rs` | 185 | `drain_changes()` is destructive. After drain, model has no pending changes and a second drain returns empty Vec. Adapter receives drained changes, but if adapter had failed, changes would be lost. | `P2: destructive changelog — fixed by revisioned sync` | **pin-m1r1** | Update expectation to assert desired post-M1R-01 behavior: journal retains batch, retry possible. Re-mark with `#[ignore = "resolved in M1R-01 — drain_changes removal"]`. | M1R-C1, M1R-C4 |
| P2-2 | `error_during_apply_loses_changes_from_model` | `tests/sync_characterization.rs` | 223 | Adapter configured to fail after 2 ops. `sync_model` drains 3 changes, adapter applies 2, then fails. After error: no changes remain on model, adapter partially applied, no way to determine subset. | `P2: destructive changelog — fixed by revisioned sync` | **pin-m1r1** | Same as P2-1. Update expectation to assert journal-based recovery. Re-mark. | M1R-C1, M1R-C4 |
| P2-3 | `two_adapters_cannot_both_sync_same_changes` | `tests/sync_characterization.rs` | 281 | Single-consumer changelog. After adapter A drains and applies 3 changes, adapter B's drain returns empty. B applies 0 changes. | `P2: destructive changelog — fixed by revisioned sync` | **pin-m1r1** | Update expectation: SyncCoordinator supports independent cursors. Both adapters should receive same changes. Re-mark. | M1R-C1, M1R-C4 |
| P2-4 | `sync_model_leaves_nothing_for_second_adapter` | `tests/sync_characterization.rs` | 313 | Same as P2-3 but via `sync_model` convenience method. After adapter A syncs, adapter B gets 0 changes. | `P2: destructive changelog — fixed by revisioned sync` | **pin-m1r1** | Same as P2-3. Update expectation to assert both adapters receive changes via independent cursors. Re-mark. | M1R-C1, M1R-C4 |
| P2-5 | `no_recovery_path_after_partial_apply` | `tests/sync_characterization.rs` | 356 | Partial apply leaves model in undefined state. After adapter fails mid-batch: no recoverable changes on model, adapter partially mutated. Attempted recovery approaches (call sync_model again, reset adapter and sync) all get nothing. | `P2: destructive changelog — fixed by revisioned sync` | **pin-m1r1** | Update expectation: `ApplyOutcome::RequiresRebuild` provides recovery path. Re-mark. | M1R-C1, M1R-C4 |
| P2-6 | `reset_has_no_revision_check` | `tests/sync_characterization.rs` | 411 | After reset, adapter clears state. No way to check if adapter is synchronized with model: no revision counter accessible to adapter, no `is_synchronized` method on SolverAdapter. | `P2: destructive changelog — fixed by revisioned sync` | **pin-m1r1** | Update expectation: `AdapterCursor` tracks `applied_revision`. Re-mark. | M1R-C1, M1R-C4 |
| P2-7 | `no_staleness_detection_after_mutation` | `tests/sync_characterization.rs` | 459 | After adapter syncs (2 changes), model mutates further (adds variable, 1 change). No adapter-aware mechanism signals staleness — caller must re-sync manually. No compile-time or runtime guard against stale reads. | `P2: destructive changelog — fixed by revisioned sync` | **pin-m1r1** | Update expectation: cursor revision comparison detects staleness. Re-mark. | M1R-C1, M1R-C4 |
| **Total** | **11** | | | | | **2 fix-now + 9 pin-m1r1** | | |

### Category Summary

| Category | Count | Tests | Action |
|----------|-------|-------|--------|
| fix-now | 2 | P1-1, P1-2 | Remove `#[ignore]`, update assertion to match canonical cell behavior |
| pin-m1r1 | 9 | P1-3, P1-4, P2-1 through P2-7 | Update expectation to match desired post-M1R-01 behavior, re-mark with `#[ignore = "resolved in M1R-01 — drain_changes removal"]` |

### Requirement Mapping

| Requirement ID | Description | Tests |
|---------------|-------------|-------|
| M1R-C5 | Canonical cell behavior | P1-1, P1-2 |
| M1R-H7 | Semi-continuous recovery | P1-3 |
| M1R-C2 | Solve policy outside Model | P1-4 |
| M1R-C1 | Destructive drain removal | P2-1 through P2-7 |
| M1R-C4 | Health model | P2-1 through P2-7 |

### IMPORTANT: Test File Location

Test files exist only on branch `planning/roml-M1-native-backends-release`, not on this planning branch. Plan 04 (wave 2) uses git worktree to check out the candidate branch, fix the tests, and commit to a feature branch.
