# Phase 28 — Code Review Report (Pass 1: Specification and Correctness)

**Phase:** P28 — SolvePlan, Starts, Hints, Effective-Plan Reporting
**Review pass:** Pass 1 — Specification and correctness
**Date:** 2026-08-04
**Reviewed commit:** `f15fe61` (base `d2fdbf0`; diff `d2fdbf0..f15fe61`)
**Files reviewed:** 14 (core + wiring + tests + docs/evidence)

---

## Verdict

**HOLD — 1 × P1 (blocks merge).** The single P1 finding is a direct violation of
SM-08.2 and the phase's central "never silently ignore/simulate" blocking
decision: the plan executor does not gate on the `MultipleMipStarts`
capability, so a plan carrying two or more starts (or a qualified start plus a
`ConvertHintToStart` conversion) applies every start through
`Highs_setSparseSolution` and only the **last** start's incumbent survives —
the earlier starts are silently dropped while the effective plan records them
as applied. All remaining findings are P2 (quality/robustness gaps).

---

## Findings

### P1-01 — Multiple starts are silently dropped; `MultipleMipStarts` capability is never enforced

**File:** `src/solver/facade.rs:832-884` (`resolve_plan_features`), `:970-986` (`apply_warm_starts`)
**Requirement violated:** SM-08.2 ("multiple starts are supported only when declared by the backend; otherwise behavior follows explicit policy"), the "never simulate silently" blocking decision, and SM-04.5 (recording accuracy).

**Failure scenario.** `resolve_plan_features` classifies each start (full/partial)
and checks only `MipStart`/`PartialMipStart` — it never reads
`BackendFeature::MultipleMipStarts` (confirmed: zero references in
`src/solver/facade.rs`). A plan with two starts against HiGHS:

1. Both starts pass `plan.validate` (disjoint variable sets).
2. `resolve_plan_features` records **both** as `AppliedFeature { feature: "mip_start", detail: "mip_start[0]" / "mip_start[1]" }`.
3. `apply_warm_starts` calls `Highs_setSparseSolution` for start[0], then start[1]. The pinned header's dense `Highs::setSolution` (`Highs.cpp:2492-2531`) calls `invalidateSolverData()` and overwrites `solution_` on **every** call (`new_primal_solution` is true each time, since the sparse wrapper builds a full `col_value` vector with `kHighsUndefined` fill, `Highs.cpp:2625-2639`).
4. Only start[1]'s values actually seed the solve. Start[0] is silently dropped — yet the metadata claims it was applied.

