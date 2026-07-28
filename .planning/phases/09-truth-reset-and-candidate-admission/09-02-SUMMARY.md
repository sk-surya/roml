---
phase: 09-truth-reset-and-candidate-admission
plan: 02
type: execute
objective: Cross-reference every M1.0-M1.5 completion claim against source code and test evidence on the candidate branch, producing a formal requirement disposition table using STATE.md vocabulary.
status: complete
completed: 2026-07-18
tasks:
  total: 2
  completed: 2
  auto: 2
  checkpoint: 0
commits:
  - f56d675: feat(09-truth-reset-and-candidate-admission): add M1.0-M1.5 claim disposition table with git-verified evidence
  - 9171602: feat(09-truth-reset-and-candidate-admission): document 5 legacy source patterns with git-verified line numbers
key_files:
  created:
    - docs/release/evidence/M1R/artifacts/02-claim-reconciliation.md
  modified: []
  deleted: []
verdicts:
  - M1.0: Partially Satisfied
  - M1.1: Failed
  - M1.2: Accepted
  - M1.3: Partially Satisfied
  - M1.4: Partially Satisfied
  - M1.5: Partially Satisfied
---

# Phase 09 Plan 02: Claim Reconciliation Summary

Cross-referenced every M1.0-M1.5 sub-claim from RESEARCH.md against source code
and test evidence on the candidate branch `planning/roml-M1-native-backends-release`,
producing a 38-row requirement disposition table. All dispositions verified by
concrete `git` commands — no verdict relies on evidence-document claims.

## Completed Tasks

- **Task 1:** Cross-referenced M1.0-M1.5 claims against 20+ git commands
  (git show, git grep, git ls-tree, git log). Produced 38-row disposition table
  with STATE.md vocabulary (Accepted, Partially Satisfied, Failed, Locally Verified).
  Added discrepancy analysis comparing fresh verdicts to RESEARCH.md.

- **Task 2:** Documented 5 legacy source patterns with git-verified file:line
  references: drain_changes (model/mod.rs:747), SolverAdapter (adapter.rs:675),
  solver_options (model/mod.rs:124), panic construction (adapter.rs:182,186,206,218),
  Change type (changelog.rs:22). Each with severity rating and M1R phase assignment.

## Key Decisions

1. M1.1 contract freeze is **Failed** — legacy `SolverAdapter` and `drain_changes()`
   remain public despite protocol types being frozen. M1R-01 must address both.
2. M1.2 highs-sys migration is **Accepted** at milestone level. Notably, `ffi.rs`
   was NOT removed (contrary to RESEARCH.md claim) — it was repurposed as a 65-line
   re-export shim. The migration replaced handwritten FFI with `highs-sys` but the
   file persisted.
3. M1.4 CI workflow exists but has never executed — recorded as "workflow created"
   (Accepted) but "no CI runs" (Failed). Dominant: Partially Satisfied.
4. No discrepancy between RESEARCH.md milestone-level verdicts and fresh verification
   for any milestone. Sub-claim details were more nuanced (ffi.rs persistence,
   total test counts).

## Deviations from Plan

None — plan executed exactly as written.

## Threat Flags

None. This plan produces an evidence document only; no new runtime surface introduced.

## M1R-G1 Compliance

The claim disposition table uses only STATE.md vocabulary:
- Accepted (source evidence confirms)
- Partially Satisfied (some evidence but gaps)
- Failed (contradicted or absent)
- Locally Verified (machine-verified but not CI)
- Cannot Verify (not used)
- Owner-Blocked (not used)

## Verification

- [x] docs/release/evidence/M1R/artifacts/02-claim-reconciliation.md exists
- [x] Claim disposition table covers M1.0-M1.5 with correct STATE.md vocabulary
- [x] Every disposition is backed by a git command, not RESEARCH.md copy
- [x] All 5 legacy patterns documented with verified line numbers
- [x] Pattern severities match RESEARCH.md (3 Critical, 2 High)
- [x] Each pattern maps to the correct M1R phase

## Self-Check: PASSED

All claims verified against source code; all task commits exist and are recorded above.
