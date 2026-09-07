# MPY State

**Objective:** deliver the typed, ergonomic, performant ROML Python interface for repeated MPC solves.
**Status:** MPY-00 in progress — P31 prerequisite merged; P34 closure now active.
**Observed base:** `659c30c93fb5b2da056d6ecce245c977dbd7fd3e`.
**Current main:** `4cbe13caa9a621b43593da76259e9c5b88fc4d18` (PR #49 P31 merged).
**Current phase:** MPY-00 — prerequisite intake (P31 done; P34 in progress).
**P34 progress:** branch `phase-roml-P34-m3-qualification`; leaf ledger
124/127 PASS (3 P34-owned outputs pending); Q01–Q14 corpus green
(core 14 + native 14); import/repair + orchestration workflows green;
perf gate PASS (+1.99% vs 5% allowance); packed consumers 5/5;
fault matrix 20/20 mapped; NLP readiness 0 BLOCKED; review gauntlet
running.
**Authorization:** owner authorized the packet, prerequisite review/remediation/normal merges and subsequent Python implementation on 2026-09-07. Python runtime implementation is conditional on MPY-00 closure; publication is not authorized.
**Decisions:** PyO3 + maturin, no public C ABI, core stays isolated, standard CPython 3.13/3.14, initial HiGHS backend, Python milestone precedes M4.
**Known review work:** PR #49 objective completeness and staged solve budgets, existing P34 closure, stale governance reconciliation.
**Blockers:** none established for planning. Runtime Rust/Python tests have not been run by the planning agent; its environment lacks a Rust toolchain.
**Next gate:** executor refreshes PR inventory, reviews/merges this planning PR, and resolves prerequisite implementation PRs before MPY-01.

| Phase | State | Evidence |
|---|---|---|
| MPY-00 | in progress (P31 merged, P34 active) | `evidence/PR-INVENTORY.md`; PR #49 merge `4cbe13c` |
| MPY-01 | not started | none |
| MPY-02 | not started | none |
| MPY-03 | not started | none |
| MPY-04 | not started | none |
| MPY-05 | not started | none |
| MPY-06 | not started | none |
