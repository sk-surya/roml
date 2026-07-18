---
phase: 11-highs-projection-session-rewrite
plan: 02
subsystem: roml-highs
tags: [projection, snapshot-rebuild, delta-application, callbacks, mip, lazy-constraints]
provides: [projection.rs, callback.rs]
requires: [bindings.rs, error.rs, lifecycle.rs, index_map.rs]
affects: []
key-files:
  created: []
  modified:
    - roml-highs/src/projection.rs
    - roml-highs/src/callback.rs
decisions:
  - Use kHighsCallbackMip* constants from highs-sys for correct callback-type values
  - Highs_addVar/Highs_addRow return column/row indices (not status codes); check < 0 for errors
  - Callback returns void per HighsCCallbackType; user interrupt signalled via data_in.user_interrupt
metrics:
  duration: ~12 minutes active work
  total_insertions: 1187
  total_deletions: 16
status: complete
---

# Phase 11 Plan 02: HiGHS Projection/Session Rewrite — Summary

Rebuilt the projection.rs and callback.rs modules from scratch, implementing snapshot rebuild, delta application, and the callback bridge. All 7 tasks from the phase packet executed; both modules compile, pass clippy, and pass all 10 tests.

## Architecture

### projection.rs

The core model synchronisation module containing:

- **`check_semicontinuous()`** — M1R-H7 rejection guard. Scans snapshot variables for `semicontinuous_lower: Some(_)` and returns `BackendError::unsupported("semi-continuous variables")` with `HealthEffect::RequiresRebuild` before any HiGHS call.

- **`rebuild_from_snapshot()`** — Full deterministic HiGHS model rebuild. Sequence per Research Architecture Patterns:
  1. Semi-continuous rejection (before any modification)
  2. `Highs_clear()` to reset the model
  3. Clear all caches (col_map, row_map, var_bounds, con_bounds, obj_costs, obj_senses, active_obj)
  4. Add variables with integrality, activity, bound caching
  5. Add constraints as empty rows with bound caching
  6. Fill constraint coefficient cells via `Highs_changeCoeff`
  7. Register objectives with cached costs and senses
  8. Set inactive constraints to [-inf, inf] (unconstrained)

- **`apply_delta_batch()`** — Applies a `DeltaBatch` of ModelOp variants. All 16 variants are handled:
  - `AddVariable`, `RemoveVariable`, `SetVariableBounds`, `SetVariableActive`, `SetVariableType`
  - `AddConstraint`, `RemoveConstraint`, `SetConstraintBounds`, `SetConstraintActive`
  - `SetCell`, `RemoveCell`
  - `AddObjective`, `RemoveObjective`, `SetActiveObjective`, `SetObjectiveCell`
  - `SetParameter` (skipped with log warning for model-internal params)
  - Re-index after every delete (RemoveVariable, RemoveConstraint)
  - Zero all costs on objective switch (Pitfall 5)
  - Partial application is impossible — returns on first error

- **Key patterns implemented:**
  - Infinity normalisation (Pitfall 6): `normalize_bound()` maps `f64::INFINITY` to cached `inf`
  - `Highs_addVar`/`Highs_addRow` return indices (>= 0 on success, -1 on failure) — checked differently from status-returning functions
  - Every other HiGHS call return code checked via `check_highs_status`
  - Semi-continuous pre-validation before any HiGHS state modification

### callback.rs

The callback bridge between HiGHS C callbacks and ROML `CallbackHandler`:

- **`CallbackState`** — Holds boxed handler, column/row map pointers, HiGHS handle, interrupt flag
- **`callback_trampoline`** — `unsafe extern "C"` fn registered with `Highs_setCallback`. Handles 6 callback types:
  - `kHighsCallbackMipLogging` (5): Log message string
  - `kHighsCallbackMipInterrupt` (6): Set `data_in.user_interrupt` on handler request
  - `kHighsCallbackMipSolution` (3): Informational candidate solution report
  - `kHighsCallbackMipImprovingSolution` (4): Informational incumbent report
  - `kHighsCallbackMipGetCutPool` (7): Read-only diagnostic — no-op
  - `kHighsCallbackMipDefineLazyConstraints` (8): Invoke handler, inject cuts via `Highs_addRow`
  - Unknown types: Safely ignored with warning
