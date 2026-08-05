# Phase 28 — Code Review Report (Pass 1: Specification and Correctness)

**Phase:** P28 — SolvePlan, Starts, Hints, Effective-Plan Reporting
**Review pass:** Pass 1 — Specification and correctness
**Date:** 2026-08-04
**Initially reviewed commit:** `f15fe61` (base `d2fdbf0`)
**Re-review commits:** `838e577` (round 2), `14e2e72` (round 3)

---

## Verdict

**CLEAR — 0 × P0, 0 × P1.** The blocking P1-01 and the owner's P1-1/P1-2 from
the second disposition are correctly fixed and re-verified by me on `14e2e72`
(source trace + tests run). The phase may merge pending Pass 2 sign-off and the
gate-result recording.

---

## Re-review (round 2) — after fix commit `838e577`

Verified each orchestrator-supplied fix against the source and by running the
test suites myself.

### 1. MultipleMipStarts gate covers all three policy paths; ConvertHintToStart guard not bypassable — VERIFIED

`resolve_plan_features` (`src/solver/facade.rs:850-884`) now computes
`multiple = index >= 1` and gates the second+ start on
`caps.supports(BackendFeature::MultipleMipStarts)` through the full policy
ladder:

- **Reject** → `SolveError::Plan(PlanError::UnsupportedFeature { feature: "MultipleMipStarts", .. })` before any backend mutation — `second_start_rejects_without_multiple_mip_starts_capability` asserts the exact feature name.
- **ConvertStartToTemporaryFixing** → the second start merges into overlay `temporary_fixings` and is recorded as a `PlanAdjustment` whose reason names `MultipleMipStarts` — `second_start_follows_conversion_policy_under_convert_to_fixing` asserts the adjustment AND the applied values (`x=3.0` native start, `y=5.0` converted fixing).
- **ConvertHintToStart** → a second start has no applicable conversion and is recorded as a `PlanRejection` (the `else` branch).

The `ConvertHintToStart` second-start guard at `src/solver/facade.rs:923-932`
(`if !starts.is_empty() && !multiple_mip_starts_qualified`) was analyzed for
bypass: (a) 2 qualified starts + hints → start[1] rejects via the ladder, then
hints see `starts` non-empty → rejected; (b) unqualified starts under
ConvertHintToStart are recorded as rejections (never pushed), so `starts`
stays empty and the hints legitimately convert to the ONE native start;
(c) start[0] qualified + hints → guard fires → hints recorded as rejection
(`hint_to_start_conversion_never_silently_creates_second_start` asserts
`solution.value(x) == 3.0`, i.e. the explicit start is not overwritten).
No ordering bypass exists — the starts loop (832-884) runs before the hints
branch (886-943).

**Residual P2 note (accepted, theoretical):** for `index >= 1`, the gate checks
`multiple_mip_starts_qualified` *instead of* AND-ing with the per-start
`MipStart`/`PartialMipStart` check. A hypothetical backend declaring
`MultipleMipStarts` without `MipStart`/`PartialMipStart` would accept a second
start whose base feature is unqualified. `MultipleMipStarts` is never qualified
in this phase (HiGHS: `Unsupported`), so this is unreachable today.
*(This residual is resolved in round 3 by the composed gate — see below.)*

### 2. Trait defaults cannot let a backend silently ignore an overlay request — VERIFIED

`OverlaySession::apply_overlay`/`rollback_overlay`/`verify_overlay_clean` now
default to typed `Unsupported` `BackendError`s (`src/solver/session.rs:147-190`).
The executor's overlay path (`src/solver/facade.rs:650-657`) maps a default
`apply_overlay` failure to `force_rebuild_on_next_sync()` + typed
`SolveError::Rollback` — nothing is silent, and the default does no native
mutation. This also resolves the P2-01 D27 burden: an M2 backend author needs
only an empty `impl OverlaySession for MyBackend {}` to keep `solve`/`solve_with`.
*(Superseded in round 3 by the unbounded `solve_base` — see below.)*

