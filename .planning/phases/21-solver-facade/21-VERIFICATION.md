---
phase: 21-solver-facade
verified: 2026-08-02T17:01:25Z
updated: 2026-08-02
status: human_needed
score: 15/15 must-haves verified
behavior_unverified: 0
overrides_applied: 0
human_verification:
  - test: "Independent protocol review approves recovery semantics (plan Gate): terminal failures return without retry, at most one rebuild retry for recoverable/dirty failures, stale results are never reported as current, and solve options do not leak across repeated solves."
    expected: "The protocol review (PR #21) approves the recovery semantics after the review findings are resolved: terminal delta failures no longer trigger a rebuild retry; per-solve option resets prevent option leakage; the SolverSession public surface is limited to new/solve/solve_with; legacy add_integer(Bounds) is fallible."
    why_human: "The phase plan's Gate explicitly requires 'independent protocol review approves recovery semantics' — an automated verifier confirms behavior but cannot supply the independent review itself."
---

# Phase 21: Solver Facade and Unified Result — Verification Report

**Phase Goal:** close the complete build-synchronize-solve-result loop without exposing protocol internals.
**Verified:** 2026-08-02T17:01:25Z
**Updated:** 2026-08-02 (review round 1: 4 findings + 1 flag resolved; protocol review re-review pending)
**Status:** human_needed — automated truths 15/15; plan Gate additionally requires independent protocol review of recovery semantics
**Re-verification:** Yes — re-verified after review fixes at head (terminal no-retry, options reset, 3-method surface, fallible add_integer)

## Goal Achievement

