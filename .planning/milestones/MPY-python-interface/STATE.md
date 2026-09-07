# MPY State

**Objective:** deliver the typed, ergonomic, performant ROML Python interface for repeated MPC solves.
**Status:** MPY-00 in progress — P31 prerequisite merged; P34 closure now active.
**Observed base:** `659c30c93fb5b2da056d6ecce245c977dbd7fd3e`.
**Current main:** `17e8b79275d488de335df02bac1f0a80d7ec9808` (PR #51 P34 merged; M3 complete).
**Current phase:** MPY-00 COMPLETE — P31 and P34 prerequisites merged.
MPY-01 (binding toolchain and package boundary) is authorized.
**Authorization:** owner authorized the packet, prerequisite review/remediation/normal merges and subsequent Python implementation on 2026-09-07. Python runtime implementation is conditional on MPY-00 closure; publication is not authorized.
**Decisions:** PyO3 + maturin, no public C ABI, core stays isolated, standard CPython 3.13/3.14, initial HiGHS backend, Python milestone precedes M4.
**Known review work:** PR #49 objective completeness and staged solve budgets, existing P34 closure, stale governance reconciliation.
**Blockers:** none established for planning. Runtime Rust/Python tests have not been run by the planning agent; its environment lacks a Rust toolchain.
**Next gate:** executor refreshes PR inventory, reviews/merges this planning PR, and resolves prerequisite implementation PRs before MPY-01.

| Phase | State | Evidence |
|---|---|---|
| MPY-00 | complete | `evidence/PR-INVENTORY.md`; PR #49 merge `4cbe13c`; PR #51 merge `17e8b79` |
| MPY-01 | complete | `evidence/DEPENDENCIES.md`; import from built wheel; independent CLEAR review |
| MPY-02 | complete | golden LP, identity/errors, production.py, ergonomics review |
| MPY-03 | complete | arrays/CSR, atomic updates, 39 tests, interface review, perf notes |
| MPY-04 | complete | detached solves, outcomes, warm starts, duals, 61 tests, lifecycle review |
| MPY-01 | not started | none |
| MPY-02 | not started | none |
| MPY-03 | not started | none |
| MPY-04 | not started | none |
| MPY-05 | not started | none |
| MPY-06 | not started | none |
