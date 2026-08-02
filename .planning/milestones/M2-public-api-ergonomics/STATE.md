# M2 State — Public API Ergonomics

## Current state

**Milestone:** M2 Public API Ergonomics  
**Branch:** `docs/public-api-ergonomics-gsd-ultra`  
**Base:** `main@ac473911bc2239e940b8c2019dee3e01a445701e`  
**Status:** planning complete; awaiting implementation  
**Current phase:** P20 — contract baseline and API characterization

## Objective ledger

| Item | State | Evidence | Next gate |
|---|---|---|---|
| Repository inspection | Complete | `RESEARCH.md` | P20 characterization tests |
| API direction | Accepted for planning | `DECISIONS.md` | signature compile tests |
| Solver façade | Planned | P21 plan | end-to-end solve green |
| Unified solution/status | Planned | P21 plan | mapping tests green |
| Modeling ergonomics | Planned | P22 plan | named model examples green |
| Surface curation | Planned | P23 plan | public API review green |
| Consumer qualification | Planned | P24 plan | fresh packed consumers green |

## Accepted assumptions

- The backend session contract is sufficiently strong and remains frozen for M2.
- Generic orchestration can live in core and access model-private synchronization state.
- The default user-facing HiGHS type is `roml_highs::Highs`.
- Method-first modeling is canonical.
- `constraint!` is optional pure syntax sugar; effectful macros are migration-only.
- Existing raw ID types remain available through an advanced API while semantic aliases are added.
- One automatic rebuild retry per solve attempt is sufficient and prevents retry loops.
- M2 may make documented pre-1.0 breaking changes with migration notes.

## Blockers

No owner decision blocks P20. The following issues must be resolved during P20 rather than guessed during implementation:

1. Exact compatibility window for deprecated effectful macros.
2. Whether `SolveStatus` replaces `SolverStatus` immediately or first ships as an alias.
3. Exact name of the generic core façade: planned as `SolverSession<B>` unless compile ergonomics prove materially worse.
4. Whether semantic aliases are type aliases or new transparent wrappers; default is aliases to avoid operator-overload churn.

## Immediate next actions

1. Create a worktree from this branch's approved descendant for P20.
2. Record `cargo public-api` and rustdoc inventory at the exact implementation base.
3. Add compile-pass fixtures for the target quickstart and incremental workflow.
4. Add a characterization fixture showing current README/guide examples do not compile against current exports.
5. Freeze signatures in an ADR amendment if P20 discovers a required deviation.

## Phase ledger

| Phase | Status | Requirement range | Completion evidence |
|---|---|---|---|
| P20 | Ready | API-04, API-07, API-08, API-10 | pending |
| P21 | Blocked on P20 | API-01, API-02, API-03 | pending |
| P22 | Blocked on P21 | API-04, API-05, API-06 | pending |
| P23 | Blocked on P22 | API-06, API-07, API-08 | pending |
| P24 | Blocked on P23 | API-09, API-10, all | pending |

## State update protocol

After each phase:

- record the exact completion SHA;
- list closed requirement IDs;
- link tests and evidence;
- record any decision change with rationale;
- identify residual risks and the next gate;
- never mark a skipped check as passing;
- keep historical findings rather than rewriting them as if they never existed.