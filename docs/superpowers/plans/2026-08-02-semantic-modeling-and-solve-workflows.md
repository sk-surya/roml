# Semantic Modeling and Solve Workflows Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Checkboxes are execution state, not estimates.

**Goal:** Add a canonical semantic modeling IR, capability-aware backend compiler, solution-reuse workflows, infeasibility diagnostics, soft constraints, lexicographic objectives, and common exact MILP constructs while preserving ROML's incremental/recoverable solver guarantees and leaving an additive path for future nonlinear programming.

**Architecture:** Canonical `Model` state stores functions, sets, constructs, objective policy, declared domains, fixings, metadata, and revisions. A per-solver `CompilationSession` lowers canonical snapshots/deltas into backend IR with exact compilation identity, typed capabilities, exact bridges, generated-entity origins, and formulation reports. `SolverSession` applies solve-scoped overlays, starts, hints, and objective stages transactionally before mapping backend results back to user entities.

**Tech Stack:** Rust 1.85, existing ROML arenas/revisions/journal/snapshots, `roml-highs` through pinned `highs-sys`, property/differential tests, GitHub Actions, cargo fmt/clippy/test/doc/public-api/package.

## Global constraints

- Preserve M2 ordinary `Model`, `LinExpr`, `Highs::solve`, `solve_with`, and `Solution` source usage unless an executable contradiction is reviewed and approved.
- Core `roml` remains solver-free.
- Canonical state contains no backend indices, native handles, selected Big-M values, or solve overlays.
- High-level constructs remain canonical entities; bridge expansion occurs only in the compiler.
- Exact semantics are never silently compiled as relaxations.
- Big-M requires finite proof or explicit validated user input; no default Big-M constant exists.
- Every generated variable, row, objective artifact, and overlay artifact has an `EntityOrigin`.
- Unsupported features are applied, exactly bridged, explicitly adjusted, or rejected; never ignored.
- Overlay rollback uncertainty marks the session `RequiresRebuild`.
- Primitive compiled delta execution remains observationally equivalent to compiled rebuild.
- M3 implements linear scalar functions only; nonlinear tracing/differentiation are excluded.
- MSRV remains Rust 1.85.
- No publication, tag, or release is part of this plan.

---

## Cross-task interface contract

These names and semantics are authoritative for M3 planning. A dependent phase may change them only through an approved decision/spec/requirements amendment.

