---
phase: 20-public-api-contract
plan: 20
subsystem: api
tags: [public-api, rust, roml-highs, cargo-public-api, characterization, tdd, baseline-evidence]

# Dependency graph
requires:
  - phase: M2-planning-packet (DECISIONS.md, EXECUTION.md, REQUIREMENTS.md)
    provides: locked API decisions (D1-D13), requirement IDs, execution/evidence protocol
provides:
  - exact current-main public API baseline (cargo public-api inventory + full test matrix)
  - compile characterization that the documented HighsAdapter/solve_model path is missing
  - frozen target compile contracts for the P21/P22 golden path and incremental workflows
  - per-item disposition of the entire public surface (golden path / sugar / advanced / deprecated / remove)
  - repeated-solve HighsSession protocol baseline for the P21 SolverSession/Highs facade
affects: [21-solver-facade, 22-modeling-ergonomics, 23-surface-curation, 24-consumer-qualification]

# Actuals (#2632) — pairs with the plan's `estimate` to calibrate future estimates.
# estimateTokens scale (chars/4) over the realized diff of the handwritten source/docs;
# the two machine-generated cargo-public-api evidence dumps are captured artifacts, not counted.
actuals:
  tokens: 10783
  tasks: 5
  commits: 6

# Tech tracking
tech-stack:
  added:
    - cargo-public-api 0.52.0 (public API inventory + drift evidence, API-07.5)
  patterns:
    - UI-fixture characterization: intentionally non-compiling contracts live in tests/ui/
      (not auto-discovered), so default suites stay green while freezing target/drift code
    - compile-pass current-surface characterization (tests/public_api_compile.rs) as the
      executable counterpart to the non-compiling target contracts
    - evidence-first baseline docs recording command, toolchain, SHA, exit status, and test counts

key-files:
  created:
    - docs/release/evidence/M2_P20_BASELINE.md
    - docs/release/evidence/M2_P20_public_api_roml.txt
    - docs/release/evidence/M2_P20_public_api_roml_highs.txt
    - docs/release/PUBLIC_API_M2_DISPOSITION.md
    - tests/ui/current_readme_drift.rs
    - tests/ui/target_quickstart.rs
    - tests/ui/target_incremental.rs
    - tests/public_api_compile.rs
    - roml-highs/tests/repeated_session_baseline.rs
  modified: []

key-decisions:
  - "P20 produces characterization-only evidence and contract fixtures; no production API is implemented (plan Gate)."
  - "Target signatures are frozen verbatim from the M2 packet in tests/ui/ and must not be weakened without amending DECISIONS.md and review (plan Task 3)."
  - "Task 5's direct HighsSession test lives in roml-highs/tests/repeated_session_baseline.rs (the crate it exercises); tests/public_api_compile.rs is the current-surface compile-pass characterization."
  - "Five dispositions adopted for every public item: golden path / optional syntax sugar / advanced backend extension / compatibility-deprecated / internal exposure to remove."

patterns-established:
  - "Non-compiling UI fixtures under tests/ui/ document drift and target contracts without breaking cargo test --all-targets (API-10.1)."
  - "Repeated-solve baseline records revision, health, termination, objective, and solution availability per step for P21 facade parity."

requirements-completed: [API-04, API-07, API-08, API-10]