### 3. Stale-hint check runs before any backend call — VERIFIED

`SolvePlan::validate` (`src/solver/plan.rs:146-152`) now rejects a hint keyed
by a variable absent from the model with
`PlanError::Assignment(AssignmentError::StaleVariable)`. `solve_plan` calls
`plan.validate(model)` first (`src/solver/facade.rs:598`), before
`resolve_plan_features` and `synchronize_base`, so no backend call/mutation can
precede the check. `stale_hint_variable_rejects_in_plan_validation` asserts
`solves == 0`.

### 4. Warm-start failure fix + test genuinely prove the rebuild — VERIFIED

`apply_warm_starts` failure now calls `force_rebuild_on_next_sync()`
(`src/solver/facade.rs:671-674`). The fake's `rebuilds` counter increments only
in the `CompiledRebuild` branch of `PlanTestBackend::synchronize`
(`tests/solve_plan.rs:838-841`), and the failed start's values are inserted
into `current_start` *before* the injected error (`:988-994`) — the worst case.
`warm_start_failure_forces_rebuild_and_blocks_stale_incumbent` asserts:
(1) `rebuilds == 1` after the clean base solve (the delta-path base
establishment is a `CompiledRebuild`), (2) the failed attempt never reaches the
solver (`solves == 1`), (3) the next solve of the UNCHANGED model hits
`rebuilds == 2` (the reset compiler forces the F4 snapshot-rebuild branch), and
(4) the stale start value `5.0` does not seed it (`again.value(x) == clean_x`).
Without the fix the no-sync fast path would reuse `current_start = 5.0` and the
test would fail — the proof is genuine, not tautological.

### 5. No existing test weakened — VERIFIED

The fix commit `838e577` touches only 8 files; `roml-highs/tests/conformance.rs`
is unchanged in the fix. The `model_classes: ["mip"]` removal from the
`MipStart`/`PartialMipStart` declaration does not affect any test (the
`model_classes` assertions in `src/compiler/capability.rs` / `tests/typed_capabilities.rs`
are generic `FeatureLimitations` fixture tests, not start-feature assertions).
The objective-override `PlanAdjustment` is only pushed when an override is
present, so the empty-plan equivalence assertions (which require
`adjustments.is_empty()`) still hold.

### 6. Tests run by me

| Command | Result |
|---|---|
| `cargo test -p roml --test solve_plan` | **30/30 passed** (was 24; +3 multi-start, +1 warm-start-failure, +1 stale-hint, +1 override) |
| `cargo test -p roml-highs --test solve_plan` | **8/8 passed** |
| `cargo test -p roml --test solve_overlay` | **61/61 passed** (no effective-plan regression) |
| `cargo test -p roml-highs --test conformance` | **4/4 passed** |
| `cargo test -p roml --all-targets` | all green |
| `cargo test -p roml-highs --all-targets` | all green (139) |

### Disposition of the remaining P2s (accepted, documented)

- **P2-05 (empty-overlay metadata):** accepted as a design decision — an empty
  overlay has no content, so the plain `C_base` path is the equivalence
  guarantee; the old always-apply behavior for a degenerate empty overlay was
  not load-bearing.
- **P2-06 (LP/hints feasibility proof):** accepted — hints are vacuous by
  design on the pinned backend (always `Unsupported`, never applied), and the
  MIP start feasibility proof is real on HiGHS.
- **P2-08 (`kWarning` → error):** accepted — duplicate native indices are
  unreachable through a validated plan (per-start `BTreeMap` keys +
  `DuplicateStartVariable` rejection), so a warning can never be emitted.

---

## Findings (Pass 1, as originally issued)

### P1-01 — Multiple starts silently dropped; `MultipleMipStarts` capability never enforced — **FIXED in `838e577`, refined in `14e2e72`, re-verified**

