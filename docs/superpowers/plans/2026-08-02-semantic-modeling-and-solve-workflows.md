# Semantic Modeling and Solve Workflows Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a canonical semantic modeling IR, capability-aware backend compiler, solution-reuse workflows, infeasibility diagnostics, soft constraints, lexicographic objectives, and common exact MILP constructs while preserving ROML's incremental/recoverable solver guarantees and leaving an additive path for future nonlinear programming.

**Architecture:** Canonical `Model` state stores functions, sets, constructs, objective policy, declared domains, fixings, metadata, and revisions. A per-solver `CompilationSession` lowers canonical snapshots/deltas into backend IR with typed capabilities, exact bridges, generated-entity origins, and formulation reports. `SolverSession` applies solve-scoped overlays, starts, hints, and objective stages transactionally before mapping backend results back to user entities.

**Tech Stack:** Rust 1.85, existing ROML arenas/revisions/journal/snapshots, `roml-highs` through pinned `highs-sys`, property/differential tests, GitHub Actions, cargo fmt/clippy/test/doc/public-api/package.

## Global Constraints

- Preserve M2 ordinary `Model`, `LinExpr`, `Highs::solve`, `solve_with`, and `Solution` source usage unless a reviewed executable contradiction is documented.
- Core `roml` remains solver-free and contains no native HiGHS dependency.
- Canonical semantic state contains no backend indices, native handles, selected Big-M values, or solve overlays.
- High-level constructs remain canonical entities; bridge expansion occurs only in the compiler.
- Exact semantics are never silently compiled as relaxations.
- Big-M requires finite proof or explicit validated user input; no default Big-M constant is permitted.
- Every generated variable/constraint has an `EntityOrigin`.
- Unsupported features are applied, bridged exactly, adjusted explicitly, or rejected; never ignored.
- Overlay rollback uncertainty marks the session `RequiresRebuild`.
- Primitive compiled delta execution remains observationally equivalent to compiled rebuild.
- M3 implements linear scalar functions only; nonlinear expression tracing and differentiation are excluded.
- MSRV remains Rust 1.85.
- No publication, tag, or release is part of this plan.

---

## Planned file structure

### Canonical model and public semantics

```text
src/identity.rs                     opaque model lineage
src/metadata.rs                     entity metadata and source information
src/function/mod.rs                 scalar function/set exports
src/function/scalar.rs              ScalarFunction and IntoScalarFunction
src/function/set.rs                 ScalarSet and FunctionConstraint
src/construct/mod.rs                construct IDs/store/common lifecycle
src/construct/indicator.rs          indicator/reification specs
src/construct/minmax.rs             min/max relation specs
src/construct/absolute.rs           abs/positive-part/clamp specs
src/construct/boolean.rs            Boolean/cardinality specs
src/construct/product.rs            supported exact products
src/construct/piecewise_linear.rs   PWL points/relation/extrapolation
src/construct/soft.rs               soft constraint/signed correction
src/assignment.rs                   PrimalAssignment/MipStart/Hints/locks
src/objective_policy.rs             single/weighted/lexicographic policy
src/model/variable.rs               declared domain and fixing
src/model/mod.rs                    public mutations/accessors
src/snapshot.rs                     canonical semantic snapshot
src/delta.rs                        canonical semantic operations
src/solution/mod.rs                 assignment/violation/objective access
src/solution/metadata.rs            lineage/effective plan/stages
```

### Compiler and backend IR

```text
src/compiler/mod.rs
src/compiler/backend_ir.rs
src/compiler/capability.rs
src/compiler/session.rs
src/compiler/origin.rs
src/compiler/report.rs
src/compiler/bounds.rs
src/compiler/bridge/mod.rs
src/compiler/bridge/indicator.rs
src/compiler/bridge/minmax.rs
src/compiler/bridge/absolute.rs
src/compiler/bridge/boolean.rs
src/compiler/bridge/product.rs
src/compiler/bridge/piecewise_linear.rs
src/compiler/bridge/soft.rs
```

### Solve orchestration and analysis

```text
src/solver/plan.rs
src/solver/overlay.rs
src/solver/effective_plan.rs
src/solver/infeasibility.rs
src/solver/multiobjective.rs
src/solver/session.rs
src/solver/facade.rs
src/solver/backend.rs
```

### HiGHS backend

```text
roml-highs/src/compiler.rs
roml-highs/src/start.rs
roml-highs/src/iis.rs
roml-highs/src/relaxation.rs
roml-highs/src/multiobjective.rs
roml-highs/src/session.rs
roml-highs/src/facade.rs
```

### Tests and evidence

```text
tests/semantic_ir.rs
tests/lineage_metadata.rs
tests/compiler_identity.rs
tests/compiler_bridges.rs
tests/fixing_assignment.rs
tests/solve_overlay.rs
tests/objective_policy.rs
tests/common_constructs.rs
tests/piecewise_linear.rs
roml-highs/tests/solve_plan.rs
roml-highs/tests/iis_reports.rs
roml-highs/tests/soft_constraints.rs
roml-highs/tests/lexicographic.rs
roml-highs/tests/formulation_equivalence.rs
docs/release/evidence/M3_*.md
```

---

## Task 1: Capture M3 baseline and characterization suite

**Phase:** P25

**Files:**
- Create: `docs/release/evidence/M3_P25_SEMANTIC_IR.md`
- Create: `tests/m3_baseline_characterization.rs`
- Modify: none before baseline capture

**Interfaces:**
- Consumes: current M2 public API and test matrix.
- Produces: executable characterization tests and untouched command/public-API/package baselines used by every later task.

- [ ] **Step 1: Record exact base and environment**

Run:

```bash
git rev-parse HEAD
rustc --version --verbose
cargo --version
```

Write exact output to `docs/release/evidence/M3_P25_SEMANTIC_IR.md` under `Baseline and environment`.

- [ ] **Step 2: Capture untouched core and HiGHS gates**

Run:

```bash
cargo fmt --all -- --check
cargo check -p roml --all-targets
cargo clippy -p roml --all-targets -- -D warnings
cargo test -p roml --all-targets
cargo check -p roml-highs --all-targets
cargo clippy -p roml-highs --all-targets -- -D warnings
cargo test -p roml-highs --all-targets
RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps
RUSTDOCFLAGS='-D warnings' cargo doc -p roml-highs --no-deps
```

Expected: all currently qualified commands pass. Record failures as baseline facts; do not repair them in this step.

- [ ] **Step 3: Capture public API and package inventories**

Run:

```bash
mkdir -p docs/release/evidence/m3-baseline
cargo public-api -p roml > docs/release/evidence/m3-baseline/roml.txt
cargo public-api -p roml-highs > docs/release/evidence/m3-baseline/roml-highs.txt
cargo package --list -p roml > docs/release/evidence/m3-baseline/roml-package.txt
cargo package --list -p roml-highs > docs/release/evidence/m3-baseline/roml-highs-package.txt
```

- [ ] **Step 4: Write characterization tests**

Create `tests/m3_baseline_characterization.rs` with tests that pin:

```rust
use roml::prelude::*;

#[test]
fn m2_linear_api_remains_the_m3_compatibility_baseline() -> Result<(), Box<dyn std::error::Error>> {
    let mut model = Model::named("baseline");
    let x = model.add_variable(continuous().bounds(0.0, 10.0).named("x"))?;
    let p = model.add_parameter(parameter(2.0).named("p"))?;
    let c = model.add_constraint((x).le(4.0).named("capacity"))?;
    let o = model.maximize(p * x + 1.0)?;

    assert_eq!(model.variable_name(x)?, Some("x"));
    assert_eq!(model.constraint_name(c)?, Some("capacity"));
    assert_eq!(model.active_objective(), Some(o));
    assert!(model.pprint().contains("capacity"));
    Ok(())
}

#[test]
fn primitive_snapshot_is_deterministic() -> Result<(), Box<dyn std::error::Error>> {
    let mut model = Model::named("snapshot");
    let x = model.add_variable(continuous().bounds(0.0, 5.0))?;
    model.add_constraint((2.0 * x).le(8.0))?;
    model.minimize(x)?;
    model.commit()?;

    assert_eq!(model.snapshot(), model.snapshot());
    Ok(())
}
```

Use current exact public snapshot access signature; if snapshot is advanced-only, import it through the existing advanced path without changing production code.

- [ ] **Step 5: Run characterization tests**

Run:

```bash
cargo test -p roml --test m3_baseline_characterization -- --nocapture
```

Expected: PASS.

- [ ] **Step 6: Commit baseline evidence**

```bash
git add docs/release/evidence/M3_P25_SEMANTIC_IR.md \
  docs/release/evidence/m3-baseline tests/m3_baseline_characterization.rs
git commit -m "test(m3): capture semantic modeling baseline"
```

---

## Task 2: Add model lineage and entity metadata

**Phase:** P25

**Files:**
- Create: `src/identity.rs`
- Create: `src/metadata.rs`
- Create: `tests/lineage_metadata.rs`
- Modify: `src/lib.rs`
- Modify: `src/model/mod.rs`
- Modify: `src/solution/metadata.rs`
- Modify: `src/prelude.rs` or current prelude definition

**Interfaces:**
- Produces:

```rust
pub struct ModelLineageId(u64);
pub struct EntityMetadata { ... }
pub struct ModelSource { ... }

impl Model {
    pub fn lineage(&self) -> ModelLineageId;
    pub fn metadata(&self, entity: impl Into<EntityRef>) -> Result<&EntityMetadata, ModelError>;
    pub fn set_metadata(&mut self, entity: impl Into<EntityRef>, metadata: EntityMetadata) -> Result<(), ModelError>;
}
```

- Later tasks consume lineage for assignments and metadata for origins/reports.