### Identity

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ModelLineageId(u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ModelInstanceId(u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CompilationId(u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RecipeFingerprint([u8; 32]);
```

Authority rules:

- `ModelLineageId` governs assignment compatibility across clones.
- `ModelInstanceId + ModelRevision` identifies exact canonical state.
- `CompilationId` identifies exact compiled/backend state and is used for stale-state safety.
- `RecipeFingerprint` is deterministic evidence/cache support only and is never authority.

### Metadata

```rust
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

Existing entity name storage remains authoritative for names.

### Function-in-set

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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormulationPreference {
    Auto,
    Portable,
    NativeRequired,
}
```

Parameter dependencies are derived from `ConstructKind` and may be cached only with an invariant proving equality.

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
    source_instance: Option<ModelInstanceId>,
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

Weighted objectives use finite nonnegative weights and normalize each objective to minimization according to its own sense.

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

#[derive(Clone, Debug, PartialEq)]
pub enum CompiledObjectivePolicy {
    Single(CompiledObjectiveId),
    Weighted(Vec<CompiledWeightedObjective>),
    Lexicographic(Vec<CompiledObjectiveLevel>),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CompiledWeightedObjective {
    pub objective: CompiledObjectiveId,
    pub weight: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CompiledObjectiveLevel {
    pub objective: CompiledObjectiveId,
    pub absolute_tolerance: f64,
    pub relative_tolerance: f64,
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
pub struct BackendSnapshot {
    pub compilation_id: CompilationId,
    pub source_instance: ModelInstanceId,
    pub source_revision: ModelRevision,
    pub variables: Vec<CompiledVariable>,
    pub linear_rows: Vec<CompiledLinearRow>,
    pub native_constraints: Vec<BackendConstraint>,
    pub objectives: Vec<CompiledObjective>,
    pub objective_policy: CompiledObjectivePolicy,
    pub origin_map: OriginMap,
    pub report: CompilationReport,
    pub recipe_fingerprint: RecipeFingerprint,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BackendDeltaBatch {
    pub from_compilation: CompilationId,
    pub to_compilation: CompilationId,
    pub from_revision: ModelRevision,
    pub to_revision: ModelRevision,
    pub operations: Vec<BackendOp>,
    pub recipe_fingerprint: RecipeFingerprint,
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
    RemoveLinearCoefficient { constraint: CompiledConstraintId, variable: CompiledVariableId },
    AddObjective(CompiledObjective),
    RemoveObjective(CompiledObjectiveId),
    SetObjectiveCoefficient { objective: CompiledObjectiveId, variable: CompiledVariableId, value: f64 },
    RemoveObjectiveCoefficient { objective: CompiledObjectiveId, variable: CompiledVariableId },
    SetObjectiveConstant { objective: CompiledObjectiveId, value: f64 },
    SetObjectiveSense { objective: CompiledObjectiveId, sense: Sense },
    SetObjectivePolicy(CompiledObjectivePolicy),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EntityOrigin {
    UserVariable(Variable),
    UserConstraint(Constraint),
    UserObjective(Objective),
    Construct { construct: Construct, role: GeneratedRole },
    SolveOverlay { overlay: OverlayId, role: GeneratedRole },
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
```

### Solve plan and exact result identity

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

#[derive(Clone, Debug, Default, PartialEq)]
pub struct EffectiveSolvePlan {
    pub applied_features: Vec<AppliedFeature>,
    pub adjustments: Vec<PlanAdjustment>,
    pub rejections: Vec<PlanRejection>,
    pub objective_stages: Vec<ObjectiveStageResult>,
}
```

`SolveMetadata` adds `model_lineage`, `model_instance`, `model_revision`, and `compilation_id`.

### Infeasibility analysis

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct InfeasibilityReport {
    pub lineage: ModelLineageId,
    pub instance: ModelInstanceId,
    pub revision: ModelRevision,
    pub compilation_id: CompilationId,
    pub backend: BackendIdentity,
    pub kind: InfeasibilityKind,
    pub scope: AnalysisScope,
    pub minimality: MinimalityClaim,
    pub completion: CompletionStatus,
    pub members: Vec<ConflictMember>,
    pub statistics: AnalysisStatistics,
}
```

---

## Planned module structure

```text
src/identity.rs
src/metadata.rs
src/function/{mod,scalar,set}.rs
src/construct/{mod,indicator,minmax,absolute,boolean,product,piecewise_linear,soft}.rs
src/assignment.rs
src/objective_policy.rs
src/compiler/{mod,backend_ir,capability,session,origin,report,bounds}.rs
src/compiler/bridge/{mod,indicator,minmax,absolute,boolean,product,piecewise_linear,soft}.rs
src/solver/{plan,overlay,effective_plan,infeasibility,feasibility_relaxation,multiobjective}.rs
roml-highs/src/{compiler,start,iis,relaxation,multiobjective}.rs
```

---

## Task 1 — Capture baseline and characterization

**Phase:** P25  
**Files:** create `docs/release/evidence/M3_P25_SEMANTIC_IR.md`, `tests/m3_baseline_characterization.rs`, and public/package baseline artifacts.

- [ ] Record exact base SHA, Rust/Cargo versions, and supported HiGHS modes.
- [ ] Run untouched fmt/check/clippy/test/doc commands for `roml` and `roml-highs`.
- [ ] Capture `cargo public-api` and `cargo package --list` output.
- [ ] Add characterization tests for fluent linear modeling, deterministic snapshot, parameter update, objective constant, solution metadata, and one-rebuild-retry behavior.
- [ ] Commit as `test(m3): capture semantic modeling baseline`.

Verification:

```bash
cargo test -p roml --test m3_baseline_characterization -- --nocapture
```

## Task 2 — Add lineage, instance identity, and metadata

**Phase:** P25  
**Files:** create `src/identity.rs`, `src/metadata.rs`, `tests/lineage_metadata.rs`; modify `src/model/mod.rs`, `src/solution/metadata.rs`, `src/lib.rs`.

- [ ] Write failing tests: independent models differ in lineage/instance; clone preserves lineage but receives new instance; solution records all state IDs.
- [ ] Allocate opaque IDs through checked atomic counters with zero reserved.
- [ ] Implement manual `Default` and `Clone` for `Model`.
- [ ] Add metadata store keyed by `EntityRef`; metadata changes are canonical but non-solver-affecting.
- [ ] Export lineage/instance/metadata types through reviewed public surfaces.
- [ ] Commit as `feat(model): add lineage instance and metadata`.

Verification:

```bash
cargo test -p roml --test lineage_metadata
cargo test -p roml --all-targets
```

## Task 3 — Add function-in-set canonical constraints

**Phase:** P25  
**Files:** create `src/function/**`, `tests/semantic_ir.rs`; modify constraint/model/snapshot/delta/lib modules.

- [ ] Write failing conversion tests for `.le`, `.ge`, `.eq`, and `.between`.
- [ ] Implement `ScalarFunction`, `ScalarSet`, `FunctionConstraint`, and `IntoScalarFunction`.
- [ ] Keep the existing coefficient index authoritative in P25 and reconstruct linear functions deterministically.
- [ ] Extend canonical snapshot/delta with semantic function/set data while invariant-checking transitional legacy fields.
- [ ] Commit as `feat(model): add linear function-in-set semantics`.

Verification:

```bash
cargo test -p roml --test semantic_ir
cargo test -p roml --test m3_baseline_characterization
cargo test -p roml --all-targets
```

## Task 4 — Add canonical construct lifecycle

**Phase:** P25  
**Files:** create `src/construct/mod.rs`; modify model/snapshot/delta/metadata/lib and semantic tests.

- [ ] Write add/clone/snapshot/activity/remove/stale-generation tests using a private fixture payload.
- [ ] Implement one generation-safe construct arena.
- [ ] Add self-contained construct canonical changes.
- [ ] Derive parameter dependencies from payload; validate any cache.
- [ ] Extend model invariants for live references, metadata, and auxiliary ownership.
- [ ] Finish P25 evidence and request independent review.

Verification:

```bash
cargo test -p roml --all-targets
cargo clippy -p roml --all-targets -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps
cargo public-api -p roml
```

## Task 5 — Define backend IR and exact compilation identity

**Phase:** P26  
**Files:** create `src/compiler/{mod,backend_ir,origin,report}.rs`, `tests/compiler_identity.rs`, P26 evidence; modify `src/advanced.rs`, `src/lib.rs`.

- [ ] Write builder-finalization tests that reject any generated entity without origin.
- [ ] Allocate dense deterministic compiled IDs and a unique checked `CompilationId` per compiled state.
- [ ] Store source instance/revision and objective policy in `BackendSnapshot`.
- [ ] Define backend deltas with exact from/to compilation IDs.
- [ ] Implement bidirectional origin queries and structured compilation reports.
- [ ] Implement deterministic recipe fingerprinting solely for evidence/cache use.
- [ ] Commit as `feat(compiler): define backend IR and compilation identity`.

Verification:

```bash
cargo test -p roml --test compiler_identity
```

## Task 6 — Implement typed capabilities

**Phase:** P26  
**Files:** create `src/compiler/capability.rs`; modify `src/solver/backend.rs`, `src/solver/request.rs`, `src/solver/conformance.rs`, `roml-highs/src/session.rs`.

- [ ] Characterize every legacy capability mapping before replacement.
- [ ] Implement `BackendCapabilitySet` keyed by `BackendFeature`.
- [ ] Migrate request validation and conformance tests.
- [ ] Build version-aware HiGHS capability sets; unqualified M3 features remain unsupported.
- [ ] Remove the transitional flat conversion before P26 merge.
- [ ] Commit as `feat(backend): add typed feature capabilities`.

Verification:

```bash
cargo test -p roml solver::conformance
cargo test -p roml-highs --all-targets
```

## Task 7 — Add identity compiler and migrate synchronization

**Phase:** P26  
**Files:** create `src/compiler/session.rs`, `roml-highs/src/compiler.rs`, `docs/migration/M3_BACKEND_IR.md`; modify solver session/facade/reference/conformance, HiGHS session, advanced exports, differential harness.

- [ ] Compile primitive linear snapshots one-to-one, including active compiled objective policy.
- [ ] Compile primitive deltas with exact from/to compilation IDs; use rebuild on uncertainty.
- [ ] Add explicit coefficient-removal and objective-policy backend operations.
- [ ] Migrate ReferenceBackend first and run recovery/differential tests.
- [ ] Migrate HiGHS; it receives no canonical `ModelSnapshot` afterward.
- [ ] Preserve compile-before-mutation and one-rebuild-retry invariants.
- [ ] Add fixed-seed compiled-delta versus rebuild equality.
- [ ] Commit as `feat(sync): compile canonical state into backend IR` and request architecture review.

Verification:

```bash
cargo test -p roml --all-targets
cargo test -p roml-highs --all-targets
cargo clippy -p roml --all-targets -- -D warnings
cargo clippy -p roml-highs --all-targets -- -D warnings
```

## Task 8 — Unify domains and add persistent fixing

**Phase:** P27  
**Files:** modify variable/model/snapshot/delta/compiler; create `tests/fixing_assignment.rs`, P27 evidence.

- [ ] Write a reference state-machine test over continuous/integer/binary/semi domains, bound changes, fixing, and unfix.
- [ ] Replace fragmented variable state with one record containing declared domain, fixing, activity, and name.
- [ ] Add named integrality tolerance and atomic fixing validation.
- [ ] Add canonical `SetVariableFixing` operation.
- [ ] Compile effective bound deltas when supported; rebuild otherwise.
- [ ] Commit as `feat(model): add first-class variable fixing`.

Verification:

```bash
cargo test -p roml --test fixing_assignment
cargo test -p roml --all-targets
cargo test -p roml-highs --all-targets
```

## Task 9 — Add assignments, locks, and reversible overlays

**Phase:** P27  
**Files:** create `src/assignment.rs`, overlay/effective-plan modules, `tests/solve_overlay.rs`; modify solution/session/backend/compiler/HiGHS session.

- [ ] Test assignment compatibility by lineage, generation, and value/domain validation; instance/revision are provenance, not compatibility authority.
- [ ] Implement `Solution::primal_assignment()` excluding compiler-only variables.
- [ ] Test all lock selectors and continuous bands.
- [ ] Compile overlays against one exact `CompilationId`.
- [ ] Implement explicit fallible apply/rollback receipts; do not rely only on `Drop`.
- [ ] Inject failure during validation, apply, solve, extraction, rollback, and post-rollback verification.
- [ ] Assert every later clean solve equals fresh rebuild and no canonical revision changed.
- [ ] Commit as `feat(solve): add assignments locks and overlays` and request P27 review.

Verification:

```bash
cargo test -p roml --test solve_overlay -- --nocapture
cargo test -p roml --all-targets
cargo test -p roml-highs --all-targets
```

## Task 10 — Add SolvePlan, starts, and hints

**Phase:** P28  
**Files:** create plan/start modules, solve-plan tests, HiGHS knowledge/evidence; modify assignment/facade/session/metadata/capabilities.

- [ ] Prove `solve`, `solve_with`, and empty `solve_plan` equivalence.
- [ ] Validate lineages, entities, finite values, conflicts, duplicates, and conversion policy before backend mutation.
- [ ] Route all solve façades through one plan executor.
- [ ] Audit exact bundled/system official HiGHS APIs for starts, partial starts, clearing, multiple starts, hints, return codes, and version availability.
- [ ] Implement only qualified support; absent hints reject by default.
- [ ] Prove starts/hints leave canonical and compiled feasible-region signatures unchanged.
- [ ] Record applied/converted/rejected features and exact compilation ID in solution metadata.
- [ ] Commit as `feat(solve): add solve plans starts and hints` and request P28 review.

Verification:

```bash
cargo test -p roml-highs --test solve_plan -- --nocapture
cargo test -p roml --all-targets
cargo test -p roml-highs --all-targets
```

## Task 11 — Add normalized IIS/conflict analysis

**Phase:** P29  
**Files:** create core infeasibility/report modules and tests; modify origin/capability/session/lib; create P29 evidence.

- [ ] Write synthetic mapping tests for row sides, declared bounds, fixings, locks, and generated construct rows.
- [ ] Require kind, scope, minimality, completion, instance/revision, and exact compilation ID.
- [ ] Reject stale mapping when backend conflict `CompilationId` differs from the origin map.
- [ ] Render stable text/Markdown/structured reports using original names/metadata/provenance.
- [ ] Commit as `feat(analysis): add origin-aware infeasibility reports`.

Verification:

```bash
cargo test -p roml --test infeasibility_report
cargo test -p roml --all-targets
```

## Task 12 — Implement qualified HiGHS IIS

**Phase:** P29  
**Files:** create `roml-highs/src/iis.rs`, tests, official-API knowledge; modify HiGHS exports/session/facade and capabilities.

- [ ] Audit exact bundled/minimum-system symbols, fields, status codes, scope, model classes, minimality, and version availability.
- [ ] Upgrade official bindings only through a separate reviewed dependency commit when necessary; do not handwrite ABI declarations.
- [ ] Add version capability tests.
- [ ] Extract compiled row/column-side members tagged with exact `CompilationId`; core performs user mapping.
- [ ] Add deterministic row, bound, fixing, and lock fixtures.
- [ ] Commit as `feat(highs): add version-qualified IIS analysis`.

Verification:

```bash
cargo test -p roml-highs --test iis_reports -- --nocapture
cargo test -p roml-highs --all-targets
```

## Task 13 — Add soft constraints and feasibility relaxation

**Phase:** P30  
**Files:** create soft construct/bridge and feasibility-relaxation modules, HiGHS relaxation module/tests/knowledge/evidence; modify model/solution/compiler/HiGHS facade.

- [ ] Pin upper/lower/equality/ranged and signed-correction algebra before implementation.
- [ ] Create stable canonical violation variables with lower/upper/correction origins.
- [ ] Compile adjusted rows and objective contributions without mutating canonical user rows.
- [ ] Define weighted violation as minimization and translate sign correctly into maximize targets.
- [ ] Add solution lower/upper/total violation accessors.
- [ ] Audit native HiGHS feasibility-relaxation mutation/lifecycle and use a temporary rebuilt session when reversal is uncertain.
- [ ] Prove solve-scoped relaxation does not create persistent soft handles or canonical changes.
- [ ] Commit as `feat(model): add soft constraints and relaxation`.

Verification:

```bash
cargo test -p roml-highs --test soft_constraints
cargo test -p roml --all-targets
cargo test -p roml-highs --all-targets
```

## Task 14 — Add objective policies and lexicographic execution

**Phase:** P31  
**Files:** create objective-policy/core/HiGHS multiobjective modules, tests, official knowledge/evidence; modify model/snapshot/delta/session/overlay/solution.

- [ ] Reject empty policies, stale/inactive/duplicate objectives, negative/non-finite weights, and negative/non-finite tolerances.
- [ ] Store policy canonically; `minimize`/`maximize` set `Single`.
- [ ] Normalize weighted objectives to minimization according to original sense.
- [ ] Implement portable lexicographic stage bounds for positive, zero, and negative optima.
- [ ] Execute temporary objectives and lock rows through overlays tagged with exact compilation/overlay IDs.
- [ ] Audit native HiGHS priorities/weights/tolerances/senses and select native only on exact semantic match.
- [ ] Add native/portable differential corpus and all-objective/stage result storage.
- [ ] Commit as `feat(solve): add objective policies and lexicographic execution`.

Verification:

```bash
cargo test -p roml --test objective_policy
cargo test -p roml-highs --test lexicographic -- --nocapture
cargo test -p roml --all-targets
cargo test -p roml-highs --all-targets
```

## Task 15 — Add interval bounds and bridge framework

**Phase:** P32 foundation  
**Files:** create bound analyzer, bridge contract/modules, tests; modify compiler session/report.

- [ ] Test interval arithmetic over coefficient signs, constants, fixed variables, infinite bounds, and parameters.
- [ ] Implement deterministic linear propagation and reject NaN.
- [ ] Implement one-sided Big-M helpers returning construct-aware `UnboundedBigM`.
- [ ] Implement bridge finalization with deterministic generated order, dependency capture, origins, and report entries.
- [ ] Commit as `feat(compiler): add safe bridge infrastructure`.

Verification:

```bash
cargo test -p roml --test compiler_bridges
cargo test -p roml --all-targets
```

## Task 16 — Add indicators, reification, Boolean, and cardinality

**Phase:** P32  
**Files:** create indicator/Boolean constructs and equivalence tests; modify model/backend IR/compiler/bridge modules; create P32 evidence.

- [ ] Reject non-binary activators, duplicate cardinality inputs, invalid `k`, and continuous exact reification without separation.
- [ ] Store exact semantic payloads and per-construct formulation preference.
- [ ] Select qualified native indicators or finite-bound exact bridges.
- [ ] Implement reification as two implications; infer unit gap only from proven integrality.
- [ ] Implement exact Boolean/cardinality rows.
- [ ] Enumerate small binary domains and compare semantic/reference/native/portable feasible sets.
- [ ] Commit as `feat(model): add logical semantic constructs`.

Verification:

```bash
cargo test -p roml --test common_constructs indicator
cargo test -p roml-highs --test formulation_equivalence indicator
```

## Task 17 — Add min/max, abs, clamp, and exact products

**Phase:** P32  
**Files:** create minmax/absolute/product constructs and bridges; modify model/tests/equivalence evidence.

- [ ] Prove exact and epigraph/hypograph feasible sets differ without objective assumptions.
- [ ] Implement no-binary max epigraph/min hypograph rows.
- [ ] Implement bounded exact selector/native formulations with bound evidence.
- [ ] Implement exact abs, positive part, and clamp while preserving top-level construct origins.
- [ ] Implement binary-binary and binary-times-bounded-linear products.
- [ ] Reject continuous-times-continuous exact requests.
- [ ] Add fixed-input randomized direct-evaluation tests.
- [ ] Commit as `feat(model): add algebraic semantic constructs` and request OR review.

Verification:

```bash
cargo test -p roml --test common_constructs
cargo test -p roml-highs --test formulation_equivalence
cargo test -p roml --all-targets
```

## Task 18 — Add PWL semantics and formulations

**Phase:** P33  
**Files:** create PWL construct/bridge/tests/evidence; modify model/backend IR/capability/compiler and equivalence tests.

- [ ] Reject non-finite, duplicate, out-of-order, and underspecified points.
- [ ] Classify affine/convex/concave/nonconvex from segment slopes.
- [ ] Implement direct interpolation/extrapolation evaluator.
- [ ] Compile convex epigraph and concave hypograph with zero binary variables.
- [ ] Compile exact graph through qualified native PWL, SOS2, or deterministic exact segment binaries.
- [ ] Verify random fixed-input output for all curvature classes.
- [ ] Report curvature, relation, representation, generated counts, and binary-avoidance reason.
- [ ] Commit as `feat(model): add piecewise linear functions` and request OR review.

Verification:

```bash
cargo test -p roml --test piecewise_linear
cargo test -p roml-highs --test formulation_equivalence pwl
cargo test -p roml --all-targets
cargo test -p roml-highs --all-targets
```

## Task 19 — Integrate construct-aware diagnostics and public workflows

**Phase:** P34 integration  
**Files:** modify IIS/origin tests and user docs; create solution-lock/IIS/soft/lexicographic/construct/PWL examples plus NLP boundary document.

- [ ] Add indicator, soft-bound, and PWL conflict fixtures mapping to original constructs/roles.
- [ ] Write compiled public examples using only golden-path APIs.
- [ ] Document semantic guarantee versus native support versus bridge support versus version limit.
- [ ] Document exact additive extension exercise for quadratic and nonlinear scalar functions.
- [ ] Run examples, doctests, and rustdoc with warnings denied.
- [ ] Commit as `docs(m3): complete semantic modeling workflows`.

Verification:

```bash
cargo test -p roml-highs --examples
cargo test -p roml --doc
cargo test -p roml-highs --doc
RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps
RUSTDOCFLAGS='-D warnings' cargo doc -p roml-highs --no-deps
```

## Task 20 — Qualify M3 and close state

**Phase:** P34  
**Files:** create property/native-portable/benchmark/evidence files; modify M3 state/traceability/README and the actual current CI workflows discovered from repository state.

- [ ] Build deterministic native/portable corpus covering every M3 semantic workflow.
- [ ] Run fixed-seed fixing, bound, bridge, origin, delta/rebuild, exact-compilation-ID, and overlay-leak property suites.
- [ ] Enforce primitive parameter-update median overhead below 5% or 50 microseconds per solve attempt, whichever is larger.
- [ ] Run full fmt/check/clippy/test/doc/deny/machete/audit/public-api/package matrix.
- [ ] Compile and run fresh packed consumers for every golden workflow.
- [ ] Complete principal engineering, OR formulation, native/unsafe, and NLP-readiness reviews; close all P0/P1 findings.
- [ ] Close every SM requirement with evidence and exact final SHA.
- [ ] Stop before publication.

Full qualification commands:

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

---

## Task-level evidence and commit rule

Every task follows:

1. write failing semantic/characterization test;
2. run and record expected failure;
3. implement the smallest complete behavior;
4. run focused tests;
5. run phase gate;
6. update evidence and traceability;
7. commit one coherent unit;
8. request independent review at phase boundary.

Every phase PR includes:

```text
Requirements: SM-xx.y
Phase: Pnn
Base/head SHA
Focused checks and results
Full checks and results
Backend/version matrix
Public API diff
Skipped checks with reason
Residual risks
Evidence path
```

## Plan self-review result

- SM-01 through SM-15 map to phases/tasks.
- Canonical and compiler boundaries land before user constructs.
- Lineage, instance, and exact compilation identities have distinct authority.
- Backend snapshot includes objective policy.
- Backend deltas include explicit coefficient removal and exact compilation transition.
- No task introduces backend state into `Model`.
- No bridge may use arbitrary Big-M.
- Overlay rollback/rebuild semantics are explicit.
- IIS reports require exact scope/minimality/version/compilation identity.
- Soft penalty sense and signed correction are tested first.
- Lexicographic formulas cover positive, zero, and negative objectives.
- Exact and one-sided min/max/PWL relations remain distinct.
- Convex PWL epigraph/concave hypograph require zero-binary evidence.
- NLP readiness is an extension exercise, not hidden NLP implementation.
- P34 includes public API, package, fresh-consumer, performance, and independent review evidence.

## Execution handoff

Use subagent-driven development with one fresh implementation agent per task or tightly coupled task group and two-stage review after each phase. Use isolated worktrees and do not exceed M3 WIP limits.