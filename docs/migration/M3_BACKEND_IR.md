# M3 Backend-IR Migration Guide (SM-03.8)

**Phase:** P26 — compiler boundary, backend IR, capabilities, origins, exact compilation identity
**Audience:** advanced backend authors implementing [`BackendSession`](roml::solver::session::BackendSession)
**Status:** backend-contract amendment (design §22) — the advanced synchronization contract now flows through backend IR.

This guide documents the M3 backend-contract migration (SM-03.8). It is the required companion to the design §22 amendment: how a backend author migrates a session that currently consumes canonical [`ModelSnapshot`] / [`DeltaBatch`] to consume compiled backend IR ([`BackendSnapshot`] / [`BackendDeltaBatch`]), and what the identity compiler guarantees.

> **Ordinary M2 users are unaffected (D27).** `Model`, `LinExpr`, `Highs::solve`,
> `solve_with`, and `Solution` remain source-compatible. Only *advanced backend
> authors* who implement `BackendSession::synchronize` and/or call
> `Synchronization` directly must migrate.

---

## 1. What changed

Before P26, synchronization carried canonical state:

```rust
// Before (M2 canonical sync)
pub enum Synchronization {
    DeltaBatch(DeltaBatch),     // canonical incremental ops
    Rebuild(ModelSnapshot),     // canonical full state
}
```

After P26 (design §22, pinned by the Task 0 acceptance record B6), the advanced
contract adds compiled variants, and the *production* path (the
`SolverSession` façade) uses only the compiled variants:

```rust
// After (M3 compiled sync)
pub enum Synchronization {
    DeltaBatch(DeltaBatch),               // retained for compat; not used by the production path
    Rebuild(ModelSnapshot),               // retained for compat; not used by the production path
    CompiledDeltaBatch(BackendDeltaBatch), // compiled incremental ops (backend IR)
    CompiledRebuild(BackendSnapshot),      // compiled full state (backend IR)
}
```

The identity compiler ([`CompilationSession`]) lowers canonical snapshots/deltas
into backend IR *before* any backend mutation. The `SolverSession` façade owns a
`CompilationSession` and sends `CompiledRebuild` / `CompiledDeltaBatch` to the
backend. The HiGHS session migrates first and handles only the compiled
variants; a canonical sync request is rejected with
`HealthEffect::RequiresRebuild`.

## 2. What the compiler guarantees (the contract backend authors rely on)

`CompilationSession` provides:

- **`compile_snapshot(source_instance, &ModelSnapshot, policy, capabilities) -> BackendSnapshot`**
  — one-to-one projection: every variable → one [`CompiledVariable`], every
  constraint → one [`CompiledLinearRow`], every objective → one
  [`CompiledObjective`], all with dense deterministic compiled ids
  ([`CompiledVariableId`]/[`CompiledConstraintId`]/[`CompiledObjectiveId`]),
  distinct from user handles (SM-02.4). Inactive variables fold to fixed
  `[0,0]` bounds; inactive rows fold to unbounded bounds. The active objective
  compiles to [`CompiledObjectivePolicy::Single`]; no active objective compiles
  to [`CompiledObjectivePolicy::None`] (A32 / acceptance point B1).
- **`compile_delta(&DeltaBatch, from_compilation, policy, capabilities) -> BackendDeltaBatch`**
  — primitive linear deltas lower incrementally with exact `from_compilation`
  / `to_compilation` and revisions (B2, D28). Any delta op the identity
  compiler cannot prove incrementally equivalent (variable/constraint activity
  changes, variable-type changes, semi-continuous bounds, semantic construct
  ops) returns [`CompileError::RebuildRequired`] — **no** `BackendDeltaBatch`
  is emitted, and the caller performs one deterministic snapshot rebuild
  (design §18, D22; acceptance point F-B1).
- **Origin completeness (SM-02.5, D5):** every compiled entity carries an
  [`EntityOrigin`] in the snapshot's [`OriginMap`]; the delta batch carries the
  origins of entities it adds in `BackendDeltaBatch.origin_additions`. A
  backend can translate compiled solution values back to user ids via these
  origins.
- **Exact identity (D28):** stale-state checks compare exact `CompilationId`,
  never a fingerprint or revision.

## 3. Migration steps for a backend author

### 3.1 Read the backend IR shapes

Read `src/compiler/backend_ir.rs` (the `BackendSnapshot`, `BackendDeltaBatch`,
`BackendOp`, and compiled-id types), `src/compiler/origin.rs` (`EntityOrigin`,
`OriginMap`), and `src/compiler/capability.rs` (`BackendCapabilitySet`,
`BackendFeature`, `CompilationPolicy`). The compiler surface is exported through
`roml::advanced` (compiler internals are deliberately NOT in the ordinary
prelude — SM-03.x / API-07.2).

### 3.2 Replace snapshot rebuild