- [ ] **Step 1: Write failing lineage tests**

Create `tests/lineage_metadata.rs`:

```rust
use roml::prelude::*;

#[test]
fn independent_models_have_distinct_lineages_and_clones_preserve_lineage() {
    let a = Model::new();
    let b = Model::new();
    let a_clone = a.clone();

    assert_ne!(a.lineage(), b.lineage());
    assert_eq!(a.lineage(), a_clone.lineage());
}
```

- [ ] **Step 2: Run the test and verify failure**

```bash
cargo test -p roml --test lineage_metadata independent_models_have_distinct_lineages_and_clones_preserve_lineage
```

Expected: compile failure because `Model::lineage` does not exist.

- [ ] **Step 3: Implement process-unique lineage allocation**

In `src/identity.rs`:

```rust
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_LINEAGE: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ModelLineageId(u64);

impl ModelLineageId {
    pub(crate) fn allocate() -> Self {
        let id = NEXT_LINEAGE.fetch_add(1, Ordering::Relaxed);
        assert_ne!(id, 0, "model lineage counter exhausted");
        Self(id)
    }
}
```

Do not expose the numeric value as a stable serialization contract.

Add `lineage: ModelLineageId` to `Model`; implement manual `Default` so new models allocate and `Clone` preserves the field.

- [ ] **Step 4: Run lineage tests**

```bash
cargo test -p roml --test lineage_metadata independent_models_have_distinct_lineages_and_clones_preserve_lineage
```

Expected: PASS.

- [ ] **Step 5: Write failing metadata tests**

Add:

```rust
#[test]
fn metadata_round_trips_for_named_entities() -> Result<(), Box<dyn std::error::Error>> {
    let mut model = Model::new();
    let x = model.add_variable(continuous().named("x"))?;
    let metadata = EntityMetadata::new()
        .description("dispatch level")
        .group("dispatch")
        .tag("interval:0")
        .source(ModelSource::new().external_key("dispatch/x/0"));

    model.set_metadata(x, metadata.clone())?;
    assert_eq!(model.metadata(x)?, &metadata);
    Ok(())
}
```

- [ ] **Step 6: Implement metadata types and typed entity references**

Use one internal `HashMap<EntityRef, EntityMetadata>` where `EntityRef` is:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EntityRef {
    Variable(Variable),
    Constraint(Constraint),
    Objective(Objective),
    Parameter(Parameter),
    Construct(Construct),
}
```

Implement `From<Variable>`, `From<Constraint>`, `From<Objective>`, and `From<Parameter>`. Add construct conversion in Task 4.

Metadata mutation is canonical diagnostic state but does not affect solver state. Record it separately from solver-affecting changes or mark its `Change` as non-solver-affecting.

- [ ] **Step 7: Add lineage to solve metadata**

Extend `SolveMetadata`:

```rust
pub model_lineage: ModelLineageId,
```

Update all constructors/tests so a solved `Solution` records both lineage and revision.

- [ ] **Step 8: Run focused and core tests**

```bash
cargo test -p roml --test lineage_metadata
cargo test -p roml --all-targets
```

- [ ] **Step 9: Commit**

```bash
git add src/identity.rs src/metadata.rs src/lib.rs src/model/mod.rs \
  src/solution/metadata.rs tests/lineage_metadata.rs
git commit -m "feat(model): add lineage and entity metadata"
```

---

## Task 3: Introduce linear function-in-set canonical constraints

**Phase:** P25

**Files:**
- Create: `src/function/mod.rs`
- Create: `src/function/scalar.rs`
- Create: `src/function/set.rs`
- Create: `tests/semantic_ir.rs`
- Modify: `src/model/constraint.rs`
- Modify: `src/model/mod.rs`
- Modify: `src/snapshot.rs`
- Modify: `src/delta.rs`
- Modify: `src/expr/linear.rs`
- Modify: `src/lib.rs`

**Interfaces:**
- Produces:

```rust
#[non_exhaustive]
pub enum ScalarFunction { Linear(LinExpr) }
#[non_exhaustive]
pub enum ScalarSet { LessEqual(ValueExpr), GreaterEqual(ValueExpr), EqualTo(ValueExpr), Interval { lower: ValueExpr, upper: ValueExpr } }
pub struct FunctionConstraint { pub function: ScalarFunction, pub set: ScalarSet }
pub trait IntoScalarFunction { fn into_scalar_function(self) -> ScalarFunction; }
```

- Existing `ConstraintSpec` converts losslessly to `FunctionConstraint`.

- [ ] **Step 1: Write failing conversion and snapshot tests**

```rust
#[test]
fn existing_constraint_builder_is_stored_as_linear_function_in_set() -> Result<(), Box<dyn std::error::Error>> {
    let mut model = Model::new();
    let x = model.add_variable(continuous())?;
    let c = model.add_constraint((2.0 * x + 1.0).le(5.0))?;

    let semantic = model.function_constraint(c)?;
    assert!(matches!(semantic.function, ScalarFunction::Linear(_)));
    assert!(matches!(semantic.set, ScalarSet::LessEqual(_)));
    Ok(())
}
```

- [ ] **Step 2: Verify the test fails**

```bash
cargo test -p roml --test semantic_ir existing_constraint_builder_is_stored_as_linear_function_in_set
```

- [ ] **Step 3: Implement function/set types**

Implement `IntoScalarFunction` for `LinExpr`, `Variable`, and supported scalar expression inputs by converting through existing `LinExpr` conversions. Keep operator overloading unchanged.

- [ ] **Step 4: Store function constraints canonically**

Refactor constraint storage so a user constraint owns `FunctionConstraint` plus metadata/activity. Preserve existing coefficient indexing by projecting `ScalarFunction::Linear` into canonical cells during insertion and mutation.

Do not duplicate coefficient truth: the function expression and canonical cell store must have one authoritative relationship. Choose one of these two and document it in code:

- function expression is authoritative and coefficient index is a derived index updated atomically; or
- coefficient index is authoritative and `function_constraint` reconstructs the function.

Use the current coefficient index as authoritative to minimize P25 risk; store set semantics and reconstruct the linear function deterministically.

- [ ] **Step 5: Extend snapshots without changing backend behavior**

Add canonical function/set fields to `ConstraintEntry`. Keep existing bounds/cells during the transition so current backends remain operational until P26. Add assertions that linear set and legacy bounds agree exactly.

- [ ] **Step 6: Extend canonical deltas**

Add a self-contained semantic operation for constraint-function/set addition/update while retaining current primitive operations through P26 migration. Mark the legacy duplication as transitional in rustdoc and evidence.

- [ ] **Step 7: Run focused equivalence tests**

```bash
cargo test -p roml --test semantic_ir
cargo test -p roml --test m3_baseline_characterization
cargo test -p roml --all-targets
```

- [ ] **Step 8: Commit**

```bash
git add src/function src/model/constraint.rs src/model/mod.rs src/snapshot.rs \
  src/delta.rs src/expr/linear.rs src/lib.rs tests/semantic_ir.rs
git commit -m "feat(model): add linear function-in-set semantics"
```

---

## Task 4: Add the canonical construct store and lifecycle

**Phase:** P25

**Files:**
- Create: `src/construct/mod.rs`
- Create: `src/construct/test_support.rs` under `#[cfg(test)]` or equivalent private fixture
- Modify: `src/model/mod.rs`
- Modify: `src/snapshot.rs`
- Modify: `src/delta.rs`
- Modify: `src/metadata.rs`
- Modify: `src/lib.rs`
- Modify: `tests/semantic_ir.rs`

**Interfaces:**
- Produces:

```rust
pub type Construct = ConstructId;
#[non_exhaustive]
pub enum ConstructKind { /* concrete variants added later */ }
pub struct ConstructEntry { id, kind, active, metadata, dependencies }

impl Model {
    pub fn construct(&self, id: Construct) -> Result<&ConstructEntry, ModelError>;
    pub fn set_construct_active(&mut self, id: Construct, active: bool) -> Result<(), ModelError>;
    pub fn remove_construct(&mut self, id: Construct) -> Result<(), ModelError>;
}
```

- [ ] **Step 1: Write lifecycle tests using a private fixture variant**

Test add/snapshot/clone/deactivate/remove/stale-handle behavior. The fixture must not enter the public enum; use a private test constructor around a minimal internal payload.

- [ ] **Step 2: Implement generation-safe construct IDs/store**

Mirror existing arena identity semantics. IDs are never reused without generation change. Common record fields:

```rust
pub(crate) struct ConstructData {
    pub kind: ConstructKind,
    pub active: bool,
    pub dependencies: Vec<Parameter>,
}
```

Metadata remains in the common metadata store keyed by `EntityRef::Construct`.

- [ ] **Step 3: Add construct canonical changes**

```rust
ConstructAdded { construct, kind }
ConstructRemoved { construct }
ConstructActivityChanged { construct, active }
ConstructUpdated { construct, old, new }
```

Ensure each operation is self-contained enough for canonical replay and audit. Large payload cloning is acceptable in M3 correctness-first code; optimize only after profiling.

- [ ] **Step 4: Add constructs to canonical snapshots**

Sort construct entries by stable ID. Snapshot equality must include semantic payload, activity, and dependencies.

- [ ] **Step 5: Update invariant checker**

Validate:

- referenced user entities exist;
- dependencies refer to live parameters;
- auxiliary ownership has no cycles;
- inactive constructs are not compiled later;
- metadata keys never point to stale entities.

- [ ] **Step 6: Run property/lifecycle tests**

```bash
cargo test -p roml --test semantic_ir
cargo test -p roml --all-targets
```

- [ ] **Step 7: Update P25 evidence and commit**

