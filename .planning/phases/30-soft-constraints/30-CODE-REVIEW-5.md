---
phase: 30-soft-constraints
reviewed: 2026-08-14T17:26:23Z
depth: deep
files_reviewed: 31
files_reviewed_list:
  - roml-highs/src/compiler.rs
  - roml-highs/src/session.rs
  - roml-highs/tests/conformance.rs
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
  critical: 0
  warning: 2
  info: 0
  total: 2
status: issues_found
---

# Phase 30: Code Review Report

**Reviewed:** 2026-08-14T17:26:23Z  
**Depth:** deep  
**Files Reviewed:** 31  
**Status:** issues_found  
**Verdict:** ACCEPTED — no P0/P1 findings remain; two P2 test-coverage warnings remain.

## Summary

The final review covered the P30 plan/context, review 4, the evidence ledger,
the full P30 Rust implementation/test scope, and HEAD
`ef69c2a5ab99264a4e9600bf895594a5fc75984a`. CR-01 is resolved. Inactive
provider candidates are still checked for finiteness, unknown IDs, and
duplicates through `seen_candidates`, but are excluded from `candidate_values`.
Both active-row validation and relaxed-member evaluation map a known inactive
variable to canonical `0.0`, matching the compiler's inactive `[0, 0]` column.

The new mixed-row regression injects inactive `=2.0` into
`inactive + active >= 2` and reports the required violation of `2.0`; it passes.
No P0 or P1 issue remains after the prior fixes and this CR-01 correction.

## CR-01 Resolution

**File:** `src/solver/relaxation.rs:739-880, 916-950`  
**Result:** RESOLVED

`seen_candidates` preserves duplicate tracking for every provider entry,
including inactive variables. Only active entries are inserted into
`candidate_values`; therefore inactive values cannot satisfy or otherwise
change active constraint expressions. The shared `value` closure returns
`0.0` for known inactive variables and is used by both active-constraint
validation and per-restriction violation calculation. Unknown and duplicate
entries remain typed numerical errors.

The regression at `tests/feasibility_relaxation.rs:288-316` passes, as does the
existing inactive-candidate regression at `:258-285`.

## Prior P0/P1 Dispositions

- P0-01, softened rows left hard: **closed**. The compiler widens finite sides
  of the original row before adding signed violation rows; algebra and
  qualification tests pass.
- P0-02, missing HiGHS P30 capability declaration: **closed**. HiGHS exposes
  the qualified portable bridge capabilities, and the real capability test
  passes.
- P0-03, HiGHS unable to apply portable repair overlays: **closed**. Temporary
  variables, rows, objective projection, and rollback execute through a real
  HiGHS session; the differential suite passes.
- Review 1 P1-01 through P1-04: **closed**. Soft-bridge dependencies trigger
  rebuilds for affected changes; candidate completeness/domain/objective
  validation, cleanup/rebuild composition, and exact compilation identity
  checks are implemented and covered.
- Review 2 persistent-fixing declared-domain P1: **closed**. Persistent-fixing
  candidates are checked against declared bounds, and the injected out-of-domain
  candidate test passes.
- Review 3 inactive-candidate P1 / CR-01: **closed** by the current split
  between duplicate tracking and active evaluation values, with the mixed-row
  regression passing.

## Narrative Findings (AI reviewer)

### Warnings

### WR-01: Mixed-row inactive-variable regression is reference-backend-only

**Classification:** WARNING  
**File:** `tests/feasibility_relaxation.rs:288-316`  
**Issue:** The regression directly exercises the core evidence validator with
  an injected reference-session candidate, but does not run the same mixed-row
  fixture through real HiGHS extraction. The production fix is in the core
  validator and is behaviorally covered, but adapter-level candidate mapping
  remains indirectly covered for this specific inactive mixed-row shape.
**Fix:** Add an equivalent inactive-variable mixed-row fixture to the real
  HiGHS differential test, or add an adapter-level assertion that inactive
  compiled columns are passed as canonical zero before report validation.

### WR-02: Acceptance-policy test does not exercise a feasible termination

**Classification:** WARNING  
**File:** `tests/feasibility_relaxation.rs:344-364`  
**Issue:** `feasible_acceptance_is_not_promoted_to_optimality` only toggles the
  request field and asserts enum values. It never makes a backend return
  `TerminationStatus::Feasible`, so it does not prove that `AcceptFeasible`
  yields `FeasibleRepair` while `RequireOptimal` yields `Unknown`.
**Fix:** Extend the controlled reference/fault backend to return a feasible
  incumbent without an optimality proof, then assert both acceptance-policy
  classifications and the retained numerical evidence.

## Evidence and Residual Gates

Exact-head checks passed:

- P30 core focused suites: 37 tests passed, including the new mixed-row
  regression and all fault/P29/softening qualification suites.
- Full `roml` all-targets: 293 library tests and all integration targets passed.
- Full `roml-highs` all-targets and the real HiGHS P30 differential suite passed.
- `cargo fmt`, denied-warning clippy for `roml` and `roml-highs`, rustdoc,
  package-list checks, quality-policy checks, and `git diff --check` passed.

The P30 evidence document is content-pinned to the exact reviewed HEAD, though
that documentation edit is currently a working-tree change and must be
preserved or re-pinned if the implementation SHA changes.

MOSEK/Xpress SDK, ABI, licensing/runtime, native-relaxation, cross-platform,
and MSRV qualification remain external or explicitly deferred gates. P31
priority/lexicographic execution is out of scope. No publication, tag, or
release action was performed.

---

_Reviewed: 2026-08-14T17:26:23Z_  
_Reviewer: the agent (gsd-code-reviewer)_  
_Depth: deep_
