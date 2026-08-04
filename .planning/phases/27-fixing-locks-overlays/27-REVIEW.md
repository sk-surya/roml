---
phase: 27-fixing-locks-overlays
reviewed: 2026-08-03T00:00:00Z
depth: standard
files_reviewed: 18
files_reviewed_list:
  - src/assignment.rs
  - src/solver/overlay.rs
  - src/solver/facade.rs
  - src/solver/backend.rs
  - src/solver/reference.rs
  - src/solver/request.rs
  - src/solution/mod.rs
  - src/solution/metadata.rs
  - src/model/mod.rs
  - src/snapshot.rs
  - src/delta.rs
  - src/compiler/session.rs
  - src/compiler/origin.rs
  - roml-highs/src/session.rs
  - roml-highs/src/compiler.rs
  - tests/fixing_assignment.rs
  - tests/solve_overlay.rs
  - tests/solver_facade.rs
findings:
  critical: 2
  warning: 4
  info: 5
  total: 11
status: issues_found
---

# Phase 27: Code Review Report

**Reviewed:** 2026-08-03
**Depth:** standard
**Files Reviewed:** 18
**Status:** issues_found

## Summary

Reviewed the P27 changes: persistent fixing (declared vs effective bounds, fix/unfix, `SetVariableFixing` compile path), `PrimalAssignment`/`SolutionLock`/`LockSelector`/`ContinuousLock`, the reversible `SolveOverlay` lifecycle (`compile_overlay` → apply → solve → extract → rollback → verify), and the C_overlay vs C_base tagging. The happy-path overlay lifecycle is implemented correctly: `solve_with_overlay` compiles against `compiler.current_compilation()` (C_base), tags the result against the fresh C_overlay (not C_base), always attempts rollback, and honors the accepted deviations (zero-z objective-lock stub, apply-failure → `SolveError::Rollback`, weak-HiGHS verify). Two defects are rated critical: a non-finite-value validation hole in `validate_for`/`validate_value_in_domain` that can push NaN into the HiGHS native model, and the fact that several HiGHS `apply_overlay` failure paths leave the session half-overlaid with `Ready` health while the facade relies on the backend to have self-marked `RequiresRebuild` — a concrete overlay-leak path on the next solve. The `Within` lock band also replaces rather than intersects bounds and can loosen a variable's declared domain.

## Critical Issues

### CR-01: Non-finite values pass `validate_for`/`validate_value_in_domain` and reach the native solver as NaN bounds

**File:** `src/assignment.rs:74` and `src/solver/overlay.rs:414`

**Issue:** Both validators gate on `value < bounds.lower || value > bounds.upper`. For `NaN` both comparisons are false, so NaN passes; for `+inf` with an infinite upper bound the comparison is also false. `Model::fix` explicitly rejects non-finite values (`ModelError::NonFiniteValue`, `src/model/mod.rs:750-752`), but the overlay path — `SolveOverlay` temporary fixings and lock assignment values — has no finiteness check. A `NaN` temp-fixing compiles to `Bounds::new(NaN, NaN)` (`src/solver/overlay.rs:275-288`) and is pushed into `Highs_changeColBounds(raw, idx, NaN, NaN)` (`roml-highs/src/session.rs:691-697`); HiGHS's own `lower <= upper` validation is defeated by NaN comparisons, so the native model can be corrupted or the call fails in a way the session does not recover from. This is a validation gap at a public-API boundary that the rest of the library already guards against.

**Fix:**
```rust
// in both PrimalAssignment::validate_for and validate_value_in_domain:
if !value.is_finite() {
    return Err(/* a typed non-finite error, e.g. AssignmentError or
                 OverlayError::Assignment(AssignmentError::ValueOutOfBounds { .. }) */);
}
```

### CR-02: Mid-apply overlay failure can leave the HiGHS session half-overlaid with `Ready` health — the facade does not itself force a rebuild

**File:** `roml-highs/src/session.rs:678-772` and `src/solver/facade.rs:568-571`