```bash
git add src/construct src/model/mod.rs src/snapshot.rs src/delta.rs \
  src/metadata.rs src/lib.rs tests/semantic_ir.rs \
  docs/release/evidence/M3_P25_SEMANTIC_IR.md
git commit -m "feat(model): add canonical semantic construct lifecycle"
```

- [ ] **Step 8: Run P25 phase gate**

Run the baseline matrix, public API diff, and package list. Request independent review before P26.

---

## Task 5: Define backend IR, compiled identities, and origin maps

**Phase:** P26

**Files:**
- Create: `src/compiler/mod.rs`
- Create: `src/compiler/backend_ir.rs`
- Create: `src/compiler/origin.rs`
- Create: `src/compiler/report.rs`
- Create: `tests/compiler_identity.rs`
- Create: `docs/release/evidence/M3_P26_COMPILER_BACKEND_IR.md`
- Modify: `src/lib.rs`
- Modify: `src/advanced.rs`

**Interfaces:**
- Produces:

```rust
pub struct CompiledVariableId(u32);
pub struct CompiledConstraintId(u32);
pub struct BackendSnapshot { ... }
pub struct BackendDeltaBatch { ... }
#[non_exhaustive] pub enum BackendConstraint { Indicator(...), Sos1(...), Sos2(...), PiecewiseLinear(...) }
pub enum EntityOrigin { UserVariable(...), UserConstraint(...), UserObjective(...), Construct { ... }, SolveOverlay { ... } }
pub struct OriginMap { ... }
pub struct CompilationReport { ... }
```

- [ ] **Step 1: Write failing origin completeness tests**

Construct a small backend snapshot builder and assert finalization fails when any compiled entity lacks origin.

```rust
assert!(matches!(builder.finish(), Err(CompileError::OriginMappingFailure { .. })));
```

- [ ] **Step 2: Implement distinct compiled IDs**

Use dense deterministic IDs allocated in sorted canonical order. Do not reuse user ID numeric components.

- [ ] **Step 3: Implement backend snapshot types**

Include compiled variables, linear rows, normalized native constraints, objectives, origin map, and report. Keep fields private where possible; expose read-only iterators/accessors under `roml::backend`.

- [ ] **Step 4: Implement origin map bidirectional queries**

Required queries:

```rust
origin.variable(compiled_id) -> &EntityOrigin
origin.constraint(compiled_id) -> &EntityOrigin
origin.variables_for_construct(construct) -> iterator
origin.constraints_for_construct(construct) -> iterator
```

- [ ] **Step 5: Implement compilation report records**

```rust
pub enum RepresentationKind { IdentityLinear, Native(BackendFeature), Bridge(BridgeKind) }
pub struct ConstructCompilation { construct, representation, generated_variables, generated_constraints, notes }
```

No backend prose is required; reports are structured and renderable.

- [ ] **Step 6: Test deterministic ordering and origin completeness**

```bash
cargo test -p roml --test compiler_identity
```

- [ ] **Step 7: Commit**

```bash
git add src/compiler src/lib.rs src/advanced.rs tests/compiler_identity.rs \
  docs/release/evidence/M3_P26_COMPILER_BACKEND_IR.md
git commit -m "feat(compiler): define backend IR and origin mapping"
```

---

## Task 6: Replace flat capabilities with typed feature support

**Phase:** P26

**Files:**
- Create: `src/compiler/capability.rs`
- Modify: `src/solver/backend.rs`
- Modify: `src/solver/session.rs`
- Modify: `src/solver/conformance.rs`
- Modify: `roml-highs/src/session.rs`
- Modify: tests referencing `BackendCapabilities`

**Interfaces:**
- Produces:

```rust
#[non_exhaustive] pub enum BackendFeature { Lp, Mip, IncrementalBounds, IncrementalRows, MipStart, PartialMipStart, MultipleMipStarts, VariableHints, InitialBasis, Iis, FeasibilityRelaxation, Indicator, Sos1, Sos2, NativePiecewiseLinear, NativeMultiObjective }
pub struct FeatureSupport { pub level: SupportLevel, pub limitations: FeatureLimitations }
pub trait CapabilityProvider { fn support(&self, feature: BackendFeature) -> FeatureSupport; }
```

- [ ] **Step 1: Add compatibility tests for current capabilities**

For ReferenceBackend and HiGHS, assert LP/MIP/duals/reduced-cost/current incremental capabilities map to equivalent typed features before removing old field reads.

- [ ] **Step 2: Implement typed registry**

Use a `BTreeMap<BackendFeature, FeatureSupport>` or exhaustive match. `FeatureLimitations` stores structured constraints such as minimum version, model class, maximum starts, and notes.

- [ ] **Step 3: Add a temporary conversion from legacy capabilities**

```rust
impl From<LegacyBackendCapabilities> for BackendCapabilitySet
```

Use only during migration. Mark private/deprecated and remove by the end of P26.

- [ ] **Step 4: Migrate validation and metadata callers**

Replace direct Boolean reads with `supports(feature)`/`support(feature)`. Ensure unsupported solve options still reject exactly as before.

- [ ] **Step 5: Add version-aware HiGHS capability construction**

Capability creation consumes authoritative backend version information already exposed by the session. Do not enable new M3 features yet; report them unsupported until their phases qualify them.

- [ ] **Step 6: Run conformance tests**

```bash
cargo test -p roml solver::conformance
cargo test -p roml-highs --all-targets
```

- [ ] **Step 7: Remove legacy capability struct/public exposure**

Update migration docs/evidence. Keep a compatibility alias only if public API review requires one documented pre-1.0 window.

- [ ] **Step 8: Commit**

```bash
git add src/compiler/capability.rs src/solver/backend.rs src/solver/session.rs \
  src/solver/conformance.rs roml-highs/src/session.rs tests roml-highs/tests
git commit -m "feat(backend): add typed feature capabilities"
```

---

## Task 7: Implement the identity compiler and migrate backend synchronization

**Phase:** P26

**Files:**
- Create: `src/compiler/session.rs`
- Create: `roml-highs/src/compiler.rs`
- Modify: `src/solver/session.rs`
- Modify: `src/solver/facade.rs`
- Modify: `src/solver/conformance.rs`
- Modify: `src/reference_backend.rs` or current reference backend path
- Modify: `roml-highs/src/session.rs`
- Modify: `src/advanced.rs`
- Create: `docs/migration/M3_BACKEND_IR.md`

**Interfaces:**
- Produces:

```rust
pub struct CompilationSession { ... }
pub enum BackendSynchronization { Delta(BackendDeltaBatch), Rebuild(BackendSnapshot) }
pub trait BackendSession { fn synchronize(&mut self, sync: BackendSynchronization) -> Result<SyncReceipt, BackendError>; ... }
```

- [ ] **Step 1: Write identity-compiler tests**

For a primitive linear model, assert:

- same user variable/row counts;
- same bounds/types/cells/objective;
- deterministic compiled IDs;
- user origins for all entities;
- `RepresentationKind::IdentityLinear`.

- [ ] **Step 2: Implement `CompilationSession::compile_snapshot`**

Compile only primitive linear canonical state. If an active semantic construct exists, return `CompileError::UnsupportedConstruct` until bridge tasks land.

- [ ] **Step 3: Implement primitive compiled deltas**

Translate canonical add/remove/bound/cell/objective operations into backend operations with compiled ID mapping. If mapping or recipe certainty is absent, return `CompilationDecision::Rebuild` rather than guessing.

- [ ] **Step 4: Migrate ReferenceBackend first**

Change its synchronization input to backend IR. Run all core synchronization/recovery/differential tests.

- [ ] **Step 5: Migrate HiGHS**

Move current snapshot/delta translation into `roml-highs/src/compiler.rs` or a focused backend-IR translator. It must not receive `ModelSnapshot` after migration.

- [ ] **Step 6: Preserve one-rebuild-retry invariant**

`SolverSession` flow becomes:

```rust
commit -> compiler decision -> backend synchronization -> solve
```

A compile failure returns before backend mutation. A backend apply failure follows existing health/rebuild policy with at most one automatic rebuild retry.

- [ ] **Step 7: Add randomized compiled delta versus rebuild tests**

Reuse existing random legal mutation generators. Compare backend-observable state after compiled deltas to a fresh compiled snapshot rebuild at the same revision.

- [ ] **Step 8: Run P26 full gate**

```bash
cargo test -p roml --all-targets
cargo test -p roml-highs --all-targets
cargo clippy -p roml --all-targets -- -D warnings
cargo clippy -p roml-highs --all-targets -- -D warnings
```

- [ ] **Step 9: Commit migration and update evidence**

```bash
git add src/compiler/session.rs src/solver src/advanced.rs \
  roml-highs/src/compiler.rs roml-highs/src/session.rs docs/migration/M3_BACKEND_IR.md \
  docs/release/evidence/M3_P26_COMPILER_BACKEND_IR.md tests roml-highs/tests
git commit -m "feat(sync): compile canonical models into backend IR"
```

Request independent architecture review before P27/P32.

---

## Task 8: Unify declared domains and add persistent fixing

**Phase:** P27

**Files:**
- Modify: `src/model/variable.rs`
- Modify: `src/model/mod.rs`
- Modify: `src/snapshot.rs`
- Modify: `src/delta.rs`
- Modify: `src/compiler/session.rs`
- Create: `tests/fixing_assignment.rs`
- Create: `docs/release/evidence/M3_P27_FIXING_LOCKS_OVERLAYS.md`

**Interfaces:**
- Produces:

```rust
pub struct VariableDomain { bounds: Bounds, var_type: VarType, semi: Option<SemiDomain> }
pub struct VariableFixing { value: f64, provenance: FixingProvenance }
impl Model { fix, unfix, fixing, declared_bounds, effective_bounds }
```

