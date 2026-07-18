# Test Fix Report

**Plan:** 09-04
**Branch:** `fix/m1r-00-ignored-tests`
**Fix Commit:** [`629ccd3ba5ec06b1569f8320a2a803e6325223eb`](https://github.com/sk-surya/roml/commit/629ccd3ba5ec06b1569f8320a2a803e6325223eb)
**Date:** 2026-07-18
**Requirement:** M1R-G2
**Predecessor Artifact:** [03-test-classification-contamination.md](03-test-classification-contamination.md)

## Summary

All 11 ignored tests on the inherited candidate branch (`planning/roml-M1-native-backends-release`) have been resolved. **2 tests are now actively running** (P1-1, P1-2). **9 tests are pinned** with `#[ignore]` annotations documenting the M1R-01 resolution gap.

## Per-Test Disposition

| ID | Test Name | File | Fix Category | Status | Ignore Annotation |
|----|-----------|------|--------------|--------|-------------------|
| P1-1 | `duplicate_coefficient_for_same_cell` | `tests/model_characterization.rs` | fix-now | **Fixed** | Removed |
| P1-2 | `duplicate_coefficient_in_objective` | `tests/model_characterization.rs` | fix-now | **Fixed** | Removed |
| P1-3 | `set_semicontinuous_low_lower_emits_change_without_bounds_update` | `tests/model_characterization.rs` | pin-m1r1 | **Pinned (M1R-01)** | `resolved in M1R-01 — drain_changes removal` |
| P1-4 | `solve_options_stored_on_model_and_consumed_during_solve` | `tests/model_characterization.rs` | pin-m1r1 | **Pinned (M1R-01)** | `resolved in M1R-01 — solve policy removal from Model` |
| P2-1 | `drained_changes_are_lost_on_adapter_error` | `tests/sync_characterization.rs` | pin-m1r1 | **Pinned (M1R-01)** | `resolved in M1R-01 — drain_changes removal` |
| P2-2 | `error_during_apply_loses_changes_from_model` | `tests/sync_characterization.rs` | pin-m1r1 | **Pinned (M1R-01)** | `resolved in M1R-01 — drain_changes removal` |
| P2-3 | `two_adapters_cannot_both_sync_same_changes` | `tests/sync_characterization.rs` | pin-m1r1 | **Pinned (M1R-01)** | `resolved in M1R-01 — drain_changes removal` |
| P2-4 | `sync_model_leaves_nothing_for_second_adapter` | `tests/sync_characterization.rs` | pin-m1r1 | **Pinned (M1R-01)** | `resolved in M1R-01 — drain_changes removal` |
| P2-5 | `no_recovery_path_after_partial_apply` | `tests/sync_characterization.rs` | pin-m1r1 | **Pinned (M1R-01)** | `resolved in M1R-01 — drain_changes removal` |
| P2-6 | `reset_has_no_revision_check` | `tests/sync_characterization.rs` | pin-m1r1 | **Pinned (M1R-01)** | `resolved in M1R-01 — drain_changes removal` |
| P2-7 | `no_staleness_detection_after_mutation` | `tests/sync_characterization.rs` | pin-m1r1 | **Pinned (M1R-01)** | `resolved in M1R-01 — drain_changes removal` |

## Summary Statistics

| Metric | Value |
|--------|-------|
| Total ignored tests resolved | 11 |
| Fix-now (active, #[ignore] removed) | 2 (P1-1, P1-2) |
| Pinned for M1R-01 | 9 |
| Stale #[ignore] annotations remaining | **0** |

## Commit History

The fix branch `fix/m1r-00-ignored-tests` contains 3 commits on top of the candidate base:

1. `d10ef71` — `fix(M1R-00): remove #[ignore] and update assertions for P1-1, P1-2`
2. `7cb022f` — `fix(M1R-00): pin P1-3, P1-4 with M1R-01 resolution annotations`
3. `629ccd3` — `fix(M1R-00): pin P2-1 through P2-7 with M1R-01 resolution annotations`

## Verification

- `cargo check -p roml --tests` passes
- `cargo test -p roml --test model_characterization duplicate_coefficient -- --test-threads=1` — 2 passed, 0 failed, 0 ignored
- model_characterization.rs: 2 `#[ignore]` annotations (both with M1R-01 reason)
- sync_characterization.rs: 7 `#[ignore]` annotations (all with M1R-01 drain_changes removal reason)

## Notes

- Branch `fix/m1r-00-ignored-tests` contains the committed changes
- Zero stale `#[ignore]` annotations remain. 2 active tests (P1-1, P1-2). 9 pinned tests with M1R-01 resolution annotations.
- M1R-01 will remove the `#[ignore]` on the 9 pinned tests and verify against the revisioned protocol.
