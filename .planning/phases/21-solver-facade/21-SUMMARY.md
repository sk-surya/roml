---
phase: 21-solver-facade
plan: 21
subsystem: api
tags: [public-api, rust, solver-facade, solver-session, highs, tdd, unified-result]
---

# Dependency graph
requires:
  - phase: 20-public-api-contract
    provides: frozen target contracts (tests/ui/target_*.rs), repeated-solve session baseline, surface disposition, collision migration plan
provides:
  - generic core SolverSession<B> orchestration (commit -> sync -> solve -> normalize)
  - unified SolveStatus / SolveMetadata / Solution / SolveError result model
  - ergonomic SolveOptions with pre-sync validation
  - roml_highs::Highs user-facing façade (new/solve/solve_with)
  - end-to-end repeated-solve and recovery qualification on real HiGHS
  - promoted, executing M2 target contracts (quickstart + incremental)
affects: [22-modeling-ergonomics, 23-surface-curation, 24-consumer-qualification]

# Actuals
actuals:
  tokens: 14500
  tasks: 6
  commits: 10

# Tech tracking
tech-stack:
  added:
    - SolverSession<B> generic orchestration in roml core (src/solver/facade.rs)
    - SolveStatus + SolverStatus compatibility alias (src/solver/mod.rs)
    - SolveMetadata + SynchronizationMode (src/solution/metadata.rs)
    - SolveError (src/solver/error.rs)
    - SolveOptions builder API (src/solver/options.rs)
    - roml_highs::Highs façade (roml-highs/src/facade.rs)
    - VariableDef/ParameterDef definition builders + fallible Model entry points (D7/D10)
  patterns:
    - test-first orchestration with fault-injecting reference backends (terminal, delta rejection, rebuild failure knobs)
    - exhaustive status mapping without wildcard arms
    - running rustdoc quickstart examples (doctests execute the M2 quickstart)
    - promoted target contracts: frozen P20 UI fixtures become auto-discovered compile-and-run tests

key-files:
  created:
    - src/solver/facade.rs
    - src/solver/error.rs
    - src/solver/options.rs
    - src/solution/metadata.rs
    - roml-highs/src/facade.rs
    - tests/solver_facade.rs
    - tests/status_mapping.rs
    - tests/solve_options.rs
    - roml-highs/tests/facade_tests.rs
    - roml-highs/tests/target_quickstart.rs (promoted from tests/ui/)
    - roml-highs/tests/target_incremental.rs (promoted from tests/ui/)
  modified:
    - src/solver/mod.rs
    - src/solution/mod.rs
    - src/lib.rs
    - src/model/mod.rs (fallible entry points, definition builders, coordinator access)
    - src/model/variable.rs, src/model/parameter.rs (VariableDef/ParameterDef)
    - roml-highs/src/lib.rs
    - docs/release/evidence/M2_P20_BASELINE.md (promoted fixture references)
    - docs/release/PUBLIC_API_M2_DISPOSITION.md (promoted fixture references)

key-decisions:
  - "First-solve synchronization is delta-first when the journal retains the full initial batch chain; snapshot rebuild remains the fallback. The contract asserted is 'synchronized and correct', not a specific mode (documented in facade_tests.rs)."
  - "With real HiGHS every supported model change applies as a delta (the projection implements all 18 ModelOp variants); the only delta rejection (semi-continuous domains) is also rejected by snapshot rebuild, so it surfaces as Err(Synchronization) — never a stale or fabricated result (API-03.3/01.5). Rebuild-retry recovery semantics are covered by core fault-backend tests."
  - "SolveStatus ships now with SolverStatus as a compatibility alias (M2 STATE open item 2 resolved toward the target name)."
  - "Objective constants are included exactly once: the backend projection applies the offset and normalize_result copies it unchanged; objective-constant propagation on the incremental path was added (commit 283ff05)."

requirements-completed: [API-01, API-02, API-03]