- [ ] **Step 1: Write reference-state-machine tests**

Cover:

- continuous fix/unfix;
- integer near-integral acceptance/rejection using model tolerance;
- binary normalization;
- declared-bound change while fixed;
- semi-domain preservation;
- stale variable rejection;
- atomic failure.

- [ ] **Step 2: Refactor variable storage**

Replace fragmented fields/side map with one `VariableRecord`:

```rust
pub(crate) struct VariableRecord {
    pub domain: VariableDomain,
    pub fixing: Option<VariableFixing>,
    pub active: bool,
    pub name: Option<String>,
}
```

Remove `Model::semicontinuous_lower` after all snapshot/delta/backend tests migrate.

- [ ] **Step 3: Implement effective-domain derivation**

`effective_bounds()` returns fixed equal bounds when fixing exists; otherwise declared bounds. Keep domain type unchanged.

- [ ] **Step 4: Add canonical fixing changes**

```rust
SetVariableFixing { var, fixing: Option<VariableFixing> }
```

Do not represent unfix as a guessed bound restore; compiler derives effective bounds from canonical state.

- [ ] **Step 5: Compile fixing to backend bound updates**

If backend supports incremental bounds, emit `SetCompiledVariableBounds`; otherwise rebuild. Include bound origin in compiler metadata.

- [ ] **Step 6: Run focused and randomized tests**

```bash
cargo test -p roml --test fixing_assignment
cargo test -p roml --all-targets
cargo test -p roml-highs --all-targets
```

- [ ] **Step 7: Commit**

```bash
git add src/model/variable.rs src/model/mod.rs src/snapshot.rs src/delta.rs \
  src/compiler/session.rs tests/fixing_assignment.rs \
  docs/release/evidence/M3_P27_FIXING_LOCKS_OVERLAYS.md
git commit -m "feat(model): add first-class variable fixing"
```

---

## Task 9: Add primal assignments and solution extraction

**Phase:** P27

**Files:**
- Create: `src/assignment.rs`
- Modify: `src/solution/mod.rs`
- Modify: `src/solution/metadata.rs`
- Modify: `src/lib.rs`
- Modify: prelude
- Modify: `tests/fixing_assignment.rs`

**Interfaces:**
- Produces:

```rust
pub struct PrimalAssignment { lineage, source_revision, values }
impl PrimalAssignment { new, set, remove, value, iter, select }
impl Solution { pub fn primal_assignment(&self) -> PrimalAssignment; pub fn select_assignment(...) -> PrimalAssignment; }
```

- [ ] **Step 1: Write failing lineage and validation tests**

Test independent-model mismatch, clone-lineage acceptance, stale removed/recreated handle rejection, non-finite values, and partial assignments.

- [ ] **Step 2: Implement assignment construction**

Validate variable belongs to lineage/model at application time. Construction from `Model` captures lineage but may accept only handles known to that model.

- [ ] **Step 3: Add solution lineage/revision extraction**

`Solution::primal_assignment()` copies only user variable values, excluding compiled/generated-only entities. Stable canonical auxiliary variables such as future soft slacks are user-addressable and included.

- [ ] **Step 4: Add selection helpers**

```rust
pub fn select<I: IntoIterator<Item = Variable>>(&self, variables: I) -> PrimalAssignment;
pub fn integer_variables(&self, model: &Model) -> Result<PrimalAssignment, AssignmentError>;
```

- [ ] **Step 5: Run tests and commit**

```bash
cargo test -p roml --test fixing_assignment
cargo test -p roml --all-targets
git add src/assignment.rs src/solution src/lib.rs tests/fixing_assignment.rs
git commit -m "feat(solution): add lineage-bound primal assignments"
```

---

## Task 10: Implement reversible solve overlays and solution locks

**Phase:** P27

**Files:**
- Create: `src/solver/overlay.rs`
- Create: `src/solver/effective_plan.rs`
- Create: `tests/solve_overlay.rs`
- Modify: `src/solver/session.rs`
- Modify: `src/solver/backend.rs`
- Modify: `src/compiler/backend_ir.rs`
- Modify: `src/compiler/origin.rs`
- Modify: `roml-highs/src/session.rs`
- Modify: `roml-highs/src/facade.rs`

**Interfaces:**
- Produces:

```rust
pub struct SolutionLock { assignment, selector, continuous }
pub struct SolveOverlay { operations: Vec<OverlayOperation> }
pub struct OverlayReceipt { reverse: Vec<BackendOverlayOperation>, health_token: ... }
pub trait OverlaySession { apply_overlay, rollback_overlay }
```

- [ ] **Step 1: Write lock-selector tests**

Verify all/integer/binary/explicit/exclusion selection and exact/band continuous bounds.

- [ ] **Step 2: Write failure-injection lifecycle tests**

Inject failure at:

1. overlay validation;
2. first operation;
3. middle operation;
4. solve;
5. result extraction;
6. rollback;
7. post-rollback health verification.

After each case, the next clean solve must equal a fresh rebuilt backend.

- [ ] **Step 3: Implement overlay compilation**

Validate lineage/entities before backend mutation. Convert locks to temporary compiled bound changes with `EntityOrigin::SolveOverlay`.

- [ ] **Step 4: Implement backend overlay trait**

Keep it optional/bounded rather than adding overlay methods to every unrelated trait. ReferenceBackend implements exact reversible state snapshots for tests. HiGHS stores previous bounds/temporary rows required for reversal.

- [ ] **Step 5: Integrate transactional lifecycle**

Use an internal guard whose `finish()` performs explicit rollback and returns errors. Do not rely solely on `Drop` for fallible rollback. `Drop` may mark session dirty as a last-resort safety action.

- [ ] **Step 6: Add model revision invariance assertions**

Before and after lock solve, assert canonical model revision and snapshot are unchanged.

- [ ] **Step 7: Run focused/full tests**

```bash
cargo test -p roml --test solve_overlay -- --nocapture
cargo test -p roml-highs --all-targets
cargo test -p roml --all-targets
```

- [ ] **Step 8: Commit and finish P27 evidence**

```bash
git add src/solver/overlay.rs src/solver/effective_plan.rs src/solver/session.rs \
  src/solver/backend.rs src/compiler roml-highs/src tests/solve_overlay.rs \
  docs/release/evidence/M3_P27_FIXING_LOCKS_OVERLAYS.md
git commit -m "feat(solve): add reversible solution-lock overlays"
```

Request review before P28.

---

## Task 11: Add SolvePlan and effective-plan metadata

**Phase:** P28

**Files:**
- Create: `src/solver/plan.rs`
- Modify: `src/solver/options.rs`
- Modify: `src/solver/facade.rs`
- Modify: `src/solver/session.rs`
- Modify: `src/solution/metadata.rs`
- Modify: `roml-highs/src/facade.rs`
- Create: `roml-highs/tests/solve_plan.rs`
- Create: `docs/release/evidence/M3_P28_SOLVE_PLAN_STARTS_HINTS.md`

**Interfaces:**
- Produces:

```rust
pub struct SolvePlan { options, overlay, mip_starts, hints, objective_override, unsupported }
pub enum UnsupportedFeaturePolicy { Reject, IgnoreExplicitly, Convert(ConversionPolicy) }
pub struct EffectiveSolvePlan { applied_features, adjustments, rejections, objective_stages }
```

- [ ] **Step 1: Write API compatibility tests**

Assert:

```rust
highs.solve(&mut model)
highs.solve_with(&mut model, options)
highs.solve_plan(&mut model, SolvePlan::new())
```

produce equivalent results/effective options on a deterministic model.

- [ ] **Step 2: Implement `SolvePlan` validation**

Validation occurs before canonical synchronization or backend mutation when possible. Validate assignment lineage, finite values, duplicate/conflicting overlays, and unsupported policy syntax.

- [ ] **Step 3: Refactor façade methods**

`solve()` builds `SolvePlan::new()`. `solve_with()` builds `SolvePlan::new().options(options)`. One internal `execute_plan` owns orchestration.

- [ ] **Step 4: Store effective-plan metadata**

Keep existing effective configuration and add applied/adjusted/rejected solve features. Existing metadata access remains source-compatible.

- [ ] **Step 5: Run tests and commit**

```bash
cargo test -p roml-highs --test solve_plan
cargo test -p roml --all-targets
cargo test -p roml-highs --all-targets
git add src/solver/plan.rs src/solver/options.rs src/solver/facade.rs \
  src/solver/session.rs src/solution/metadata.rs roml-highs/src/facade.rs \
  roml-highs/tests/solve_plan.rs docs/release/evidence/M3_P28_SOLVE_PLAN_STARTS_HINTS.md
git commit -m "feat(solve): add explicit solve plans"
```

---

## Task 12: Add MIP starts and variable hints with version-qualified HiGHS support

**Phase:** P28

**Files:**
- Modify: `src/assignment.rs`
- Modify: `src/solver/plan.rs`
- Modify: `src/compiler/capability.rs`
- Create: `roml-highs/src/start.rs`
- Create: `docs/knowledge/highs-starts-hints.md`
- Modify: `roml-highs/src/session.rs`
- Modify: `roml-highs/tests/solve_plan.rs`

**Interfaces:**
- Produces:

```rust
pub struct MipStart { assignment, repair: RepairPolicy, name: Option<String> }
pub enum RepairPolicy { BackendDefault, RejectIncomplete, AllowRepair }
pub struct VariableHints { entries }
pub struct VariableHint { value, priority }
```

- [ ] **Step 1: Audit official APIs before production code**

Inspect exact pinned bundled and minimum system HiGHS headers/generated bindings. Record:

- start symbols and signatures;
- partial-start semantics;
- multiple-start support;
- hint support or absence;
- return codes;
- version availability;
- whether starts persist across solves and how they are cleared.