**File:** `src/solver/facade.rs:832-884`, `:970-986` (original).
**Disposition:** Fixed as described in the re-review sections; the three new
tests cover Reject / ConvertStartToTemporaryFixing / ConvertHintToStart, and
`apply_warm_starts` no longer receives a multi-start list the backend cannot
hold. The composed gate in `14e2e72` additionally removes the round-2 residual
(primitive AND multiple composition).

### P2-01 — `solve`/`solve_with` trait-bound tightening (D27) — **FIXED**

`src/solver/session.rs:147-190` — lifecycle methods defaulted; M2 migration is
now an empty impl line. *(Round 3 supersedes the mechanism with the unbounded
`solve_base` — see the round-3 section.)*

### P2-02 — `SolvePlan::validate` never validates hint variable existence — **FIXED**

`src/solver/plan.rs:146-152` — stale/foreign hint variables reject with typed
`PlanError::Assignment(StaleVariable)` before any backend call.

### P2-03 — Warm-start failure leaves partial incumbent, no rebuild — **FIXED**

`src/solver/facade.rs:671-674` — `force_rebuild_on_next_sync()` on failure;
test proves the rebuild and that the failed start cannot seed the next solve.

### P2-04 — `objective_override` applied but never recorded — **FIXED**

`src/solver/facade.rs:617-627` — recorded as a `PlanAdjustment`
(`key: "objective_override"`); `objective_override_is_recorded_in_effective_plan`
asserts it.

### P2-05 / P2-06 / P2-08 — **ACCEPTED with documented dispositions** (see above)

### P2-07 — `model_classes: ["mip"]` declared but unenforced — **FIXED**

`roml-highs/src/session.rs:620-627` — the limitation is dropped; the
declaration now traces to the audit (`Highs_setSparseSolution` accepts any
model class).

---

## Second review round (owner disposition, PR #32)

### P1-1 — D27 source compatibility: solve/solve_with require OverlaySession — **FIXED**

Verified: `solve`/`solve_with` lived in the `+ OverlaySession` impl block, so a backend implementing exactly the pre-P28 traits (`BackendSession + SessionHealth + BackendMetadata`) could not call them — default trait methods do not implement the trait for a type; the executor-added boilerplate impls on the legacy test backends were the symptom.

Fix (compile-time regression first — verified it catches the break by reverting the facade: 4 E0599s; restored):

- `solve`/`solve_with` moved back into the UNBOUNDED impl block (`B: BackendSession + SessionHealth + BackendMetadata`), implemented through a shared plain core **`solve_base`** (options validation + capability preflight + C_base sync + solve + exact `CompilationId` gate + metadata normalization).
- `solve_plan` (bounded) delegates to `solve_base` when the effective plan has no overlay content, no objective override, and no starts/hints — the `solve == solve_with == empty solve_plan` equivalence is one code path.
- The three boilerplate `OverlaySession` impls on the legacy test backends (`RecordingBackend`, `LineageTestBackend`, `TestBackend`) are **removed** — their solve/solve_with call sites compile unchanged, the runtime proof.
- New compile-time regression `d27_plain_solve_callable_without_overlay_session`: a `PreP28Backend` implementing exactly the pre-P28 traits (no `OverlaySession`) drives a generic bound-checked call of `solve`/`solve_with` and an end-to-end optimal solve. If the facade ever re-bounds the plain paths, this test file fails to compile.

### P1-2 — start capabilities not composed (MultipleMipStarts replaces the primitive) — **FIXED**

Verified: for `index >= 1` the qualification became ONLY `MultipleMipStarts`, replacing the underlying `MipStart`/`PartialMipStart` check — `MultipleMipStarts` alone would qualify a second start whose primitive was undeclared.

Fix (3 cross-product tests first → RED, then implementation → GREEN):

