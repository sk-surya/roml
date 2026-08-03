---
phase: 26-compiler-backend-ir
reviewed: 2026-08-03T00:00:00Z
depth: standard
files_reviewed: 22
files_reviewed_list:
  - src/compiler/mod.rs
  - src/compiler/backend_ir.rs
  - src/compiler/origin.rs
  - src/compiler/report.rs
  - src/compiler/capability.rs
  - src/compiler/session.rs
  - src/solver/backend.rs
  - src/solver/request.rs
  - src/solver/reference.rs
  - src/solver/facade.rs
  - src/solver/conformance.rs
  - src/advanced.rs
  - src/lib.rs
  - src/snapshot.rs
  - src/delta.rs
  - roml-highs/src/compiler.rs
  - roml-highs/src/session.rs
  - tests/compiler_identity.rs
  - tests/compiler_sync.rs
  - tests/differential_harness.rs
  - tests/typed_capabilities.rs
  - docs/migration/M3_BACKEND_IR.md
findings:
  critical: 2
  warning: 5
  info: 4
  total: 11
status: issues
---

# Phase 26: Code Review Report

**Reviewed:** 2026-08-03
**Depth:** standard
**Files Reviewed:** 22
**Status:** issues_found

## Summary

The P26 compiler boundary is well-structured and the core invariants are mostly honored: exact `CompilationId` allocation is checked and non-wrapping; builder finalization enforces the origin-completeness stopping condition; `compile_delta` commits session state only on full success (rebuild-on-uncertainty never emits a partial delta); A31-aware op consumption is implemented correctly (updates ride `SetCell`/`SetConstraintBounds`/`RemoveCell`); the fixed-seed compiled-delta-vs-rebuild test is sound for its additive/update scope and does validate Path B against a canonical-derived final snapshot; the `SetParameter` no-op is provably safe because `apply_parameter_change` re-emits `CoefficientValueChanged` (→ `SetCell`) for every dependent cell. Compile-before-mutation and the one-rebuild-retry invariant hold in the façade.

The defects concentrate in two places: (1) the compiled *removal* path (`RemoveVariable`/`RemoveObjective`) leaves the reference backend's compiled state internally inconsistent — stale row/objective coefficients referencing removed variables and a dangling objective policy — and this surface is entirely untested; and (2) the exact-`CompilationId` stale-state contract (D28/SM-03.9) is enforced by the reference backend but *not* by the migrated HiGHS session, which checks only `from_revision`. Additional gaps: incomplete capability gating in `compile_delta`, no source-instance guard against cross-model `SolverSession` reuse, and `expect()` panics in the HiGHS FFI projection on origin-less snapshot entries.

## Critical Issues

### CR-01: Compiled `RemoveVariable` leaves stale coefficients in rows/objectives in the reference backend

**File:** `src/solver/reference.rs:531-533` (and `roml-highs/src/compiler.rs:342-358` for contrast)
**Issue:** `ReferenceBackend::apply_compiled_op(BackendOp::RemoveVariable(id))` removes the variable from `compiled_variables` only. It does **not** purge the `(CompiledVariableId, f64)` entries referencing the removed variable from `compiled_rows[*].1` or `compiled_objectives[*].1`. The canonical M2 path (`apply_op(ModelOp::RemoveVariable)`, `reference.rs:137-143`) explicitly prunes `constraint_cells`/`objective_cells`, so the compiled path diverges from the canonical projection: after a compiled remove-variable delta, the compiled state contains rows/objectives with coefficients for a variable that no longer exists. Any consumer projecting this state would build a corrupt model, and the compiled commuting square (`compiled_rebuild(rN) == apply(compiled_deltas)`) fails for removal ops. HiGHS happens to be correct here only because `Highs_deleteColsBySet` removes the column from all native rows. This defect is masked: `dx_fixed_seed_compiled_delta_equals_compiled_rebuild` explicitly excludes removals (`tests/differential_harness.rs:2070-2073`), and no other test applies a compiled `RemoveVariable` end-to-end.
**Fix:** Mirror the canonical cleanup in `apply_compiled_op`:
```rust
BackendOp::RemoveVariable(id) => {
    self.compiled_variables.remove(id);
    for (_, (_, coeffs)) in self.compiled_rows.iter_mut() {
        coeffs.retain(|(v, _)| v != id);
    }
    for (_, (_, coeffs, _)) in self.compiled_objectives.iter_mut() {
        coeffs.retain(|(v, _)| v != id);
    }
}
```

### CR-02: Removing the active objective leaves `compiled_objective_policy` dangling (compiler + reference backend)