Commit `docs/knowledge/highs-starts-hints.md` before native implementation.

- [ ] **Step 2: Write backend-independent semantic tests**

Prove starts/hints leave canonical snapshot and feasible-region fingerprint unchanged. Test default unsupported rejection and explicit conversion metadata.

- [ ] **Step 3: Implement core start/hint types**

Reject non-finite values and duplicate entries. Hints may be mutually inconsistent because they are independent guidance; document this explicitly.

- [ ] **Step 4: Implement HiGHS start application from official bindings**

Apply only supported features. Ensure starts are cleared/replaced according to official lifecycle. Version capability set must match compiled/runtime interface.

- [ ] **Step 5: Implement hint behavior**

If official HiGHS support is absent, return `PlanRejection`/`UnsupportedFeature` under default policy. Do not map hints to starts unless the user selected that conversion.

- [ ] **Step 6: Add full/partial/unsupported tests**

Use small deterministic MIPs. Assert effective-plan metadata, not solver runtime improvements.

- [ ] **Step 7: Run and commit**

```bash
cargo test -p roml-highs --test solve_plan -- --nocapture
cargo test -p roml --all-targets
cargo test -p roml-highs --all-targets
git add src/assignment.rs src/solver/plan.rs src/compiler/capability.rs \
  roml-highs/src/start.rs roml-highs/src/session.rs roml-highs/tests/solve_plan.rs \
  docs/knowledge/highs-starts-hints.md docs/release/evidence/M3_P28_SOLVE_PLAN_STARTS_HINTS.md
git commit -m "feat(highs): add qualified MIP start support"
```

Finish P28 independent review before P29–P31.

---

## Task 13: Define normalized infeasibility analysis and reporting

**Phase:** P29

**Files:**
- Create: `src/solver/infeasibility.rs`
- Create: `src/report/infeasibility.rs` or keep renderer beside analysis if no report module exists
- Modify: `src/solver/session.rs`
- Modify: `src/compiler/origin.rs`
- Modify: `src/compiler/capability.rs`
- Create: `tests/infeasibility_report.rs`
- Create: `docs/release/evidence/M3_P29_IIS_CONFLICTS.md`

**Interfaces:**
- Produces:

```rust
pub trait InfeasibilityAnalysisSession { fn analyze_infeasibility(&mut self, request: &InfeasibilityRequest) -> Result<BackendConflict, BackendError>; }
pub struct InfeasibilityReport { lineage, revision, backend, kind, scope, minimality, completion, members, statistics }
pub enum ConflictMember { ConstraintSide { ... }, VariableBound { ... }, Construct { ... } }
```

- [ ] **Step 1: Write report semantic tests with synthetic backend conflicts**

Cover row lower/upper/equality, declared bounds, persistent fixing, overlay lock, and generated construct row. Assert all map to original entities/roles.

- [ ] **Step 2: Implement normalized types without backend assumptions**

Require kind/scope/minimality/completion fields at construction. No default that overclaims irreducibility.

- [ ] **Step 3: Implement origin-aware mapper**

Mapper consumes the exact `BackendSnapshot`/`OriginMap` used for the infeasible solve revision. Reject stale/missing compilation fingerprints.

- [ ] **Step 4: Implement text and Markdown renderers**

Render stable ROML content only. Backend free-form prose may appear in an optional details field but is not used in golden tests.

- [ ] **Step 5: Add `SolverSession::analyze_infeasibility` orchestration**

Ensure model is synchronized at the requested revision. If last result is not infeasible and backend requires prior infeasible solve, return `AnalysisError::ModelNotInfeasible`.

- [ ] **Step 6: Run tests and commit**

```bash
cargo test -p roml --test infeasibility_report
cargo test -p roml --all-targets
git add src/solver/infeasibility.rs src/report src/compiler/origin.rs \
  src/compiler/capability.rs tests/infeasibility_report.rs \
  docs/release/evidence/M3_P29_IIS_CONFLICTS.md
git commit -m "feat(analysis): add normalized infeasibility reports"
```

---

## Task 14: Implement version-qualified HiGHS IIS support

**Phase:** P29

**Files:**
- Create: `roml-highs/src/iis.rs`
- Create: `docs/knowledge/highs-iis.md`
- Modify: `roml-highs/src/session.rs`
- Modify: `roml-highs/src/facade.rs`
- Create: `roml-highs/tests/iis_reports.rs`
- Modify: support matrix docs

**Interfaces:**
- Consumes normalized analysis types from Task 13.
- Produces native HiGHS conflict members over compiled IDs plus exact capability limitations.

- [ ] **Step 1: Audit official bundled/system APIs**

Record exact symbols, structures, status codes, supported model classes, minimality guarantees, presolve/original-model scope, and version availability. If current bindings lack required official symbols, prepare a separate dependency-upgrade commit to the first qualified version; do not handwrite ABI declarations.

- [ ] **Step 2: Add compile/version capability tests**

Bundled and system configurations must either expose `BackendFeature::Iis` with precise limitations or report unsupported.

- [ ] **Step 3: Implement native extraction**

Check every return code. Convert native row/column bound participation to compiled IDs and side-specific members. Do not map to user IDs in the backend crate.

- [ ] **Step 4: Add deterministic infeasible fixtures**

Include:

- conflicting named rows;
- row versus declared variable bound;
- persistent fixing conflict;
- temporary lock conflict;
- bridged indicator conflict after P32 is available, initially guarded/added later.

- [ ] **Step 5: Assert report guarantees**

Tests assert kind/scope/minimality from documented behavior, not generic `Iis` labels.

- [ ] **Step 6: Run and commit**

```bash
cargo test -p roml-highs --test iis_reports -- --nocapture
cargo test -p roml-highs --all-targets
cargo test -p roml --all-targets
git add roml-highs/src/iis.rs roml-highs/src/session.rs roml-highs/src/facade.rs \
  roml-highs/tests/iis_reports.rs docs/knowledge/highs-iis.md \
  docs/release/evidence/M3_P29_IIS_CONFLICTS.md docs
git commit -m "feat(highs): add version-qualified IIS analysis"
```

---

## Task 15: Implement soft constraints and violation variables

**Phase:** P30

**Files:**
- Create: `src/construct/soft.rs`
- Create: `src/compiler/bridge/soft.rs`
- Modify: `src/construct/mod.rs`
- Modify: `src/model/mod.rs`
- Modify: `src/objective_policy.rs` if already present, otherwise Task 17 consumes penalty priority
- Modify: `src/solution/mod.rs`
- Create: `roml-highs/tests/soft_constraints.rs`
- Create: `docs/release/evidence/M3_P30_SOFT_CONSTRAINTS.md`

**Interfaces:**
- Produces:

```rust
pub fn soft() -> SoftConstraintSpec;
pub struct SoftConstraintHandle { constraint, lower_violation, upper_violation, construct }
impl Model { pub fn soften(&mut self, constraint: Constraint, spec: SoftConstraintSpec) -> Result<SoftConstraintHandle, ModelError>; pub fn harden(&mut self, handle: SoftConstraintHandle) -> Result<(), ModelError>; }
impl Solution { pub fn violation(&self, handle: SoftConstraintHandle) -> Option<ConstraintViolation>; }
```

- [ ] **Step 1: Write algebra tests before compiler code**

For each constraint shape, assert the semantic bridge equations:

```text
upper: a(x) - s_u <= u
lower: a(x) + s_l >= l
range: l - s_l <= a(x) <= u + s_u
```

Use direct expression evaluation over sample assignments.

- [ ] **Step 2: Implement builder validation**

Validate sides against constraint bounds, finite/nonnegative maximum violations, finite penalty weights, and objective handles. Equality/ranged `Auto` creates both sides.

- [ ] **Step 3: Create stable auxiliary variables**

Add auxiliary `Variable` records with `EntityOrigin`/metadata indicating soft construct and lower/upper role. Return handles. Ordinary variable listing may include them only through an explicit include-auxiliary option; direct handle access always works.

- [ ] **Step 4: Implement soft bridge**

The canonical original constraint remains a semantic constraint with soft attachment. Compiler emits adjusted linear row(s), violation variable bounds, and penalty objective contributions.

- [ ] **Step 5: Implement objective sign handling**

Penalty semantics mean minimizing weighted violation. For a maximize target, subtract penalty; for minimize, add penalty. Add exact objective-cell tests including parameter-dependent weights.

- [ ] **Step 6: Implement signed correction separately**

Use positive/negative parts for L1 penalty. Reject ambiguous free-signed linear penalty requests.

- [ ] **Step 7: Add solution violation accessors and examples**

`ConstraintViolation { lower, upper, total }` derives from stable auxiliary values.

- [ ] **Step 8: Run solver tests and commit**

```bash
cargo test -p roml-highs --test soft_constraints
cargo test -p roml --all-targets
cargo test -p roml-highs --all-targets
git add src/construct/soft.rs src/compiler/bridge/soft.rs src/model/mod.rs \
  src/solution/mod.rs roml-highs/tests/soft_constraints.rs \
  docs/release/evidence/M3_P30_SOFT_CONSTRAINTS.md
git commit -m "feat(model): add semantic soft constraints"
```

---

## Task 16: Add solve-scoped feasibility relaxation

**Phase:** P30

**Files:**
- Create: `src/solver/feasibility_relaxation.rs`
- Create: `roml-highs/src/relaxation.rs`
- Modify: `src/compiler/capability.rs`
- Modify: `roml-highs/src/facade.rs`
- Modify: `roml-highs/tests/soft_constraints.rs`
- Modify: `docs/knowledge/highs-feasibility-relaxation.md`

**Interfaces:**
- Produces:

```rust
pub struct FeasibilityRelaxationRequest { row_penalties, bound_penalties, measure }
pub struct FeasibilityRelaxationResult { solution, violations, total_penalty, effective_request }
pub enum RelaxationMeasure { L1, LInfinity, Cardinality }
```

M3 implements only measures officially supported and qualified; unsupported measures reject.

- [ ] **Step 1: Audit official HiGHS API and lifecycle**

Record whether relaxation mutates the native model, how to restore it, supported penalty arrays/measures, and result semantics.

- [ ] **Step 2: Implement core request/result types**

Requests reference original ROML constraints/bounds and compile through origin maps to native arrays.

- [ ] **Step 3: Implement solve-scoped native operation**

Use overlay/rebuild safety. If the native API mutates the model irreversibly or restoration is uncertain, execute on a temporary rebuilt session rather than the persistent session.

- [ ] **Step 4: Add distinction tests**

Assert feasibility relaxation leaves canonical model unchanged and does not create persistent soft-constraint handles.

- [ ] **Step 5: Run and commit**

```bash
cargo test -p roml-highs --test soft_constraints feasibility_relaxation
cargo test -p roml-highs --all-targets
git add src/solver/feasibility_relaxation.rs roml-highs/src/relaxation.rs \
  roml-highs/src/facade.rs roml-highs/tests/soft_constraints.rs \
  docs/knowledge/highs-feasibility-relaxation.md
git commit -m "feat(analysis): add solve-scoped feasibility relaxation"
```

---

## Task 17: Add canonical objective policies

**Phase:** P31

**Files:**
- Create: `src/objective_policy.rs`
- Modify: `src/model/mod.rs`
- Modify: `src/snapshot.rs`
- Modify: `src/delta.rs`
- Modify: `src/solution/mod.rs`
- Create: `tests/objective_policy.rs`
- Create: `docs/release/evidence/M3_P31_LEXICOGRAPHIC.md`

**Interfaces:**
- Produces:

```rust
pub enum ObjectivePolicy { Single(Objective), Weighted(Vec<WeightedObjective>), Lexicographic(Vec<ObjectiveLevel>) }
pub struct ObjectiveLevel { objective, absolute_tolerance, relative_tolerance }
impl Model { pub fn set_objective_policy(&mut self, policy: ObjectivePolicy) -> Result<(), ModelError>; pub fn objective_policy(&self) -> &ObjectivePolicy; }
```

- [ ] **Step 1: Write validation tests**

Reject stale/inactive objectives, duplicates in one policy, negative/non-finite tolerances, empty policies, and ambiguous weighted senses unless normalized semantics are specified.

- [ ] **Step 2: Implement builders**

```rust
lexicographic().first(obj).then(obj2, degradation().relative(1e-3))
weighted().term(obj, 1.0).term(obj2, 0.5)
```

- [ ] **Step 3: Store policy canonically**

Add snapshot/delta operations. Existing `minimize`/`maximize` set `ObjectivePolicy::Single` automatically.

- [ ] **Step 4: Extend solution objective storage**

Store a map of objective values and ordered stage results while preserving `objective_value()` as primary convenience.

- [ ] **Step 5: Run tests and commit**

```bash
cargo test -p roml --test objective_policy
cargo test -p roml --all-targets
git add src/objective_policy.rs src/model/mod.rs src/snapshot.rs src/delta.rs \
  src/solution/mod.rs tests/objective_policy.rs docs/release/evidence/M3_P31_LEXICOGRAPHIC.md
git commit -m "feat(model): add objective policies"
```

---

## Task 18: Implement portable lexicographic execution

**Phase:** P31

**Files:**
- Create: `src/solver/multiobjective.rs`
- Modify: `src/solver/session.rs`
- Modify: `src/solver/overlay.rs`
- Modify: `src/solution/metadata.rs`
- Create: `roml-highs/tests/lexicographic.rs`

**Interfaces:**
- Produces portable sequential stage execution and objective-lock overlay rows.

- [ ] **Step 1: Define degradation formulas in tests**

For minimization stage optimum `z*`:

```text
f(x) <= z* + abs_tol + rel_tol * |z*|
```

For maximization:

```text
f(x) >= z* - abs_tol - rel_tol * |z*|
```

Use these exact formulas for M3. Test positive, zero, and negative objective values.

- [ ] **Step 2: Implement stage executor**

For each level:

1. set temporary stage objective;
2. solve;
3. validate stage status under `RequireOptimal` or explicit `UseBestFeasible`;
4. record value/status;
5. add temporary lock row before next stage.

- [ ] **Step 3: Guarantee cleanup**

All temporary objectives/rows use overlay lifecycle. A failure at any stage rolls back all stage artifacts or marks rebuild required.

- [ ] **Step 4: Add deterministic service/cost/switch tests**

Assert final solution, all stage values, lock bounds, and no subsequent-solve leakage.

- [ ] **Step 5: Run and commit**

```bash
cargo test -p roml-highs --test lexicographic portable
cargo test -p roml --all-targets
cargo test -p roml-highs --all-targets
git add src/solver/multiobjective.rs src/solver/session.rs src/solver/overlay.rs \
  src/solution/metadata.rs roml-highs/tests/lexicographic.rs
git commit -m "feat(solve): add portable lexicographic execution"
```

---

## Task 19: Qualify native HiGHS multiobjective execution

**Phase:** P31

**Files:**
- Create: `roml-highs/src/multiobjective.rs`
- Create: `docs/knowledge/highs-multiobjective.md`
- Modify: `roml-highs/src/session.rs`
- Modify: `src/compiler/capability.rs`
- Modify: `roml-highs/tests/lexicographic.rs`

- [ ] **Step 1: Audit official semantics**

Record priorities, weights, absolute/relative tolerances, sense handling, version availability, and result reporting.

- [ ] **Step 2: Define semantic-match predicate**

Native path is eligible only when every ROML level maps exactly to official semantics. Otherwise select portable path and report why.

- [ ] **Step 3: Implement native translator and clear lifecycle**

Ensure objective definitions do not persist unexpectedly across later single-objective solves.

- [ ] **Step 4: Build native-versus-portable differential corpus**

Cover mixed senses only if ROML policy permits them and backend semantics match. Assert objective values and degradation constraints within declared tolerances.

- [ ] **Step 5: Run and commit**

```bash
cargo test -p roml-highs --test lexicographic -- --nocapture
cargo test -p roml-highs --all-targets
git add roml-highs/src/multiobjective.rs roml-highs/src/session.rs \
  src/compiler/capability.rs roml-highs/tests/lexicographic.rs \
  docs/knowledge/highs-multiobjective.md docs/release/evidence/M3_P31_LEXICOGRAPHIC.md
git commit -m "feat(highs): add qualified native multiobjective execution"
```

---

## Task 20: Add bound analysis and safe bridge infrastructure

**Phase:** P32/P33 foundation

**Files:**
- Create: `src/compiler/bounds.rs`
- Create: `src/compiler/bridge/mod.rs`
- Modify: `src/compiler/session.rs`
- Modify: `src/compiler/report.rs`
- Create: `tests/compiler_bridges.rs`

**Interfaces:**
- Produces:

```rust
pub struct Interval { lower: f64, upper: f64 }
pub struct BoundTrace { terms: Vec<BoundSource>, result: Interval }
pub fn scalar_bounds(snapshot: &ModelSnapshot, function: &ScalarFunction) -> Result<(Interval, BoundTrace), BoundAnalysisError>;
pub trait Bridge { fn compile(&self, context: &mut BridgeContext) -> Result<BridgeOutput, CompileError>; }
```

- [ ] **Step 1: Write interval arithmetic tests**

Cover positive/negative coefficients, constants, fixed variables, infinite bounds, and parameter values.

- [ ] **Step 2: Implement deterministic linear interval propagation**

For each term `a*x`, use `[a*l, a*u]` for `a >= 0` and `[a*u, a*l]` for `a < 0`. Sum with checked finite/infinite handling; reject NaN.

- [ ] **Step 3: Implement Big-M derivation helpers**

Provide construct-specific helpers that derive the exact needed one-sided relaxation constant from expression bounds. Return `UnboundedBigM` when finite proof is absent.

- [ ] **Step 4: Add bridge context/output types**

Bridge output includes generated variables/rows/native constraints, origins, representation, dependencies, and notes. Finalization validates origin completeness.

- [ ] **Step 5: Run tests and commit**

```bash
cargo test -p roml --test compiler_bridges bound
cargo test -p roml --all-targets
git add src/compiler/bounds.rs src/compiler/bridge/mod.rs src/compiler/session.rs \
  src/compiler/report.rs tests/compiler_bridges.rs
git commit -m "feat(compiler): add bound analysis and bridge contracts"
```

---

## Task 21: Implement indicators and reification

**Phase:** P32

**Files:**
- Create: `src/construct/indicator.rs`
- Create: `src/compiler/bridge/indicator.rs`
- Modify: `src/construct/mod.rs`
- Modify: `src/model/mod.rs`
- Modify: `src/compiler/backend_ir.rs`
- Modify: `src/compiler/session.rs`
- Create: `tests/common_constructs.rs`
- Create: `docs/release/evidence/M3_P32_COMMON_CONSTRUCTS.md`

**Interfaces:**
- Produces:

```rust
pub fn when_one() -> IndicatorActivation;
pub fn when_zero() -> IndicatorActivation;
impl Model { pub fn add_indicator(&mut self, binary: Variable, activation: IndicatorActivation, constraint: ConstraintSpec) -> Result<Construct, ModelError>; pub fn reify(&mut self, constraint: ConstraintSpec, spec: ReificationSpec) -> Result<Variable, ModelError>; }
```

- [ ] **Step 1: Write semantic validation tests**