# Coverage metadata (#1602) — characterization portions of API-04/07/08/10 (TRACEABILITY.md).
coverage:
  - id: D1
    description: "Exact current-main public API baseline: base SHA, toolchain, full core+HiGHS matrix with exit statuses and test counts, normalized cargo public-api inventory for roml (7431 lines) and roml-highs (80 lines)."
    requirement: "API-10"
    verification:
      - kind: other
        ref: "docs/release/evidence/M2_P20_BASELINE.md (baseline matrix + public-api inventory)"
        status: pass
    human_judgment: true
    rationale: "Baseline commands, public-item counts, and skipped-check reasons require reviewer confirmation per the P20 gate (EXECUTION.md evidence standards)."
  - id: D2
    description: "Documented solve API drift characterized: README's HighsAdapter/solve_model does not compile; frozen in tests/ui/current_readme_drift.rs with E0432/E0599 captures."
    requirement: "API-08"
    verification:
      - kind: other
        ref: "tests/ui/current_readme_drift.rs + recorded compile failures in M2_P20_BASELINE.md"
        status: pass
    human_judgment: true
    rationale: "Intentionally non-compiling fixture is characterization evidence, not a regression; reviewer must confirm the captured failure matches the documented API."
  - id: D3
    description: "Frozen target compile contracts for the P21/P22 golden-path quickstart and incremental one-Highs re-solve, exactly as specified in the plan."
    requirement: "API-04"
    verification:
      - kind: other
        ref: "tests/ui/target_quickstart.rs"
        status: pass
      - kind: other
        ref: "tests/ui/target_incremental.rs"
        status: pass
    human_judgment: true
    rationale: "Target fixtures do not compile yet by design; reviewer confirms the signatures match DECISIONS.md and the plan's exact forms and are not weakened."
  - id: D4
    description: "Compile-pass characterization of the current roml prelude surface that does compile and run today."
    requirement: "API-10"
    verification:
      - kind: integration
        ref: "tests/public_api_compile.rs#readme_method_style_compiles_and_runs"
        status: pass
      - kind: integration
        ref: "tests/public_api_compile.rs#current_prelude_surface_compiles"
        status: pass
      - kind: integration
        ref: "tests/public_api_compile.rs#low_level_and_between_forms_compile"
        status: pass
    human_judgment: false
  - id: D5
    description: "Per-item disposition of the entire public surface (root/prelude, model, expressions, macros, IDs/coefficients, solution/status/result, backend/sync, callback/capability) with replacement signatures and deprecation order."
    requirement: "API-07"
    verification:
      - kind: other
        ref: "docs/release/PUBLIC_API_M2_DISPOSITION.md"
        status: pass
    human_judgment: true
    rationale: "The disposition table and deprecation order are an API review artifact required by the P20 gate; automation cannot judge naming/curation coherence."
  - id: D6
    description: "Repeated-solve HighsSession protocol baseline: rebuild+solve, parameter delta re-solve (12.0->20.0), bound delta re-solve (12.0->8.0), and dirty-path deterministic snapshot recovery."
    requirement: "API-02"
    verification:
      - kind: integration
        ref: "roml-highs/tests/repeated_session_baseline.rs#repeated_solve_rebuild_then_parameter_delta"
        status: pass
      - kind: integration
        ref: "roml-highs/tests/repeated_session_baseline.rs#repeated_solve_bound_delta_updates_optimal"
        status: pass
      - kind: integration
        ref: "roml-highs/tests/repeated_session_baseline.rs#dirty_path_recovers_via_deterministic_snapshot_rebuild"
        status: pass
    human_judgment: false

# Metrics
duration: 20min
completed: 2026-08-02
status: complete
---

# Phase 20: Public API Contract Baseline Summary

**Frozen M2 public API baseline, drift, target contracts, dispositions, and repeated-solve protocol behavior — with zero production code changes**

## Performance

- **Duration:** ~20 min
- **Started:** 2026-08-02T06:04:00Z (estimated; pre-commit baseline build work)
- **Completed:** 2026-08-02T06:24:15Z
- **Tasks:** 5
- **Files modified:** 9 created (7 handwritten + 2 machine-generated public-api evidence dumps)

## Accomplishments