**File:** `src/compiler/session.rs:529-538` and `src/solver/reference.rs:572-574`
**Issue:** `compile_delta` maps `ModelOp::RemoveObjective` to a bare `BackendOp::RemoveObjective(id)` — it never emits `SetObjectivePolicy(CompiledObjectivePolicy::None)` when the removed objective is the active one — and `ReferenceBackend::apply_compiled_op(RemoveObjective)` removes the objective without clearing `compiled_objective_policy`. The canonical path (`apply_op(RemoveObjective)`, `reference.rs:207-212`) sets `active_objective = None` when the active objective is removed, and a rebuild of the resulting canonical state yields `CompiledObjectivePolicy::None`. So after a compiled remove-active-objective delta, the reference compiled state reports `objective_policy: Single(removed_id)` referencing a non-existent compiled objective — precisely the invariant the builder rejects with `CompileError::InvalidObjectivePolicy` (`backend_ir.rs:495-497`). HiGHS compensates natively (its `RemoveObjective` arm checks `was_active` and clears costs/offset), but the compiler contract relies on the backend to infer policy changes, and the reference implementation does not. This is the same untested removal surface as CR-01.
**Fix:** Emit the policy transition at the compile boundary so the batch is self-contained (matching the A31 "ops carry the changes" principle):
```rust
ModelOp::RemoveObjective { obj } => {
    let id = *w.objective_ids.get(obj).ok_or_else(|| /* ... */)?;
    let was_active = w.compiled_policy_is_single(id); // or track active obj
    operations.push(BackendOp::RemoveObjective(id));
    if was_active {
        operations.push(BackendOp::SetObjectivePolicy(CompiledObjectivePolicy::None));
    }
    // ...remove maps...
}
```
and/or clear the policy in `ReferenceBackend::apply_compiled_op(RemoveObjective)` when `compiled_objective_policy` references the removed id.

## Warnings

### WR-01: HiGHS session never validates `CompilationId` on `CompiledDeltaBatch` — D28/SM-03.9 gap

**File:** `roml-highs/src/session.rs:117-139`
**Issue:** `synchronize(Synchronization::CompiledDeltaBatch)` checks only `batch.from_revision != self.cursor.applied_revision` before applying ops. It never compares `batch.from_compilation` against any recorded compiled state — the session tracks no `CompilationId` at all. D28/SM-03.9 (and the migration guide `M3_BACKEND_IR.md` §3.4) state the exact `CompilationId` is the *only* stale-state authority; `ReferenceBackend::apply_compiled_delta` correctly rejects a stale `from_compilation` (`reference.rs:509-514`). Because `from_revision` is not authority, two divergent clones at equal `ModelRevision` with different content would let HiGHS silently apply a delta compiled against the wrong clone's dense ids. The façade's single-compiler discipline currently masks this, but the migrated production backend implements weaker stale-state protection than the reference contract it is documented to implement.
**Fix:** Track `current_compilation: Option<CompilationId>` on `HighsSession` (set from `CompiledRebuild`'s `snapshot.compilation_id` and each accepted batch's `to_compilation`), and reject a `CompiledDeltaBatch` whose `from_compilation != current_compilation` with `HealthEffect::Recoverable`/`RequiresRebuild` before applying any op.

### WR-02: Façade first-solve delta path never delivers the compiled empty base, contradicting the reference contract

**File:** `src/solver/facade.rs:297-307`
**Issue:** `apply_deltas` compiles `ModelSnapshot::empty(backend_rev)` as the compiler's base "WITHOUT sending a rebuild to the backend", then sends the first delta against that base id. This works only for backends that accept a delta on top of an empty native state (HiGHS, `TestBackend`). A backend that follows the reference implementation's contract — `ReferenceBackend::apply_compiled_delta` requires a prior compiled base and returns `RebuildRequired("reference backend has no compiled base")` (`reference.rs:505-508`) — rejects the first delta and forces an unexpected rebuild, defeating the comment's stated purpose ("lets the first solve flow through compiled deltas (Delta sync mode)"). The reference specification and the production path disagree about whether a compiled base must be established before the first delta. At minimum the claim is misleading; a backend author implementing per the reference would see their first solve always rebuild.
**Fix:** Either send the compiled empty base to the backend as part of establishing it (guarded so it does not count as a rebuild retry), or document and enforce a uniform contract: "a backend must accept a delta with `from_compilation` equal to an un-sent empty base on first sync."

### WR-03: Incomplete capability gating in `compile_delta` (SM-04.4 partially enforced)

**File:** `src/compiler/session.rs:368-378, 421-431, 442-512`
**Issue:** Only `AddVariable`/`AddConstraint` gate on `BackendFeature::IncrementalRows`. `SetVariableBounds`, `SetLinearRowBounds`, `SetCell`, `RemoveCell`, and every objective op are emitted without consulting `IncrementalBounds`/`IncrementalCoefficients`. A backend that declares those features unqualified (e.g. the flat→typed mapping in `facade.rs:390-401` gives `IncrementalBounds = flat.set_bounds`) still receives the ops, silently — the opposite of SM-04.4 ("an unqualified feature is rejected, never silently ignored"). HiGHS declares all incremental features native so production is unaffected today, but the gate is inconsistent with the contract and with the MIP gate on the snapshot path.
**Fix:** Gate `SetVariableBounds`/`SetLinearRowBounds` on `IncrementalBounds` and `SetCell`/`RemoveCell`/objective coefficient ops on `IncrementalCoefficients`, returning `CompileError::UnsupportedFeature` when unqualified.