# Coverage metadata
coverage:
  - id: D1
    description: "Highs::new/solve/solve_with façade (API-01.1-01.3) compiles and runs; quickstart + incremental doctests execute."
    requirement: "API-01"
    verification:
      - kind: integration
        ref: "roml-highs/tests/target_quickstart.rs#quickstart_compiles_and_runs"
        status: pass
      - kind: integration
        ref: "roml-highs/tests/target_incremental.rs#incremental_compiles_and_runs"
        status: pass
      - kind: doctest
        ref: "roml-highs/src/facade.rs (quick start example)"
        status: pass
    human_judgment: false
  - id: D2
    description: "Repeated solves on one Highs use deltas when valid and a snapshot rebuild when required; at most one rebuild retry; terminal state errors without retry."
    requirement: "API-02"
    verification:
      - kind: integration
        ref: "roml-highs/tests/facade_tests.rs#bound_delta_second_solve / parameter_delta_second_solve (Delta mode)"
        status: pass
      - kind: integration
        ref: "tests/solver_facade.rs (fault-backend: one rebuild retry, terminal no retry, revision invariant, stale invalidation)"
        status: pass
      - kind: integration
        ref: "roml-highs/tests/facade_tests.rs#unsupported_model_returns_error_never_stale"
        status: pass
    human_judgment: false
  - id: D3
    description: "Unified result model: one Solution, one SolveStatus (optimal/feasible-limit/infeasible/unbounded/interrupted/numerical/limit distinctions), metadata, and SolveError semantics (API-03.1-03.4)."
    requirement: "API-03"
    verification:
      - kind: integration
        ref: "tests/status_mapping.rs (exhaustive TerminationStatus -> SolveStatus/SolveError mapping)"
        status: pass
      - kind: integration
        ref: "roml-highs/tests/facade_tests.rs#first_solve_from_new_model (metadata: backend name, revision, sync mode)"
        status: pass
      - kind: integration
        ref: "tests/solver_facade.rs + facade_tests.rs#failed_option_validation_leaves_state_unchanged"
        status: pass
    human_judgment: false
  - id: D4
    description: "Objective constant included exactly once: façade value == direct backend value == model expression evaluation."
    requirement: "API-03"
    verification:
      - kind: integration
        ref: "tests/solver_facade.rs objective-constant tests + commit 283ff05 (propagate objective constants on the incremental path)"
        status: pass
    human_judgment: false

# Metrics
duration: 3h (Tasks 1-4 user-driven; Tasks 5-6 + gate 1h)
completed: 2026-08-02
status: complete
---

# Phase 21: Solver Façade and Unified Result — Summary

**The M2 golden-path solve path is complete: `Highs::solve(&mut Model)` backed by generic core orchestration, one user-facing result model, and executing target contracts.**

## Accomplishments

- **Unified status and metadata** (Task 1): `SolveStatus` with exhaustive, wildcard-free mapping from every `TerminationStatus`; `SolverStatus` kept as a compatibility alias; `SolveMetadata` (backend name, model revision, effective configuration, synchronization mode `Delta`/`Rebuild`/`NoChange`).
- **Normalized results** (Task 2): `normalize_result` converts backend `SolveResult` + active-objective metadata into the golden-path `Solution`; objective constant included exactly once (projection applies the offset; incremental path propagates it — no double-counting proof via façade vs backend vs expression evaluation).
- **Core orchestration** (Task 3): `SolverSession<B>` in `roml` implements the 9-step algorithm — commit-before-mutation, terminal-no-retry, rebuild-or-delta decision, sequential delta application, one rebuild retry on recoverable failure, solve exactly once, normalize. Fault-backend tests assert at most one rebuild retry, backend revision == committed revision before solve, and prior-solution invalidation after mutation (API-01.5).
- **Solve options** (Task 4): `SolveOptions` builder API (`time_limit`, `relative_gap`, `absolute_gap`, `threads`, `output`, `random_seed`, `backend_option`) with validation (non-negative durations/gaps, positive threads) **before** synchronization; failed validation leaves model and backend state unchanged; effective configuration preserved in metadata.
- **HiGHS façade** (Task 5): `roml_highs::Highs` thin wrapper over `SolverSession<HighsSession>`; `HighsSession` remains exported for framework authors; the M2 quickstart is a **running** rustdoc example.
- **End-to-end qualification** (Task 6 + gate): 7 facade tests on real HiGHS (first solve, no-change re-solve, bound delta 12.0→8.0, parameter delta 12.0→20.0, objective switch, unsupported-model error, failed-option-validation state preservation); the frozen P20 target contracts were promoted to auto-discovered compile-and-run tests and **execute**.

## Task Commits

1. **Task 1: normalize solve status and metadata** - `ae58c2d` (feat)
2. **Task 2a: validated definition builders + fallible entry points** - `e89e3de` (feat; D7/D10 migration pulled into P21)
3. **Task 2b: normalize backend results into solution** - `7234bfe` (feat)
4. **Task 2c: propagate objective constants on the incremental path** - `283ff05` + `7268e4b` (feat, style)
5. **Task 3: orchestrate model synchronization in core** - `4bb2e88` (feat)
6. **Task 4: add ergonomic solve options** - `c4f5020` (feat)
7. **Task 5: add user-facing solver facade** - `5abd813` (feat(highs))
8. **Task 6 + gate: qualify facade repeated solve semantics** - `8774ac2` (test; target-contract promotion)
9. **Fix: broken intra-doc links** - `12bcfe5` (fix)

