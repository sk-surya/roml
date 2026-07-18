# Phase 12 CONTEXT: Native differential and fault qualification

**Date:** 2026-07-18
**Phase:** 12 (M1R-03) — Native differential and fault qualification
**Goal:** prove that HiGHS incremental behavior equals rebuild behavior and that native failures preserve recovery
**Requirements:** M1R-Q1–Q5

## Domain

Correctness/qualification phase. Run the shared contract suite (parameterized on BackendFixture) against both ReferenceBackend and HighsSession. Prove commuting square, deterministic rebuild, fault injection recovery, and multi-cursor independence — all locally (no CI required).

## Canonical refs

- `.planning/ROADMAP.md` — M1R-03 section
- `.planning/REQUIREMENTS.md` — M1R-Q1–Q5
- `.planning/TRACEABILITY.md` — Evidence format
- `.planning/phases/12-native-differential-and-fault-qualification/phase.md` — Phase packet
- `src/solver/reference.rs` — ReferenceBackend (needs BackendSession impl)
- `src/solver/session.rs` — BackendSession trait
- `roml-highs/src/session.rs` — HighsSession BackendSession impl
- `roml-highs/tests/contract_tests.rs` — Existing C1-C11 contract tests
- `src/delta.rs`, `src/revision.rs`, `src/sync.rs` — Protocol types

## Locked requirements

| ID | Description |
|---|---|
| M1R-Q1 | ReferenceBackend and HiGHS run same parameterized conformance suite |
| M1R-Q2 | Seeded mutation traces prove incremental-vs-rebuild equivalence over all admitted operations |
| M1R-Q3 | Failure injection covers every multi-call apply boundary and deterministic recovery |
| M1R-Q4 | Multi-adapter lag/catch-up and independent cursors verified |
| M1R-Q5 | Objective offsets, primal, duals, reduced costs, basis, statuses, option negotiation have focused tests |

## Decisions

### D1: ReferenceBackend must implement BackendSession
The critical blocker is that ReferenceBackend has its own `apply_batch()`/`rebuild()` API instead of the `BackendSession` trait. Make it implement `BackendSession` (synchronize, solve, close) so both backends run the same parameterized tests.

### D2: Create BackendFixture trait
A minimal trait that creates a backend instance from a test seed/name. Both ReferenceBackendFactory and HighsSessionFactory implement it. Contract tests parameterize on it.

### D3: Port differential harness from worktree
A 2012-line differential harness exists at `.claude/worktrees/phase-roml-P0-release-baseline/tests/differential_harness.rs`. Copy it into main tree as `tests/differential_harness.rs` and adapt from ReferenceBackend's custom API to the parameterized BackendFixture pattern.

### D4: Port FaultInjectingBackend
The worktree has a `FaultInjectingBackend` wrapper with `FaultConfig` (recoverable, dirty, rebuild recovery). Port it to wrap `BackendSession`. Fault sites at each `synchronize` boundary.

### D5: Run locally
No CI needed for this phase. HiGHS is installed and working (18 contract tests pass). The phase gate is a correctness proof, not a platform matrix.

### D6: Minimal passing set
5 work items for the gate:
1. ReferenceBackend implements BackendSession
2. BackendFixture trait + parameterized test runner
3. Differential harness (commuting square + seeded traces)
4. Fault injection (FaultInjectingBackend wrapping BackendSession)
5. Independent verification pass

## Deferred ideas
- CI integration — handled by Phase 13 (M1R-04)
- Cross-platform matrix — handled by Phase 13
- Performance benchmarks — handled by Phase 14 (M1R-05)