The M2 golden-path loop is closed: `Highs::solve(&mut Model)` (and `solve_with`) perform commit → synchronization → solve → normalization internally, backed by the generic core `SolverSession<B>` orchestration, with one unified `Solution`/`SolveStatus`/`SolveMetadata`/`SolveError` result model. No protocol internals (snapshots, delta batches, cursors, `commit`, `synchronize`) leak into the user surface. Independently re-ran the full verification matrix: fmt clean, clippy `-D warnings` clean on both crates, rustdoc `-D warnings` clean, `roml` all-targets 481 passed / 0 failed, `roml-highs` all-targets 85 passed / 0 failed, roml-highs doctests 2 passed (executing quickstart examples), roml doctests 8 pre-existing ignored.

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Target quickstart compiles AND solves on real HiGHS | ✓ VERIFIED | `roml-highs/tests/target_quickstart.rs#quickstart_compiles_and_runs` (run: 1 passed) + roml-highs doctest quickstart (2 doctests passed) |
| 2 | Parameter updates re-solve through one `Highs` with no user-side sync calls | ✓ VERIFIED | `roml-highs/tests/facade_tests.rs#parameter_delta_second_solve` (12.0→20.0, run: passed); `roml-highs/tests/target_incremental.rs#incremental_compiles_and_runs` (run: passed) |
| 3 | No user-side synchronization calls required | ✓ VERIFIED | Fixtures use only `Highs::new/solve/solve_with`; `Highs` façade exposes exactly `new/solve/solve_with` (`roml-highs/src/facade.rs:49-83`); no sync type imported in any facade test |
| 4 | All existing backend contract tests remain green | ✓ VERIFIED | Full suites re-run: `roml` 481/0, `roml-highs` 85/0; P20 baselines `repeated_session_baseline.rs` (3) and `tests/public_api_compile.rs` (3) all pass |
| 5 | Generic `SolverSession<B>` orchestration in core implements the 9-step algorithm | ✓ VERIFIED | `src/solver/facade.rs:97-263` `solve_with` implements commit-before-mutation (line 129), health/revision inspect, terminal-no-retry, rebuild/delta decision, sequential delta batches, one rebuild retry, post-sync invariant, solve exactly once, normalize (steps 0-9) |
| 6 | Controlled model access to revision batches and snapshots inside core | ✓ VERIFIED | `model.coordinator.batches_for_cursor` (`src/solver/facade.rs:249`) and `model.take_snapshot` (line 233) via `pub(crate) coordinator` (`src/model/mod.rs:141`); roml-highs never accesses model internals |
| 7 | Normalized `Solution`, `SolveStatus`, `SolveMetadata`, `SolveError` | ✓ VERIFIED | `src/solution/metadata.rs` (SolveMetadata: backend_name, model_revision, effective_configuration, synchronization mode), `src/solver/mod.rs:46-71` (SolveStatus, 12 statuses, no wildcard match), `src/solver/error.rs` (SolveError variants) |
| 8 | `roml_highs::Highs` façade | ✓ VERIFIED | `roml-highs/src/facade.rs:49-83` `new() -> Result<Self, HighsError>`, `solve`, `solve_with`; `HighsSession` remains exported (`roml-highs/src/lib.rs:70`) |
| 9 | Automatic commit/delta/rebuild synchronization | ✓ VERIFIED | `bound_delta_second_solve`, `parameter_delta_second_solve` (Delta); `requires_rebuild_health_recovers_via_snapshot_rebuild` (Rebuild); `no_change_second_solve_uses_no_sync` (NoChange); metadata records mode |
| 10 | One-rebuild retry limit | ✓ VERIFIED | `tests/solver_facade.rs#at_most_one_rebuild_retry_when_rebuild_also_fails` (run: passed — exactly 1 rebuild, 0 solves); `#recoverable_delta_failure_triggers_one_rebuild_then_solves` (run: passed) |
| 11 | Stale-solution invalidation | ✓ VERIFIED | `tests/solver_facade.rs#failed_solve_invalidates_prior_solution` (run: passed — stale solution cleared after failed sync); `#prior_solution_invalidated_after_mutation` (run: passed) |
| 12 | Repeated-solve and failure-recovery tests | ✓ VERIFIED | 7 end-to-end facade tests (run: 7 passed), 12 core fault-backend tests (run: 12 passed), 3 baseline tests (run: 3 passed) |
| 13 | API-03.3: mathematical termination → `Ok(Solution)` w/o primal values; operational failure → `Err` | ✓ VERIFIED | `normalize_infeasible_has_no_primal_values`, `normalize_missing_primal_values_yields_empty_values` (facade.rs:364-381,334-359); `normalize_uninterpretable_termination_returns_solve_error` (facade.rs:385-405); `error/unknown_termination_maps_to_solve_error` (status_mapping.rs:152-170) |
| 14 | API-03.5: objective constant included exactly once | ✓ VERIFIED | `objective_constant_appears_exactly_once` (facade.rs:412-461, +5/−5/0; façade == backend == expression eval); `objective_constant_is_reported_exactly_once_end_to_end` (solver_facade.rs:579-602); `objective_constant_delta.rs` (3 tests); real-HiGHS `c9_objective_offset_constant` (contract_tests.rs:1370, 2x+10 → 10.0); projection applies `Highs_changeObjectiveOffset` on rebuild (projection.rs:310) and delta (projection.rs:791-830) |
| 15 | API-02.4: backend errors retain identity, category, health effect | ✓ VERIFIED | `SolveError::from_backend` (error.rs:53-61) wraps whole `BackendError`; `license_backend_error_maps_to_license_solve_error`, `backend_error_maps_to_solve_error_preserving_identity_and_category` (status_mapping.rs:175-203) |

**Score:** 15/15 truths verified (0 present-but-behavior-unverified).

All behavior-dependent truths (retry bound, terminal-no-retry ordering, revision-before-solve invariant, stale invalidation transition, option-validation ordering, constant-once invariant, quickstart/incremental execution) are exercised by passing behavioral tests that were independently re-run.

### Required Artifacts

