---
phase: 30-soft-constraints
reviewed: 2026-08-14T16:22:59Z
depth: deep
files_reviewed: 28
files_reviewed_list:
  - roml-highs/tests/soft_constraints_differential.rs
  - src/advanced.rs
  - src/compiler/bridge/mod.rs
  - src/compiler/bridge/soft_constraint.rs
  - src/compiler/capability.rs
  - src/compiler/origin.rs
  - src/compiler/session.rs
  - src/construct/mod.rs
  - src/construct/soft_constraint.rs
  - src/lib.rs
  - src/model/mod.rs
  - src/solution/mod.rs
  - src/solver/facade.rs
  - src/solver/mod.rs
  - src/solver/overlay.rs
  - src/solver/reference.rs
  - src/solver/relaxation.rs
  - tests/feasibility_relaxation.rs
  - tests/feasibility_relaxation_faults.rs
  - tests/feasibility_relaxation_p29.rs
  - tests/relaxation_provider_policy.rs
  - tests/soft_constraint_solution.rs
  - tests/soft_constraints_algebra.rs
  - tests/soft_constraints_contract.rs
  - tests/soft_constraints_lifecycle.rs
  - tests/soft_constraints_origins.rs
  - tests/soft_constraints_qualification.rs
  - tests/support/soft_constraints_reference.rs
findings:
  critical: 3
  warning: 6
  info: 0
  total: 9
status: issues_found
---

# Phase 30: Code Review Report

**Reviewed:** 2026-08-14T16:22:59Z  
**Depth:** deep  
**Files Reviewed:** 28  
**Status:** issues_found — P30 is NOT ACCEPTED. P0/P1 findings remain.

## Summary

The review covered commits `5fe8f5b..a99cf37`, the P30 governing/context/plan/summary documents, and the P30 evidence ledger. The targeted P30 core suites and `cargo fmt --all -- --check` pass, but the implementation has three ship-blocking correctness/integration defects and four P1 lifecycle or verification defects. In particular, persistent softening does not relax the original compiled constraint, and the actual HiGHS capability/overlay surface cannot execute the claimed P30 workflows.

## Critical Issues

### P0-01: Persistent softening leaves the original constraint hard

**Classification:** P0 (BLOCKER)  
**File:** `src/compiler/session.rs:392-415`; `src/compiler/bridge/soft_constraint.rs:118-149`

**Issue:** Snapshot compilation always emits the original primitive row with its original finite bounds. The soft-constraint bridge then adds `f(x) + v_lo >= l` and/or `f(x) - v_up <= u`, but never widens/removes/replaces the original hard side. For an original `f(x) >= l`, both rows require `f(x) >= l`, so `v_lo` is forced to zero; the same applies to an upper side. The penalty objective therefore cannot buy a violation, making the canonical softening feature functionally hard rather than soft. This violates SM-10.1/SM-10.2 and the persistent softening invariant.

**Fix:** Compile each softened finite side as a transformed relaxed row, or widen/remove that side on the original row before adding the signed violation row. Add an end-to-end solve test where the hard model is infeasible but the softened model returns a positive violation and the expected penalty.

### P0-02: HiGHS never advertises the required persistent soft-constraint capability

**Classification:** P0 (BLOCKER)  
**File:** `roml-highs/src/session.rs:564-573,629-641`; `src/compiler/bridge/soft_constraint.rs:36-41`; `roml-highs/tests/soft_constraints_differential.rs:18-25`

**Issue:** `soft_constraint::compile` requires `BackendFeature::SoftConstraint`, but `BRIDGE_SUPPORTED_M3_FEATURES` omits that feature. Consequently `highs_capability_set()` returns no support for P30 persistent soft constraints, and a real `HighsSession` rejects every such model at compilation. The qualification test avoids the real capability path by constructing a synthetic capability set and manually inserting `SoftConstraint`; it does not test `highs_capability_set()` or a real HiGHS session.

**Fix:** Add `BackendFeature::SoftConstraint` to the authoritative HiGHS bridge capability declaration only with the documented exact-row qualification, and test the real `highs_capability_set()` through an actual HiGHS compile/solve path. If the capability is intentionally unsupported, the P30 claim and public workflow must reject it explicitly rather than claiming HiGHS qualification.

### P0-03: HiGHS cannot apply the portable relaxation overlay it receives

**Classification:** P0 (BLOCKER)  
**File:** `src/solver/relaxation.rs:604-623,909-933`; `roml-highs/src/session.rs:846-958`

**Issue:** The portable P30 overlay emits `AddTemporaryVariable` for every violation variable and `AddTemporaryObjective` for the weighted-L1 objective. The HiGHS apply loop implements only temporary bounds, temporary rows, objective policy, and rollback-only row removal; the wildcard arm rejects the new variable/objective operations as unsupported. Therefore the default `PortableOnly` repair path fails for HiGHS before solving, despite P30 evidence claiming a HiGHS matrix pass.

**Fix:** Implement and test native HiGHS projection for temporary columns and the temporary objective, including ID/index mapping, objective coefficients, and exact rollback restoration. The qualification test must call the actual adapter and execute a repair, not only compile with manually injected capabilities.

## Warnings

### P1-01: Soft-constraint bridge dependencies omit the original constraint variables and coefficient parameters

**Classification:** P1 (BLOCKER)  
**File:** `src/construct/mod.rs:290-344`; `src/construct/soft_constraint.rs:143-148`; `src/compiler/bridge/soft_constraint.rs:83-85,154-184`; `src/compiler/session.rs:978-1008,1221-1233`