```rust
// Before: parse ModelSnapshot and apply canonical entities.
match sync {
    Synchronization::Rebuild(snapshot) => self.apply_canonical_snapshot(&snapshot)?,
    // ...
}

// After: consume BackendSnapshot. Use the origin map to translate compiled
// entities back to user ids where your solution/result surface needs them.
match sync {
    Synchronization::CompiledRebuild(snapshot) => self.apply_backend_snapshot(&snapshot)?,
    // ...
}
```

`BackendSnapshot` fields: `compilation_id`, `source_instance`,
`source_revision`, `variables`, `linear_rows`, `native_constraints` (always
empty in P26), `objectives`, `objective_policy`, `origin_map`, `report`,
`recipe_fingerprint`.

### 3.3 Replace delta application

```rust
// Before: apply canonical ModelOps.
Synchronization::DeltaBatch(batch) => self.apply_canonical_delta(&batch)?,

// After: apply compiled BackendOps. Exact from/to ids are on the batch.
Synchronization::CompiledDeltaBatch(batch) => self.apply_backend_delta(&batch)?,
```

`BackendOp` is the pinned 15-variant enumeration (acceptance point B3),
including `RemoveLinearCoefficient`, `RemoveObjectiveCoefficient`, and
`SetObjectivePolicy`. It is `#[non_exhaustive]`; handle unknown variants with a
wildcard (or rebuild).

### 3.4 Preserve the invariants

- **Compile-before-mutation:** the full canonical state must compile to a
  `BackendSnapshot` before any backend mutation. The façade does this for the
  ordinary path; a direct-sync backend author must do the same.
- **One-rebuild-retry:** a failed incremental sync recovers via exactly one
  deterministic snapshot rebuild.
- **Exact `CompilationId` authority:** never gate stale-state safety on
  `RecipeFingerprint` equality; compare `CompilationId`.
- **A31:** the compiler consumes `SetCell`/`SetConstraintBounds`/`RemoveCell`
  ops for updates to pre-existing functions — never treat `DeltaBatch.functions`
  as exhaustive for pre-existing constraints.

### 3.5 Use the migration reference

The reference implementation is `src/solver/reference.rs` (`ReferenceBackend`
with `rebuild_compiled`/`apply_compiled_delta`/`compiled_normalized_view`) and
`roml-highs/src/compiler.rs` (backend IR → HiGHS native). The shared conformance
suite (`src/solver/conformance.rs`) now exercises the compiled contract.

## 4. Example: snapshot rebuild against a native solver

```rust,ignore
fn apply_backend_snapshot(&mut self, snapshot: &BackendSnapshot) -> Result<(), BackendError> {
    self.clear();
    // Keep a compiled-id → user-id map for solution translation (SM-02.5).
    self.user_vars.clear();
    for v in &snapshot.variables {
        if let Some(EntityOrigin::UserVariable(var)) = snapshot.origin_map.variable_origin(v.id) {
            self.user_vars.insert(v.id, *var);
        }
        self.add_variable(v.id, v.bounds, v.var_type)?;
    }
    for r in &snapshot.linear_rows {
        self.add_row(r.id, r.bounds, &r.coefficients)?;
    }
    // Project the active objective policy (Single(id) / None in P26).
    match &snapshot.objective_policy {
        CompiledObjectivePolicy::Single(id) => self.activate_objective(*id)?,
        _ => self.clear_objective()?,
    }
    Ok(())
}
```

## 5. Capability gating

The compiler consults a [`BackendCapabilitySet`] during compilation (SM-04.4):
an unqualified feature is rejected, never silently ignored. A backend author
reports native support by building a typed capability set (see
`roml-highs::highs_capability_set` for the HiGHS example). The P26 primitive
linear surface gates on `Lp`, `Mip`, `IncrementalBounds`, `IncrementalRows`,
and `IncrementalCoefficients`.

## 6. Migrated and unaffected

- **Migrated first:** `ReferenceBackend` (`src/solver/reference.rs`) consumes
  compiled IR; its recovery/differential tests pass on the compiled path.
- **Migrated:** HiGHS (`roml-highs`) — `roml-highs/src/compiler.rs` translates
  backend IR into HiGHS native calls; after migration the HiGHS session
  receives no canonical `ModelSnapshot`.
- **Unaffected:** ordinary M2 modeling/solve APIs (D27); `roml-mosek` /
  `roml-xpress` remain out of scope for P26.

## 7. Open follow-ups

- `BackendConstraint` payloads (`Indicator`/`Sos1`/`Sos2`/`PiecewiseLinear`)
  land with the P32/P33 bridge tasks (acceptance point F-G).
- `Weighted`/`Lexicographic` objective policies are reachable only from the P31
  canonical `ObjectivePolicy` (design §15).
- Full per-solve feature recording lands with `EffectiveSolvePlan` in P28
  (SM-04.5 foundation delivered in P26).
