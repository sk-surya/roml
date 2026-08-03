# Semantic Modeling and Solve Workflows Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a canonical semantic modeling IR, capability-aware backend compiler, solution-reuse workflows, infeasibility diagnostics, soft constraints, lexicographic objectives, and common exact MILP constructs while preserving ROML's incremental/recoverable solver guarantees and leaving an additive path for future nonlinear programming.

**Architecture:** Canonical `Model` state stores functions, sets, constructs, objective policy, declared domains, fixings, metadata, and revisions. A per-solver `CompilationSession` lowers canonical snapshots and deltas into backend IR with typed capabilities, exact bridges, generated-entity origins, and formulation reports. `SolverSession` applies solve-scoped overlays, starts, hints, and objective stages transactionally before mapping backend results back to user entities.

**Tech Stack:** Rust 1.85, existing ROML arenas/revisions/journal/snapshots, `roml-highs` through pinned `highs-sys`, property and differential tests, GitHub Actions, cargo fmt/clippy/test/doc/public-api/package.

## Global Constraints

- Preserve M2 ordinary `Model`, `LinExpr`, `Highs::solve`, `solve_with`, and `Solution` source usage unless a reviewed executable contradiction is documented.
- Core `roml` remains solver-free and contains no native HiGHS dependency.
- Canonical semantic state contains no backend indices, native handles, selected Big-M values, or solve overlays.
- High-level constructs remain canonical entities; bridge expansion occurs only in the compiler.
- Exact semantics are never silently compiled as relaxations.
- Big-M requires finite proof or explicit validated user input; no default Big-M constant is permitted.
- Every generated variable and constraint has an `EntityOrigin`.
- Unsupported features are applied, bridged exactly, adjusted explicitly, or rejected; never ignored.
- Overlay rollback uncertainty marks the session `RequiresRebuild`.
- Primitive compiled delta execution remains observationally equivalent to compiled rebuild.
- M3 implements linear scalar functions only; nonlinear expression tracing and differentiation are excluded.
- MSRV remains Rust 1.85.
- No publication, tag, or release is part of this plan.

---

## Cross-task interface contract

These names and fields are authoritative for the plan. A phase may change them only through an approved M3 decision amendment before dependent work starts.

