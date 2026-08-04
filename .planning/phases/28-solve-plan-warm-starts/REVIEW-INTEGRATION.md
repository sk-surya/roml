# REVIEW — P28 (SolvePlan, Starts, Hints, Effective-Plan Reporting)

**Review pass:** Pass 2 — Integration and operations
**Reviewer role:** gsd-integration-checker
**Branch:** `phase-roml-P28-solve-plan-warm-starts` @ `f15fe61` (base `main@40af9f4`)
**Scope:** `40ef027`, `286c6c7`, `cd46ebb`, `f15fe61` vs plan commit `d2fdbf0`
**Date:** 2026-08-04

## Verdict summary

**MERGEABLE — no P0 or P1 findings.** One P2 edge-case finding on the
warm-start failure path; two P2 informational notes. All seven Pass-2 review
dimensions verify as wired end-to-end with the evidence file's stated totals
reproduced (roml 911, roml-highs 139, `--test solve_plan` 24/8).

---

## What I ran

| Command | Result |
|---------|--------|
| `cargo test -p roml --test solve_plan` | 24 passed |
| `cargo test -p roml-highs --test solve_plan` | 8 passed |
| `cargo test -p roml --all-targets` | 911 passed |
| `cargo test -p roml-highs --all-targets` | 139 passed |
| `cargo public-api -p roml` | exit 0; new surface present |
| `cargo public-api -p roml-highs` | exit 0; only 2 methods added |
| `git -C . status` / `git -C . log --oneline main..HEAD` | clean tree; 4 commits on plan commit |

**Skipped:** `clippy`, `RUSTDOCFLAGS` doc, `fmt` (evidence covers these and they
are not integration-wiring gates). Never ran workspace-wide, `roml-mosek`, or
`roml-xpress` (per review instructions).

---

## Verification of the Pass-2 dimensions

### 1. Incremental/rebuild behavior — PASS
- Starts/hints cause **no** canonical revision advance and **no** compiled
  identity change: `apply_mip_starts` (`roml-highs/src/start.rs:43-96`) never
  touches `current_compilation`, and
  `highs_qualified_start_leaves_feasible_region_signature_unchanged`
  (`roml-highs/tests/solve_plan.rs:264-293`) asserts revision and the exact
  base `CompilationId` are unchanged after a qualified start solve.
- Dependency-affecting deltas still `RebuildRequired`: unchanged P26/P27 path
  (not in the P28 diff).
- P27 overlay lifecycle preserved inside the plan executor:
  `solve_plan` (`src/solver/facade.rs:591-721`) preserves compile → apply →
  solve → exact-`CompilationId` gate → `rollback_and_verify`. The exact gate
  uses `C_overlay` on an overlay solve and `C_base` otherwise (`:688`).

### 2. Failure recovery — PASS with one P2 edge case
- Start-application failure maps to a **typed** `BackendError` via
  `check_highs_status` (`roml-highs/src/error.rs:23`), never a panic or
  unchecked return. `highs_failed_sparse_solution_maps_to_typed_backend_error`
  (`roml-highs/tests/solve_plan.rs:301-341`) passes.
- No stale-start leak after a solve with a start followed by a solve of a
  **changed** model: both `no_stale_start_leakage_into_subsequent_solve` tests
  pass (the model change forces a rebuild that clears the incumbent).
- **P2 — see Finding 1** for the uncovered multi-start partial-failure path on
  an *unchanged* model.

### 3. Cross-platform/version — PASS
- `highs_capability_set` (`roml-highs/src/session.rs:586-661`) declares
  `MipStart`/`PartialMipStart` Native (audit-cited, `model_classes:["mip"]`)
  and `MultipleMipStarts`/`VariableHints`/`InitialBasis` Unsupported with
  notes citing `highs_mip_start_api.md`. Version notes record the bundled
  1.15.0 and the CI floor 1.9.0.
- `highs_capability_set_declares_start_hint_features_per_audit` verifies both
  `(1,15,0)` and `(1,9,0)`. The audit record (`docs/knowledge/highs_mip_start_api.md`)
  traces every declaration to a pinned symbol with line refs; the floor is
  documented.

### 4. Public API diff — PASS
- `cargo public-api` confirms the new surface (`SolverSession::solve_plan`,
  `EffectiveSolvePlan`, `SolveMetadata::effective_plan`,
  `SolveOptions: PartialEq`). roml-highs adds only `apply_mip_starts` /
  `apply_variable_hints`. **No bridge/session internals leaked** (the internal
  `HighsOverlayState`, `start` module, `ResolvedPlanFeatures`, and session
  fields are not public).
- `SolveOptions` gaining `PartialEq` is the evidence-documented deviation and
  is intentional (needed for `SolvePlan: PartialEq`).

### 5. Package/docs impact — PASS
- Evidence file has baseline captures, per-task RED failures, audit summary,
  full verification matrix, public API diff, deviations, and residual risks
  (Reviewer findings / Gate result correctly reserved for the review gates).
- `docs/knowledge/highs_mip_start_api.md` is a complete audit record.
- `TRACEABILITY.md` carries the P28 closure record (correctly marked pending
  review). `STATE.md`/`ROADMAP.md` and the M2 migration docs are **untouched**
  (verified absent from the diff).