- Starts loop: `qualified = primitive_qualified && (!multiple || multiple_mip_starts_qualified)` — full = `MipStart && (first || MultipleMipStarts)`, partial = `PartialMipStart && (first || MultipleMipStarts)`; the error message names `MultipleMipStarts` when the primitive holds and the multiple gate fails, else the primitive.
- Hint conversion: the generated assignment is classified full/partial (coverage of active integer/binary variables), then the SAME composed gates apply — a conversion never bypasses the primitive, never silently overwrites the plan's own start.
- Tests: `second_start_requires_primitive_and_multiple_composed` (MultipleMipStarts without MipStart rejects a full start naming the primitive; partial starts pass under PartialMipStart+MultipleMipStarts), `full_starts_compose_mip_start_with_multiple_gate` (two full starts on a continuous model under MipStart+MultipleMipStarts; a partial start rejects naming PartialMipStart even with both declared), `hint_conversion_applies_composed_gates_with_classification` (partial converted start → recorded rejection naming PartialMipStart; full converted start applies).

---

## Re-review (round 3) — after fix commit `14e2e72` (reviewer verification of the owner disposition)

Verified each of the owner's round-3 fixes against the source and by running
the test suites myself. All dispositions confirmed.

### 1. `solve`/`solve_with` in the UNBOUNDED impl; `solve_plan`'s plain shortcut delegates to `solve_base` — VERIFIED

Grep of `src/solver/facade.rs` confirms the structure:

- Unbounded impl at `:163-166` (`B: BackendSession + SessionHealth + BackendMetadata`) contains `solve` (`:461`), `solve_with` (`:475`), and the private plain core `solve_base` (`:492`).
- Bounded impl at `:544-546` (`+ OverlaySession`) contains `solve_plan` (`:631`), whose plain shortcut is `return self.solve_base(model, plan.options, &mut effective)` (`:661`).

`solve_with` calls `solve_base` directly; `solve_plan` with an empty effective
plan (no overlay content, no override, no starts/hints) calls the same
`solve_base`. The `solve == solve_with == empty solve_plan` equivalence is one
code path. `solve_base` uses `model.active_objective()` (correct — a plain plan
has no override) and normalizes with `overlay_id = None`, matching the plain
path. Rust resolves `self.solve_base(...)` from the bounded block because the
bounded bounds are a superset of the unbounded ones.

### 2. Equivalence tests pass — VERIFIED

`solve_solve_with_and_empty_solve_plan_are_equivalent` (roml) and
`highs_solve_solve_with_and_empty_solve_plan_are_equivalent` (HiGHS) both pass
within their suites (34/34 and 8/8). Identical status, objective, primal,
`SynchronizationMode`, revision, and `compilation_id` are asserted for
`solve`/`solve_with`/empty-`solve_plan`.

### 3. `PreP28Backend` regression genuinely requires no `OverlaySession` — VERIFIED

`PreP28Backend` (`tests/solve_plan.rs`) implements exactly three traits:
`BackendSession`, `SessionHealth`, `BackendMetadata` — no `OverlaySession` impl
anywhere. The generic helper
`assert_plain_solve_callable<B: BackendSession + SessionHealth + BackendMetadata>`
makes the compile-time claim: if `solve`/`solve_with` ever move back into the
bounded block, `session.solve(model)`/`session.solve_with(...)` fail to resolve
(E0599) and the test file stops compiling. The runtime leg (an end-to-end
optimal solve through `SolverSession<PreP28Backend>`) proves the plain path
works for a backend with no overlay surface at all.

### 4. Composed gate covers all four cross-product corners; hint conversion cannot bypass either gate — VERIFIED

For the starts loop, `qualified = primitive_qualified && (!multiple || multiple_mip_starts_qualified)`. The four corners (traced against the code):

| Start | primitive | multiple | qualified | error names |
|---|---|---|---|---|
| 1st | MipStart ✓ | — | ✓ | — |
| 1st | MipStart ✗ | — | ✗ | `MipStart` |
| 2nd | MipStart ✓, MultipleMipStarts ✓ | ✓ | ✓ | — |
| 2nd | MipStart ✓, MultipleMipStarts ✗ | ✗ | ✗ | `MultipleMipStarts` |
| 2nd | MipStart ✗, MultipleMipStarts ✓ | ✗ | ✗ | `MipStart` (primitive) |