**Issue:** `derive_variable_dependencies` returns an empty list for `SoftConstraint`, while the bridge reads every original constraint coefficient. The payload dependency derivation records only the penalty weight parameters. A later coefficient-cell update therefore emits a `SetLinearCoefficient` for the original row, but leaves the generated violation row with the old coefficient; a parameterized coefficient update follows the same incremental path. Incremental projection can consequently diverge from a fresh snapshot rebuild, violating the canonical revision/incremental equivalence invariant. Removing a referenced variable is also not caught by the construct dependency guard.

**Fix:** Include all variables and parameter expressions used by the referenced original constraint in the soft construct dependency graph, or conservatively return `RebuildRequired` for every affected constraint coefficient/parameter/variable delta. Add differential tests comparing incremental and full rebuild projections after coefficient, parameter, and variable lifecycle changes.

### P1-02: Relaxation reports trust incomplete or invalid candidates and substitute zero values

**Classification:** P1 (BLOCKER)  
**File:** `src/solver/facade.rs:1050-1080`; `src/solver/relaxation.rs:725-828`; `roml-highs/src/solution.rs:259-272`

**Issue:** For `Optimal` or `Feasible`, the façade checks only that `result.solution` exists. `report_members` silently maps every missing variable to `0.0` at line 738, then computes violations from that fabricated assignment. HiGHS also drops non-finite column values while building `variable_values`, creating exactly the incomplete candidate shape the façade accepts. No validation proves that all required user variables are present and finite, that the candidate satisfies the base and temporary rows/domains, or that the reported objective is finite and agrees with the computed weighted-L1 total. A malformed backend/FFI result can therefore be returned as `OptimalRepair` or `FeasibleRepair` with false evidence.

**Fix:** Make extraction/reporting fail on missing or non-finite required values; validate all user-variable domains, base constraints, overlay constraints, and the objective/evidence relationship before classifying an outcome. Never default a missing solver value to zero.

### P1-03: Apply failures do not preserve cleanup/error composition when native mutation may have begun

**Classification:** P1 (BLOCKER)  
**File:** `src/solver/facade.rs:1007-1012,1529-1539`

**Issue:** If `backend.apply_overlay` returns an error, the façade immediately returns a stringified `Backend` error after resetting only its compiler. It has no receipt or partial-apply state with which to attempt rollback, and it does not preserve the backend's error category/health effect in a `Cleanup` result. The shared overlay contract requires primary and cleanup failures to be retained and requires a known rebuild boundary when restoration cannot be proven. The existing fault test injects a failure before delegating to the backend (`tests/feasibility_relaxation_faults.rs:141-153`), so it does not cover a partial native apply.

**Fix:** Make overlay application transactional or return an apply-failure receipt/state that permits rollback. On uncertain/partial apply, mark the backend `RequiresRebuild`, attempt cleanup or deterministic rebuild, and return a composite error retaining both the primary and cleanup/rebuild evidence.

### P1-04: Solution violation accessors fabricate values and ignore compilation identity

**Classification:** P1 (BLOCKER)  
**File:** `src/solution/mod.rs:250-289,325-338`; `src/solution/metadata.rs:46-60`

**Issue:** `SolveMetadata` carries an exact `compilation_id`, but `validate_model_identity` checks only model instance and revision. The violation accessor evaluates through `value_or_zero`, so an incomplete or stale solve result can be reported against the current model as though missing variables were zero. This is especially unsafe for overlay/P30 results, where compilation identity is the mechanism that distinguishes the exact solved state. The public `ViolationError` has no typed missing-value or compilation-mismatch path.

**Fix:** Add a strict, typed validation path for solver-derived/P30 solutions that checks the expected compilation and overlay identity, rejects incompatible synthetic/stale solutions as appropriate, and returns a missing-value error instead of evaluating absent variables as zero. Keep the explicitly documented permissive `value_or_zero` helper separate from correctness-critical violation calculation.

## Info

### P2-01: Temporary variable/objective provenance is weaker than temporary-row provenance

**Classification:** P2 (WARNING)  
**File:** `src/solver/overlay.rs:260-300,325-345`; `src/solver/relaxation.rs:604-610`

**Issue:** Overlay validation requires temporary rows to carry a `SolveOverlay` origin for the exact overlay, but temporary variables and objectives only require that some origin exists. A variable/objective attributed to a user entity or another overlay can pass validation. The relaxation objective is also assigned `GeneratedRole::FeasibilityRelaxationViolationRow`, a row role rather than an objective-specific role, weakening origin audits.

**Fix:** Require exact `EntityOrigin::SolveOverlay { overlay: self.overlay_id, .. }` provenance for temporary variables and objectives, and add a dedicated objective role (or an explicit documented relaxation-objective role) with tests for cross-overlay and wrong-kind origins.

### P2-02: Required acceptance and qualification tests do not exercise the claimed behaviors

**Classification:** P2 (WARNING)  
**File:** `tests/feasibility_relaxation.rs:253-273`; `roml-highs/tests/soft_constraints_differential.rs:8-35`; `docs/release/evidence/P30_SOFT_CONSTRAINTS_RELAXATION.md:15-29`

**Issue:** `feasible_acceptance_is_not_promoted_to_optimality` only changes a plan field and asserts enum equality; it never produces an unproven feasible backend result. The HiGHS “differential” test compiles with manually supplied capabilities and never runs a real adapter solve. The evidence ledger therefore overstates coverage when it records the P30 outcome/provider and HiGHS qualification surfaces as pass.

**Fix:** Add a controlled backend result with `Feasible` termination and no optimality proof, assert `AcceptFeasible` versus `RequireOptimal`, and run persistent softening plus portable repair through `HighsSession` using `highs_capability_set()`. Update the evidence ledger only after those tests pass.

---

_Reviewed: 2026-08-14T16:22:59Z_  
_Reviewer: the agent (independent P30 review)_  
_Depth: deep_
