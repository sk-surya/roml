---
phase: 27-fixing-locks-overlays
verified: 2026-08-03T17:45:00Z
status: passed
score: 4/4 gate clauses verified (plus 2 supporting must-haves verified)
behavior_unverified: 0
overrides_applied: 0
---

# Phase 27: Persistent Fixing, Assignments, Locks, and Reversible Overlays — Verification Report

**Phase Goal:** support hard solution reuse while protecting canonical history and backend state.
**Verified:** 2026-08-03
**Status:** passed
**Re-verification:** No (initial verification; no prior VERIFICATION.md existed)
**Branch:** `phase-roml-P27-fixing-locks-overlays`
**Requirements:** SM-05 (all), SM-06 (all), SM-07.3–SM-07.6, SM-02.2 (secondary)

## Verdict

The phase goal is achieved. All four ROADMAP P27 gate clauses are implemented, wired, and — critically — behaviorally proven by passing tests that exercise the state transitions and invariants each gate asserts (revision invariance, stale-state rejection before mutation, transactional apply/rollback, and clean-solve-equals-fresh-rebuild after every injected failure). All prior review findings (2 critical, 4 warning, 5 info) are resolved in the commit trail with TDD regression tests, and the re-verification counts (743 roml / 118 roml-highs) reproduce exactly. No M2 regression: `solve`/`solve_with`/`Highs::solve` and the M2 characterization harness (backend_contract 47, differential_harness 33, semicontinuous_recovery 3) are all green.

## Must-Haves Checked with Evidence

### 1. fix/unfix survives rebuild (SM-05; phase gate 1; WR-02 fix) — VERIFIED

| Evidence | Location | Status |
|---|---|---|
| `fixing_survives_snapshot_rebuild` — snapshot carries declared bounds + fixing; rebuild reproduces declared `[0,10]` and effective `[4,4]` | `tests/fixing_assignment.rs:394` | ✓ behavioral test passes |
| WR-02: `SetVariableFixing`/`SetVariableBounds` on an inactive variable force `CompileError::RebuildRequired`, matching `compile_snapshot`'s `[0,0]` fold — delta and rebuild paths agree | `src/compiler/session.rs:564-574, 604-614`; snapshot fold `session.rs:308-318` | ✓ wired |
| `fixing_change_on_inactive_variable_forces_rebuild_required`, `bounds_change_on_inactive_variable_forces_rebuild_required`, `effective_bounds_of_fixed_inactive_variable_fold_to_zero` | `tests/fixing_assignment.rs:565, 625, 671` | ✓ behavioral tests pass |

### 2. Locks never advance model revision (SM-07.3; phase gate 2) — VERIFIED

| Evidence | Location | Status |
|---|---|---|
| `solve_with_overlay_tags_result_with_overlay_compilation_and_restores_base` — after a full overlay solve (fixings + locks + override), `model.current_revision()` unchanged and `!model.has_pending_changes()` | `tests/solve_overlay.rs:1282` | ✓ behavioral test passes |
| `temporary_fixings_and_locks_never_advance_the_model_revision` — compile surface is read-only on the model | `tests/solve_overlay.rs:1024` | ✓ behavioral test passes |
| Overlay ops never emit `Change`/`ModelOp`/revision advance — `compile_overlay` takes `&Model` and `&CompilationSession` | `src/solver/overlay.rs:255` | ✓ structural |

### 3. Exact compilation mismatches reject before mutation (SM-07.5/D28/SM-03.9; phase gate 3) — VERIFIED

| Evidence | Location | Status |
|---|---|---|
| Compile-time stale-base rejection before any op: `compile_overlay_rejects_a_stale_base_compilation`, `compile_overlay_rejects_a_compiler_bound_to_another_model`, `compile_overlay_rejects_invalid_assignment_before_any_op` | `tests/solve_overlay.rs:941, 967, 996` | ✓ behavioral tests pass |
| Apply-time stale rejection before native mutation: `stale_overlay_apply_rejects_before_mutation` (reference) and `highs_stale_overlay_apply_rejects_before_mutation` (HiGHS) | `tests/solve_overlay.rs:1224`; `roml-highs/src/session.rs:2041` | ✓ behavioral tests pass |
| Result validated against the fresh `C_overlay`, not `compiler.current_compilation()` (which stays `C_base`) — `overlay_solve_validates_against_c_overlay_not_c_base`; extraction mismatch → `SolveError::CompilationMismatch` — `injected_extraction_failure_never_leaks` | `tests/solve_overlay.rs:1333, 1688`; facade `src/solver/facade.rs:595` | ✓ behavioral tests pass |