Reject non-binary activators, stale entities, non-finite separation, and exact continuous reification without separation.

- [ ] **Step 2: Implement canonical payloads**

Store activator, activation value, function/set, exactness, separation, and formulation override.

- [ ] **Step 3: Implement native indicator backend IR path**

Under `Auto`, select native only when capability supports the exact function/set form and activation semantics.

- [ ] **Step 4: Implement Big-M bridge**

Derive one-sided M from `BoundAnalysis`. Record values/traces. Reject unbounded expressions.

- [ ] **Step 5: Implement reification as two implications**

For continuous expressions, use explicit separation for complement. For proven integer-valued expressions, permit inferred unit gap and record proof source.

- [ ] **Step 6: Add explicit reference-formulation differential tests**

Enumerate binary values and bounded small domains. Compare feasible assignments/native/portable results.

- [ ] **Step 7: Run and commit**

```bash
cargo test -p roml --test common_constructs indicator
cargo test -p roml-highs --test formulation_equivalence indicator
cargo test -p roml --all-targets
git add src/construct/indicator.rs src/compiler/bridge/indicator.rs src/model/mod.rs \
  src/compiler tests/common_constructs.rs docs/release/evidence/M3_P32_COMMON_CONSTRUCTS.md
git commit -m "feat(model): add indicators and reification"
```

---

## Task 22: Implement Boolean and cardinality constructs

**Phase:** P32

**Files:**
- Create: `src/construct/boolean.rs`
- Create: `src/compiler/bridge/boolean.rs`
- Modify: `src/model/mod.rs`
- Modify: `tests/common_constructs.rs`
- Modify: `roml-highs/tests/formulation_equivalence.rs`

**Interfaces:**
- Produces `implies`, `iff`, `any_of`, `all_of`, `exactly_one`, `at_most`, and `at_least`.

- [ ] **Step 1: Write truth-table tests**

Enumerate all binary assignments for implication, equivalence, any, and all. Enumerate cardinality counts for small vectors.

- [ ] **Step 2: Implement canonical payloads and validation**

Reject non-binary variables, empty collections where semantics are ambiguous, invalid `k`, and duplicates unless deduplicating is explicitly documented. Choose rejection for duplicates to surface modeling mistakes.

- [ ] **Step 3: Implement exact linear bridges**

Use standard bounded linear formulations. Assign generated roles/names deterministically.

- [ ] **Step 4: Run differential tests and commit**

```bash
cargo test -p roml --test common_constructs boolean
cargo test -p roml-highs --test formulation_equivalence boolean
git add src/construct/boolean.rs src/compiler/bridge/boolean.rs src/model/mod.rs \
  tests/common_constructs.rs roml-highs/tests/formulation_equivalence.rs
git commit -m "feat(model): add Boolean and cardinality constructs"
```

---

## Task 23: Implement min/max, absolute value, positive part, and clamp

**Phase:** P32

**Files:**
- Create: `src/construct/minmax.rs`
- Create: `src/construct/absolute.rs`
- Create: `src/compiler/bridge/minmax.rs`
- Create: `src/compiler/bridge/absolute.rs`
- Modify: `src/model/mod.rs`
- Modify: `tests/common_constructs.rs`
- Modify: `roml-highs/tests/formulation_equivalence.rs`

**Interfaces:**
- Produces:

```rust
model.max_of(functions, exact()) -> Result<Variable, ModelError>
model.max_of(functions, epigraph()) -> Result<Variable, ModelError>
model.min_of(functions, exact()/hypograph())
model.abs(function, exact())
model.positive_part(function)
model.clamp(function, lower, upper)
```

- [ ] **Step 1: Write exactness distinction tests**

Assert epigraph permits values above max while exact relation does not. Assert hypograph permits values below min. Do not rely on objective context.

- [ ] **Step 2: Implement one-sided linear formulations**

Epigraph max and hypograph min create only comparison rows and no binaries.

- [ ] **Step 3: Implement exact bounded formulations**

Use native general constraints if qualified; otherwise selector binaries with bound-derived M and exactly-one selection. Reject when bounds are insufficient.

- [ ] **Step 4: Implement abs/positive part/clamp using semantic recipes**

Do not recursively erase constructs in canonical state. Bridges may reuse internal bridge helpers while preserving top-level origin roles.

- [ ] **Step 5: Add randomized evaluation tests**

Generate bounded functions and compare output variable to direct Rust evaluation at feasible fixed inputs.

- [ ] **Step 6: Run and commit**

```bash
cargo test -p roml --test common_constructs minmax
cargo test -p roml-highs --test formulation_equivalence minmax
git add src/construct/minmax.rs src/construct/absolute.rs \
  src/compiler/bridge/minmax.rs src/compiler/bridge/absolute.rs src/model/mod.rs \
  tests/common_constructs.rs roml-highs/tests/formulation_equivalence.rs
git commit -m "feat(model): add min max and absolute constructs"
```

---

## Task 24: Implement supported exact products

**Phase:** P32

**Files:**
- Create: `src/construct/product.rs`
- Create: `src/compiler/bridge/product.rs`
- Modify: `src/model/mod.rs`
- Modify: `tests/common_constructs.rs`
- Modify: `roml-highs/tests/formulation_equivalence.rs`

**Interfaces:**
- Produces exact binary-binary and binary-times-bounded-linear products only.

- [ ] **Step 1: Write domain rejection tests**

Continuous-continuous product must return an error that names the unsupported nonconvex exact operation and suggests a future explicitly named relaxation, not silently create McCormick inequalities.

- [ ] **Step 2: Implement binary-binary linearization**

For `z = x*y`:

```text
z <= x
z <= y
z >= x + y - 1
z >= 0
```

- [ ] **Step 3: Implement binary-times-bounded-function bridge**

Use exact four-inequality formulation from finite lower/upper bounds. Record bound trace.

- [ ] **Step 4: Add exhaustive/random tests**

Enumerate binary values and random bounded scalar values fixed through overlays. Compare direct product.

- [ ] **Step 5: Run and commit**

```bash
cargo test -p roml --test common_constructs product
cargo test -p roml-highs --test formulation_equivalence product
git add src/construct/product.rs src/compiler/bridge/product.rs src/model/mod.rs \
  tests/common_constructs.rs roml-highs/tests/formulation_equivalence.rs
git commit -m "feat(model): add exact binary product constructs"
```

Finish P32 evidence/review.

---

## Task 25: Implement piecewise-linear semantic validation and classification

**Phase:** P33

**Files:**
- Create: `src/construct/piecewise_linear.rs`
- Create: `tests/piecewise_linear.rs`
- Create: `docs/release/evidence/M3_P33_PWL_BOUNDS.md`
- Modify: `src/model/mod.rs`

**Interfaces:**
- Produces:

```rust
pub struct PiecewisePoint { x: f64, y: f64 }
pub enum PwlRelation { Epigraph, Hypograph, ExactGraph }
pub enum Extrapolation { Reject, ExtendEndSegments, ConstantEnds }
pub enum Curvature { Convex, Concave, Affine, NonConvex }
impl Model { pub fn piecewise_linear(&mut self, input: Variable, points: impl IntoIterator<Item = PiecewisePoint>, spec: PwlSpec) -> Result<Variable, ModelError>; }
```

- [ ] **Step 1: Write point validation tests**

Reject NaN/infinite points, duplicate/out-of-order breakpoints, too few points, and domain/extrapolation contradictions.

- [ ] **Step 2: Implement curvature classification**

Compute segment slopes and classify nondecreasing as convex, nonincreasing as concave, all-equal as affine, otherwise nonconvex. Use a documented numeric comparison policy; exact input f64 values remain finite.

- [ ] **Step 3: Validate relation/curvature combinations**

Convex epigraph and concave hypograph qualify for no-binary row bridge. Exact graph remains exact regardless of curvature.

- [ ] **Step 4: Add direct evaluation helpers for tests**

Implement interpolation/extrapolation evaluation in test-support or public diagnostic API as appropriate.

- [ ] **Step 5: Run and commit**

```bash
cargo test -p roml --test piecewise_linear validation
cargo test -p roml --all-targets
git add src/construct/piecewise_linear.rs src/model/mod.rs tests/piecewise_linear.rs \
  docs/release/evidence/M3_P33_PWL_BOUNDS.md
git commit -m "feat(model): add piecewise linear semantics"
```

---

## Task 26: Implement PWL bridges and native/SOS2 selection

**Phase:** P33

**Files:**
- Create: `src/compiler/bridge/piecewise_linear.rs`
- Modify: `src/compiler/backend_ir.rs`
- Modify: `src/compiler/session.rs`
- Modify: `src/compiler/capability.rs`
- Create: `roml-highs/tests/formulation_equivalence.rs` if not already created
- Modify: `tests/piecewise_linear.rs`

- [ ] **Step 1: Implement convex epigraph/concave hypograph row bridges**

Use supporting segment inequalities. Assert compiled model contains zero generated binary variables.

- [ ] **Step 2: Implement exact SOS2 bridge**

Create convex-combination lambda variables, sum-to-one row, input/output interpolation rows, and normalized `BackendConstraint::Sos2` when supported. Every lambda/row receives construct role origins.

- [ ] **Step 3: Implement portable exact binary fallback**

Use segment-selection binaries and local interpolation with validated bounds. Keep formulation deterministic and documented.

- [ ] **Step 4: Add native PWL selection only after backend qualification**

If HiGHS official API has an exact native PWL primitive matching semantics, add translator/capability. Otherwise use SOS2/binary and report native unsupported.

- [ ] **Step 5: Add randomized direct-evaluation tests**

Fix input values at random points and solve for output; compare to direct interpolation. Test convex, concave, affine, and nonconvex exact curves.