- Recorded the exact current-main baseline: base SHA `d1391fb`, toolchain (`rustc 1.97.1`, `aarch64-apple-darwin`), full core (399 passed) and HiGHS (73 passed) matrices, and normalized `cargo public-api` inventories for both crates (API-07.5/API-10.1; absolute repo paths replaced with `$REPO`).
- Characterized the documentation drift: README/MODELING_API `HighsAdapter::solve_model` does not exist in production code; compile evidence (E0432 unresolved import, E0599 no method) frozen in `tests/ui/current_readme_drift.rs` and `tests/ui/current_solve_model_method.rs`, reproduced by `scripts/p20-capture-drift.sh`.
- Defined frozen target compile contracts for the golden-path quickstart and incremental one-`Highs` re-solve, exactly as planned, plus a green compile-pass characterization of the current `roml` prelude surface.
- Classified every public entry point with exactly one of five dispositions, with replacement signatures (D7 builders, `Highs` façade) and a replacement-before-deprecation order (D12/API-08).
- Baselined repeated-solve `HighsSession` behavior: rebuild→solve (obj 12.0), parameter delta re-solve (obj 20.0), bound delta re-solve (obj 8.0), and deterministic dirty-path snapshot recovery — the expected parity targets for P21.

## Task Commits

Each task committed atomically on `phase-roml-P20-api-contract`:

1. **Task 1: Record exact baseline** - `fa8e52b` (docs)
2. **Task 2: Characterize documentation drift** - `cdc28fe` (test)
3. **Task 3: Add target compile contracts** - `811bd51` (test)
4. **Task 4: Inventory and disposition every public entry point** - `4403f82` (docs)
5. **Task 5: Baseline repeated-solve protocol behavior** - `fc1e23d` (test)
6. **Formatting fix** - `890a98a` (style)

**Plan metadata commit:** `20-SUMMARY.md` committed in the final metadata commit (this phase).

## Files Created

- `docs/release/evidence/M2_P20_BASELINE.md` - Base SHA, environment, full core+HiGHS matrix, public API item counts, drift compile captures, skipped-check reasons, and the repeated-solve behavior table.
- `docs/release/evidence/M2_P20_public_api_roml.txt` - Normalized `cargo public-api -p roml` output (7431 lines; repo paths replaced with `$REPO`).
- `docs/release/evidence/M2_P20_public_api_roml_highs.txt` - Normalized `cargo public-api -p roml-highs` output (80 lines; confirms no HighsAdapter/solve_model).
- `docs/release/PUBLIC_API_M2_DISPOSITION.md` - Per-item disposition table (all 9 required categories) with replacement signatures and deprecation order.
- `tests/ui/current_readme_drift.rs` - Frozen README `HighsAdapter::solve_model` example (UI fixture, not auto-discovered).
- `tests/ui/target_quickstart.rs` - Frozen P21/P22 golden-path target contract.
- `tests/ui/target_incremental.rs` - Frozen P21/P22 incremental one-`Highs` target contract.
- `tests/public_api_compile.rs` - Green compile-pass tests of the current `roml` prelude surface (3 tests).
- `roml-highs/tests/repeated_session_baseline.rs` - Repeated-solve `HighsSession` baseline (3 tests).

## Decisions Made

- The plan was executed with **no production code changes**; all deliverables are characterization tests, contract fixtures, docs, and evidence, exactly as the plan's Files list and Gate require.
- Task 5's direct `HighsSession` test was placed in `roml-highs/tests/repeated_session_baseline.rs` (the crate it exercises) rather than in `tests/public_api_compile.rs`; that file instead carries the current-surface compile-pass characterization (the only other top-level test the plan names). No signature or requirement was weakened.
- Five dispositions applied per public item, following DECISIONS.md D1-D13: golden path (method-first modeling, `Solution`), optional sugar (`constraint!`/`objective!`), advanced backend extension (protocol/sync/session types moving to `roml::advanced`/`roml::backend`), compatibility/deprecated (effectful macros, raw creation methods, infallible `set_parameter`), and internal-exposure-to-remove (`Generation`, `IdArena`, `ModelConstants`, `deltas_since`).

## Deviations from Plan