### 4. No overlay leaks after any injected failure (SM-07.6; phase gate 4) — VERIFIED

| Evidence | Location | Status |
|---|---|---|
| Failure-injection matrix at validation/compile/apply/solve/extraction/rollback/verify — every scenario asserts canonical revision invariance AND clean-solve-equals-fresh-rebuild (`run_overlay_scenario` harness) | `tests/solve_overlay.rs:1467, 1514, 1546, 1604, 1675, 1688, 1708, 1723` | ✓ behavioral tests pass |
| CR-02: `injected_mid_apply_failure_forces_rebuild_not_no_sync_reuse` — mid-apply failure leaves session `RequiresRebuild`; next plain solve forces a rebuild (rebuild counter increases) and equals a fresh rebuild; facade also defensively forces rebuild on any apply failure (`force_rebuild_on_next_sync`, `facade.rs:572-578, 661`) | `tests/solve_overlay.rs:1622` | ✓ behavioral test passes |
| HiGHS `apply_overlay` marks the cursor `RequiresRebuild` on every early-return error path (CR-02) and rollback deletes rows / restores bounds via pinned `highs-sys`; rollback receipts are explicit (`OverlayApplyReceipt`/`OverlayRollbackOutcome`), never `Drop`-only | `roml-highs/src/session.rs:647-818, 829-914`; `src/solver/overlay.rs:163-194` | ✓ wired + behavioral |
| IN-03: `test_backend_solve_honors_overlay_bounds_so_leaks_are_detectable` — the matrix's clean-solve assertion actually catches an overlay-bounds leak | `tests/solve_overlay.rs:1577` | ✓ behavioral test passes |
| HiGHS round-trip: apply → solve → rollback → verify proves the native row count and objective return to `C_base`, and a post-rollback solve equals the base solve | `roml-highs/src/session.rs:1925` | ✓ behavioral test passes |

### Supporting must-have: assignment validation (SM-02.2/SM-06.6) — VERIFIED

`PrimalAssignment::validate_for` gates on lineage equality (D4), live entity generation (`StaleVariable` via `variable_domain`), and value/domain compatibility with tolerance-aware integrality — plus the CR-01 finiteness guard (`AssignmentError::NonFiniteValue`, checked BEFORE the range comparison that NaN/±inf defeat). Tests: `validate_for_rejects_independent_lineage`, `validate_for_rejects_stale_generation_variable`, `validate_for_rejects_out_of_domain_value`, `validate_for_rejects_non_integral_value_on_integer_variable`, `validate_for_rejects_non_finite_values`, `overlay_rejects_non_finite_temporary_fixing_and_lock_values`, `clones_at_same_revision_both_validate_an_assignment` (`tests/solve_overlay.rs:147-313`). All pass.

### Supporting must-have: SolveOverlay contract matches the plan's pinned shape — VERIFIED

`SolveOverlay { id, temporary_fixings: BTreeMap<Variable,f64>, locks: Vec<SolutionLock>, objective_locks: Vec<ObjectiveLock>, cutoffs: Vec<ObjectiveCutoff> }` (`src/solver/overlay.rs:36-52`) matches the plan's pinned shape exactly; `OverlayId::allocate` via the checked atomic counter (zero reserved, typed `IdentityOverflow`, `src/compiler/origin.rs:28-50`). `compile_overlay` produces the enumerated mapping: temp fixings → equal `SetTemporaryVariableBounds`; locks → selector-resolved bounds (`Exact` → `[v,v]`, `Within` → band intersected with the declared domain, WR-01); objective locks/cutoffs → `AddTemporaryRow` with `GeneratedRole::ObjectiveLockRow`/`CutoffRow` `SolveOverlay` origins (D5); objective override → `SetObjectivePolicy(CompiledObjectivePolicy::Single)`; a fresh `CompilationId` `C_overlay` distinct from `C_base` (D28). Test: `compile_overlay_produces_the_pinned_mapping_and_origins` + `compile_overlay_objective_override_maps_to_single_objective_policy` (`tests/solve_overlay.rs:779, 894`). All pass.

## Requirement Coverage

