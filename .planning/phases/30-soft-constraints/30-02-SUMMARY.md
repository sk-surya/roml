---
phase: 30-soft-constraints
plan: 02
status: complete
subsystem: compiler-and-solutions
tags: [soft-constraints, algebra, origins, violations, tdd]
requires: [30-01]
provides: [exact persistent soft-constraint bridge, original violation accessors, signed correction]
affects: [30-03, 30-04]
actuals:
  tokens: 25600
  tasks: 3
  commits: 5
tech-stack:
  added: []
  patterns: [typed bridge dispatch, stable generated origins, original-expression solution evidence]
key-files:
  created:
    - src/compiler/bridge/soft_constraint.rs
    - tests/soft_constraints_algebra.rs
    - tests/soft_constraints_origins.rs
    - tests/soft_constraint_solution.rs
    - tests/support/soft_constraints_reference.rs
  modified:
    - src/compiler/bridge/mod.rs
    - src/compiler/origin.rs
    - src/compiler/session.rs
    - src/solution/mod.rs
    - roml-highs/tests/soft_constraints_differential.rs
decisions:
  - "Lower and upper sides are separate generated roles; equality and ranged constraints emit both sides deterministically."
  - "PenaltyTarget::Objective uses +weight for minimization and -weight for maximization; priority targets remain P31."
  - "Violation accessors evaluate original canonical constraint terms and do not infer signed correction from generated slacks."
requirements-completed: [SM-10.2, SM-10.3, SM-10.4, SM-10.5, SM-10.6, SM-10.7, SM-10.8]
---

# Phase 30 Plan 02: Exact algebra and violation evidence

Persistent soft constraints now compile into exact portable signed side rows,
carry finite caps and evaluated weights, preserve generated origin roles, and
expose typed original-constraint violation and signed-correction evidence.

## Accomplishments

- Added the bridge registry/dispatch and lower/upper violation variable/row
  roles with complete construct provenance.
- Covered lower, upper, equality, and ranged constraints, zero/positive caps,
  parameterized weights, both objective senses, and atomic numeric failures.
- Added raw lower/upper/total violation accessors, tolerance presentation, and
  explicit positive/negative signed correction.
- Added ReferenceBackend/HiGHS compiled-row differential coverage.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking issue] Replaced the transitional compiler rejection**

- **Found during:** Task 30-02 tracer
- **Issue:** New persistent constructs made existing exhaustive compiler paths
  reject before the exact bridge existed.
- **Fix:** Implemented the exact bridge and typed `SoftConstraint` capability
  path before enabling full phase checks.
- **Commit:** `cd94d2b`

**2. [Rule 1 - Bug] Preserved typed solution/report identity**

- **Found during:** Task 30-04 solution/report integration
- **Issue:** Relaxation reports retain immutable candidate solutions and need
  structural comparison in qualification tests.
- **Fix:** Added `PartialEq` to the existing immutable `Solution` value object.
- **Commit:** `ed5f600`

## Verification

Focused algebra/origin/solution tests passed; the full `roml` and `roml-highs`
matrices subsequently passed at the P30 candidate head.

## Next Phase Readiness

Ready for solve-scoped portable relaxation execution. No native relaxation or
P31 objective-priority behavior was added.
