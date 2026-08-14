---
phase: 30-soft-constraints
plan: 03
status: complete
subsystem: solver-relaxation
tags: [weighted-l1, solve-overlay, cleanup, provider-policy, tdd]
requires: [30-02]
provides: [portable relaxation executor, typed outcomes, provider policy, cleanup composition]
affects: [30-04, 31-lexicographic-objectives]
actuals:
  tokens: 35400
  tasks: 2
  commits: 5
tech-stack:
  added: []
  patterns: [solve-scoped compiled objective, transactional rollback, composite operational errors]
key-files:
  created:
    - src/solver/relaxation.rs
    - tests/feasibility_relaxation.rs
    - tests/feasibility_relaxation_faults.rs
    - tests/relaxation_provider_policy.rs
  modified:
    - src/solver/facade.rs
    - src/solver/overlay.rs
    - src/solver/reference.rs
    - src/solver/mod.rs
    - src/compiler/origin.rs
    - src/compiler/session.rs
decisions:
  - "Portable weighted-L1 repair is compiled as temporary variables, widened base rows, generated side rows, and an isolated objective policy."
  - "NativeRequired rejects before synchronization; PreferNative records an explicit portable fallback reason."
  - "Any post-apply status, extraction, rollback, or verification failure attempts cleanup and preserves primary plus cleanup information."
requirements-completed: [SM-10.6, SM-10.7, SM-10.9]
---

# Phase 30 Plan 03: Portable weighted-L1 repair summary

One solve-scoped portable weighted-L1 repair now runs through the existing
compiled overlay lifecycle, returns exact identity/provider/outcome metadata,
and cannot emit a successful report until cleanup is verified.

## Accomplishments

- Added frozen plan, scope, provider, acceptance, outcome, numeric, and report
  types with portable defaults and no P31 priority surface.
- Added temporary violation variables/rows/objective operations and staged
  ReferenceBackend apply/rollback support.
- Added preflight validation, cap/weight evaluation, exact compilation checks,
  fallback metadata, and composite cleanup/rebuild errors.
- Added end-to-end tracer, provider, cleanup, injected backend-fault, and
  rebuild contract tests.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Extraction failures now always roll back**

- **Found during:** lifecycle review after tracer completion
- **Issue:** status conversion, evidence extraction, or solution normalization
  could return before cleanup.
- **Fix:** route every post-apply failure through the rollback/error-composition
  helper and force rebuild on uncertain verification.
- **Commit:** `ec79294`

**2. [Rule 2 - Missing critical functionality] Added cap propagation to repair variables**

- **Found during:** exact persistent/temporary algebra integration
- **Issue:** the portable repair compiler initially created unbounded deviation
  variables even when the canonical soft policy supplied a finite cap.
- **Fix:** carry validated persistent caps into solve-scoped variable bounds.
- **Commit:** `e4675d3`

## Verification

Portable relaxation focused tests passed (11 tests across tracer, provider, and
cleanup/fault files). Full core and HiGHS matrices, denied-warning clippy,
rustdoc, package, and policy checks passed at closure.

## Next Phase Readiness

Ready for P29 provenance composition and exact-head qualification evidence.
Native relaxation remains explicitly unqualified and deferred.