| Requirement | Clauses | Status | Evidence |
|---|---|---|---|
| SM-05 Persistent fixing | 1–7 | ✓ SATISFIED | `VariableDomain`+`Option<VariableFixing>` in one record (variable.rs); `Model::fix`/`unfix`/`declared_bounds`/`effective_bounds`/`variable_domain`/`integrality_tolerance` (model/mod.rs:753-823); equal-bound compile fold (compiler/session.rs:310); `unfix` restores current declared bounds (test); `BoundsExcludeFixing` atomicity guard (model/mod.rs:866-873); incremental `SetVariableBounds` under `IncrementalBounds` (compiler/session.rs:588-619); 23 fixing_assignment tests green |
| SM-06 Assignments/reuse | 1–6 | ✓ SATISFIED | `PrimalAssignment` packet (assignment.rs:28-40); `Solution::primal_assignment` + `subset` (solution/mod.rs:125); `SolutionLock`/`LockSelector`(5 variants)/`ContinuousLock`(Exact/Within) distinct public types (assignment.rs:172-209); `validate_for` gating before backend mutation; MipStart/VariableHints documented as distinct P28 types (D8) |
| SM-07.3 Revision invariance | — | ✓ SATISFIED | Revision-invariance tests after full overlay solve |
| SM-07.4 Transactional apply/rollback | — | ✓ SATISFIED | Explicit `OverlayApplyReceipt`/`OverlayRollbackOutcome`; `OverlaySession` trait (session.rs:136-171); rollback always attempted on failure (facade.rs:586, 600, 605); WR-04 staged transactional apply in the reference backend (reference.rs:846-913) |
| SM-07.5 Rollback uncertainty → RequiresRebuild | — | ✓ SATISFIED | `rollback_uncertainty_marks_requires_rebuild_and_forces_rebuild` (tests/solve_overlay.rs:1412); D7/D22 invariant; IN-04 reason surfaced |
| SM-07.6 Failure injection, no leaks | — | ✓ SATISFIED | Full 7-stage failure matrix + leak tests (see gate 4) |
| SM-02.2 (secondary) Lineage + generation validation | — | ✓ mechanism SATISFIED | `validate_for` (assignment.rs:59-104); see residual-risk note 1 for the traceability statement |

## Tests Run

| Command | Result |
|---|---|
| `cargo test -p roml --all-targets` | 0 — **743 passed; 0 failed** across 32 test binaries |
| `cargo test -p roml-highs --all-targets` | 0 — **118 passed; 0 failed** |
| `cargo clippy -p roml --all-targets -- -D warnings` | 0 — clean |
| `cargo clippy -p roml-highs --all-targets -- -D warnings` | 0 — clean |
| `cargo fmt --all -- --check` | 0 — clean |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps` / `-p roml-highs --no-deps` | 0 — clean |
| `cargo test -p roml --test fixing_assignment` | 0 — 23 passed |
| `cargo test -p roml --test solve_overlay` | 0 — 44 passed |
| `cargo test -p roml-highs --lib overlay` | 0 — 3 passed (round trip, stale apply, verify detects wrong bound state) |
| M2 harness (`backend_contract`, `differential_harness`, `semicontinuous_recovery`) | 0 — 47 + 33 + 3 passed |

All verification commands above were run fresh by the verifier against the `phase-roml-P27-fixing-locks-overlays` branch.

## Residual Risks

1. **Traceability statement for SM-02.2 is not literal in TRACEABILITY.md (documentation-level, not functional).** The evidence doc's Task 9 section states the SM-02.2 clause-level closure "will be stated in TRACEABILITY.md", but TRACEABILITY.md's P27 section (`Closes: SM-05, SM-06, SM-07.3–SM-07.6`) does not name SM-02.2; its SM-02 row records only "SM-02.2 foundations only (validation mechanism P27)". The mechanism itself is fully implemented and tested (`validate_for` covers the entire clause text: lineage + entity generations + value/domain). Recommend an explicit SM-02.2 closure line in the P27 section of TRACEABILITY.md.
2. **Objective-lock rows compile with a zero reference optimum in P27** (accepted deviation D9-3): `f(x) <= absolute_tolerance` (min) / `f(x) >= -absolute_tolerance` (max). The real stage optimum `z` and the `relative_tolerance` term arrive in P31 (design §15.2). Within P27 scope, documented, and the tolerance validation (IN-02) is in place.
3. **`Highs_deletionRow` does not exist in pinned `highs-sys` 1.15.0** (accepted deviation D10-1); the HiGHS overlay rollback removes rows via `Highs_deleteRowsBySet`. All overlay native calls go through `roml-highs/src/bindings.rs` (`pub use highs_sys::*` — the sole ABI owner). A separate hand-written `ffi.rs` exists for a limited legacy function set but is not on the overlay path.
4. **WR-03's original repro did not reproduce** (evidence note) — `objective_expression` uses evaluated cells, so the parameterized-coefficient failure was not reachable as reported; the prescribed compiled-base resolution was nonetheless implemented with the `parameterized_objective_compiles_overlay_rows` regression test. No open defect.

---

_Verified: 2026-08-03_
_Verifier: Claude (gsd-verifier)_
