---
phase: 09-truth-reset-and-candidate-admission
plan: 03
subsystem: governance/candidate-admission
tags: [test-classification, branch-contamination, split-and-replay]
requires: [09-02]
provides: [09-04]
affects: [evidence-directory, feature-branch-replay]
tech-stack: []
key-files:
  created:
    - docs/release/evidence/M1R/artifacts/03-test-classification-contamination.md
  modified: []
decisions: []
started: 2026-07-18T00:25:00Z
completed: 2026-07-18T00:35:00Z
duration: 10m
status: complete
---

# Phase 09 Plan 03: Test Classification and Branch Contamination Analysis

Classify all 11 ignored tests by fix category and analyze branch contamination for the split-and-replay strategy.

## One-liner

All 11 ignored tests on the candidate branch classified as 2 fix-now (P1-1, P1-2) and 9 pin-m1r1 (P1-3, P1-4, P2-1 through P2-7); branch contamination analysis identifies 4 planning-only, 4 contaminated implementation, 3 stale-claim, and 2 evidence-directory commits; 6 clean feature branches defined with exact cherry-pick commands and merge-base discrepancy documented.

## Tasks Executed

| Task | Name | Type | Status | Commit | Key Files |
|------|------|------|--------|--------|-----------|
| 1 | Classify all 11 ignored tests with per-test disposition | auto | complete | 9fbd9ac | 03-test-classification-contamination.md |
| 2 | Analyze branch contamination and define split-and-replay strategy | auto | complete | e3e10b5 | 03-test-classification-contamination.md |

## Task Details

### Task 1: Classify all 11 ignored tests with per-test disposition

All 11 `#[ignore]` annotations confirmed on candidate branch `planning/roml-M1-native-backends-release` via `git grep`. Each test's source read via `git show` to record file:line, current behavior, ignore reason string, and test body structure.

**Disposition summary:**
- **fix-now (2):** P1-1 (`duplicate_coefficient_for_same_cell`), P1-2 (`duplicate_coefficient_in_objective`) — remove `#[ignore]`, update assertions to match canonical cell behavior
- **pin-m1r1 (9):** P1-3, P1-4, P2-1 through P2-7 — update expectations to match desired post-M1R-01 behavior, re-mark with `#[ignore = "resolved in M1R-01 — drain_changes removal"]`

Each test mapped to a M1R requirement ID (M1R-C5, M1R-H7, M1R-C2, M1R-C1, M1R-C4).

### Task 2: Analyze branch contamination and define split-and-replay strategy

Branch contamination verified via `git log --oneline --name-only planning/roml-M1-native-backends-release --not main -- .planning/` and cross-referenced against the implementation commit list.

**Key findings:**
- Merge base: **82e2ed95** (actual) vs **ef37c88** (researched in 09-RESEARCH.md) — main has advanced
- 4 planning-only: 073106f, bc8e3e0, dba71c0, 302b098
- 4 contaminated: c4db3e0, 22074c0, fb2cb88, 97f8792
- 3 stale claims: a94b75e, 537a035, 85a6396
- 2 evidence directory: 5d117cf, 8a1212f
- 7 clean implementation: c1d5e90, c48f8c3, 00a586c, 084e58e, cf8dc8b, 0fe295b, 649c635
- 6 clean feature branches defined

**Discrepancy documented:** 22074c0 does not touch `src/` files despite its commit message referencing "protocol types, status lattice, taxonomy". The protocol type implementations are in earlier candidate branch history (between 82e2ed95 and ef37c88).

## Deviations from Plan

**1. [Rule 2 - Accuracy] 22074c0 file content discrepancy documented**

- **Found during:** Task 2
- **Issue:** The plan classifies 22074c0 as a "contaminated implementation commit" and provides a cherry-pick command with path constraints (`-- src/ tests/ roml-highs/ roml-mosek/ roml-xpress/ benches/`). However, `git show --name-only 22074c0` shows this commit only touches `.planning/milestones/ROML-M1-STATE.md` and `docs/release/evidence/M1/M1.1_CONTRACT_FREEZE.md`. It does not touch any `src/`, `tests/`, or implementation files.
- **Fix:** Documented the discrepancy with an explanatory note in the Branch Contamination Analysis section. The protocol type implementations referenced in the commit message (DeltaBatch, Revision, Journal, etc.) are in prior commits in the candidate branch history (3982fec, 39a70a5, fac96fa, dbed1db) between merge-base and ef37c88.
- **Files modified:** 03-test-classification-contamination.md
- **Commit:** e3e10b5

## Acceptance Criteria Verification

- [x] docs/release/evidence/M1R/artifacts/03-test-classification-contamination.md exists
- [x] All 11 ignored tests listed with file:line, current behavior, fix category, requirement link
- [x] Fix categories: 2 fix-now, 9 pin-m1r1
- [x] 4 planning-only commits verified via git log
- [x] 4 contaminated implementation commits identified
- [x] 3 stale-completion-claim commits documented
- [x] 6 clean feature branches with exact cherry-pick commands
- [x] Actual merge base SHA (82e2ed95) recorded

## Threat Mitigation Status

| Threat | Disposition | Status |
|--------|-------------|--------|
| T-09-03-01: Test classification labels (Tampering) | mitigate | Mitigated — each test confirmed via `git grep` and `git show` before assigning fix category |
| T-09-03-02: Cherry-pick command accuracy (Tampering) | mitigate | Mitigated — exact commands documented with path-exclusion flags and merge-base discrepancy noted |
| T-09-03-03: Contaminated commit identification (Repudiation) | mitigate | Mitigated — all identifications backed by `git log --name-only` output |

## Self-Check: PASSED

All files verified exist and commits checked in git log.