| Artifact | Expected | Status | Details |
| -------- | -------- | ------ | ------- |
| `src/solver/facade.rs` | `SolverSession<B>` + `normalize_result` | ✓ VERIFIED | Exists, substantive (477 lines), wired (exported `roml::SolverSession`; used by `roml_highs::Highs`) |
| `src/solver/error.rs` | `SolveError` | ✓ VERIFIED | Exists, substantive, wired (root-exported; used by facade + Highs) |
| `src/solver/options.rs` | `SolveOptions` builder API | ✓ VERIFIED | Exists, substantive, wired (validated in `solve_with` before sync) |
| `src/solution/metadata.rs` | `SolveMetadata` + `SynchronizationMode` | ✓ VERIFIED | Exists, substantive, wired (attached in `normalize_result`) |
| `src/solver/mod.rs` | `SolveStatus` + alias | ✓ VERIFIED | Exists, substantive, wired (`from_termination`, exhaustive match) |
| `roml-highs/src/facade.rs` | `Highs` façade | ✓ VERIFIED | Exists, substantive, wired (wraps `SolverSession<HighsSession>`), running rustdoc example |
| `tests/solver_facade.rs` | fault-backend orchestration tests | ✓ VERIFIED | 12 tests, run 12 passed |
| `tests/status_mapping.rs` | exhaustive status mapping | ✓ VERIFIED | 18 tests, run 18 passed |
| `tests/solve_options.rs` | options + validation tests | ✓ VERIFIED | 12 tests, run 12 passed |
| `tests/objective_constant_delta.rs` | delta-path constant propagation | ✓ VERIFIED | 3 tests, run 3 passed |
| `roml-highs/tests/facade_tests.rs` | end-to-end facade tests | ✓ VERIFIED | 7 tests, run 7 passed |
| `roml-highs/tests/target_quickstart.rs` | promoted P20 fixture (compile+run) | ✓ VERIFIED | 1 test, run 1 passed |
| `roml-highs/tests/target_incremental.rs` | promoted P20 fixture (compile+run) | ✓ VERIFIED | 1 test, run 1 passed |

### Key Link Verification

| From | To | Via | Status | Details |
| ---- | --- | --- | ------ | ------- |
| `Highs::solve/solve_with` | `SolverSession<HighsSession>` | delegation (`roml-highs/src/facade.rs:68-82`) | ✓ WIRED | `solve` → `inner.solve`; no duplicated sync logic |
| `SolverSession::solve_with` | `Model` | `model.commit()` → `coordinator.batches_for_cursor` / `take_snapshot` | ✓ WIRED | commit-before-mutation; crate-private coordinator access |
| `SolverSession::solve_with` | `BackendSession` | `synchronize(Rebuild|DeltaBatch)` + `solve(&request)` | ✓ WIRED | call + response handling (`src/solver/facade.rs:196-199, 232-262`) |
| `solve_with` | `normalize_result` | `active_objective` + committed revision + sync mode | ✓ WIRED | result → `Solution` with full metadata (`src/solver/facade.rs:201-212`) |
| `SolveStatus::from_termination` | every `TerminationStatus` variant | exhaustive match, no wildcard | ✓ WIRED | 12 variants explicit (`src/solver/mod.rs:82-104`) |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
| -------- | ------------- | ------ | ------------------ | ------ |
| `Highs` facade → `Solution` | objective_value | real HiGHS `Highs_getObjectiveValue` incl. offset (c9, facade_tests assert 12.0/8.0/20.0) | ✓ real solver output | ✓ FLOWING |
| `Solution::metadata().model_revision` | committed revision | `model.commit()` result propagated through `normalize_result` | ✓ real model state | ✓ FLOWING |
| `Solution::metadata().synchronization` | sync mode | orchestration decision (Delta/Rebuild/NoChange) | ✓ real decision | ✓ FLOWING |
| `Solution::metadata().effective_configuration` | negotiated config | `SolveResult::effective_configuration` copied unchanged | ✓ real negotiation | ✓ FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
| -------- | ------- | ------ | ------ |
| at-most-one rebuild retry | `cargo test -p roml --test solver_facade` (12 tests) | all pass | ✓ PASS |
| terminal-no-retry, revision invariant, stale invalidation | same binary | all pass | ✓ PASS |
| option validation before sync leaves state unchanged | `cargo test -p roml --test solve_options` (12 tests) | all pass | ✓ PASS |
| exhaustive status mapping | `cargo test -p roml --test status_mapping` (18 tests) | all pass | ✓ PASS |
| constant exactly once (delta path) | `cargo test -p roml --test objective_constant_delta` (3 tests) | all pass | ✓ PASS |
| quickstart + incremental execute | `cargo test -p roml-highs --test target_quickstart --test target_incremental` | 1+1 pass | ✓ PASS |
| repeated solve lifecycle on real HiGHS | `cargo test -p roml-highs --test facade_tests` (7 tests) | all pass | ✓ PASS |
| P20 baseline repeated-solve parity | `cargo test -p roml-highs --test repeated_session_baseline` (3 tests) | all pass | ✓ PASS |
| doctests (quickstart examples) | `cargo test -p roml-highs --doc` | 2 pass | ✓ PASS |
| full suites / API-10.1 | `cargo test -p roml --all-targets`; `-p roml-highs --all-targets` | 481/0, 85/0 | ✓ PASS |
| fmt / clippy / rustdoc gates | `cargo fmt --check`; `clippy -D warnings`; `RUSTDOCFLAGS=-D warnings cargo doc` | all clean | ✓ PASS |