## Files Created/Modified

See `key-files` frontmatter. Highlights: `src/solver/facade.rs` (orchestration), `src/solution/metadata.rs`, `src/solver/error.rs`, `src/solver/options.rs`, `roml-highs/src/facade.rs`, `tests/solver_facade.rs` (fault backends), `roml-highs/tests/facade_tests.rs` + promoted `target_quickstart.rs`/`target_incremental.rs`.

## Decisions Made

- `SolveStatus` is the golden-path status type; `SolverStatus` ships as a compatibility alias (M2 STATE open item 2).
- First-solve sync mode is an orchestration detail (delta-first when the journal retains the chain); the contract is correct synchronization, not a fixed mode.
- Unsupported model changes (semi-continuous) surface as `Err(Synchronization)` after at most one rebuild retry — never a stale or fabricated solution.
- Rebuild-retry recovery (one retry, then correct solve) is a core-level contract proven with fault backends; with real HiGHS all 18 delta ops are supported so the retry path is only reachable on the genuinely-unsupported model path.

## Deviations from Plan

- **Task 6 "rebuild-required recovery" (end-to-end)**: not directly reachable with real HiGHS — every supported model change applies as a delta; the only delta rejection (semi-continuous) is also rejected by rebuild. The end-to-end test asserts the error path (never a fabricated result); the successful one-retry recovery semantics are covered by `tests/solver_facade.rs` fault-backend tests. Documented in `facade_tests.rs` module docs.
- **D7/D10 migration landed in P21** (commits `e89e3de`, `283ff05`): the P20 collision-migration plan (fallible entry points, definition builders, generic `add_constraint`) was implemented here, ahead of its nominal P22 slot, because the target contracts needed it. P22's remaining scope narrows accordingly (see below).
- **Target fixtures promoted within P21** (plan expected P21/P22): once `Highs` landed, the fixtures' bodies compile and execute on real HiGHS — the gate is met.

## Issues Encountered

- **rustdoc `-D warnings`**: three broken/redundant intra-doc links introduced with the fallible entry points and status mapping; fixed in `12bcfe5`.
- **First-solve sync-mode assertion**: initially asserted `Rebuild`; actual orchestration correctly chose the delta chain (journal retains r0→r1). Relaxed to `Delta | Rebuild` with the contract documented.
- **Semi-continuous as rebuild trigger**: originally planned as the end-to-end rebuild-recovery trigger; the snapshot rebuild rejects it too, so the model is genuinely unsolvable with HiGHS — reframed as the error-path test.

## Next Phase Readiness

- **P22 (modeling ergonomics)** — much of its planned surface already landed in P21 via the D7/D10 migration: `Model::named`, `VariableDef`/`ParameterDef` builders (`continuous`/`integer`/`binary`/`parameter` with `.named`/`.bounds`), semantic aliases (`Variable`/`Constraint`/`Objective`/`Parameter`), canonical `add_constraint(spec)`/`minimize`/`maximize`, fallible `set_parameter`. Remaining P22 work per the M2 ROADMAP: names in diagnostics/model formatting, sparse cell semantics (D11 `set_coefficient`/`add_to_coefficient`/`remove_coefficient`), representative LP/MILP/sparse/parameterized compile tests, and validation-error coverage (API-06).
- **P23 (surface curation)** — disposition table now has executing fixtures for the golden path; prelude reduction, `roml::advanced`/`roml::backend` moves, deprecations, and MIGRATION.md remain.
- **P24** — README rewrite to the accepted API (the quickstart now compiles and runs as a doctest), guides, examples, and fresh-consumer qualification.
- No blockers. `cargo package --list -p roml` packaging hygiene (P20 finding: no `include` filter) still open for P24.

## Self-Check: PASSED

- All 6 plan tasks executed; 10 commits on `phase-roml-P21-solver-facade` since main.
- Target contracts compile AND execute (2/2 integration tests + 2 doctests).
- Verification matrix green: fmt clean; clippy `-D warnings` both crates clean; `cargo test -p roml --all-targets` 481 passed / 0 failed; `cargo test -p roml-highs --all-targets` 85 passed / 0 failed; rustdoc `-D warnings` both crates clean; doctests pass (roml: 8 pre-existing ignored; roml-highs: 2 executing quickstart examples).
- P20 baselines still green: `repeated_session_baseline.rs` (3/3) and `public_api_compile.rs` (3/3) unchanged.
- No modifications to `.planning/STATE.md`, `.planning/ROADMAP.md`, or untracked local artifacts.

---
*Phase: 21-solver-facade*
*Completed: 2026-08-02*
