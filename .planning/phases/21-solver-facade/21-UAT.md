---
status: passed
phase: 21-solver-facade
source: [21-VERIFICATION.md]
started: 2026-08-02
updated: 2026-08-02
---

## Current Test

number: 1
name: Independent protocol review approves recovery semantics
expected: |
  The protocol review (PR #21) approves: terminal failures return without
  retry; at most one rebuild retry for recoverable/dirty failures; stale
  results are never reported as current; solve options do not leak across
  repeated solves; SolverSession exposes only new/solve/solve_with; legacy
  add_integer(Bounds) is fallible.
awaiting: user response

## Tests

### 1. Independent protocol review approves recovery semantics

Review the recovery semantics per the phase plan Gate: terminal-no-retry (including license errors), one-rebuild retry bound, stale-result invalidation, per-solve option self-containment, the three-method `SolverSession` surface, and fallible `add_integer(Bounds)`.

expected: The protocol review approves the recovery semantics and the resolved findings.
result: passed

## Summary

total: 1
passed: 1
issues: 0
pending: 0
skipped: 0
blocked: 0

## Gaps

None. Independent protocol review (PR #21, rounds 1-3) approved the recovery
semantics at head `9457898` on 2026-08-02: terminal delta failures leave the
session terminal, recoverable failures require rebuild, cursor-advance
failure marks terminal, Highs_resetOptions resets the complete native option
table (including arbitrary backend options), successful backend options are
recorded in effective metadata, and real-HiGHS regression tests cover option
isolation and failed-delta health. The P21 gate passes.