- [ ] **Step 6: Add formulation report assertions**

Report curvature, relation, chosen representation, generated counts, and why binaries were introduced or avoided.

- [ ] **Step 7: Run and commit**

```bash
cargo test -p roml --test piecewise_linear
cargo test -p roml-highs --test formulation_equivalence pwl
cargo test -p roml --all-targets
cargo test -p roml-highs --all-targets
git add src/compiler/bridge/piecewise_linear.rs src/compiler/backend_ir.rs \
  src/compiler/session.rs src/compiler/capability.rs tests/piecewise_linear.rs \
  roml-highs/tests/formulation_equivalence.rs docs/release/evidence/M3_P33_PWL_BOUNDS.md
git commit -m "feat(compiler): add exact piecewise linear formulations"
```

Finish P33 independent OR formulation review.

---

## Task 27: Integrate construct-aware IIS and reporting

**Phase:** P29/P34 integration

**Files:**
- Modify: `src/solver/infeasibility.rs`
- Modify: `src/compiler/origin.rs`
- Modify: `roml-highs/tests/iis_reports.rs`
- Modify: `docs/release/evidence/M3_P29_IIS_CONFLICTS.md`

- [ ] **Step 1: Add bridged indicator conflict fixture**

Construct an infeasible model where one generated bridge row participates. The report must identify the original indicator and generated role, not expose only compiled row IDs.

- [ ] **Step 2: Add soft-constraint/fixing provenance fixtures**

Hard constraints in conflict should map normally; active soft violation variables should prevent infeasibility unless bounded too tightly, in which case report the soft construct and violation bound origin.

- [ ] **Step 3: Add PWL construct conflict fixture**

Conflict from exact graph/domain must map to PWL construct plus relevant user variable bounds.

- [ ] **Step 4: Run and commit**

```bash
cargo test -p roml-highs --test iis_reports -- --nocapture
git add src/solver/infeasibility.rs src/compiler/origin.rs \
  roml-highs/tests/iis_reports.rs docs/release/evidence/M3_P29_IIS_CONFLICTS.md
git commit -m "test(iis): map bridged conflicts to semantic constructs"
```

---

## Task 28: Complete public API, documentation, migration, and examples

**Phase:** P34

**Files:**
- Modify: `README.md`
- Modify: `MODELING_API.md`
- Modify: `MIGRATION.md`
- Modify: `CHANGELOG.md`
- Create: `docs/SEMANTIC_MODELING.md`
- Create: `docs/FORMULATION_COMPILER.md`
- Create: `docs/INFEASIBILITY_DIAGNOSTICS.md`
- Create: `docs/SOLUTION_REUSE.md`
- Create: `docs/MULTIOBJECTIVE.md`
- Create: `docs/NLP_EXTENSION_BOUNDARY.md`
- Create: `roml-highs/examples/solution_lock.rs`
- Create: `roml-highs/examples/iis_report.rs`
- Create: `roml-highs/examples/soft_constraint.rs`
- Create: `roml-highs/examples/lexicographic.rs`
- Create: `roml-highs/examples/modeling_constructs.rs`
- Create: `roml-highs/examples/piecewise_linear.rs`

- [ ] **Step 1: Write compiled examples first**

Each example must be included in an integration test or doctest and use only public golden-path imports.

- [ ] **Step 2: Update README with one bounded M3 section**

Keep the M2 quickstart first. Add semantic workflows after the basic solve path; do not overwhelm the initial user.

- [ ] **Step 3: Document semantic/native/bridge distinction**

Every construct table states semantic guarantee, portable bridge, native backend support, version limitations, and failure behavior.

- [ ] **Step 4: Document backend-author migration**

Explain canonical versus backend IR, typed capabilities, origins, compiled deltas, and optional IIS/start/multiobjective traits.

- [ ] **Step 5: Document NLP extension exercise**

`docs/NLP_EXTENSION_BOUNDARY.md` shows exact additive changes required for `ScalarFunction::Quadratic` and a future nonlinear graph without implementing them.

- [ ] **Step 6: Run docs/examples**

```bash
cargo test -p roml-highs --examples
cargo test -p roml --doc
cargo test -p roml-highs --doc
RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps
RUSTDOCFLAGS='-D warnings' cargo doc -p roml-highs --no-deps
```

- [ ] **Step 7: Commit**

```bash
git add README.md MODELING_API.md MIGRATION.md CHANGELOG.md docs \
  roml-highs/examples roml-highs/tests
git commit -m "docs(m3): document semantic modeling workflows"
```

---

## Task 29: Add qualification corpus and performance regression gates

**Phase:** P34

**Files:**
- Create: `tests/m3_property.rs`
- Create: `roml-highs/tests/m3_native_portable.rs`
- Create: `benches/m3_orchestration.rs` or existing benchmark location
- Create: `docs/release/evidence/M3_QUALIFICATION.md`
- Create: `docs/release/evidence/M3_PUBLIC_API.md`
- Create: `docs/release/evidence/M3_PERFORMANCE.md`
- Create: `docs/release/evidence/M3_NLP_READINESS.md`
- Modify: CI workflows as needed

- [ ] **Step 1: Build deterministic native/portable corpus**

Include indicators, exact max/min, abs, Boolean/cardinality, products, soft constraints, lexicographic objectives, and PWL. Compare status, objective values, user variable values where unique, and semantic constraint satisfaction.

- [ ] **Step 2: Add randomized property suites**

Use fixed seeds recorded in evidence. Test:

- fix/unfix state machine;
- interval bounds contain sampled values;
- bridge output equals direct semantics;
- origin maps are complete;
- compiled deltas equal rebuild;
- overlays never leak.

- [ ] **Step 3: Measure ordinary primitive overhead**

Benchmark untouched M2-style parameter update solve and M3 identity-compiler path. Gate median added overhead below 5% or 50 microseconds per solve attempt, whichever is larger. If exceeded, profile and optimize identity compilation/cache before acceptance.

- [ ] **Step 4: Measure compile/report workloads**

Record, but do not set unsupported marketing claims for, compile time and generated model size by construct count.

- [ ] **Step 5: Run full matrix**

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps
cargo deny check
cargo machete
cargo audit
cargo public-api -p roml
cargo public-api -p roml-highs
cargo package --list -p roml
cargo package --list -p roml-highs
cargo package -p roml --locked
cargo package -p roml-highs --locked
```

Record unsupported commercial-adapter failures separately; do not weaken core/HiGHS gates.

- [ ] **Step 6: Test fresh packed consumers**

Create temporary projects that consume packed `roml`/`roml-highs` artifacts and compile/run each golden workflow.

- [ ] **Step 7: Run independent reviews**

Required:

- principal engineering/architecture review;
- OR formulation correctness review;
- unsafe/native API review;
- NLP-readiness extension review.

Resolve all P0/P1 findings.

- [ ] **Step 8: Commit qualification evidence**

```bash
git add tests roml-highs/tests benches .github/workflows docs/release/evidence
git commit -m "test(m3): qualify semantic modeling milestone"
```

---

## Task 30: Final state, traceability, and integration gate

**Phase:** P34

**Files:**
- Modify: `.planning/milestones/M3-semantic-modeling-workflows/STATE.md`
- Modify: `.planning/milestones/M3-semantic-modeling-workflows/TRACEABILITY.md`
- Modify: `.planning/milestones/M3-semantic-modeling-workflows/README.md`
- Modify: root planning state/roadmap only through a reviewed integration amendment

- [ ] **Step 1: Close every requirement with evidence links**

No requirement remains `Planned` or `In progress`. Any deferred item must be outside M3 scope and explicitly justified; mandatory acceptance criteria cannot be deferred while marking the milestone complete.

- [ ] **Step 2: Record final merge/CI facts**

List each phase PR, merge commit, CI result, evidence file, reviewer, and residual risk.

- [ ] **Step 3: Run placeholder/consistency scan**

```bash
rg -n "TBD|TODO|FIXME|implement later|fill in" \
  .planning/milestones/M3-semantic-modeling-workflows \
  docs/superpowers/specs/2026-08-02-semantic-modeling-and-solve-workflows-design.md \
  docs/superpowers/plans/2026-08-02-semantic-modeling-and-solve-workflows.md
```

Every match must be removed or be a quoted prohibition rather than an unresolved requirement.

- [ ] **Step 4: Verify exact final SHA**

Re-run mandatory P34 gates on the final integrated head. Evidence must reference that SHA.

- [ ] **Step 5: Commit state closure**

```bash
git add .planning/milestones/M3-semantic-modeling-workflows docs/release/evidence
git commit -m "docs(m3): close semantic modeling milestone"
```

- [ ] **Step 6: Stop before publication**

Do not tag, publish, or create a release. Report the verified state and request a separate owner decision for any publication activity.

---

## Plan self-review checklist

Before execution begins, reviewers must confirm:

- every SM-01 through SM-15 requirement maps to at least one task;
- type names/signatures used by later tasks match their defining task;
- P26 lands before concrete construct bridges;
- no task adds backend-specific state to canonical `Model`;
- no bridge uses an arbitrary Big-M;
- overlays have explicit rollback/rebuild semantics;
- IIS guarantees are version/scope aware;
- soft penalties handle objective sense correctly;
- lexicographic degradation formulas cover negative objectives;
- exact and one-sided constructs remain distinct;
- PWL convex one-sided formulations have a zero-binary test;
- NLP readiness is an extension exercise, not unimplemented NLP code;
- P34 includes public API, package, fresh-consumer, performance, and independent review evidence.

## Execution handoff

Recommended execution is **subagent-driven development** with one fresh implementation agent per task or tightly coupled task group and two-stage review after each phase. Use isolated worktrees and do not exceed the WIP limits in the M3 execution protocol.
