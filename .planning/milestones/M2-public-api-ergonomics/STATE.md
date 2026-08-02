# M2 State — Public API Ergonomics

## Current state

**Milestone:** M2 Public API Ergonomics  
**Branch:** `docs/public-api-ergonomics-gsd-ultra`  
**Base:** `main@ac473911bc2239e940b8c2019dee3e01a445701e`  
**Status:** P22 executed on `phase-roml-P22-modeling-ergonomics`; verified 11/11, gate passed, pending merge/PR  
**Current phase:** P22 — modeling ergonomics and entity names (gate passed)

## Objective ledger

| Item | State | Evidence | Next gate |
|---|---|---|---|
| Repository inspection | Complete | `RESEARCH.md` | P20 characterization tests |
| API direction | Accepted for planning | `DECISIONS.md` | signature compile tests |
| P20 contract baseline | Complete — UAT passed | `M2_P20_BASELINE.md`, `PUBLIC_API_M2_DISPOSITION.md`, `tests/ui/*.rs`, `repeated_session_baseline.rs` | merge PR #20, then P21 |
| Solver façade | Complete — gate passed | `src/solver/facade.rs`, `roml-highs/src/facade.rs`, `tests/solver_facade.rs`, `roml-highs/tests/facade_tests.rs` | merge P21 branch, then P22 |
| Unified solution/status | Complete — gate passed | `SolveStatus`/`SolveMetadata`/`SolveError`/`Solution` + status mapping tests | merge P21 branch, then P22 |
| Modeling ergonomics | Complete — gate passed | `tests/modeling_ergonomics.rs`, `tests/named_entities.rs`, name getters, D11 sparse trio, named diagnostics | merge P22 branch, then P23 |
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

1. Merge the P22 branch (`phase-roml-P22-modeling-ergonomics` → main) via PR; then start P23 (`phase-roml-P23-surface-curation`).
2. P23 (surface curation): prelude reduction, `roml::advanced`/`roml::backend` namespace moves, deprecations (effectful macros, `constrain`/`constraint` aliases, `add_var`/`add_binary`/`add_integer`/`add_parameter(f64)` wrappers), `MIGRATION.md` + `CHANGELOG.md` entries. P23 must not remove/deprecate an API until its replacement has compile-pass coverage (D12). The deferred-items list (`.planning/phases/22-modeling-ergonomics/deferred-items.md`) carries the surface-consistency subset for P23 (e.g. `set_variable_bounds` validation, `VarId - VarId`, raw `*_coefficient` NaN acceptance, stale-var atomicity in `add_constraint_spec_impl`).
3. Optional follow-up (informational): real-HiGHS delta-path objective-constant e2e assertion (covered at other layers).
4. Before P24 qualification: the `roml` package contains repo-level files (`.planning/`, `tools/`, `.foundry.toml`) — no `include` filter; packaging hygiene must be addressed (API-10.3). Also the `roml-mosek`/`roml-xpress` adapters fail to compile against the P21+ solver API (out of M2 scope; recorded in deferred-items).

## P21 execution record (verified facts only)

