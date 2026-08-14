---
phase: 30-soft-constraints
plan: 01
status: complete
subsystem: core-model
tags: [soft-constraints, canonical-model, revisioned-deltas, tdd]
requires:
  - phase: 29-iis-conflict-analysis
    provides: semantic restriction identity conventions and stable model handles
provides:
  - frozen persistent soft-constraint policy vocabulary
  - canonical SoftConstraint construct with stable violation roles
  - atomic builder validation and lifecycle cascade behavior
affects: [30-02, 30-03, 30-04, 31-lexicographic-objectives]
actuals:
  tokens: 6637
  tasks: 2
  commits: 2
tech-stack:
  added: []
  patterns: [generation-safe construct arena, pre-mutation validation, canonical construct delta]
key-files:
  created:
    - src/construct/soft_constraint.rs
    - tests/soft_constraints_contract.rs
    - tests/soft_constraints_lifecycle.rs
    - docs/release/evidence/P30_SOFT_CONSTRAINTS_API.md
  modified:
    - src/construct/mod.rs
    - src/model/mod.rs
    - src/compiler/session.rs
    - src/lib.rs
    - src/advanced.rs
key-decisions:
  - "Persistent softening is represented in the existing construct arena and ordinary revisioned delta path."
  - "P30 exposes only PenaltyTarget::None and PenaltyTarget::Objective; priority and ObjectivePolicy remain P31-owned."
  - "Deleting an original constraint cascades anchored soft constructs to preserve referential integrity."
patterns-established:
  - "Validate all softening inputs before identity allocation or changelog mutation."
requirements-completed: [SM-10.1, SM-10.3, SM-10.4]
coverage:
  - id: D1
    description: "Public persistent softening API and canonical construct visibility"
    requirement: SM-10.1
    verification:
      - kind: unit
        ref: tests/soft_constraints_contract.rs#persistent_softening_is_public_stable_and_revisioned
        status: pass
    human_judgment: false
  - id: D2
    description: "Atomic lifecycle validation and constraint-removal cascade"
    requirement: SM-10.3
    verification:
      - kind: unit
        ref: tests/soft_constraints_lifecycle.rs#invalid_softening_inputs_are_atomic_and_typed
        status: pass
      - kind: unit
        ref: tests/soft_constraints_lifecycle.rs#removing_the_original_constraint_cascades_the_persistent_soft_construct
        status: pass
    human_judgment: false
---

# Phase 30 Plan 01: Persistent Soft-Constraint API Summary

**Canonical persistent soft constraints now have a frozen public policy surface, stable lower/upper violation roles, and atomic lifecycle semantics through the existing construct/revision machinery.**

## Performance

- **Tasks:** 2
- **Files changed:** 10
- **Task commits:** `a1cccb8`, `6bef50d`

## Accomplishments

- Added `SoftConstraint`, `ViolationPolicy`, `PenaltyPolicy`, `PenaltyTarget`, `ViolationRole`, and `ViolationSide` without introducing P31 priority/objective-policy concepts.
- Added `Model::soften_constraint` with finite/nonnegative cap and weight validation, live-objective validation, duplicate prevention, and snapshot/delta visibility.
- Added clone, stale/inactive, atomic rejection, and original-constraint cascade regression coverage.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing critical functionality] Cascaded soft constructs on original-constraint removal**

- **Found during:** Task 30-01 lifecycle tests
- **Issue:** Removing the primitive constraint could leave a persistent soft construct with a dead original-constraint anchor.
- **Fix:** Remove anchored soft constructs before deleting the constraint and its coefficients.
- **Files modified:** `src/model/mod.rs`
- **Verification:** `removing_the_original_constraint_cascades_the_persistent_soft_construct`; model invariants pass.
- **Committed in:** `6bef50d`

**2. [Rule 3 - Blocking issue] Added explicit compiler rejection until the P30 algebra bridge lands**

- **Found during:** Task 30-00 enum integration
- **Issue:** Existing exhaustive compiler matches could not represent the new construct.
- **Fix:** Added a typed `CompileError::UnsupportedFeature` branch; no soft construct is silently ignored before Plan 02 implements compilation.
- **Files modified:** `src/compiler/session.rs`
- **Verification:** core check and contract tests pass; the branch is scheduled for replacement by the exact bridge in Plan 02.
- **Committed in:** `a1cccb8`

## Verification

```text
cargo fmt --all -- --check
cargo test -p roml --test soft_constraints_contract --test soft_constraints_lifecycle -- --nocapture
```

Result: formatter clean; 5 focused tests passed, 0 failed.

## Next Phase Readiness

Ready for Plan 30-02 compiler algebra work. The temporary typed compiler rejection must be replaced by the exact four-side bridge before P30 closure; it is not a shipped unsupported behavior.

---
*Phase: 30-soft-constraints*
*Plan: 01*
*Completed: 2026-08-14*