### WR-04: `CompilationSession` does not guard cross-model reuse (source instance stored, never checked)

**File:** `src/compiler/session.rs:274` (write) vs `compile_delta` (no check)
**Issue:** `compile_snapshot` records `self.source_instance`, but `compile_delta` never verifies that the incoming `DeltaBatch` belongs to that instance. A single `SolverSession` reused across two different `Model` instances that happen to be at overlapping revisions will silently compile the second model's deltas against the first model's compiled base (wrong `variable_ids`/`row_ids` → wrong dense ids and coefficients). This is exactly the D28 cross-instance divergence the exact-`CompilationId` authority is meant to prevent, and `DeltaBatch` carries no instance id to catch it. The façade should force a rebuild (or reset the compiler) when `model.instance() != self.compiler`'s recorded `source_instance`.
**Fix:** In `SolverSession::apply_deltas` (or `compile_delta`), require the compiler's `source_instance` to equal the model's instance; on mismatch return a rebuild-required error so the caller re-establishes the compiled base.

### WR-05: HiGHS FFI projection panics on origin-less snapshot entries instead of returning a typed error

**File:** `roml-highs/src/compiler.rs:197, 209, 218`
**Issue:** `rebuild_from_backend_snapshot` uses `.expect("every compiled variable/row/objective has a recorded origin")` on `snapshot.origin_map.*_origin(...)`. `BackendSnapshot` has all-`pub` fields and can be constructed directly (bypassing `BackendSnapshotBuilder::finalize`), so a malformed snapshot reaches the FFI projection and panics instead of producing a `BackendError` through the `Result`-returning `synchronize` contract. The builder protects the production path, but the panic is unreachable-error-by-construction only for the builder-produced path.
**Fix:** Replace `.expect` with a checked lookup that returns `BackendError::new(..., ErrorCategory::InvalidInput, HealthEffect::RequiresRebuild)` on a missing origin, consistent with `synchronize`'s error surface.

## Info

### IN-01: `CompilationPolicy` is accepted but ignored in both compile paths

**File:** `src/compiler/session.rs:139, 306`
**Issue:** `compile_snapshot`/`compile_delta` take `_policy: &CompilationPolicy` and never read it; `Portable`/`NativeRequired` have no effect in P26. Defensible (no constructs exist yet; A29's preference-honoring lands with P32), but the parameter silently accepts policies the compiler does not honor.
**Fix:** Document the P26 no-op explicitly, or drop the parameter until P32.

### IN-02: `dangling_policy_objective` does not validate weight/tolerance constraints

**File:** `src/compiler/backend_ir.rs:533-562`
**Issue:** `CompiledWeightedObjective.weight` is documented "Nonnegative weight" and `CompiledObjectiveLevel` tolerances are documented nonnegative, but `dangling_policy_objective` (and `finalize`) only check that referenced ids exist. A negative weight or negative tolerance is accepted. Unreachable in P26 (only `Single`/`None` are emitted), but the validation gap will be inherited by P31.
**Fix:** Extend `finalize` to reject non-positive weights/tolerances with a typed error.

### IN-03: `compile_snapshot` silently drops snapshot cells whose variable is not present

**File:** `src/compiler/session.rs:202-211`
**Issue:** Row/objective coefficient collection filters via `variable_ids.get(&cell.cell_key.1)` and silently skips cells referencing a variable absent from `snapshot.variables`. The canonical `ReferenceBackend::rebuild` would retain such a cell. Valid models cannot produce orphan cells, but the divergence is silent rather than a typed rejection.
**Fix:** Optionally return `CompileError` (or record a report decision) for orphan cells instead of dropping them.

### IN-04: Differential-test comment overstates removal coverage

**File:** `tests/differential_harness.rs:2070-2073`
**Issue:** The comment claims "the removal surface is exercised by the compiler's op-mapping tests," but no test compiles-and-applies a removal op end-to-end: `compiler_sync.rs` exercises `AddVariable`/`SetCell`/`RemoveCell`/`SetConstraintBounds`/`SetActiveObjective`, and the conformance suite adds only. This is the exact untested surface where CR-01/CR-02 defects live. The comment is misleading and the coverage gap let these defects through.
**Fix:** Add a compiled removal round-trip test (add variable + cell, then remove variable; assert the compiled view matches a rebuild with no stale coefficients), and a remove-active-objective test asserting `objective_policy` becomes `None`.

---

_Reviewed: 2026-08-03_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