**Issue:** The `OverlaySession` contract (`src/solver/session.rs:144-146`) and the facade doc (`src/solver/facade.rs:565-567`) both require that an apply failure leave the backend marked `RequiresRebuild` ("a partially applied overlay is never silently reused"). The HiGHS implementation violates this for most of its failure paths: a missing compiled variable in `SetTemporaryVariableBounds` (`session.rs:682-687`) or `AddTemporaryRow` (`session.rs:712-718`), a failing `Highs_changeColBounds` (`session.rs:691-697`, `Recoverable` per `roml-highs/src/error.rs:40-46`), and a failing `SetObjectivePolicy` projection (`session.rs:751-763`) all return an error WITHOUT `self.cursor.mark_rebuild()`. Only the `Highs_addRows` failure marks the cursor (`session.rs:734-737`). If op N fails after ops 1..N-1 already mutated the native model, `current_compilation` stays C_base and the cursor stays `Ready`; the facade maps the apply error to `SolveError::Rollback` and returns without marking the session. A subsequent plain `solve` then takes the no-sync path (health Ready + revision match, `src/solver/facade.rs:310-341`) and solves against the half-overlaid native state — a genuine overlay leak. CR-01's NaN path is a concrete reachable trigger when the NaN op is not the first op.

**Fix:** Mark `self.cursor.mark_rebuild()` on every early-return error path inside `HighsSession::apply_overlay` (and have the facade defensively mark/rebuild on any apply failure, rather than trusting the backend to have self-marked).

## Warnings

### WR-01: `Within` lock band replaces bounds instead of intersecting — it can LOOSEN a variable's declared domain

**File:** `src/solver/overlay.rs:441-459` (with `src/solver/reference.rs:841-842` and `roml-highs/src/session.rs:688-701`)

**Issue:** `continuous_band_bounds` returns `Bounds::new(value - absolute, value + absolute)` without clipping to the variable's declared bounds, and the backends apply it by REPLACING the base bounds (`entry.0 = *bounds` / `Highs_changeColBounds`). A band that extends past a declared bound — e.g. value 1.0, `absolute` 2.0 on a variable with declared `[0,10]` → band `[-1,3]` — loosens the lower bound. The overlay solve can then return a solution violating the model's declared bounds, and the resulting `PrimalAssignment` fails `validate_for` (`ValueOutOfBounds`). SM-06.3/06.5 defines a lock as a feasible-region *restriction*; a lock must never widen the domain. The within-band test (`tests/solve_overlay.rs:469-494`) only exercises a band fully inside the declared domain and misses this.

**Fix:** intersect the band with the declared domain:
```rust
let domain = model.variable_domain(variable).ok_or(/* stale */)?;
let lower = (value - absolute).max(domain.bounds.lower);
let upper = (value + absolute).min(domain.bounds.upper);
Ok(Bounds::new(lower, upper))
```

### WR-02: Fixing/bounds changes on an INACTIVE variable diverge between the delta and rebuild compile paths

**File:** `src/compiler/session.rs:530-550` vs `src/compiler/session.rs:282-286`

**Issue:** `compile_snapshot` folds an inactive variable to `[0,0]` regardless of its fixing (`session.rs:282-286`), but `compile_delta`'s `SetVariableFixing` lowers to `BackendOp::SetVariableBounds` with the raw `effective_bounds` (`session.rs:546-549`) with no activity fold. Repro: `fix(x, 4)` → commit; `set_variable_active(x, false)` → commit (activity change forces `RebuildRequired`, so the rebuild compiles x to `[0,0]`); `unfix(x)` → commit (the delta now sets x's bounds to the declared `[0,10]`). The backend is left at `[0,10]` for an inactive variable, while a rebuild from the same revision compiles `[0,0]` — the solver can then assign a nonzero value to an inactive variable. `Model::effective_bounds` (`src/model/mod.rs:702-711`) also returns `[value,value]` for a fixed inactive variable, disagreeing with the snapshot fold, so the model API and the two compile paths each disagree. The same gap predates P27 for plain `SetVariableBounds`, but P27's new `SetVariableFixing` op inherits it and P27 claims fixing-as-bounds compile correctness.

**Fix:** make `SetVariableFixing` (and `SetVariableBounds`) on a variable known to be inactive force a `CompileError::RebuildRequired` like `SetVariableActive`, or track activity in `CurrentCompilation` and fold it identically in both `compile_snapshot` and `compile_delta`.

### WR-03: `objective_compiled_terms` rejects parameterized objective coefficients with a misleading `ObjectiveNotFound` error

**File:** `src/solver/overlay.rs:467-491`

**Issue:** The overlay compiler re-derives each objective's row from the canonical `model.objective_expression` and requires `term.coeff.as_constant()` (`overlay.rs:480-483`). For a parameterized coefficient (`ValueExpr::Param` or a composite expression), `as_constant()` returns `None` (`src/value_expr/mod.rs:170-174`), so any cutoff or objective-lock referencing an objective with a parameterized coefficient fails with `OverlayError::ObjectiveNotFound` even though the objective exists and is compiled. The compiled base already carries the evaluated coefficients (the snapshot's `evaluated_value` cells), so this re-derivation is both unnecessary and wrong for legitimate models (e.g. `add_objective_coefficient(obj, x, price)`).

