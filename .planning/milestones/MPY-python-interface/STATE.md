# MPY State

**Objective:** deliver the typed, ergonomic, performant ROML Python interface for repeated MPC solves.
**Status:** planning packet authored; runtime implementation not started.
**Observed base:** `659c30c93fb5b2da056d6ecce245c977dbd7fd3e`.
**Current phase:** MPY-00 — prerequisite intake.
**Authorization:** owner authorized the packet, prerequisite review/remediation/normal merges and subsequent Python implementation on 2026-09-07. Python runtime implementation is conditional on MPY-00 closure; publication is not authorized.
**Decisions:** PyO3 + maturin, no public C ABI, core stays isolated, standard CPython 3.13/3.14, initial HiGHS backend, Python milestone precedes M4.
**Known review work:** PR #49 objective completeness and staged solve budgets, existing P34 closure, stale governance reconciliation.
**Blockers:** none established for planning. Runtime Rust/Python tests have not been run by the planning agent; its environment lacks a Rust toolchain.
**Next gate:** executor refreshes PR inventory, reviews/merges this planning PR, and resolves prerequisite implementation PRs before MPY-01.

| Phase | State | Evidence |
|---|---|---|
| MPY-00 | not started | planning observations in PR-INTAKE.md only |
| MPY-01 | not started | none |
| MPY-02 | not started | none |
| MPY-03 | not started | none |
| MPY-04 | not started | none |
| MPY-05 | not started | none |
| MPY-06 | not started | none |
