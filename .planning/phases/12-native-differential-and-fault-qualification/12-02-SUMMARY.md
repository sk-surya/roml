---
phase: 12-native-differential-and-fault-qualification
plan: 02
subsystem: solver
tags: [differential, fault-injection, q5, commuting-square]
requires:
  - 12-01 (ReferenceBackend BackendSession impl, BackendFixture trait)
provides:
  - FaultInjectingBackend wrapping ReferenceBackend with fault injection tests
  - 23 differential harness tests (16 round-trip + random + multi-adapter + fault + determinism + semi-continuous)
  - 6 M1R-Q5 solve observables tests
tech-stack:
  added: []
  patterns: [BackendSession-based synchronize pattern, commuting square via synchronize API]
key-files:
  created:
    - tests/differential_harness.rs (1777 lines, 23 tests)
    - roml-highs/tests/solve_observables_tests.rs (308 lines, 6 tests)
  modified: []
decisions: []
metrics:
  duration: 16m
  completed-date: 2026-07-18
status: complete
---

# Phase 12 Plan 02: Native differential and fault qualification

**One-liner:** Port FaultInjectingBackend and 23-test differential harness from worktree, adapt to BackendSession synchronize API, add 6 Q5 solve observables tests for HiGHS.

## Tasks Completed

| # | Name | Type | Commit | Files |
|---|------|------|--------|-------|
| 1 | Port FaultInjectingBackend with fault injection tests | auto | 62a9e16 | tests/differential_harness.rs (+502 lines) |
| 2 | Add single-op round-trips, mutation traces, multi-adapter tests | auto | 0570250 | tests/differential_harness.rs (+1276 lines) |
| 3 | Add rebuild determinism, semi-continuous partial-apply, Q5 tests | auto | f6975a5 | tests/differential_harness.rs (+448 lines), roml-highs/tests/solve_observables_tests.rs (+308 lines) |

### Task 1 — FaultInjectingBackend

- Ported `FaultOutcome` enum (Recoverable/Dirty), `FaultConfig` struct, `FaultInjectingBackend` struct wrapping `ReferenceBackend`
- Three fault injection tests pass:
  - `dx_fault_injection_recoverable_failure`: state unchanged, cursor not advanced
  - `dx_fault_injection_dirty_failure`: partial apply, cursor not advanced, rebuild recovers
  - `dx_fault_injection_rebuild_recovery`: rebuild after dirty failure matches direct snapshot projection

### Task 2 — Round-trip tests, mutation traces, multi-adapter

- Ported `assert_commuting_square` / `assert_commuting_square_multi` helpers adapted to use `synchronize(Synchronization::Rebuild(...))` / `synchronize(Synchronization::DeltaBatch(...))` instead of direct `rebuild`/`apply_batch` calls
- All 16 `ModelOp` variants have commuting-square round-trip coverage
- `dx_random_mutation_sequences`: fixed seed 42, 5 batches of 30 ops, verifies commuting square at each boundary
- `dx_multi_adapter_cursor_independence`: SyncCoordinator with 3 batches, backend B lags at r1 then catches up to r3, both produce identical normalized views

### Task 3 — Rebuild determinism, semi-continuous, Q5 tests

- `dx_rebuild_determinism`: moderately complex snapshot (2 vars, 2 cons, 1 obj, 1 param, 3 cells), two independent backends produce identical views and cursors
- `dx_semicontinuous_partial_apply`: journal preserves both batches, state after rebuild includes ordinary bounds AND semi-continuous lower bound, no delta lost
- 6 M1R-Q5 solve observables tests for HighsSession:
  - `q5_objective_offset`: objective constant stored but not applied (AD-9 gap)
  - `q5_dual_values`: binding constraint has non-zero dual via SolutionView
  - `q5_reduced_costs`: reduced costs available in solve result
  - `q5_option_negotiation_applied`: time limit and threads applied, not rejected
  - `q5_option_negotiation_extra`: unknown option rejected, solve still succeeds
  - `q5_status_infeasible_or_unbounded`: unbounded maps to Unbounded

## Deviations from Plan

None — plan executed exactly as written.

## Known Stubs

None.

## Threat Flags

None — no new network endpoints, auth paths, or file access patterns introduced.

## Verification Results

| Test Target | Result |
|-------------|--------|
| `cargo test -p roml --test differential_harness` | 23/23 PASS |
| `cargo test -p roml-highs --test solve_observables_tests` | 6/6 PASS |
| `cargo test -p roml-highs --test contract_tests` | 18/18 PASS (unmodified) |
| `cargo test -p roml --test conformance` | 1/1 PASS (unmodified) |

Pre-existing failures unrelated to this plan: 3 logging tests (roml --lib), flaky SIGSEGV in `sense_to_highs_mapping` (roml-highs --lib).

## Self-Check: PASSED

- tests/differential_harness.rs: exists (1777 lines)
- roml-highs/tests/solve_observables_tests.rs: exists (308 lines)
- Commit 62a9e16: found
- Commit 0570250: found
- Commit f6975a5: found
