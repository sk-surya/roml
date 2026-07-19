# Phase 14 CONTEXT: Performance and ergonomics acceptance

**Date:** 2026-07-19
**Phase:** 14 (M1R-05) — Performance and ergonomics acceptance
**Goal:** make ROML understandable, measurable, and honest in performance, ergonomics, and support claims
**Requirements:** M1R-E1–E5

## Domain

Documentation and measurement phase. Update user-facing docs, create benchmarks, write migration guide, audit public API. No code changes to the core or HiGHS adapter.

## Decisions

### D1: Minimal benchmark suite
Port existing criterion benchmarks from worktree. Add solve pipeline benchmarks (rebuild, delta compile/apply, solve, extraction). Construction-focused benchmarks already exist.

### D2: Documentation scope
Update README.md, audit MODELING_API.md, create CHANGELOG.md, create MIGRATION.md. No new doc generation tooling.

### D3: Migration guide
Map every removed type (SolverAdapter, SolverModelExt, SolveOptions, SolverStatus, drain_changes, Change, ChangeLog) to its new equivalent (BackendSession, SolveRequest, TerminationStatus, DeltaBatch, current_revision). Include before/after code examples.

### D4: No bulk-path admission
Deferred to M2. The phase.md lists it but the ROADMAP's parallel execution policy puts it post-M1R.

### D5: Public API review
Audit every pub export in src/lib.rs and prelude. Verify no implementation types leak into the public surface.

## Canonical refs
- `.planning/ROADMAP.md` — M1R-05 section
- `.planning/REQUIREMENTS.md` — M1R-E1–E5
- `.planning/phases/14-performance-and-ergonomics-acceptance/phase.md`

## Locked requirements
| ID | Description |
|---|---|
| M1R-E1 | Benchmarks isolate construction, propagation, delta compilation, native apply, rebuild, solve, extraction |
| M1R-E2 | Every benchmark records dataset, seed, machine, compiler, backend version, statistical method |
| M1R-E3 | Bulk paths require scalar equivalence and measured benefit |
| M1R-E4 | Performance cannot weaken correctness, replayability, error classification |
| M1R-E5 | Public examples cover initial solve, parameter update, structural update, failure/rebuild, requested/effective configuration |

## Deferred ideas
- Bulk-path admission (M1R-E3) — deferred to M2
- Memory/allocation profiling — post-M1R
- Performance governance/regression budgets — M5 scope