- Panic catching via `catch_unwind` at handler invocation boundaries (T-11-11)
- `register_callback()` / `clear_callback()`: Lifecycle management
- `build_callback_data()`: Maps HiGHS solution arrays back to ROML `VarId` via `reverse_map()`
- `inject_lazy_constraints()`: Translates `CallbackCut` terms to `Highs_addRow` calls

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3] HighsCCallbackType returns void, not c_int**
- **Found during:** Task 3 compilation
- **Issue:** The plan's pseudocode showed the callback trampoline returning `c_int` for interrupt signalling, but the actual `HighsCCallbackType` typedef in highs-sys 1.15.0 specifies `void` return. HiGHS interruption is signalled via `data_in.user_interrupt`, not return value.
- **Fix:** Changed trampoline return type to `()`, updated interrupt logic to set `(*data_in).user_interrupt = 1`.
- **Files modified:** `roml-highs/src/callback.rs`
- **Commit:** `def682c`

**2. [Rule 3] Highs_addVar/Highs_addRow return indices, not status codes**
- **Found during:** Task 1 implementation analysis
- **Issue:** The plan's pseudocode used `check_highs_status(ret)` for `Highs_addVar` and `Highs_addRow`, but these C API functions return the column/row index on success (>= 0) and -1 on failure — not the `STATUS_OK` (0) / non-zero convention. Using `check_highs_status` would reject the second variable (index 1 ≠ STATUS_OK).
- **Fix:** Added inline `if col < 0` checks for index-returning functions; uses `from_native_status` for error construction.
- **Files modified:** `roml-highs/src/projection.rs`
- **Commit:** `d6b5004`

**3. [Rule 3] Missing `value_expr` field in `SetObjectiveCell` pattern**
- **Found during:** Task 1 compilation
- **Issue:** The `ModelOp::SetObjectiveCell` has a `value_expr` field that was not matched in the pattern.
- **Fix:** Added `..` to ignore the field in the match arm.
- **Files modified:** `roml-highs/src/projection.rs`
- **Commit:** `d6b5004`

**4. [Rule 2] Added collapsible_if clippy fix**
- **Found during:** Clippy run after Task 3
- **Issue:** Nested `if` statements in interrupt callback could be collapsed.
- **Fix:** Combined into single `if state.user_interrupt && !data_in.is_null()`.
- **Files modified:** `roml-highs/src/callback.rs`
- **Commit:** `def682c`

## Known Stubs

- **`SetParameter`** in `apply_delta_batch`: Parameters with no HiGHS equivalent are skipped with a log warning. No HiGHS-internal parameter mapping is implemented — deferred to a future plan that maps ROML parameters to HiGHS options.
- **Objective constant** in `SetObjectiveCell`: The `constant` value is captured via `let _ = constant` for future extraction use (Plan 03). Not applied to `Highs_changeObjectiveOffset` since ROML manages constants per-objective and HiGHS's offset is model-global.

## Threat Flags

No new security-relevant surface introduced beyond what is documented in the plan's threat model.

## Test Results

| Test | Result |
|------|--------|
| `normalize_bound_maps_infinity` | PASS |
| `check_semicontinuous_accepts_continuous` | PASS |
| `check_semicontinuous_rejects_semicontinuous` | PASS |
| `var_type_to_integrality_mapping` | PASS |
| `sense_to_highs_mapping` | PASS |
| `build_callback_data_with_null_data_out` | PASS |
| `inject_empty_cuts_is_noop` | PASS |
| All 10 tests (including 3 pre-existing index_map tests) | PASS |

## Self-Check

**Created files check:**
- `roml-highs/src/projection.rs`: FOUND (759 insertions)
- `roml-highs/src/callback.rs`: FOUND (428 insertions)

**Commits check:**
- `d6b5004`: FOUND — "feat(11-highs-projection-session-rewrite): implement snapshot rebuild and delta application"
- `def682c`: FOUND — "feat(11-highs-projection-session-rewrite): implement callback bridge with trampoline and lazy constraints"

**Verification checks:**
- `cargo check -p roml-highs`: PASS (0 errors, 18 pre-existing dead_code warnings)
- `cargo clippy -p roml-highs`: PASS (no clippy errors in our code; 2 pre-existing warnings from roml crate)
- `cargo test -p roml-highs`: PASS (10/10 tests pass)
- `grep -c 'fn rebuild_from_snapshot'` = 1
- `grep -c 'fn apply_delta_batch'` = 1
- `grep -c 'reindex_after_delete'` = 2 (both in delta apply, not rebuild)
- `grep -c 'check_highs_status'` = 26 (all HiGHS status calls checked)
- `grep -c 'fn register_callback\|fn clear_callback'` = 2
- `grep -c '// SAFETY:'` = 8
- `grep -c 'catch_unwind'` = 3
- `grep -c 'kHighsCallbackMip'` = 12

## Self-Check: PASSED