**Fix:** resolve the objective's coefficients from the compiled base (`compiler.compiled_objective_id` + the compiled objective's coefficient vector), and report `ObjectiveNotFound` only when the objective is genuinely absent from the compiled base.

### WR-04: `ReferenceBackend::apply_overlay` partially mutates state on mid-apply failure with no way to roll back

**File:** `src/solver/reference.rs:829-889`

**Issue:** `apply_overlay` mutates `compiled_variables`/`compiled_rows` as it iterates operations, but `overlay_state` is stored only at the end (line 889). If an op fails after earlier ops succeeded (e.g. a second `SetTemporaryVariableBounds` referencing an unknown compiled variable, `reference.rs:832-838`), the backend is left half-overlaid with `current_compilation` still C_base and `overlay_state == None`. `rollback_overlay` then errors ("no applied overlay to roll back", `reference.rs:908-914`), so the partial state can never be rolled back and can be silently reused. The per-op reference checks are good defense-in-depth but do not make the apply transactional. Compare `compile_delta`'s copy-on-write on a scratch `CurrentCompilation` (`src/compiler/session.rs:462`), which commits only on full success.

**Fix:** stage the overlay mutations on a clone of `compiled_variables`/`compiled_rows` (or capture a full base snapshot at apply start and restore it on any failure) so a failed apply is either fully applied or fully rejected.

## Info

### IN-01: HiGHS `verify_overlay_clean` checks only row/col counts and the compilation id, not the actual bound/objective state

**File:** `roml-highs/src/session.rs:883-906`

**Issue:** The reference backend's `verify_overlay_clean` compares the full normalized compiled view (`src/solver/reference.rs:946-963`), but the HiGHS verification compares only `Highs_getNumRow`/`Highs_getNumCol` and `current_compilation`. A rollback that restored wrong bound values or wrong objective costs while keeping the same row/col counts would pass. Since rollback restores the exact captured `prior_bounds`/`prior_policy` and every restore failure marks `RequiresRebuild`, the practical risk is low, but the verification could cheaply also compare `var_bounds`, `active_obj`, and `compiled_objective_policy` against the captured base.

### IN-02: `ObjectiveLock::relative_tolerance` is dead in P27 and `absolute_tolerance` is unvalidated

**File:** `src/solver/overlay.rs:78-86, 314-344`

**Issue:** `relative_tolerance` is never read in P27 (accepted zero-z stub; P31 supplies the relative term), and `absolute_tolerance` is not validated — a negative value silently produces a loosened/absurd degradation row. Consider validating both tolerances (finite, non-negative) at compile time.

### IN-03: The facade failure-injection matrix would not catch an overlay-bounds leak

**File:** `tests/solve_overlay.rs:1170-1211, 1651-1681`

**Issue:** `OverlayTestBackend::solve` reports the objective from fixed unit `var_values` (all 1.0) and ignores the overlay's temporary bounds/rows, so the "clean solve after an injected failure equals a fresh rebuild" assertions (`run_overlay_scenario`) hold even if the overlay's bounds leaked into the backend. The HiGHS round-trip test (`roml-highs/src/session.rs:1880-1991`) and the reference round-trip test (`tests/solve_overlay.rs:872-920`) do verify real state, so leak coverage exists at the backend level; the facade-level matrix is weaker than it appears.

### IN-04: `OverlayRollbackOutcome::RequiresRebuild { reason }` is never surfaced

**File:** `src/solver/facade.rs:629-633`

**Issue:** `rollback_and_verify` discards the `reason` string on the `RequiresRebuild` path. The diagnostic value (why the rollback could not be proven clean) never reaches the caller or the logs. Consider `warn!`-ing it.

### IN-05: The canonical (non-compiled) `ReferenceBackend::rebuild` drops the snapshot's fixing

**File:** `src/solver/reference.rs:360-425`

**Issue:** `rebuild` inserts `(v.bounds, v.var_type, v.active)` from `VariableEntry` and never reads `VariableEntry.fixing` (`src/snapshot.rs:74-92`), so a canonical rebuild of a fixed model loses the fixing (declared bounds only), while `apply_op(SetVariableFixing)` applies the effective bounds. The compiled path (`rebuild_compiled`) is correct because `compile_snapshot` folds the fixing first, and the facade never uses the canonical path, so this affects only the legacy M2 characterization harness — but the canonical rebuild-vs-delta commuting square no longer holds for fixed models.

---

_Reviewed: 2026-08-03_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