### 6. Migration accuracy — PASS
- `solve()`/`solve_with()` signatures unchanged; both now route through the
  single `solve_plan` executor (`src/solver/facade.rs:520-545`). The three
  modified existing suites (`tests/solver_facade.rs`, `tests/solve_options.rs`,
  `tests/lineage_metadata.rs`) only add default-reject `OverlaySession` impls so
  `solve`/`solve_with` remain available to non-overlay test backends (D27) —
  all pass.

### 7. E2E sanity — PASS
- Traced one end-to-end `solve_plan` execution: `validate` → typed-capability
  preflight (`resolve_plan_features`) → `synchronize_base` (commit/sync) →
  overlay compile/apply → `apply_warm_starts` (user Variable → compiled id →
  native index via `Highs_setSparseSolution`) → solve → exact `CompilationId`
  gate → `rollback_and_verify` → `EffectiveSolvePlan` in `SolveMetadata`. Every
  hop is wired and covered by a passing test.

---

## Findings

### P0 (blocking) — none

### P1 (blocking) — none

### P2 (may merge when accepted/scheduled)

**Finding 1 — Warm-start failure does not force the next solve to rebuild
(partial multi-start leak, unchanged model).**
`src/solver/facade.rs:663-669`

The overlay **apply** failure path defensively calls
`force_rebuild_on_next_sync()` (`src/solver/facade.rs:655`), but the
`apply_warm_starts` failure path does not. `apply_mip_starts`
(`roml-highs/src/start.rs:56-94`) applies starts in a loop, so when start[0]
succeeds (incumbent stored) and start[1] fails natively, the executor returns
`Err` leaving the session `Ready` at the committed revision with start[0]'s
incumbent persisted. The next `solve` of the *same unchanged* model takes the
no-sync fast path (`facade.rs:286`) and the stale incumbent persists as a
search hint.

Impact is bounded: the audit (`highs_mip_start_api.md` §Lifecycle) and the
capability note state a set solution is a search hint that "cannot change a
proven optimum", so no wrong objective/optimum results. It is a determinism /
robustness gap on an abort path the stated requirement ("a subsequent solve of
a *changed* model is deterministic") does not cover, and no test exercises it.

Suggested fix: on the `apply_warm_starts` error path, call
`self.force_rebuild_on_next_sync()` (mirroring the overlay apply path) so a
partially-applied start can never seed a later solve; add a multi-start
partial-failure → subsequent-solve test.

**Finding 2 (informational) — `apply_warm_starts` discards its `effective`
parameter.**
`src/solver/facade.rs:970-986`

`apply_warm_starts(&self, resolved, _effective)` takes `&mut EffectiveSolvePlan`
but never uses it (underscore-prefixed). The applied features are already
recorded in `resolve_plan_features`, so nothing is lost; the unused parameter is
a mild maintenance smell, not a defect.

**Finding 3 (informational) — capability set does not gate on the CI floor.**
`roml-highs/src/session.rs:586-661`

The declarations record `minimum_version` = the runtime version passed in and
note the 1.9.0 floor, but `highs_capability_set` does not downgrade/reject when
called with a version below 1.9.0. This matches the pre-existing M2/P26
behavior and is not a P28 regression; noted only for completeness.

---

## Wiring summary (existence → integration)

| Cross-phase connection | Status |
|------------------------|--------|
| `SolvePlan` types (plan.rs) → executor `solve_plan` (facade.rs) | WIRED |
| `OverlaySession::apply_mip_starts/hints` default-reject (session.rs) → backend override (roml-highs session.rs) | WIRED |
| typed-capability preflight → `resolve_plan_features` (facade.rs) | WIRED |
| start mapping user Variable → compiled id → native index (start.rs) | WIRED |
| exact `CompilationId` gate → `rollback_and_verify` (facade.rs) | WIRED |
| `EffectiveSolvePlan` → `SolveMetadata::effective_plan` → `normalize_result` | WIRED |
| `solve`/`solve_with` → `solve_plan` (single executor) | WIRED |
| capability declarations ↔ conformance + `solve_plan` tests | WIRED |

**Orphaned exports / API routes:** none. **Unprotected sensitive routes:** none.

## Requirements Integration Map

| Requirement | Integration path | Status | Issue |
|-------------|------------------|--------|-------|
| SM-07.1 | `SolvePlan`(plan.rs) → `solve_plan`(facade.rs) | WIRED | — |
| SM-07.2 | `solve`/`solve_with` → `solve_plan` equivalence | WIRED | — |
| SM-07.7 | `EffectiveSolvePlan.objective_stages` → metadata (empty in P28) | WIRED | P31 populates |
| SM-08.1 | `MipStart`(plan.rs) → `Highs_setSparseSolution`(start.rs) | WIRED | Finding 1 (P2) |
| SM-08.2 | capability matrix tests → audit record | WIRED | — |
| SM-08.3 | hints/starts feasibility-signature invariance test | WIRED | — |
| SM-08.4 | default rejection → typed `SolveError::Plan` | WIRED | — |
| SM-08.5 | conversions → `PlanAdjustment`/`AppliedFeature` recorded | WIRED | — |
| SM-08.6 | `InitialBasis` Unsupported (out of scope) | WIRED | separate artifact |
| SM-08.7 | capability declarations ← pinned header audit | WIRED | — |
| SM-04.5 | `EffectiveSolvePlan` on every solve metadata | WIRED | — |

**Requirements with no cross-phase wiring:** none — every P28 requirement has an
integration touchpoint to the executor, backend, or metadata.

---

**Disposition:** Findings 2 and 3 are informational. Finding 1 is P2 and should
be accepted and scheduled (or fixed) before merge, but does not block.
