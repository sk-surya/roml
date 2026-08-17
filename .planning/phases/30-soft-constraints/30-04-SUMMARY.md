---
phase: 30-soft-constraints
plan: 04
status: complete
subsystem: qualification-and-evidence
tags: [p29, provenance, qualification, documentation, closure]
requires: [30-03]
provides: [p29-to-p30 mapping, qualification corpus, public example, exact-head evidence]
affects: [31-lexicographic-objectives, 34-m3-qualification]
actuals:
  tokens: 24700
  tasks: 3
  commits: 6
tech-stack:
  added: []
  patterns: [all-or-error provenance mapping, exact-head evidence ledger, explicit scope fences]
key-files:
  created:
    - tests/feasibility_relaxation_p29.rs
    - tests/soft_constraints_qualification.rs
    - docs/examples/feasibility_relaxation.md
    - docs/release/evidence/P30_SOFT_CONSTRAINTS_RELAXATION.md
  modified:
    - src/solver/relaxation.rs
    - src/solver/facade.rs
    - src/solver/mod.rs
    - src/lib.rs
    - CHANGELOG.md
decisions:
  - "P29 mapping accepts primitive/imported sides and persistent fixings only, checks exact identity, and rejects the complete set on one unsupported member."
  - "P29 source declaration names are retained on repair report members through the exact-base from-report entry point."
  - "P30 closure records native/license exclusions as non-required scope fences; it does not activate P31 or claim release readiness."
requirements-completed: [SM-10.6, SM-10.9]
---

# Phase 30 Plan 04: Provenance and qualification summary

P30 now has executable P29 composition, a positive qualification corpus, public
behavior documentation, and exact-head release evidence with explicit native
and P31 scope fences.

## Accomplishments

- Added exact model-instance/revision/compilation P29 mapping for supported
  primitive/imported sides and persistent fixings, preserving source names.
- Added negative tests for unsupported locks/origins and stale identity, plus
  side/cap/weight qualification fixtures.
- Added the persistent-softening versus isolated-repair example and changelog
  behavior entry.
- Recorded full core/HiGHS test, lint, rustdoc, package, quality-policy, and
  injected cleanup-fault outcomes in
  `docs/release/evidence/P30_SOFT_CONSTRAINTS_RELAXATION.md`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing critical functionality] Added P29-aware repair entry point**

- **Found during:** provenance review
- **Issue:** mapping source names alone did not guarantee that a resulting repair
  report retained imported provenance.
- **Fix:** require an already synchronized exact base, map all P29 members before
  repair, and copy source declarations into report members.
- **Commit:** `eb85fc0`

**2. [Rule 3 - Blocking issue] Resolved denied-warning policy findings**

- **Found during:** final clippy gate
- **Issue:** manual defaults for Option/enum policy types triggered
  `clippy -D warnings`.
- **Fix:** use equivalent standard derives without behavior changes.
- **Commit:** `3ac5120`

## Verification

Evidence ledger commands all pass at recorded candidate head `e412cb4`. Native
MOSEK, Xpress, native relaxation, license, publication, and tag actions were not
required and were not represented as passes.

## Next Phase Readiness

P30 is ready for owner review/merge. P31 remains planned and inactive; no P31
objective-priority or lexicographic implementation is included here.

## Self-Check: PASSED

- All four plan summaries and the exact-head evidence file exist.
- All listed task and closure commits resolve in git history.
- Final required verification commands passed at `e412cb4`.