### Identity and metadata

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ModelLineageId(u64);

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModelSource {
    pub module: Option<String>,
    pub file: Option<String>,
    pub line: Option<u32>,
    pub external_key: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EntityMetadata {
    pub description: Option<String>,
    pub group: Option<String>,
    pub tags: Vec<String>,
    pub source: Option<ModelSource>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EntityRef {
    Variable(Variable),
    Constraint(Constraint),
    Objective(Objective),
    Parameter(Parameter),
    Construct(Construct),
}
```

### Function-in-set model

```rust
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum ScalarFunction {
    Linear(LinExpr),
}

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum ScalarSet {
    LessEqual(ValueExpr),
    GreaterEqual(ValueExpr),
    EqualTo(ValueExpr),
    Interval { lower: ValueExpr, upper: ValueExpr },
}

#[derive(Clone, Debug, PartialEq)]
pub struct FunctionConstraint {
    pub function: ScalarFunction,
    pub set: ScalarSet,
}

pub trait IntoScalarFunction {
    fn into_scalar_function(self) -> ScalarFunction;
}
```

### Canonical constructs

```rust
pub type Construct = ConstructId;

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum ConstructKind {
    Indicator(IndicatorConstraint),
    Reification(ReificationConstraint),
    MinMax(MinMaxConstraint),
    AbsoluteValue(AbsoluteValueConstraint),
    Boolean(BooleanConstraint),
    Cardinality(CardinalityConstraint),
    BinaryProduct(BinaryProductConstraint),
    PiecewiseLinear(PiecewiseLinearConstraint),
    SoftConstraint(SoftConstraintDefinition),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConstructEntry {
    pub id: Construct,
    pub kind: ConstructKind,
    pub active: bool,
    pub dependencies: Vec<Parameter>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormulationPreference {
    Auto,
    Portable,
    NativeRequired,
}
```

### Domains and fixing

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VariableDomain {
    pub bounds: Bounds,
    pub var_type: VarType,
    pub semi: Option<SemiDomain>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SemiDomain {
    Continuous { nonzero_lower: f64 },
    Integer { nonzero_lower: f64 },
}

#[derive(Clone, Debug, PartialEq)]
pub struct VariableFixing {
    pub value: f64,
    pub provenance: FixingProvenance,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FixingProvenance {
    User,
    Imported { source: String },
}
```

### Assignments and solve intent

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct PrimalAssignment {
    lineage: ModelLineageId,
    source_revision: Option<ModelRevision>,
    values: std::collections::BTreeMap<Variable, f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MipStart {
    pub assignment: PrimalAssignment,
    pub repair: RepairPolicy,
    pub name: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepairPolicy {
    BackendDefault,
    RejectIncomplete,
    AllowRepair,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct VariableHints {
    entries: std::collections::BTreeMap<Variable, VariableHint>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VariableHint {
    pub value: f64,
    pub priority: HintPriority,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct HintPriority(pub i32);

#[derive(Clone, Debug, PartialEq)]
pub struct SolutionLock {
    pub assignment: PrimalAssignment,
    pub selector: LockSelector,
    pub continuous: ContinuousLock,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LockSelector {
    AllAssigned,
    IntegerAssigned,
    BinaryAssigned,
    Variables(std::collections::BTreeSet<Variable>),
    Except(std::collections::BTreeSet<Variable>),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ContinuousLock {
    Exact,
    Within { absolute: f64 },
}
```

### Objective policy

```rust
#[derive(Clone, Debug, PartialEq)]
pub enum ObjectivePolicy {
    Single(Objective),
    Weighted(Vec<WeightedObjective>),
    Lexicographic(Vec<ObjectiveLevel>),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeightedObjective {
    pub objective: Objective,
    pub weight: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ObjectiveLevel {
    pub objective: Objective,
    pub absolute_tolerance: f64,
    pub relative_tolerance: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LexStagePolicy {
    RequireOptimal,
    UseBestFeasible,
}
```

### Backend IR and origins

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CompiledVariableId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CompiledConstraintId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CompiledObjectiveId(pub u32);

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledVariable {
    pub id: CompiledVariableId,
    pub bounds: Bounds,
    pub var_type: VarType,
    pub name: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledLinearRow {
    pub id: CompiledConstraintId,
    pub bounds: ConstraintBounds,
    pub coefficients: Vec<(CompiledVariableId, f64)>,
    pub name: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledObjective {
    pub id: CompiledObjectiveId,
    pub sense: Sense,
    pub coefficients: Vec<(CompiledVariableId, f64)>,
    pub constant: f64,
    pub name: Option<String>,
}

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum BackendConstraint {
    Indicator(CompiledIndicator),
    Sos1(CompiledSos),
    Sos2(CompiledSos),
    PiecewiseLinear(CompiledPiecewiseLinear),
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledIndicator {
    pub binary: CompiledVariableId,
    pub active_value: i32,
    pub row: CompiledLinearRow,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledSos {
    pub variables: Vec<CompiledVariableId>,
    pub weights: Vec<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledPiecewiseLinear {
    pub input: CompiledVariableId,
    pub output: CompiledVariableId,
    pub points: Vec<PiecewisePoint>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BackendSnapshot {
    pub revision: ModelRevision,
    pub variables: Vec<CompiledVariable>,
    pub linear_rows: Vec<CompiledLinearRow>,
    pub native_constraints: Vec<BackendConstraint>,
    pub objectives: Vec<CompiledObjective>,
    pub origin_map: OriginMap,
    pub report: CompilationReport,
    pub fingerprint: CompilationFingerprint,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BackendDeltaBatch {
    pub from: ModelRevision,
    pub to: ModelRevision,
    pub operations: Vec<BackendOp>,
    pub fingerprint: CompilationFingerprint,
}

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum BackendOp {
    AddVariable(CompiledVariable),
    RemoveVariable(CompiledVariableId),
    SetVariableBounds { variable: CompiledVariableId, bounds: Bounds },
    AddLinearRow(CompiledLinearRow),
    RemoveLinearRow(CompiledConstraintId),
    SetLinearRowBounds { constraint: CompiledConstraintId, bounds: ConstraintBounds },
    SetLinearCoefficient { constraint: CompiledConstraintId, variable: CompiledVariableId, value: f64 },
    AddObjective(CompiledObjective),
    RemoveObjective(CompiledObjectiveId),
    SetObjectiveCoefficient { objective: CompiledObjectiveId, variable: CompiledVariableId, value: f64 },
    SetObjectiveConstant { objective: CompiledObjectiveId, value: f64 },
    SetObjectiveSense { objective: CompiledObjectiveId, sense: Sense },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EntityOrigin {
    UserVariable(Variable),
    UserConstraint(Constraint),
    UserObjective(Objective),
    Construct { construct: Construct, role: GeneratedRole },
    SolveOverlay { overlay: OverlayId, role: GeneratedRole },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeneratedRole {
    AuxiliaryVariable { label: String },
    BridgeRow { label: String },
    ObjectiveLock { level: usize },
    LowerViolation,
    UpperViolation,
    PositiveCorrection,
    NegativeCorrection,
    SegmentSelector { segment: usize },
    ConvexWeight { point: usize },
}
```

### Capabilities and compilation

```rust
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BackendFeature {
    Lp,
    Mip,
    IncrementalBounds,
    IncrementalRows,
    IncrementalCoefficients,
    MipStart,
    PartialMipStart,
    MultipleMipStarts,
    VariableHints,
    InitialBasis,
    Iis,
    FeasibilityRelaxation,
    Indicator,
    Sos1,
    Sos2,
    NativePiecewiseLinear,
    NativeMultiObjective,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SupportLevel {
    Native,
    Unsupported,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FeatureLimitations {
    pub minimum_version: Option<String>,
    pub model_classes: Vec<String>,
    pub maximum_count: Option<usize>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeatureSupport {
    pub level: SupportLevel,
    pub limitations: FeatureLimitations,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompilationPolicy {
    Auto,
    Portable,
    NativeRequired,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CompilationFingerprint(pub u64);
```

### Solve plan and effective result

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct SolvePlan {
    pub options: SolveOptions,
    pub overlay: SolveOverlay,
    pub mip_starts: Vec<MipStart>,
    pub hints: VariableHints,
    pub objective_override: Option<ObjectivePolicy>,
    pub lex_stage_policy: LexStagePolicy,
    pub unsupported: UnsupportedFeaturePolicy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnsupportedFeaturePolicy {
    Reject,
    IgnoreExplicitly,
    Convert(ConversionPolicy),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConversionPolicy {
    HintsToMipStart,
    MipStartToTemporaryFixing,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct EffectiveSolvePlan {
    pub applied_features: Vec<AppliedFeature>,
    pub adjustments: Vec<PlanAdjustment>,
    pub rejections: Vec<PlanRejection>,
    pub objective_stages: Vec<ObjectiveStageResult>,
}
```

### Infeasibility analysis

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct InfeasibilityReport {
    pub lineage: ModelLineageId,
    pub revision: ModelRevision,
    pub backend: BackendIdentity,
    pub kind: InfeasibilityKind,
    pub scope: AnalysisScope,
    pub minimality: MinimalityClaim,
    pub completion: CompletionStatus,
    pub members: Vec<ConflictMember>,
    pub statistics: AnalysisStatistics,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ConflictMember {
    ConstraintSide { constraint: Constraint, side: BoundSide, participation: Participation },
    VariableBound { variable: Variable, side: BoundSide, provenance: BoundProvenance, participation: Participation },
    Construct { construct: Construct, role: GeneratedRole, participation: Participation },
}
```

---

## Planned file structure

```text
src/identity.rs
src/metadata.rs
src/function/mod.rs
src/function/scalar.rs
src/function/set.rs
src/construct/mod.rs
src/construct/indicator.rs
src/construct/minmax.rs
src/construct/absolute.rs
src/construct/boolean.rs
src/construct/product.rs
src/construct/piecewise_linear.rs
src/construct/soft.rs
src/assignment.rs
src/objective_policy.rs
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
src/solver/plan.rs
src/solver/overlay.rs
src/solver/effective_plan.rs
src/solver/infeasibility.rs
src/solver/feasibility_relaxation.rs
src/solver/multiobjective.rs
roml-highs/src/compiler.rs
roml-highs/src/start.rs
roml-highs/src/iis.rs
roml-highs/src/relaxation.rs
roml-highs/src/multiobjective.rs
```

---

## Task 1: Capture M3 baseline and characterization suite

**Phase:** P25

**Files:**
- Create: `docs/release/evidence/M3_P25_SEMANTIC_IR.md`
- Create: `tests/m3_baseline_characterization.rs`
- Create: `docs/release/evidence/m3-baseline/roml.txt`
- Create: `docs/release/evidence/m3-baseline/roml-highs.txt`
- Create: `docs/release/evidence/m3-baseline/roml-package.txt`
- Create: `docs/release/evidence/m3-baseline/roml-highs-package.txt`

**Produces:** untouched command, public-API, package, and behavioral baselines.

- [ ] **Step 1: Record exact environment**

```bash
git rev-parse HEAD
rustc --version --verbose
cargo --version
```

Copy exact output into `M3_P25_SEMANTIC_IR.md`.

- [ ] **Step 2: Run untouched gates**

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

Record every result. Do not repair baseline defects in this task.

- [ ] **Step 3: Capture public/package inventories**

```bash
mkdir -p docs/release/evidence/m3-baseline
cargo public-api -p roml > docs/release/evidence/m3-baseline/roml.txt
cargo public-api -p roml-highs > docs/release/evidence/m3-baseline/roml-highs.txt
cargo package --list -p roml > docs/release/evidence/m3-baseline/roml-package.txt
cargo package --list -p roml-highs > docs/release/evidence/m3-baseline/roml-highs-package.txt
```

- [ ] **Step 4: Write M2 compatibility tests**

`tests/m3_baseline_characterization.rs` must compile the current fluent model path, deterministic snapshot, parameter update, objective constant, solution metadata, and one-rebuild-retry behavior.

- [ ] **Step 5: Run and commit**

```bash
cargo test -p roml --test m3_baseline_characterization -- --nocapture
git add docs/release/evidence/M3_P25_SEMANTIC_IR.md \
  docs/release/evidence/m3-baseline tests/m3_baseline_characterization.rs
git commit -m "test(m3): capture semantic modeling baseline"
```

---

## Task 2: Add lineage and metadata

**Phase:** P25

**Files:**
- Create: `src/identity.rs`
- Create: `src/metadata.rs`
- Create: `tests/lineage_metadata.rs`
- Modify: `src/lib.rs`
- Modify: `src/model/mod.rs`
- Modify: `src/solution/metadata.rs`

**Produces:** the identity and metadata contract defined above.

- [ ] **Step 1: Write failing lineage tests**

Test that independent models differ, clones preserve lineage, and solution metadata records lineage.

- [ ] **Step 2: Implement allocation**

Use `AtomicU64` with zero reserved. `Model::default()` allocates; `Clone` copies the value. Counter exhaustion panics only after `u64::MAX - 1` allocations and is documented as process-fatal exhaustion.

- [ ] **Step 3: Write failing metadata tests**

Test round-trip metadata for variable, constraint, objective, and parameter; stale entity keys fail atomically.

- [ ] **Step 4: Implement metadata store**

Store `HashMap<EntityRef, EntityMetadata>` in `Model`. Metadata changes are canonical diagnostic changes with `affects_solver() == false`.

- [ ] **Step 5: Export curated types**

Export `ModelLineageId`, `EntityMetadata`, and `ModelSource` from the crate root and prelude; keep `EntityRef` under `advanced` until public API review proves ordinary need.

- [ ] **Step 6: Verify and commit**

```bash
cargo test -p roml --test lineage_metadata
cargo test -p roml --all-targets
git add src/identity.rs src/metadata.rs src/lib.rs src/model/mod.rs \
  src/solution/metadata.rs tests/lineage_metadata.rs
git commit -m "feat(model): add lineage and entity metadata"
```

---

## Task 3: Add function-in-set canonical constraints

**Phase:** P25

**Files:**
- Create: `src/function/mod.rs`
- Create: `src/function/scalar.rs`
- Create: `src/function/set.rs`
- Create: `tests/semantic_ir.rs`
- Modify: `src/lib.rs`
- Modify: `src/model/constraint.rs`
- Modify: `src/model/mod.rs`
- Modify: `src/snapshot.rs`
- Modify: `src/delta.rs`

**Produces:** `ScalarFunction`, `ScalarSet`, `FunctionConstraint`, and `IntoScalarFunction`.

- [ ] **Step 1: Write failing conversion tests**

Assert existing `.le`, `.ge`, `.eq`, and `.between` builders reconstruct the expected function/set values.

- [ ] **Step 2: Implement types and conversions**

Implement `IntoScalarFunction` for `LinExpr` and `Variable`. Existing expression operators remain unchanged.

- [ ] **Step 3: Preserve one coefficient authority**

Keep the canonical coefficient index authoritative in P25. `Model::function_constraint` reconstructs `ScalarFunction::Linear` deterministically and reads set values from constraint bounds.

- [ ] **Step 4: Extend snapshots and deltas**

Canonical snapshot constraint entries carry `FunctionConstraint`. During P25 transition, legacy bounds/cells remain and invariant checks prove equality.

- [ ] **Step 5: Verify compatibility**

```bash
cargo test -p roml --test semantic_ir
cargo test -p roml --test m3_baseline_characterization
cargo test -p roml --all-targets
```

- [ ] **Step 6: Commit**

```bash
git add src/function src/lib.rs src/model/constraint.rs src/model/mod.rs \
  src/snapshot.rs src/delta.rs tests/semantic_ir.rs
git commit -m "feat(model): add linear function-in-set semantics"
```

---

## Task 4: Add canonical construct lifecycle

**Phase:** P25

**Files:**
- Create: `src/construct/mod.rs`
- Modify: `src/lib.rs`
- Modify: `src/metadata.rs`
- Modify: `src/model/mod.rs`
- Modify: `src/snapshot.rs`
- Modify: `src/delta.rs`
- Modify: `tests/semantic_ir.rs`

**Produces:** generation-safe `ConstructId`, `ConstructKind`, `ConstructEntry`, model lifecycle methods, snapshot/delta entries.

- [ ] **Step 1: Write lifecycle tests**

Use a private test-only construct payload to test add, clone, snapshot, deactivate, reactivate, remove, and stale generation behavior.

- [ ] **Step 2: Implement one construct store**

Use the existing arena pattern. Do not add one map per construct family.

- [ ] **Step 3: Add canonical operations**

Add `ConstructAdded`, `ConstructRemoved`, `ConstructActivityChanged`, and `ConstructUpdated` with self-contained payloads.

- [ ] **Step 4: Extend invariants**

Validate live references, parameter dependencies, auxiliary ownership acyclicity, and metadata key validity.

- [ ] **Step 5: Run P25 gate**

```bash
cargo test -p roml --all-targets
cargo clippy -p roml --all-targets -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps
cargo public-api -p roml
```

- [ ] **Step 6: Commit and request P25 review**

```bash
git add src/construct src/lib.rs src/metadata.rs src/model/mod.rs \
  src/snapshot.rs src/delta.rs tests/semantic_ir.rs \
  docs/release/evidence/M3_P25_SEMANTIC_IR.md
git commit -m "feat(model): add semantic construct lifecycle"
```

---

## Task 5: Define backend IR and origin completeness

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

**Produces:** compiled IDs, backend snapshot/delta/ops, origins, compilation fingerprint, and report types from the contract.

- [ ] **Step 1: Write failing builder-finalization tests**

Backend snapshot finalization fails when any compiled variable, row, objective, or native constraint lacks an origin.

- [ ] **Step 2: Implement deterministic IDs**

Allocate dense IDs after sorting canonical user entities by stable handles and generated entities by `(construct, role)`.

- [ ] **Step 3: Implement bidirectional origin queries**

Support compiled-to-origin and construct-to-generated-entity lookups.

- [ ] **Step 4: Implement structured reports**

Each construct report records representation, generated counts, bound/Big-M evidence, and notes. Reports contain no backend-specific free-form requirement.

- [ ] **Step 5: Verify and commit**

```bash
cargo test -p roml --test compiler_identity
git add src/compiler src/lib.rs src/advanced.rs tests/compiler_identity.rs \
  docs/release/evidence/M3_P26_COMPILER_BACKEND_IR.md
git commit -m "feat(compiler): define backend IR and origins"
```

---

## Task 6: Implement typed capabilities

**Phase:** P26

**Files:**
- Create: `src/compiler/capability.rs`
- Modify: `src/solver/backend.rs`
- Modify: `src/solver/request.rs`
- Modify: `src/solver/conformance.rs`
- Modify: `roml-highs/src/session.rs`
- Modify: `tests/backend_conformance.rs`

**Produces:** typed `BackendFeature`, `FeatureSupport`, and `FeatureLimitations`.

- [ ] **Step 1: Characterize legacy mapping**

Write tests mapping every current Boolean capability to typed features before replacing callers.

- [ ] **Step 2: Implement `BackendCapabilitySet`**

Use `BTreeMap<BackendFeature, FeatureSupport>` and helpers `support(feature)` and `supports_native(feature)`.

- [ ] **Step 3: Migrate validation**

Solve-request validation queries typed features. Preserve existing rejection semantics.

- [ ] **Step 4: Add version-aware HiGHS set**

Populate current LP/MIP/incremental/solution features from actual backend version. New M3 features remain unsupported until qualified phases.

- [ ] **Step 5: Remove flat public usage**

Keep a private conversion only until P26 tests pass, then remove it before merge.

- [ ] **Step 6: Verify and commit**

```bash
cargo test -p roml solver::conformance
cargo test -p roml-highs --all-targets
git add src/compiler/capability.rs src/solver/backend.rs src/solver/request.rs \
  src/solver/conformance.rs roml-highs/src/session.rs tests/backend_conformance.rs
git commit -m "feat(backend): add typed feature capabilities"
```

---

## Task 7: Add identity compiler and migrate synchronization

**Phase:** P26

**Files:**
- Create: `src/compiler/session.rs`
- Create: `roml-highs/src/compiler.rs`
- Create: `docs/migration/M3_BACKEND_IR.md`
- Modify: `src/solver/session.rs`
- Modify: `src/solver/facade.rs`
- Modify: `src/solver/reference.rs`
- Modify: `src/solver/conformance.rs`
- Modify: `roml-highs/src/session.rs`
- Modify: `src/advanced.rs`
- Modify: `tests/differential_harness.rs`

**Produces:**

```rust
pub enum BackendSynchronization {
    Delta(BackendDeltaBatch),
    Rebuild(BackendSnapshot),
}

pub trait BackendSession {
    fn synchronize(&mut self, sync: BackendSynchronization) -> Result<SyncReceipt, BackendError>;
    fn solve(&mut self, request: &SolveRequest) -> Result<SolveResult, BackendError>;
    fn close(self) -> Result<(), BackendError>;
}
```

- [ ] **Step 1: Implement identity snapshot compilation**

Primitive linear models compile one-to-one with deterministic origins and `IdentityLinear` representation.

- [ ] **Step 2: Implement conservative delta compilation**

Translate known primitive operations. Return rebuild decision when compiled mapping or recipe certainty is absent.

- [ ] **Step 3: Migrate ReferenceBackend**

Run all core recovery and differential tests before touching HiGHS.

- [ ] **Step 4: Migrate HiGHS**

Move native translation behind `roml-highs/src/compiler.rs`. After migration, HiGHS receives no `ModelSnapshot`.

- [ ] **Step 5: Preserve retry invariant**

Compile failure occurs before backend mutation. Backend application still permits at most one automatic rebuild retry.

- [ ] **Step 6: Add randomized compiled-delta/rebuild equality**

Reuse fixed-seed mutation sequences from `tests/differential_harness.rs`.

- [ ] **Step 7: Run P26 gate and commit**

```bash
cargo test -p roml --all-targets
cargo test -p roml-highs --all-targets
cargo clippy -p roml --all-targets -- -D warnings
cargo clippy -p roml-highs --all-targets -- -D warnings
git add src/compiler/session.rs src/solver/session.rs src/solver/facade.rs \
  src/solver/reference.rs src/solver/conformance.rs src/advanced.rs \
  roml-highs/src/compiler.rs roml-highs/src/session.rs tests/differential_harness.rs \
  docs/migration/M3_BACKEND_IR.md docs/release/evidence/M3_P26_COMPILER_BACKEND_IR.md
git commit -m "feat(sync): compile canonical state into backend IR"
```

Request independent P26 review before P27 or P32.

---

## Task 8: Unify domains and add persistent fixing

**Phase:** P27

**Files:**
- Create: `tests/fixing_assignment.rs`
- Create: `docs/release/evidence/M3_P27_FIXING_LOCKS_OVERLAYS.md`
- Modify: `src/model/variable.rs`
- Modify: `src/model/mod.rs`
- Modify: `src/snapshot.rs`
- Modify: `src/delta.rs`
- Modify: `src/compiler/session.rs`

**Produces:** `VariableDomain`, `SemiDomain`, `VariableFixing`, and model fixing methods.

- [ ] **Step 1: Write reference state-machine tests**

Cover continuous, integer, binary, semi-continuous, semi-integer, bound changes while fixed, unfix restoration, stale handles, and atomic validation failure.

- [ ] **Step 2: Refactor `VariableStore` record**

Store domain, fixing, active flag, and name in one record. Remove the separate model-level semi-continuous map after all callers migrate.

- [ ] **Step 3: Implement fixing validation**

Use a named integrality tolerance in model constants. Normalize accepted integer/binary values.

- [ ] **Step 4: Add canonical fixing operation**

`SetVariableFixing { var, fixing: Option<VariableFixing> }` is solver-affecting. Compiler emits an effective bound update.

- [ ] **Step 5: Verify incremental and rebuild paths**

```bash
cargo test -p roml --test fixing_assignment
cargo test -p roml --all-targets
cargo test -p roml-highs --all-targets
```

- [ ] **Step 6: Commit**

```bash
git add src/model/variable.rs src/model/mod.rs src/snapshot.rs src/delta.rs \
  src/compiler/session.rs tests/fixing_assignment.rs \
  docs/release/evidence/M3_P27_FIXING_LOCKS_OVERLAYS.md
git commit -m "feat(model): add first-class variable fixing"
```

---

## Task 9: Add assignments, locks, and reversible overlays

**Phase:** P27

**Files:**
- Create: `src/assignment.rs`
- Create: `src/solver/overlay.rs`
- Create: `src/solver/effective_plan.rs`
- Create: `tests/solve_overlay.rs`
- Modify: `src/lib.rs`
- Modify: `src/solution/mod.rs`
- Modify: `src/solution/metadata.rs`
- Modify: `src/solver/session.rs`
- Modify: `src/solver/backend.rs`
- Modify: `src/compiler/backend_ir.rs`
- Modify: `src/compiler/origin.rs`
- Modify: `roml-highs/src/session.rs`

**Produces:** assignment, solution lock, overlay, receipt, and optional overlay-session APIs.

- [ ] **Step 1: Write lineage/stale assignment tests**

Independent model assignments fail before backend mutation; clone-lineage assignments succeed when handles remain live.

- [ ] **Step 2: Implement solution extraction**

`Solution::primal_assignment()` includes canonical variables and excludes compiler-only variables.

- [ ] **Step 3: Write selector and lock-band tests**

Cover every `LockSelector` and both `ContinuousLock` modes.

- [ ] **Step 4: Define overlay operations**

```rust
#[derive(Clone, Debug, PartialEq)]
pub enum OverlayOperation {
    SetVariableBounds { variable: Variable, bounds: Bounds, origin: GeneratedRole },
    AddLinearRow { row: FunctionConstraint, origin: GeneratedRole },
    SetObjective { objective: Objective },
}
```

- [ ] **Step 5: Implement explicit rollback receipt**

`apply_overlay` returns reverse operations and a health token. `rollback_overlay` is fallible. Failure sets `RequiresRebuild`.

- [ ] **Step 6: Inject failures at validation, apply, solve, extraction, and rollback**

After each case, a clean subsequent solve must equal a fresh rebuilt session.

- [ ] **Step 7: Verify and commit**

```bash
cargo test -p roml --test solve_overlay -- --nocapture
cargo test -p roml --all-targets
cargo test -p roml-highs --all-targets
git add src/assignment.rs src/solver/overlay.rs src/solver/effective_plan.rs \
  src/solution src/solver/session.rs src/solver/backend.rs src/compiler \
  roml-highs/src/session.rs tests/solve_overlay.rs \
  docs/release/evidence/M3_P27_FIXING_LOCKS_OVERLAYS.md
git commit -m "feat(solve): add assignments locks and overlays"
```

Request P27 review.

---

## Task 10: Add SolvePlan, starts, and hints

**Phase:** P28

**Files:**
- Create: `src/solver/plan.rs`
- Create: `roml-highs/src/start.rs`
- Create: `roml-highs/tests/solve_plan.rs`
- Create: `docs/knowledge/highs-starts-hints.md`
- Create: `docs/release/evidence/M3_P28_SOLVE_PLAN_STARTS_HINTS.md`
- Modify: `src/assignment.rs`
- Modify: `src/solver/facade.rs`
- Modify: `src/solver/session.rs`
- Modify: `src/solution/metadata.rs`
- Modify: `src/compiler/capability.rs`
- Modify: `roml-highs/src/facade.rs`
- Modify: `roml-highs/src/session.rs`

**Produces:** exact `SolvePlan`, `MipStart`, `VariableHints`, and effective-plan contracts.

- [ ] **Step 1: Write compatibility tests**

`solve`, `solve_with`, and empty `solve_plan` produce equivalent deterministic results and effective options.

- [ ] **Step 2: Implement plan validation**

Validate lineage, finite values, conflicting overlays, duplicate starts, hint entries, and conversion policy before backend mutation.

- [ ] **Step 3: Refactor façades**

`solve()` and `solve_with()` delegate to one internal plan executor.

- [ ] **Step 4: Audit official HiGHS start/hint API**

Record symbols, signatures, partial-start semantics, persistence/clear lifecycle, multiple-start support, hint support, return codes, and version availability.

- [ ] **Step 5: Implement only qualified native support**

Absent hint support returns typed rejection under default policy. Conversions occur only under `ConversionPolicy` and are recorded.

- [ ] **Step 6: Prove starts/hints do not alter feasibility**

Compare canonical and compiled feasible-region fingerprints before and after plan execution.

- [ ] **Step 7: Run P28 gate and commit**

```bash
cargo test -p roml-highs --test solve_plan -- --nocapture
cargo test -p roml --all-targets
cargo test -p roml-highs --all-targets
git add src/assignment.rs src/solver/plan.rs src/solver/facade.rs \
  src/solver/session.rs src/solution/metadata.rs src/compiler/capability.rs \
  roml-highs/src/start.rs roml-highs/src/facade.rs roml-highs/src/session.rs \
  roml-highs/tests/solve_plan.rs docs/knowledge/highs-starts-hints.md \
  docs/release/evidence/M3_P28_SOLVE_PLAN_STARTS_HINTS.md
git commit -m "feat(solve): add solve plans starts and hints"
```

Request P28 review before P29 through P31.

---

## Task 11: Add normalized IIS/conflict analysis

**Phase:** P29

**Files:**
- Create: `src/solver/infeasibility.rs`
- Create: `src/report/mod.rs`
- Create: `src/report/infeasibility.rs`
- Create: `tests/infeasibility_report.rs`
- Create: `docs/release/evidence/M3_P29_IIS_CONFLICTS.md`
- Modify: `src/lib.rs`
- Modify: `src/compiler/origin.rs`
- Modify: `src/compiler/capability.rs`
- Modify: `src/solver/session.rs`

**Produces:** the exact report and conflict contracts defined above plus:

```rust
pub trait InfeasibilityAnalysisSession {
    fn analyze_infeasibility(
        &mut self,
        request: &InfeasibilityRequest,
    ) -> Result<BackendConflict, BackendError>;
}
```

- [ ] **Step 1: Write synthetic mapping tests**

Cover row sides, declared bounds, persistent fixings, temporary locks, and construct-generated rows.

- [ ] **Step 2: Implement guarantee-bearing types**

Construction requires kind, scope, minimality, and completion; no overclaiming defaults exist.

- [ ] **Step 3: Implement exact compilation-fingerprint mapping**

Reject stale origin maps with `AnalysisError::StaleCompilation`.

- [ ] **Step 4: Implement text and Markdown rendering**

Render stable ROML entity names, groups, sources, bound provenance, and generated roles.

- [ ] **Step 5: Verify and commit**

```bash
cargo test -p roml --test infeasibility_report
cargo test -p roml --all-targets
git add src/solver/infeasibility.rs src/report src/lib.rs src/compiler/origin.rs \
  src/compiler/capability.rs src/solver/session.rs tests/infeasibility_report.rs \
  docs/release/evidence/M3_P29_IIS_CONFLICTS.md
git commit -m "feat(analysis): add origin-aware infeasibility reports"
```

---

## Task 12: Implement qualified HiGHS IIS

**Phase:** P29

**Files:**
- Create: `roml-highs/src/iis.rs`
- Create: `roml-highs/tests/iis_reports.rs`
- Create: `docs/knowledge/highs-iis.md`
- Modify: `roml-highs/src/lib.rs`
- Modify: `roml-highs/src/session.rs`
- Modify: `roml-highs/src/facade.rs`
- Modify: `src/compiler/capability.rs`

- [ ] **Step 1: Audit bundled and minimum system official APIs**

Record symbols, structure fields, status codes, model-class scope, presolve/original scope, minimality guarantee, and version availability. Upgrade official bindings in a separate commit only when required.

- [ ] **Step 2: Add version capability tests**

Each supported configuration reports native IIS with exact limitations or typed unsupported.

- [ ] **Step 3: Implement native extraction**

Check every return code and produce compiled row/column-side members. User mapping remains in core.

- [ ] **Step 4: Add deterministic fixtures**

Rows, row/bound, fixing, and lock conflicts must render original names and provenance.

- [ ] **Step 5: Run and commit**

```bash
cargo test -p roml-highs --test iis_reports -- --nocapture
cargo test -p roml-highs --all-targets
git add roml-highs/src/iis.rs roml-highs/src/lib.rs roml-highs/src/session.rs \
  roml-highs/src/facade.rs roml-highs/tests/iis_reports.rs \
  src/compiler/capability.rs docs/knowledge/highs-iis.md \
  docs/release/evidence/M3_P29_IIS_CONFLICTS.md
git commit -m "feat(highs): add version-qualified IIS analysis"
```

---

## Task 13: Add soft constraints and feasibility relaxation

**Phase:** P30

**Files:**
- Create: `src/construct/soft.rs`
- Create: `src/compiler/bridge/mod.rs`
- Create: `src/compiler/bridge/soft.rs`
- Create: `src/solver/feasibility_relaxation.rs`
- Create: `roml-highs/src/relaxation.rs`
- Create: `roml-highs/tests/soft_constraints.rs`
- Create: `docs/knowledge/highs-feasibility-relaxation.md`
- Create: `docs/release/evidence/M3_P30_SOFT_CONSTRAINTS.md`
- Modify: `src/construct/mod.rs`
- Modify: `src/model/mod.rs`
- Modify: `src/solution/mod.rs`
- Modify: `src/compiler/session.rs`
- Modify: `roml-highs/src/lib.rs`
- Modify: `roml-highs/src/facade.rs`

**Produces:**

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViolationSide { Lower, Upper, Both, Auto }

#[derive(Clone, Debug, PartialEq)]
pub struct SoftConstraintSpec {
    pub side: ViolationSide,
    pub max_lower: Option<f64>,
    pub max_upper: Option<f64>,
    pub penalty_weight: Option<ValueExpr>,
    pub penalty_target: PenaltyTarget,
    pub name: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PenaltyTarget {
    Objective(Objective),
    LexicographicPriority(i32),
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SoftConstraintHandle {
    pub constraint: Constraint,
    pub lower_violation: Option<Variable>,
    pub upper_violation: Option<Variable>,
    pub construct: Construct,
}
```

- [ ] **Step 1: Write algebra tests**

Pin upper, lower, equality, ranged, signed-correction, and objective-sense formulas before compiler code.

- [ ] **Step 2: Implement stable auxiliary variables**

Create canonical variables with lower/upper/correction roles and metadata. Return handles.

- [ ] **Step 3: Implement exact bridge**

Modify compiled rows, not canonical user rows. Penalty semantics always minimize weighted violation; translate sign into target objective.

- [ ] **Step 4: Add violation accessors**

`Solution::violation(handle)` returns lower, upper, and total values.

- [ ] **Step 5: Audit and implement native feasibility relaxation**

Keep it solve-scoped. When native mutation cannot be safely reversed, run on a temporary rebuilt backend session.

- [ ] **Step 6: Verify and commit**

```bash
cargo test -p roml-highs --test soft_constraints
cargo test -p roml --all-targets
cargo test -p roml-highs --all-targets
git add src/construct/soft.rs src/compiler/bridge/mod.rs src/compiler/bridge/soft.rs \
  src/solver/feasibility_relaxation.rs src/model/mod.rs src/solution/mod.rs \
  src/compiler/session.rs roml-highs/src/relaxation.rs roml-highs/src/lib.rs \
  roml-highs/src/facade.rs roml-highs/tests/soft_constraints.rs \
  docs/knowledge/highs-feasibility-relaxation.md \
  docs/release/evidence/M3_P30_SOFT_CONSTRAINTS.md
git commit -m "feat(model): add soft constraints and relaxation"
```

---

## Task 14: Add objective policies and lexicographic execution

**Phase:** P31

**Files:**
- Create: `src/objective_policy.rs`
- Create: `src/solver/multiobjective.rs`
- Create: `roml-highs/src/multiobjective.rs`
- Create: `tests/objective_policy.rs`
- Create: `roml-highs/tests/lexicographic.rs`
- Create: `docs/knowledge/highs-multiobjective.md`
- Create: `docs/release/evidence/M3_P31_LEXICOGRAPHIC.md`
- Modify: `src/lib.rs`
- Modify: `src/model/mod.rs`
- Modify: `src/snapshot.rs`
- Modify: `src/delta.rs`
- Modify: `src/solver/session.rs`
- Modify: `src/solver/overlay.rs`
- Modify: `src/solution/mod.rs`
- Modify: `src/solution/metadata.rs`
- Modify: `roml-highs/src/lib.rs`
- Modify: `roml-highs/src/session.rs`

- [ ] **Step 1: Write policy validation tests**

Reject empty policies, stale/inactive/duplicate objectives, non-finite weights, and negative/non-finite tolerances.

- [ ] **Step 2: Store policy canonically**

`minimize` and `maximize` set `Single`. Snapshot/delta include policy changes.

- [ ] **Step 3: Implement portable stage formulas**

For minimization: `f(x) <= z + abs + rel*abs(z)`. For maximization: `f(x) >= z - abs - rel*abs(z)`. Test positive, zero, and negative `z`.

- [ ] **Step 4: Execute stages through overlays**

Temporary objectives and lock rows roll back on success or failure.

- [ ] **Step 5: Audit native HiGHS semantics**

Use native path only when priorities, senses, weights, and tolerance semantics match exactly.

- [ ] **Step 6: Add native/portable differential corpus**

Compare objective values and degradation constraints.

- [ ] **Step 7: Verify and commit**

```bash
cargo test -p roml --test objective_policy
cargo test -p roml-highs --test lexicographic -- --nocapture
cargo test -p roml --all-targets
cargo test -p roml-highs --all-targets
git add src/objective_policy.rs src/solver/multiobjective.rs src/lib.rs \
  src/model/mod.rs src/snapshot.rs src/delta.rs src/solver/session.rs \
  src/solver/overlay.rs src/solution roml-highs/src/multiobjective.rs \
  roml-highs/src/lib.rs roml-highs/src/session.rs tests/objective_policy.rs \
  roml-highs/tests/lexicographic.rs docs/knowledge/highs-multiobjective.md \
  docs/release/evidence/M3_P31_LEXICOGRAPHIC.md
git commit -m "feat(solve): add lexicographic objective policies"
```

---

## Task 15: Add interval bounds and bridge framework

**Phase:** P32 foundation

**Files:**
- Create: `src/compiler/bounds.rs`
- Create: `src/compiler/bridge/indicator.rs`
- Create: `src/compiler/bridge/minmax.rs`
- Create: `src/compiler/bridge/absolute.rs`
- Create: `src/compiler/bridge/boolean.rs`
- Create: `src/compiler/bridge/product.rs`
- Create: `tests/compiler_bridges.rs`
- Modify: `src/compiler/bridge/mod.rs`
- Modify: `src/compiler/session.rs`
- Modify: `src/compiler/report.rs`

**Produces:**

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Interval { pub lower: f64, pub upper: f64 }

#[derive(Clone, Debug, PartialEq)]
pub struct BoundTrace {
    pub sources: Vec<BoundSource>,
    pub result: Interval,
}

pub trait Bridge {
    fn compile(
        &self,
        context: &mut BridgeContext,
    ) -> Result<BridgeOutput, CompileError>;
}
```

- [ ] **Step 1: Write interval tests**

Cover coefficient signs, constants, fixed variables, infinite bounds, and parameters.

- [ ] **Step 2: Implement deterministic propagation**

Use exact interval arithmetic for linear terms and reject NaN.

- [ ] **Step 3: Implement one-sided Big-M derivation helpers**

Return `UnboundedBigM` with construct/function context when finite proof is absent.

- [ ] **Step 4: Implement bridge finalization**

Validate origins, deterministic generated order, dependencies, and report entries.

- [ ] **Step 5: Verify and commit**

```bash
cargo test -p roml --test compiler_bridges
cargo test -p roml --all-targets
git add src/compiler/bounds.rs src/compiler/bridge src/compiler/session.rs \
  src/compiler/report.rs tests/compiler_bridges.rs
git commit -m "feat(compiler): add safe bridge infrastructure"
```

---

## Task 16: Add indicators, reification, Boolean, and cardinality

**Phase:** P32

**Files:**
- Create: `src/construct/indicator.rs`
- Create: `src/construct/boolean.rs`
- Create: `tests/common_constructs.rs`
- Create: `roml-highs/tests/formulation_equivalence.rs`
- Create: `docs/release/evidence/M3_P32_COMMON_CONSTRUCTS.md`
- Modify: `src/construct/mod.rs`
- Modify: `src/model/mod.rs`
- Modify: `src/compiler/backend_ir.rs`
- Modify: `src/compiler/session.rs`
- Modify: `src/compiler/bridge/indicator.rs`
- Modify: `src/compiler/bridge/boolean.rs`

- [ ] **Step 1: Write truth-table and validation tests**

Reject non-binary activators, duplicate cardinality inputs, invalid `k`, and continuous reification without explicit separation.

- [ ] **Step 2: Implement canonical payloads**

Define indicator activation, one-way body, reification separation, Boolean operator, and cardinality bounds.

- [ ] **Step 3: Implement native and portable indicators**

Select native only with exact support. Portable bridge derives finite M and records trace.

- [ ] **Step 4: Implement reification as two implications**

Infer unit separation only when integer-valued expression proof exists.

- [ ] **Step 5: Implement exact Boolean/cardinality rows**

Use deterministic linear formulations and no anonymous generated entities.

- [ ] **Step 6: Differentially verify feasible sets**

Enumerate small binary domains and compare semantic/reference/native/portable results.

- [ ] **Step 7: Commit**

```bash
cargo test -p roml --test common_constructs indicator
cargo test -p roml-highs --test formulation_equivalence indicator
git add src/construct/indicator.rs src/construct/boolean.rs src/construct/mod.rs \
  src/model/mod.rs src/compiler/backend_ir.rs src/compiler/session.rs \
  src/compiler/bridge/indicator.rs src/compiler/bridge/boolean.rs \
  tests/common_constructs.rs roml-highs/tests/formulation_equivalence.rs \
  docs/release/evidence/M3_P32_COMMON_CONSTRUCTS.md
git commit -m "feat(model): add logical semantic constructs"
```

---

## Task 17: Add min/max, abs, clamp, and exact products

**Phase:** P32

**Files:**
- Create: `src/construct/minmax.rs`
- Create: `src/construct/absolute.rs`
- Create: `src/construct/product.rs`
- Modify: `src/construct/mod.rs`
- Modify: `src/model/mod.rs`
- Modify: `src/compiler/bridge/minmax.rs`
- Modify: `src/compiler/bridge/absolute.rs`
- Modify: `src/compiler/bridge/product.rs`
- Modify: `tests/common_constructs.rs`
- Modify: `roml-highs/tests/formulation_equivalence.rs`

- [ ] **Step 1: Write exact versus one-sided tests**

Prove epigraph/hypograph feasible sets differ from exact equality without relying on objectives.

- [ ] **Step 2: Implement one-sided no-binary bridges**

Max epigraph and min hypograph create comparison rows only.

- [ ] **Step 3: Implement bounded exact bridges**

Use selector binaries and bound-derived M when no exact native primitive is qualified.

- [ ] **Step 4: Implement abs, positive part, and clamp**

Preserve top-level construct origin even when bridge helpers compose lower-level formulations.

- [ ] **Step 5: Implement exact products**

Support binary-binary and binary-times-bounded-linear only. Reject continuous-continuous exact requests.

- [ ] **Step 6: Randomized direct-evaluation tests**

Fix inputs and compare output to Rust evaluation.

- [ ] **Step 7: Commit and close P32**

```bash
cargo test -p roml --test common_constructs
cargo test -p roml-highs --test formulation_equivalence
cargo test -p roml --all-targets
git add src/construct/minmax.rs src/construct/absolute.rs src/construct/product.rs \
  src/construct/mod.rs src/model/mod.rs src/compiler/bridge/minmax.rs \
  src/compiler/bridge/absolute.rs src/compiler/bridge/product.rs \
  tests/common_constructs.rs roml-highs/tests/formulation_equivalence.rs \
  docs/release/evidence/M3_P32_COMMON_CONSTRUCTS.md
git commit -m "feat(model): add algebraic semantic constructs"
```

Request independent OR review.

---

## Task 18: Add PWL semantics and formulations

**Phase:** P33

**Files:**
- Create: `src/construct/piecewise_linear.rs`
- Create: `src/compiler/bridge/piecewise_linear.rs`
- Create: `tests/piecewise_linear.rs`
- Create: `docs/release/evidence/M3_P33_PWL_BOUNDS.md`
- Modify: `src/construct/mod.rs`
- Modify: `src/model/mod.rs`
- Modify: `src/compiler/backend_ir.rs`
- Modify: `src/compiler/capability.rs`
- Modify: `src/compiler/session.rs`
- Modify: `roml-highs/tests/formulation_equivalence.rs`

**Produces:**

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PiecewisePoint { pub x: f64, pub y: f64 }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PwlRelation { Epigraph, Hypograph, ExactGraph }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Extrapolation { Reject, ExtendEndSegments, ConstantEnds }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Curvature { Convex, Concave, Affine, NonConvex }
```

- [ ] **Step 1: Validate points and classify curvature**

Reject non-finite, duplicate, out-of-order, and underspecified points. Classify segment slopes deterministically.

- [ ] **Step 2: Implement direct evaluator**

Use it for tests and diagnostic output.

- [ ] **Step 3: Compile convex epigraph and concave hypograph**

Use supporting inequalities and assert zero generated binary variables.

- [ ] **Step 4: Compile exact graph**

Select native PWL when officially qualified, otherwise SOS2 when available, otherwise deterministic exact segment binaries.

- [ ] **Step 5: Verify random fixed-input outputs**

Cover affine, convex, concave, and nonconvex curves.

- [ ] **Step 6: Verify reports**

Record curvature, relation, representation, generated counts, and binary-avoidance reason.

- [ ] **Step 7: Commit and review**

```bash
cargo test -p roml --test piecewise_linear
cargo test -p roml-highs --test formulation_equivalence pwl
cargo test -p roml --all-targets
cargo test -p roml-highs --all-targets
git add src/construct/piecewise_linear.rs src/compiler/bridge/piecewise_linear.rs \
  src/construct/mod.rs src/model/mod.rs src/compiler/backend_ir.rs \
  src/compiler/capability.rs src/compiler/session.rs tests/piecewise_linear.rs \
  roml-highs/tests/formulation_equivalence.rs \
  docs/release/evidence/M3_P33_PWL_BOUNDS.md
git commit -m "feat(model): add piecewise linear functions"
```

---

## Task 19: Integrate construct-aware IIS and complete public workflows

**Phase:** P34 integration

**Files:**
- Modify: `src/solver/infeasibility.rs`
- Modify: `src/compiler/origin.rs`
- Modify: `roml-highs/tests/iis_reports.rs`
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

- [ ] **Step 1: Add bridged conflict fixtures**

Indicator, soft-constraint-bound, and PWL conflicts map to original constructs and generated roles.

- [ ] **Step 2: Write compiled public examples**

Each example is exercised by integration tests and imports only public golden-path APIs.

- [ ] **Step 3: Document semantic/native/bridge guarantees**

Tables state exact semantics, portable formulation, native support, version limits, and failure behavior.

- [ ] **Step 4: Document NLP extension exercise**

Show exact files and additive enum/IR/capability changes for quadratic and nonlinear scalar functions without implementing them.

- [ ] **Step 5: Verify docs/examples and commit**

```bash
cargo test -p roml-highs --examples
cargo test -p roml --doc
cargo test -p roml-highs --doc
RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps
RUSTDOCFLAGS='-D warnings' cargo doc -p roml-highs --no-deps
git add src/solver/infeasibility.rs src/compiler/origin.rs \
  roml-highs/tests/iis_reports.rs README.md MODELING_API.md MIGRATION.md \
  CHANGELOG.md docs roml-highs/examples
git commit -m "docs(m3): complete semantic modeling workflows"
```

---

## Task 20: Qualify M3 and close planning state

**Phase:** P34

**Files:**
- Create: `tests/m3_property.rs`
- Create: `roml-highs/tests/m3_native_portable.rs`
- Create: `benches/m3_orchestration.rs`
- Create: `docs/release/evidence/M3_QUALIFICATION.md`
- Create: `docs/release/evidence/M3_PUBLIC_API.md`
- Create: `docs/release/evidence/M3_PERFORMANCE.md`
- Create: `docs/release/evidence/M3_NLP_READINESS.md`
- Modify: `.planning/milestones/M3-semantic-modeling-workflows/STATE.md`
- Modify: `.planning/milestones/M3-semantic-modeling-workflows/TRACEABILITY.md`
- Modify: `.planning/milestones/M3-semantic-modeling-workflows/README.md`
- Modify: `.github/workflows/ci-core.yml`
- Modify: `.github/workflows/ci-highs.yml`

- [ ] **Step 1: Build deterministic native/portable corpus**

Cover every M3 construct, soft constraints, lexicographic policies, starts/locks, and PWL. Compare status, objectives, and semantic satisfaction.

- [ ] **Step 2: Run fixed-seed property suites**

Test fixing state machine, interval containment, bridge equivalence, origin completeness, compiled delta/rebuild equality, and overlay non-leakage.

- [ ] **Step 3: Enforce performance gate**

Primitive parameter-update median overhead must remain below 5% or 50 microseconds per solve attempt, whichever is larger. Profile and fix identity compiler/cache before acceptance when exceeded.

- [ ] **Step 4: Run full matrix**

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

Record commercial-adapter limitations separately; do not weaken core/HiGHS gates.

- [ ] **Step 5: Test fresh packed consumers**

Compile and run each golden workflow from packed artifacts in temporary projects.

- [ ] **Step 6: Complete independent reviews**

Principal engineering, OR formulation, native/unsafe, and NLP-readiness reviews must close all P0/P1 findings.

- [ ] **Step 7: Scan planning artifacts for unresolved markers**

```bash
rg -n "T[B]D|T[O]DO|F[I]XME|implement later|fill in" \
  .planning/milestones/M3-semantic-modeling-workflows \
  docs/superpowers/specs/2026-08-02-semantic-modeling-and-solve-workflows-design.md \
  docs/superpowers/plans/2026-08-02-semantic-modeling-and-solve-workflows.md
```

Every match must be a quoted prohibition or removed.

- [ ] **Step 8: Close traceability and commit**

```bash
git add tests roml-highs/tests benches docs/release/evidence \
  .planning/milestones/M3-semantic-modeling-workflows .github/workflows
git commit -m "test(m3): qualify semantic modeling milestone"
```

- [ ] **Step 9: Stop before publication**

Do not tag, publish, or create a release. Report the exact verified SHA and request a separate owner decision for publication.

---

## Plan self-review result

- Every SM-01 through SM-15 requirement maps to a phase and task.
- P25 and P26 establish canonical and compiler boundaries before concrete constructs.
- Cross-task public and backend interfaces are defined explicitly above.
- No task introduces backend state into canonical `Model`.
- No bridge is permitted to use an arbitrary Big-M.
- Overlay rollback and rebuild semantics are explicit.
- IIS scope/minimality/version limitations are mandatory report fields.
- Soft penalty sense and signed correction semantics are tested before implementation.
- Lexicographic formulas cover positive, zero, and negative objectives.
- Exact and one-sided min/max/PWL relations remain distinct.
- Convex PWL epigraph and concave PWL hypograph have zero-binary assertions.
- NLP readiness is a concrete extension exercise, not hidden NLP implementation.
- P34 includes public API, package, fresh-consumer, performance, and independent review evidence.

## Execution handoff

Use subagent-driven development with one fresh implementation agent per task or tightly coupled task group and two-stage review after each phase. Use isolated worktrees and do not exceed the WIP limits in the M3 execution protocol.