- **Task 5 test placement** - The plan's Files list did not tie `tests/public_api_compile.rs` to a task. It was used for the current-surface compile-pass characterization (Task 3) and the repeated-solve test was created at `roml-highs/tests/repeated_session_baseline.rs`. Structural interpretation only; no scope change.
- **Formatting commit** - After Task 5, `cargo fmt --all` reformatted two new test files; a `style: rustfmt new P20 test fixtures` commit (`890a98a`) was added. No behavior change (tests still pass).
- **Skipped check (documented, not a code deviation)** - `cargo package --list -p roml` returned exit 101 because the working tree contains untracked local artifacts (`.planning/`, `graphify-out/`) outside this phase's scope; `cargo package` requires a clean tree. Recorded with reason in the baseline; the `roml-highs` package list (exit 0) is unaffected. Expected to pass in a clean checkout.

No auto-fixed bugs were required; the plan executed without Rule 1-4 deviation events.

## Issues Encountered

- **`.le` resolving to `Iterator::le`** - In the new HiGHS test, `(x + y).le(4.0)` initially failed with "LinExpr is not an iterator" because `ConstraintExprExt` was not in scope. Resolved by importing the trait.
- **`deltas_since` error type** - Returns `RevisionError`, not `ModelError`; test functions use `Result<(), Box<dyn std::error::Error>>`.
- **Stale-solution invalidation on a REJECTED delta (finding for P21)** - Current `HighsSession::synchronize` clears the cached solution on *successful* sync (T-11-18) but leaves `current_solution` readable via `SolutionView` when a delta is *rejected* (mismatched base) while the cursor moves to `RequiresRebuild`. The P20 dirty-path test now asserts this readable-but-stale state explicitly (objective still 12.0), then verifies revision/health recovery and a correct post-rebuild solve. P21 (API-01.5) must decide whether a rejected/unsupported sync must also invalidate the previously reported solution when the model has advanced past it.

## User Setup Required

None - no external service configuration required. `roml-highs` builds HiGHS bundled via `highs-sys`; no system HiGHS needed.

## Next Phase Readiness

- **P21 (solver façade and unified result)** can start with: the exact target quickstart/incremental signatures (Task 3), the repeated-solve behavior table as parity targets (Task 5), and the disposition mapping of `SolverSession<B>`, `Solution`/`SolveStatus`, and `Highs` (Task 4). The stale-solution-on-rejected-delta question above should be resolved in P21's failure-recovery tests.
- **P22 (modeling ergonomics)** consumes the frozen `continuous()/integer()/binary()/parameter(value)` builder signatures and `Model::named`/`add_constraint(spec)` forms.
- **P23 (surface curation)** consumes the disposition table for prelude reduction, deprecations, and `roml::advanced`/`roml::backend` namespace moves.
- No blockers. The M2 planning blockers (compat window, SolveStatus naming, `SolverSession<B>` name, alias-vs-wrapper) remain open as recorded in M2 STATE.md and are decided during P21/P22, not here.

## Self-Check: PASSED

- All 9 phase files present (3 evidence/docs, 3 tests, 3 ui fixtures) plus the SUMMARY.
- All 7 commits present on `phase-roml-P20-api-contract`:
  `fa8e52b`, `cdc28fe`, `811bd51`, `4403f82`, `fc1e23d`, `890a98a`, `8ebbb1d`.
- Verification matrix green: `cargo fmt --all -- --check` (0), clippy both crates `-D warnings` (0),
  `cargo test -p roml --all-targets` (402 passed / 0 failed), `cargo test -p roml-highs --all-targets`
  (76 passed / 0 failed), rustdoc `-D warnings` both crates (0).
- No modifications to `.planning/STATE.md`, `.planning/ROADMAP.md`, or untracked local artifacts
  (`.planning/config.json`, `.planning/graphs/`, `graphify-out/` untouched).

---
*Phase: 20-public-api-contract*
*Completed: 2026-08-02*
