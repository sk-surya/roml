# M2 State — Public API Ergonomics

## Current state

**Milestone:** M2 Public API Ergonomics  
**Branch:** `docs/public-api-ergonomics-gsd-ultra`  
**Base:** `main@ac473911bc2239e940b8c2019dee3e01a445701e`  
**Status:** P20 executed on `phase-roml-P20-api-contract`; awaiting API review (4 UAT items)  
**Current phase:** P20 — contract baseline and API characterization (gate not yet passed)

## Objective ledger

| Item | State | Evidence | Next gate |
|---|---|---|---|
| Repository inspection | Complete | `RESEARCH.md` | P20 characterization tests |
| API direction | Accepted for planning | `DECISIONS.md` | signature compile tests |
| P20 contract baseline | Executed — awaiting review | `M2_P20_BASELINE.md`, `PUBLIC_API_M2_DISPOSITION.md`, `tests/ui/*.rs`, `repeated_session_baseline.rs` | 4 UAT review items, then P21 |
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

1. Owner reviews the 4 UAT items (`20-UAT.md` / `20-VERIFICATION.md`) for the P20 gate: baseline evidence, drift characterization, target signatures, disposition table.
2. After UAT passes, run `/gsd-verify-work 20` to mark P20 complete and advance to P21.
3. P21 resolves the stale-solution-on-rejected-sync question (API-01.5) surfaced by `repeated_session_baseline.rs`: current `HighsSession::synchronize` clears the cached solution on successful sync but leaves `SolutionView` readable when a sync is rejected (cursor → `RequiresRebuild`) — the P20 test establishes this is genuinely stale by advancing the canonical model to r2 (expected 20.0) while the session still reports the r1 solution (12.0).
4. `cargo package --list -p roml` exit-101 skip (untracked local artifacts `.planning/`, `graphify-out/` make the tree dirty) must be re-run in a clean checkout before P24 qualification.

## Phase ledger

| Phase | Status | Requirement range | Completion evidence |
|---|---|---|---|
| P20 | Executed — awaiting review | API-04, API-07, API-08, API-10 | `M2_P20_BASELINE.md`, `M2_P20_public_api_roml{,_highs}.txt`, `PUBLIC_API_M2_DISPOSITION.md`, `tests/ui/{target_quickstart,target_incremental,current_readme_drift}.rs`, `tests/public_api_compile.rs`, `roml-highs/tests/repeated_session_baseline.rs` — verification 9/9, `20-VERIFICATION.md`, `20-UAT.md` |
| P21 | Blocked on P20 review | API-01, API-02, API-03 | pending |
| P22 | Blocked on P21 | API-04, API-05, API-06 | pending |
| P23 | Blocked on P22 | API-06, API-07, API-08 | pending |
| P24 | Blocked on P23 | API-09, API-10, all | pending |

## P20 execution record (verified facts only)

- **Execution branch:** `phase-roml-P20-api-contract`, forked from `main@ac473911bc2239e940b8c2019dee3e01a445701e`, packet merged (docs only). Verification head: `473d465`.
- **Closed requirement IDs (characterization portions):** API-04, API-07, API-08, API-10. Behavioral completion of these IDs is deferred to P21-P24 per TRACEABILITY.md; no production API was implemented in P20 by design (plan Gate).
- **Independent verification:** orchestrator re-ran the phase matrix — fmt clean; `cargo test -p roml --all-targets` 402 passed / 0 failed; `cargo test -p roml-highs --all-targets` 76 passed / 0 failed; rustdoc `-D warnings` clean for both crates. Verifier scored 9/9 truths with 0 behavior-unverified.
- **Skipped check (resolved):** `cargo package --list -p roml` originally exit 101 (dirty primary tree from untracked local artifacts); re-run from a clean worktree after review — exit 0, 99 files (list in `M2_P20_BASELINE.md`). Finding: `roml`'s package currently includes repo-level files (`.planning/`, `tools/`, `.foundry.toml`) because its manifest has no `include` filter — packaging-hygiene baseline for P6/P24 (API-10.3).
- **Decision changes:** none. Open planning blockers (macro compat window, SolveStatus naming, `SolverSession<B>` name, alias-vs-wrapper) remain recorded and deferred to P21/P22 as planned.
- **Residual risk for P21:** stale-solution invalidation on rejected delta (see Immediate next actions).
- **Review cycle (PR #20, 2026-08-02):** independent review returned 6 blocking findings + 1 assertion request; all addressed: (1) rejected-delta test now asserts the readable-but-stale state; baseline/summary/verification wording consistent; (2) disposition table made internally coherent (raw IDs, `ValueExpr`, `SolverError`, `SolutionBuilder`/`Store`, `Model::constrain` vs canonical `add_constraint(spec)`); (3) concrete migration strategy recorded per signature collision (`add_variable`, `set_parameter`, `add_constraint` — Rust forbids overloads); (4) `cargo package --list -p roml` completed from clean worktree; (5) public-API evidence normalized (`/Users/...` → `$REPO`); (6) E0599 drift frozen in `tests/ui/current_solve_model_method.rs` + `scripts/p20-capture-drift.sh`; exact `assert_eq!` on active objective.
- **Review round 2:** two substantive findings addressed — (1) dirty-path test now establishes genuine staleness (canonical model advances to r2; session still reports r1 solution 12.0; recovery from the real r2 snapshot solves 20.0); (2) collision analysis completed: `add_parameter(f64)` bridge added (`Into<ParameterDef>` + `From<f64>`), `add_integer` return-type break documented, `add_constraint` characterized as input-shape-compatible with an intentional return-type break plus a required private `add_empty_constraint(bounds)` primitive (current `add_constraint_expr` calls the public `add_constraint`), `constrain!` replacement references corrected to canonical `add_constraint`. Status: awaiting re-review (round 2 fixes at head).

## State update protocol

After each phase:

- record the exact completion SHA;
- list closed requirement IDs;
- link tests and evidence;
- record any decision change with rationale;
- identify residual risks and the next gate;
- never mark a skipped check as passing;
- keep historical findings rather than rewriting them as if they never existed.