The `feature`-naming condition (`multiple && !multiple_mip_starts_qualified && primitive_qualified` → `MultipleMipStarts`, else the primitive) is correct. The three cross-product tests exercise the corners: `second_start_requires_primitive_and_multiple_composed` (MultipleMipStarts-without-MipStart rejects a full start naming `MipStart`; two partial starts qualify under `PartialMipStart + MultipleMipStarts`), `full_starts_compose_mip_start_with_multiple_gate` (two full starts on a continuous-only model qualify; a partial start rejects naming `PartialMipStart` even with `MipStart + MultipleMipStarts`), and `hint_conversion_applies_composed_gates_with_classification` (partial converted start → recorded rejection naming `PartialMipStart`; full converted start applies).

The hint conversion computes `hint_partial` (any active integer/binary variable absent from the hints), derives the primitive (`PartialMipStart`/`MipStart`), and applies the SAME composed condition `hint_primitive_qualified && (!becomes_second || multiple_mip_starts_qualified)`. It cannot bypass either gate: the primitive check is enforced (partial conversion needs `PartialMipStart` even when `MipStart` + `MultipleMipStarts` are declared — tested), and the multiple gate is enforced (`becomes_second` reflects whether a native start already exists — the starts loop runs before the hints branch, so no ordering bypass).

The round-2 residual (multiple gate replacing the primitive) is resolved: the primitive is now always AND-ed.

### 5. Removed `OverlaySession` impls left no dead code — VERIFIED

`git diff 838e577..HEAD` shows the three boilerplate impls fully removed from
`tests/solver_facade.rs`, `tests/lineage_metadata.rs`, `tests/solve_options.rs`,
with their now-unused imports cleaned up. `cargo clippy -p roml --all-targets -- -D warnings` and `cargo clippy -p roml-highs --all-targets -- -D warnings` are both clean (no dead-code/warnings).

### 6. Tests run by me

| Command | Result |
|---|---|
| `cargo test -p roml --test solve_plan` | **34/34 passed** (was 30; +1 D27 regression, +3 cross-product) |
| `cargo test -p roml --all-targets` | **35 targets, 0 failures** |
| `cargo test -p roml-highs --test solve_plan` | **8/8 passed** |
| `cargo clippy -p roml --all-targets -- -D warnings` | clean |
| `cargo clippy -p roml-highs --all-targets -- -D warnings` | clean |

### Round-3 residual note (accepted, theoretical)

The composed gate is now correct. One theoretical note remains unchanged from
round 2 and is NOT a defect: `solve_plan`'s plain shortcut delegates to
`solve_base` only after `plan.validate` + `resolve_plan_features` have run, so
a plan that is plain (no starts/hints/overlay) is always a no-op for those two
stages — no observable behavior change. The `solve_with_overlay` empty-overlay
metadata behavior (P2-05) is unchanged and remains an accepted design decision.

---

## Findings summary (final)

| Severity | Count | Disposition |
|----------|-------|-------------|
| P0 | 0 | — |
| P1 | 1 | **Fixed** in `838e577`, refined in `14e2e72`, re-verified (source trace + tests) |
| P2 | 7 | 5 fixed, 3 accepted (P2-05/06/08); round-2 residual resolved by the composed gate |

**Final verdict: CLEAR** — no P0/P1 remains on `14e2e72`. The D27 plain-solve
contract and the start-capability composition are both correctly implemented
and regression-tested. Merge is unblocked subject to Pass 2 sign-off and
gate-result recording.

---

_Reviewed: 2026-08-04_
_Reviewer: Claude (gsd-code-reviewer, Pass 1 — Specification and correctness)_