- **Execution branch:** `phase-roml-P21-solver-facade` (10 commits above main merge `456f172`); verification head `6b403d9`.
- **Closed requirement IDs:** API-01, API-02, API-03 — behavioral completion with tests (see `21-VERIFICATION.md`, 15/15 truths; matrix re-run independently by orchestrator and verifier).
- **Key decisions:** `SolveStatus` is golden-path with `SolverStatus` compatibility alias (M2 open item 2 resolved); first-solve sync is delta-first when the journal retains the chain (contract = correct sync, not a fixed mode); unsupported model changes surface as `Err(Synchronization)` after at most one rebuild retry, never a stale/fabricated result.
- **Deviations (documented):** D7/D10 fallible migration landed in P21 ahead of its P22 slot (the target contracts needed it); target fixtures promoted from `tests/ui/` to `roml-highs/tests/` and execute; end-to-end rebuild-retry recovery with real HiGHS is unreachable for supported models (all 18 delta ops supported) — recovery semantics proven at core level with fault backends.
- **Test counts:** roml 482 passed / 0 failed; roml-highs 86 passed / 0 failed; doctests execute the M2 quickstart (2/2); clippy/rustdoc/fmt clean.
- **Skipped/notes:** no skips. Informational note: HiGHS delta-path objective-constant e2e assertion deferred (covered at other layers).
- **Review cycle (PR #21, 2026-08-02):** independent protocol/API review returned 4 blocking findings + 1 flag, all resolved — (1) terminal sync errors (incl. license) no longer retried (`SolveError::is_terminal` + new fault test); (2) solve options reset to HiGHS defaults per request (no cross-solve leakage; new e2e test); (3) `SolverSession` public surface narrowed to `new`/`solve`/`solve_with` (`last_solution`/`backend`/`backend_mut` removed; stale protection structural; fault tests use shared handles); (4) independent protocol review recorded as the verification human item (`21-UAT.md`, status human_needed until re-review); (5) legacy `add_integer(Bounds)` made fallible per the recorded migration plan.
- **Review round 2 (2 residual findings, resolved):** (1) `HighsSession::synchronize` now maps a failed delta's cursor health from the error's own `HealthEffect` (terminal → `Terminal`, else `RequiresRebuild`) instead of unconditionally `RequiresRebuild`; unit + session-level regression tests. (2) `negotiate_options` now calls `Highs_resetOptions` (session-wide reset covering arbitrary `backend_option` entries) before applying the request, and successful extra options are recorded in `EffectiveConfig.adjustments`; new e2e test. **P21 gate passed at `9457898`; merged via PR #21 (`f05e83d`).**

## P22 execution record (verified facts only)

- **Execution branch:** `phase-roml-P22-modeling-ergonomics` (7 commits above the P21 merge); verification head `fa2a8f5`.
- **Closed requirement IDs:** API-04, API-05, API-06 — behavioral completion with tests (`22-VERIFICATION.md`, 11/11; matrix re-run independently by orchestrator and verifier).
- **What landed:** `VariableDef::lower_bound/upper_bound`, binary-bounds validation (`ModelError::InvalidBinaryBounds`), atomic rejection proofs; four name getters with typed stale-ID errors; `ObjectiveSpec::named` + advanced `add_objective_named`; public advanced `add_empty_constraint(bounds)`; D11 sparse trio (`set_coefficient`/`add_to_coefficient`/`remove_coefficient_at`) with one-canonical-cell semantics; name-aware `pprint` that never panics on stale IDs; `tests/modeling_ergonomics.rs` + `tests/named_entities.rs`.
- **Test counts:** roml 522 passed / 0 failed; roml-highs 89 passed / 0 failed (core-only phase, backend suites unchanged); clippy/rustdoc/fmt clean.
- **Deferred (pre-existing, out of scope):** recorded in `22/deferred-items.md` — mosek/xpress don't compile against P21+ API (M2 scope is roml + roml-highs); `set_variable_bounds` lacks validation; missing `VarId - VarId`; raw `*_coefficient` mutators accept NaN/∞. (Item 4 — `add_constraint`/`add_objective` non-atomicity — was RESOLVED in the review round, not deferred.) Surface-consistency subset is P23's scheduled work.
- **Review cycle (PR #22, 2026-08-02):** 2 blocking findings, both resolved in P22 — (1) `set_coefficient` now compares expression semantics (only a prior constant equal to the requested value is a no-op; parameter dependencies are always dropped on replacement); (2) canonical constraint/objective creation is now atomic via `validate_expression_entities` pre-validation (stale variable/parameter fails before any row/objective/changelog mutation, API-06.5); 6 new tests. roml 528/0. Status: awaiting re-review.

## Phase ledger

| Phase | Status | Requirement range | Completion evidence |
|---|---|---|---|
| P20 | Gate passed — pending merge | API-04, API-07, API-08, API-10 | `M2_P20_BASELINE.md`, `M2_P20_public_api_roml{,_highs}.txt`, `PUBLIC_API_M2_DISPOSITION.md`, `tests/ui/{target_quickstart,target_incremental,current_readme_drift,current_solve_model_method}.rs`, `scripts/p20-capture-drift.sh`, `tests/public_api_compile.rs`, `roml-highs/tests/repeated_session_baseline.rs` — verification 9/9, `20-VERIFICATION.md`, `20-UAT.md` (4/4 passed) |
| P21 | Gate passed — pending merge | API-01, API-02, API-03 | `21-VERIFICATION.md` (15/15), `21-SUMMARY.md`, executing target contracts |
| P22 | Gate passed — pending merge | API-04, API-05, API-06 | `22-VERIFICATION.md` (11/11), `22-SUMMARY.md`, `tests/modeling_ergonomics.rs`, `tests/named_entities.rs` |
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