---
phase: 30-soft-constraints
reviewed: 2026-08-14T16:52:50Z
depth: deep
files_reviewed: 16
files_reviewed_list:
  - roml-highs/src/compiler.rs
  - roml-highs/src/session.rs
  - roml-highs/tests/conformance.rs
  - roml-highs/tests/soft_constraints_differential.rs
  - src/compiler/bridge/mod.rs
  - src/compiler/bridge/soft_constraint.rs
  - src/compiler/session.rs
  - src/construct/mod.rs
  - src/solution/mod.rs
  - src/solver/facade.rs
  - src/solver/overlay.rs
  - src/solver/relaxation.rs
  - tests/feasibility_relaxation.rs
  - tests/feasibility_relaxation_faults.rs
  - tests/soft_constraints_algebra.rs
  - tests/soft_constraints_lifecycle.rs
findings:
  critical: 0
  warning: 1
  info: 0
  total: 1
status: issues_found
---

# Phase 30: Code Review Report — Re-review 2

**Reviewed:** 2026-08-14T16:52:50Z  
**Depth:** deep  
**Files Reviewed:** 16  
**Status:** NOT ACCEPTED — one P1 blocker remains.

## Summary

Re-reviewed combined `HEAD eae2788` and fixes `e653e19` and `7144fe5`, against the P30 plan/context and the prior review. The three prior P0 findings are resolved: softened sides are widened before generated rows are added, `highs_capability_set()` advertises the qualified bridge path, and a real `HighsSession` executes and rolls back the portable repair overlay. Prior P1 dependency, partial-apply cleanup, candidate completeness/objective checks, provenance, and strict identity paths are also substantially covered by code and tests.

The implementation is not accepted because persistent-fixing candidate validation disables both declared-bound checks. A malformed backend result can therefore be reported as a valid repair with a variable outside its canonical domain.

## Prior Finding Disposition

- **P0-01:** Resolved. `softened_row_bounds` removes finite sides from the original compiled row; generated signed rows retain the original side bounds. Algebra regression tests pass, including positive upper-side violation feasibility.
- **P0-02:** Resolved. HiGHS declares `SoftConstraint` and `FeasibilityRelaxation` as bridge features, and the real capability/session qualification tests pass.
- **P0-03:** Resolved. `HighsSession` now projects temporary columns, rows, and the temporary objective; the real HiGHS repair test solves to the expected weighted violation and performs a clean rollback twice.
- **P1-01:** Resolved conservatively. Soft-bridge dependencies include the original constraint, referenced variables, and coefficient parameters; affected incremental deltas return `RebuildRequired`, with algebra/lifecycle tests covering coefficient, parameter, and variable changes.
- **P1-02:** Partially resolved; the remaining domain-validation defect is P1-01 below. Missing/non-finite/duplicate/unknown candidates, incomplete active-variable sets, non-integral values, non-relaxed constraints, caps, and objective consistency are now rejected.
- **P1-03:** Resolved for the rebuild contract. Apply failures force backend rebuild health, the façade attempts cleanup with the reconstructed receipt, and primary plus cleanup failures are preserved. Partial-apply fault coverage passes.
- **P1-04:** Resolved through the strict `constraint_violation_with_identity` and `soft_constraint_violation_with_identity` APIs; exact compilation and overlay mismatches are typed and tested. Basic synthetic-solution accessors now reject missing/non-finite expression values rather than substituting zero.
- **P2-01:** Resolved for temporary variable/objective provenance: exact overlay identity and relaxation-specific roles are required and tested.
- **P2-02:** The real HiGHS capability, session synchronization, repair, rollback, and core fault suites now execute; the prior synthetic-only qualification gap is closed.

## Narrative Findings (AI reviewer)

## Warnings

### P1-01: Persistent-fixing candidates bypass declared domain validation

**Classification:** P1 (BLOCKER)  
**File:** `src/solver/relaxation.rs:766-803`

**Issue:** `relax_lower` and `relax_upper` are both set to `true` for `PersistentFixing`. The subsequent check therefore never compares the candidate against `domain.bounds` on either side. Relaxing a persistent fixing may permit deviation from the fixing value, but it must not permit leaving the variable’s declared domain. An untrusted backend/FFI result such as `x = -100` for a variable declared `[0, 10]` and fixed at `5` is accepted as finite candidate data and can be reported as an `OptimalRepair`/`FeasibleRepair`, even though the generated overlay sets the native variable bounds to `[0, 10]`.

**Fix:** Always enforce the declared domain for persistent-fixing candidates, while relaxing only the equality to the fixing value. For example, check `candidate < domain.bounds.lower || candidate > domain.bounds.upper` for `PersistentFixing`; reserve one-sided bound bypasses for the matching `VariableBound` restriction. Add a fault-injected candidate test that supplies an out-of-domain persistent-fixing value and requires a typed numerical error.

## Verification

The following read-only checks passed at the reviewed head:

- `cargo fmt --all -- --check`
- Core P30 algebra, lifecycle, solution, repair, fault, and provider-policy integration tests: all passed.
- Core relaxation/overlay/solution unit tests: all passed.
- `cargo test -p roml-highs --test soft_constraints_differential -- --nocapture`: 3 passed, including real `HighsSession` repair and rollback.

Because the P1 finding above remains, the required verdict is **NOT ACCEPTED**. No production source files were modified; only this review artifact was added.

---

_Reviewed: 2026-08-14T16:52:50Z_  
_Reviewer: the agent (gsd-code-reviewer)_  
_Depth: deep_