### Probe Execution

No probes declared by this phase (plan/summary reference no `probe-*.sh`); behavior is verified by the executed integration/doctest matrix above. N/A.

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
| ----------- | ---------- | ----------- | ------ | -------- |
| API-01.1 | P21 | `Highs::new() -> Result<Highs, HighsError>` | ✓ SATISFIED | `roml-highs/src/facade.rs:55`; used by all facade/target tests |
| API-01.2 | P21 | `solve` performs commit/sync/solve/normalize | ✓ SATISFIED | `SolverSession::solve_with` (`src/solver/facade.rs:120-213`); facade_tests + quickstart |
| API-01.3 | P21 | `solve_with` preserves option negotiation/effective config | ✓ SATISFIED | `roml-highs/src/facade.rs:76-82`; `solve_options.rs#effective_configuration_is_preserved_in_metadata` |
| API-01.4 | P21 | deltas when valid, rebuild when required | ✓ SATISFIED | bound/parameter delta tests; rebuild tests in `tests/solver_facade.rs` |
| API-01.5 | P21 | failed sync/solve never loses ops or reports stale solution | ✓ SATISFIED | `failed_solve_invalidates_prior_solution`, `prior_solution_invalidated_after_mutation` (run: passed) |
| API-02.1 | P21 | generic orchestration in `roml`, not backends | ✓ SATISFIED | `SolverSession<B>` in `src/solver/facade.rs`; roml-highs is a thin wrapper |
| API-02.2 | P21 | operates on frozen session contract; no model internals leak | ✓ SATISFIED | uses `BackendSession + SessionHealth + BackendMetadata`; coordinator access is `pub(crate)` |
| API-02.3 | P21 | at most one automatic rebuild retry | ✓ SATISFIED | `at_most_one_rebuild_retry_when_rebuild_also_fails` (run: passed) |
| API-02.4 | P21 | backend errors retain identity/op/category/health | ✓ SATISFIED | `SolveError::from_backend` + accessors; status_mapping tests (run: passed) |
| API-03.1 | P21 | one `Solution` type | ✓ SATISFIED | `crate::Solution` root/`SolutionBuilder` used by `normalize_result` |
| API-03.2 | P21 | one `SolveStatus` preserving distinctions | ✓ SATISFIED | `SolveStatus` 12 variants; `every_termination_status_maps_to_solve_status_or_error` |
| API-03.3 | P21 | mathematical → Ok(Solution) w/o primal; operational → Err | ✓ SATISFIED | normalize unit tests + status_mapping tests (run: passed) |
| API-03.4 | P21 | values/objective/duals/reduced costs/effective options/metadata/revision | ✓ SATISFIED | `normalize_optimal_result_builds_solution` (facade.rs:302-329) |
| API-03.5 | P21 | objective constants included exactly once | ✓ SATISFIED | 4-layer proof (see truth #14), all tests pass |
| API-10.1 | P20/P21 | existing core and HiGHS suites green | ✓ SATISFIED | 481 + 85 re-run green |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
| ---- | ---- | ------- | -------- | ------ |
| (none) | - | No `TBD`/`FIXME`/`XXX`/`TODO`/`PLACEHOLDER` markers in phase source files | ℹ️ none | - |

Scan covered `src/solver/facade.rs`, `error.rs`, `options.rs`, `src/solution/metadata.rs`, `src/solver/mod.rs`, `roml-highs/src/facade.rs`, `roml-highs/src/projection.rs`. Empty `variable_values: vec![]` occurrences are deliberate API-03.3 test fixtures (no-primal-values), not stubs.

### Human Verification Required

One item — the phase plan's Gate requires it:

1. **Independent protocol review approves recovery semantics.** The plan Gate states "P21 passes when … independent protocol review approves recovery semantics". PR #21 round 1 returned four blocking findings, all now resolved (see below); re-review is pending. Automated truths (15/15) cover the behaviors; the independent sign-off is the human item.

**Review round-1 findings and resolution (recorded 2026-08-02):**

| Finding | Resolution |
|---|---|
| 1. Terminal sync errors were incorrectly retried (any delta error triggered a rebuild, including terminal/license failures) | `solve_with` now returns immediately on `SolveError::is_terminal()` (`src/solver/error.rs`), which covers `License` and backend errors with `HealthEffect::Terminal`; only recoverable/dirty failures get the one rebuild retry. New test `terminal_delta_failure_returns_error_without_rebuild_retry` (rebuilds == 0, solves == 0). |
| 2. Solve options leaked across repeated solves (native HiGHS persists options; a default solve after `solve_with` silently retained the previous settings while metadata said unset) | `negotiate_options` now resets every known option to its HiGHS default before applying the request's explicit values — each request is self-contained (`roml-highs/src/session.rs`). New test `default_solve_after_solve_with_resets_options` (metadata: time limit and threads present after `solve_with`, absent after a default `solve`). |
| 3. `last_solution()`/`backend()`/`backend_mut()` were not part of the approved three-method interface and bypassed stale-result protection | All three removed from the public API; `SolverSession` exposes only `new`/`solve`/`solve_with`. Stale protection is structural: the only way to obtain a solution is `solve`, which always re-synchronizes; error paths never surface a prior solution. Fault tests reworked to shared `Rc<RefCell>` knobs; invalidation tests rewritten to assert fresh returned solutions (`prior_solution_never_reported_after_mutation`, `failed_solve_never_surfaces_prior_solution`). |
| 4. Verification marked passed without the plan-required independent protocol review | This report now records the protocol review as the human_verification item; status `human_needed` until re-review approves. |
| 5. (Flag) Legacy `add_integer(Bounds)` remained infallible, bypassing D10 validation | `Model::add_integer` is now fallible (`Result<VarId, ModelError>`) with bounds validation (invalid/non-finite bounds rejected before mutation); call sites updated. |

### Notes (non-blocking)

1. **Real-HiGHS delta path with a non-zero objective constant is not asserted end-to-end.** `c9_objective_offset_constant` proves the constant-once invariant on the real-HiGHS *rebuild* path (2x+10 → 10.0); the *delta* path constant semantics are proven at the model/journal level (`tests/objective_constant_delta.rs`) and via the core reference-backend end-to-end test (`parameter_delta_second_solve_is_fresh_and_single_counted`, values 6.0/10.0), and the projection applies `Highs_changeObjectiveOffset` on the delta path (`projection.rs:819-830`). The exact-once invariant itself (API-03.5) has a passing 4-layer proof; only the specific HiGHS delta-with-constant combination lacks a direct assertion. Informational only.
2. **Deviation — first-solve sync mode delta-first:** accurate against code. Core `TestBackend` journal retains the r0→r1 chain, so `first_solve_synchronizes_via_delta_and_reports_revision` asserts `Delta`; the real-HiGHS `first_solve_from_new_model` correctly asserts `Delta | Rebuild` and documents that the contract is "synchronized and correct," not a fixed mode.
3. **Deviation — semi-continuous error path:** accurate against code. The projection rejects `SetSemiContinuousBound` in pre-validation on both the delta path (`projection.rs:350,385-388`) and the snapshot path (`projection.rs:80-93`), so `unsupported_model_returns_error_never_stale` surfaces `Err(SolveError::Synchronization)` — never a stale/fabricated result (run: passed).
4. **Promoted fixtures / D7+D10 ahead of slot (per check #6, not failures):** `tests/ui/target_*.rs` were promoted to `roml-highs/tests/` within P21 (commit `5abd813`) and now compile AND execute; the D7/D10 fallible-migration (definition builders `continuous/integer/binary/parameter` with `.named`/`.bounds`, fallible `add_variable`/`add_parameter`, `Model::named`) landed in P21 (commits `e89e3de`, `283ff05`) ahead of its P22 slot because the target contracts needed it. P22 scope narrows accordingly per the SUMMARY.

### Gaps Summary

No gaps. The complete build-synchronize-solve-result loop is closed without exposing protocol internals, the P21 gate is met (quickstart compiles and solves; parameter updates re-solve through one `Highs`; no user-side synchronization calls; all backend contract suites green), and API-01, API-02, API-03 are each mapped to passing behavioral evidence.

---

_Verified: 2026-08-02T17:01:25Z_
_Verifier: Claude (gsd-verifier)_