The same drop occurs when a plan combines a qualified start with a
`ConvertHintToStart` conversion (the converted hint-start is appended at
`facade.rs:905` and lands *last*, overwriting the plan's own start), and
`highs_capability_set` declares `MultipleMipStarts` `Unsupported`
(`roml-highs/src/session.rs:568`).

This is precisely the silent-capability-overreach class the phase is designed to
prevent: SM-08.2 says any second start "follows explicit policy", and the
default policy is `Reject`.

**Fix.** In `resolve_plan_features`, when `plan.mip_starts.len() > 1` (or when a
hint→start conversion would produce a second start), consult
`caps.supports(BackendFeature::MultipleMipStarts)`:
- under `Reject` → return `SolveError::Plan(PlanError::UnsupportedFeature { feature: "MultipleMipStarts", .. })` before any backend mutation;
- under a conversion policy → convert the surplus starts or record a `PlanRejection` for them (never apply them natively and never record them as applied).

Add a regression test in `roml-highs/tests/solve_plan.rs` proving a two-start
plan rejects by default (or records the second start's rejection under an
explicit policy) and that `applied_features` never claims more starts than the
backend actually held.

---

### P2-01 — `solve`/`solve_with` trait-bound tightening is a public-API regression (D27)

**File:** `src/solver/facade.rs:466-468`
**Issue.** `solve` and `solve_with` moved from the `B: BackendSession + SessionHealth + BackendMetadata` impl block (base `d2fdbf0:149-169`) into the `+ OverlaySession` impl block. Any M2 backend that does not implement `OverlaySession` (three required methods: `apply_overlay`, `rollback_overlay`, `verify_overlay_clean`) can no longer call `solve`/`solve_with` at all — a source-incompatibility for M2 backend authors, despite the phase gate claiming "solve and solve_with remain compatible (D27)". The in-repo test backends were given explicit typed-`Unsupported` `OverlaySession` impls, so the suites pass, but the public API bound change is real and would appear in `cargo public-api`.
**Fix.** Either (a) keep `solve`/`solve_with` in the unbound impl and only require `OverlaySession` when the effective plan actually needs an overlay (plain solves never call the required overlay methods), or (b) if the bound is deliberate, document the break in the migration notes and the evidence file.

### P2-02 — `SolvePlan::validate` never validates hint variable existence/activity against the model

**File:** `src/solver/plan.rs:139-146`
**Issue.** The hints loop checks only `hint.value.is_finite()`. A hint keyed by a stale/foreign variable passes validation. Under `ConvertHintToStart` (`facade.rs:893-917`) the hint becomes a `MipStart` referencing an unmapped variable, which then fails inside `apply_mip_starts` (`roml-highs/src/start.rs:60-68`) **after** `synchronize_base` has committed the model and mutated the backend — violating the packet's "validate lineages, entities … before backend mutation" stopping condition. The failure is typed, but late.
**Fix.** In `plan.validate`, add an entity check per hint variable (`model.variable_domain(*variable).ok_or(...)`) and add a `PlanError` variant (or reuse `Assignment(AssignmentError::StaleVariable)`), with a test.

### P2-03 — A failed warm-start application on the plain path leaves a partial incumbent and no `RequiresRebuild` mark

**File:** `src/solver/facade.rs:663-669`, `:970-986`
**Issue.** If `apply_mip_starts` fails partway through multiple starts on the plain (no-overlay) path, the earlier start(s) were already stored as the native incumbent; the session is not marked `RequiresRebuild` (unlike the overlay-apply failure path at `:652-657` which calls `force_rebuild_on_next_sync`). A subsequent solve on the same revision (no-sync fast path) would reuse the native instance carrying the stale incumbent. Harmless for a proven optimum (a start is a search hint), but it violates the T-28-05 "stale solve intent" invariant's spirit.
**Fix.** On a warm-start application error, defensively force the next sync to rebuild (mirror `force_rebuild_on_next_sync`), or clear/roll back any applied incumbent.

### P2-04 — `objective_override` is applied but never recorded in `EffectiveSolvePlan`

**File:** `src/solver/facade.rs:613-616`, `:705-721`
**Issue.** When `plan.objective_override` is present, the override is compiled into the overlay and solved, but no `PlanAdjustment`/`AppliedFeature` entry is recorded (SM-04.5 says "all applications/conversions/rejections are recorded"). A consumer reading `effective_plan` cannot tell that an objective override was applied.
**Fix.** Record a `PlanAdjustment { key: "objective_override", requested: <model objective>, applied: <override obj>, reason: ... }` in `resolve_plan_features`/`solve_plan` when the override is present.

### P2-05 — `solve_with_overlay` with an empty overlay changes observable metadata

**File:** `src/solver/facade.rs:623-632`
**Issue.** `apply_overlay` is now decided by content. `solve_with_overlay(model, options, &empty_overlay, override)` (or `Some(override)` with an empty overlay) previously always compiled/applied the overlay, tagging the result `compilation_id = C_overlay`, `overlay_id = Some(id)`; now an empty overlay takes the plain path and reports `C_base`/`None`. This is a silent metadata-behavior change for a degenerate-but-legal input.
**Fix.** Either preserve the always-apply behavior for the explicit `solve_with_overlay` entry point, or document the change and assert it in tests.

### P2-06 — Feasibility-signature invariance proven only for MIP starts, not hints or LP

**File:** `tests/solve_plan.rs:1282-1330`, `roml-highs/tests/solve_plan.rs:264-293`
**Issue.** SM-08.3 ("hints never change feasibility") is asserted via `PlanTestBackend`, whose deterministic objective is computed *from* the applied values — the proof is tautological for a fake backend, not a real feasibility-region demonstration. On real HiGHS, hints always reject, so there is no hint feasibility proof at all, and no LP start proof exists (the requirement to prove the invariant for both MIP and LP is not met).
**Fix.** Add a real-MIP hint proof if/when a backend qualifies hints; at minimum, document in the evidence that the LP/hints legs are vacuous or untested on the pinned backend.

### P2-07 — `MipStart` capability `model_classes: ["mip"]` limitation is declared but never enforced

**File:** `roml-highs/src/session.rs:624-631`, `src/solver/facade.rs:807-810`
**Issue.** `BackendCapabilitySet::supports` (`src/compiler/capability.rs:207-211`) ignores `FeatureLimitations.model_classes`. A `MipStart` on a pure-LP model is accepted and applied via `Highs_setSparseSolution` (which HiGHS accepts), so the declared "mip model class" limitation is informational only. Not wrong today (a start on LP is a harmless hint), but a capability declaration that is not enforced invites drift.
**Fix.** Either enforce the model-class limitation in the executor (check the model's integer/binary presence) or drop the limitation from the declaration and note that starts apply to any model class.

### P2-08 — `Highs_setSparseSolution` `kWarning` (duplicate indices) is mapped to an error

**File:** `roml-highs/src/error.rs:23-34`
**Issue.** The audit correctly documents `kHighsStatusWarning = 1` and "warns (last value wins) on duplicate indices", but `check_highs_status` treats any non-`STATUS_OK` return as an error. A duplicate-index warning from `Highs_setSparseSolution` would fail the solve even though HiGHS defines it as a successful (last-wins) outcome. Unreachable through a validated plan today (values are keyed by `BTreeMap` and `plan.validate` rejects duplicate variables across starts), so theoretical.
**Fix.** Either treat `STATUS_WARNING` as success for `Highs_setSparseSolution` specifically, or document why duplicates cannot occur and assert it.

---

## Verified-sound areas (no findings)

- **Single plan executor.** `solve` → `solve_with` → `solve_plan` and `solve_with_overlay` → `solve_plan` all route through one executor; no divergent plain-solve path (evidence claim confirmed by source).
- **C_overlay lifecycle intact.** `solve_plan` re-establishes `C_base` via `synchronize_base`, compiles the overlay against the exact base, applies, solves, enforces the exact `CompilationId` gate (`expected = C_overlay` on overlay solves, `C_base` on plain solves; a `C_base`-tagged result on an overlay solve is a typed `CompilationMismatch`), rolls back and verifies, and normalizes with the result's exact id. The `compiler.current_compilation()` stays `C_base` throughout an overlay solve.
- **Default rejection (SM-08.4).** Unqualified starts/hints under `Reject` return `SolveError::Plan(PlanError::UnsupportedFeature)` before any backend mutation (verified by the `solves == 0` assertion). The `OverlaySession` default methods return typed `Unsupported` `BackendError`s; no backend can silently ignore a start/hint request.
- **Conversions explicit and recorded (SM-08.5).** `ConvertStartToTemporaryFixing` records a `PlanAdjustment`; `ConvertHintToStart` records both a `PlanAdjustment` and an `AppliedFeature` (the evidence's Rule 2 fix is present). Qualified-wins-over-conversion is implemented.
- **Exact `CompilationId` gate (F2).** Verified against both the base and overlay paths.
- **Audit record.** `docs/knowledge/highs_mip_start_api.md` matches the pinned `highs_c_api.h` verbatim (`Highs_setSparseSolution` at `:1305`, `Highs_setSolution` at `:1291`, basis setters at `:1264`/`:1274`, `kHighsStatus` constants at `:28-30`, and the confirmed absence of `Highs_setMipStart`/`Highs_clearMipStart`/`Highs_clearSolution`/hint symbols). The C++ semantics (out-of-range index → `kError`, out-of-bounds value → `kError`, duplicate → `kWarning` last-wins, empty-model → `kOk` no-op) were spot-checked in `Highs.cpp:2576-2640` and match the audit. Capability declarations trace to the audit; `VariableHints`/`MultipleMipStarts`/`InitialBasis` stay typed `Unsupported`.
- **No-stale-start determinism.** The HiGHS test exercises a changed-model second solve with a stale (infeasible) incumbent; the proven-optimum invariant holds. The audit's lifecycle claims are accurate enough to support the invariant.

---

## Findings summary

| Severity | Count | IDs |
|----------|-------|-----|
| P0 | 0 | — |
| P1 | 1 | P1-01 |
| P2 | 7 | P2-01 … P2-08 |

**Verdict: HOLD.** P1-01 (silent drop of multiple starts / unenforced `MultipleMipStarts`) blocks merge. Resolve it, re-run the phase verification matrix, and re-review before accepting.

---

_Reviewed: 2026-08-04_
_Reviewer: Claude (gsd-code-reviewer, Pass 1 — Specification and correctness)_

## Dispositions (orchestrator, 2026-08-04)

### P1-01 — second+ start silently dropped (`MultipleMipStarts` never consulted) — **FIXED**

Verified: `resolve_plan_features` checked each start only against `MipStart`/`PartialMipStart`; `MultipleMipStarts` had zero references. The pinned header's `Highs_setSparseSolution` overwrites the previous incumbent on every call, so an undeclared second start was silently dropped while recorded as applied (SM-08.2).

Fix (TDD, 3 tests first → RED, then implementation → GREEN): the starts loop now treats `index >= 1` as requiring `MultipleMipStarts` through the SAME policy ladder as any unqualified feature — `Reject` → typed `PlanError::UnsupportedFeature { feature: "MultipleMipStarts" }`; `ConvertStartToTemporaryFixing` → recorded conversion into overlay temporary fixings; and `ConvertHintToStart` records a `PlanRejection` when the conversion would create a second start (never silently overwriting the plan's own start). New tests: `second_start_rejects_without_multiple_mip_starts_capability`, `second_start_follows_conversion_policy_under_convert_to_fixing`, `hint_to_start_conversion_never_silently_creates_second_start`.

### P2-01 — `solve`/`solve_with` trait-bound regression (D27) — **FIXED**

The three `OverlaySession` lifecycle methods (`apply_overlay`, `rollback_overlay`, `verify_overlay_clean`) now have default implementations returning typed `Unsupported` `BackendError`s — the same default-reject contract the plan already specified for the warm-start methods (Task 2, SM-08.4). An M2 backend author's migration is exactly one empty `impl OverlaySession for X {}` line; `solve`/`solve_with` call sites compile unchanged. No backend can silently "support" overlays: the executor's C_overlay path fails with the typed error before any native mutation.

### P2-02 — `SolvePlan::validate` never checks hint variables — **FIXED**

The hints loop now rejects a hint keyed by a variable absent from the model (`variable_domain` miss) with `PlanError::Assignment(AssignmentError::StaleVariable)` — before any synchronization or backend mutation. New test `stale_hint_variable_rejects_in_plan_validation` asserts `solves == 0` (the packet's "validate … entities … before backend mutation" stopping condition).

### P2-03 — failed warm-start application leaves no RequiresRebuild mark — **FIXED**

The warm-start failure path now calls `force_rebuild_on_next_sync()` (CR-02 mirror) so a partially-applied incumbent is wiped by the next sync's rebuild and can never seed a later solve of the unchanged model. New test `warm_start_failure_forces_rebuild_and_blocks_stale_incumbent` proves the next solve takes the rebuild path (`rebuilds` counter) and returns the deterministic base values.

### P2-04 — `objective_override` applied but never recorded — **FIXED**

The executor records `PlanAdjustment { key: "objective_override" }` whenever the override applies (SM-04.5). New test `objective_override_is_recorded_in_effective_plan`.

### P2-05 — empty overlay changes observable metadata — **ACCEPTED, documented**

The empty-overlay plain path is the plan's intended design (Task 2: "An empty overlay keeps the plain `C_base` path so an empty `solve_plan` is exactly `solve`/`solve_with` (D27, SM-07.2)"). For the degenerate `solve_with_overlay(model, options, empty, None)` input, metadata now reports `C_base`/`overlay_id: None` instead of a compiled empty overlay; this is a consequence of the equivalence design and is recorded in the evidence.

### P2-06 — feasibility-signature proof scope (hints/LP legs) — **ACCEPTED, documented**

On the pinned HiGHS, `VariableHints` always rejects (the audit found no hint API), so the hint leg is vacuous by design; the LP-start leg is a harmless search hint (a hint cannot change a proven optimum, per the audit). The MIP-start proof on real HiGHS (`pwl_highs_...`-style equivalence in `roml-highs/tests/solve_plan.rs`) stands. Recorded in the evidence; a real hint feasibility proof becomes possible only when a backend qualifies hints.

### P2-07 — unenforced `model_classes: ["mip"]` limitation — **FIXED**

The audit shows `Highs_setSparseSolution` accepts primal solution values for any model class, so the "mip only" limitation did not trace to the audit. The declaration no longer claims it; the notes now state the audit-derived semantics.

### P2-08 — `kWarning` mapped to error — **ACCEPTED, documented**

Duplicate indices cannot occur through a validated plan (assignments are `BTreeMap`-keyed and `plan.validate` rejects duplicate variables), so the warning path is unreachable. `check_highs_status` stays strict by design; documented in the evidence.

**Residual (accepted):** none blocking. P2-05/06/08 recorded with dispositions.
