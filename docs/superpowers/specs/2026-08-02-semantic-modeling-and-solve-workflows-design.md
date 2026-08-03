# ROML Semantic Modeling and Solve Workflows Design

**Date:** 2026-08-02  
**Status:** Approved architecture; implementation not started  
**Target milestone:** M3 — Semantic Modeling and Solve Workflows  
**Baseline:** `main@d1f1ad38cec75abb671729df8efb87736861628c`

## 1. Objective

Extend ROML from an ergonomic incremental linear/MILP modeling library into a semantic optimization modeling kernel that:

- preserves high-level mathematical intent in the canonical model;
- compiles that intent into backend-native or portable formulations;
- supports persistent variable fixing and solve-scoped solution locking;
- distinguishes partial assignments, MIP starts, variable hints, and hard fixings;
- supports IIS/conflict analysis with reports in original model terms;
- makes constraint relaxation and penalty modeling straightforward;
- supports lexicographic objectives;
- provides common MILP modeling constructs without requiring users to manually reproduce standard formulations;
- retains the existing incremental/recoverable solver-session guarantees; and
- creates an explicit extension seam for quadratic and nonlinear programming without implementing NLP in M3.

The design is MILP-first, not MILP-only.

## 2. Binding constraints

1. The canonical `Model` remains solver-independent.
2. Solver options and solve-scoped instructions do not become canonical model state.
3. High-level constructs remain canonical model entities; they are not eagerly erased into rows and auxiliary variables.
4. Unsupported features are rejected or explicitly bridged; they are never silently ignored.
5. Exact semantics are never silently replaced by relaxations.
6. Big-M values are never invented without a proven finite bound or an explicit user value.
7. Generated rows and variables retain origin information back to a user entity or construct.
8. A failed solve cannot leave temporary fixings, objective locks, or other solve overlays in a persistent backend session.
9. Incremental projection and rebuild remain observationally equivalent for every supported compiled representation.
10. Rust 1.85 remains the MSRV unless changed by a separate owner-approved decision.
11. `roml-highs` is the reference backend. MOSEK and Xpress remain independent qualification tracks.
12. No publication, tag, or release is part of M3.

## 3. Architectural decision

ROML will have two solver-neutral intermediate representations.

```text
User API
  -> Canonical semantic model IR
  -> capability-aware compiler and bridge planner
  -> backend IR
  -> persistent backend session
  -> backend result
  -> origin-aware normalized ROML result
```

### 3.1 Canonical semantic model IR

The canonical model stores what the modeler declared:

- variables and declared domains;
- persistent fixings as domain restrictions with provenance;
- parameters and parameter-dependent scalar expressions;
- primitive function-in-set constraints;
- objectives and objective policies;
- semantic constructs such as indicators, min/max, absolute value, PWL, Boolean/cardinality constraints, products, and soft constraints;
- entity names, groups, tags, descriptions, and sources;
- revisions and canonical change batches.

It does not store solver indices, chosen Big-M values, backend-native handles, presolve artifacts, or temporary solve locks.

### 3.2 Backend IR

The compiler produces a backend-targeted but still solver-neutral representation:

- compiled variables and domains;
- linear rows;
- normalized native primitives such as indicator, SOS1, SOS2, and PWL graph constraints;
- objective stages;
- generated auxiliary entities;
- origin maps;
- formulation decisions and evidence;
- a deterministic representation fingerprint.

Backends translate this bounded backend IR into their native APIs. A backend does not receive the full mutable `Model`.

### 3.3 Why two IRs

A single eager linearized representation would lose semantic intent, prevent backend-specific formulations, and degrade IIS reports. A single universal backend abstraction would either expose solver details to the model or grow into a false least-common-denominator API. Two IRs preserve intent while keeping native translation bounded.

## 4. Function and set model

ROML should adopt a constrained function-in-set core rather than making every construct a bespoke row type.

```rust
#[non_exhaustive]
pub enum ScalarFunction {
    Linear(LinExpr),
}

#[non_exhaustive]
pub enum ScalarSet {
    LessEqual(ValueExpr),
    GreaterEqual(ValueExpr),
    EqualTo(ValueExpr),
    Interval { lower: ValueExpr, upper: ValueExpr },
}

pub struct FunctionConstraint {
    pub function: ScalarFunction,
    pub set: ScalarSet,
}
```

M3 implements only `ScalarFunction::Linear`. Future milestones may add:

- `Quadratic(QuadExpr)`;
- `Nonlinear(NonlinearExpr)`;
- vector functions and conic sets;
- complementarity sets.

The existing `LinExpr` operators and `.le()`, `.ge()`, `.eq()`, and `.between()` remain the ordinary API and lower directly into this representation. Users do not need to construct `ScalarFunction` manually.

`#[non_exhaustive]` communicates that function and set families will grow. Backend and compiler code must match conservatively.

## 5. Identity, lineage, and provenance

### 5.1 Model lineage

Reusable assignments require model identity in addition to generational entity IDs.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ModelLineageId(u64);
```

Rules:

- `Model::new()` and `Model::named()` allocate a new process-unique lineage.
- `Clone` preserves lineage so scenario branches can reuse assignments where entity handles remain valid.
- removed/recreated entities remain protected by current generation checks;
- assignments from unrelated lineages return `AssignmentModelMismatch`;
- M3 does not define cross-process serialized lineage identity.

### 5.2 Entity metadata

```rust
pub struct EntityMetadata {
    pub name: Option<String>,
    pub description: Option<String>,
    pub group: Option<String>,
    pub tags: Vec<String>,
    pub source: Option<ModelSource>,
}
```

`ModelSource` is structured user metadata, not Rust source-code introspection:

```rust
pub struct ModelSource {
    pub module: Option<String>,
    pub file: Option<String>,
    pub line: Option<u32>,
    pub external_key: Option<String>,
}
```

### 5.3 Generated-entity provenance

Compiler-generated entities use distinct IDs:

```rust
pub struct CompiledVariableId(u32);
pub struct CompiledConstraintId(u32);

pub enum EntityOrigin {
    UserVariable(Variable),
    UserConstraint(Constraint),
    UserObjective(Objective),
    Construct { id: Construct, role: GeneratedRole },
    SolveOverlay { id: OverlayId, role: GeneratedRole },
}
```

Generated entities never masquerade as user `Variable` or `Constraint` handles.

## 6. Semantic constructs

```rust
pub type Construct = ConstructId;

#[non_exhaustive]
pub enum ConstructKind {
    Indicator(IndicatorConstraint),
    Reification(ReifiedConstraint),
    MinMax(MinMaxConstraint),
    AbsoluteValue(AbsoluteValueConstraint),
    Boolean(BooleanConstraint),
    Cardinality(CardinalityConstraint),
    BinaryProduct(BinaryProductConstraint),
    PiecewiseLinear(PiecewiseLinearConstraint),
    SoftConstraint(SoftConstraintDefinition),
}
```

Constructs have stable IDs, metadata, activity, parameter dependencies, and explicit exactness semantics. The compiler chooses a representation but cannot change the declared meaning.

## 7. Persistent fixing

### 7.1 Representation

A variable stores its declared domain separately from an optional fixing.

```rust
pub struct VariableState {
    pub declared_domain: VariableDomain,
    pub fixing: Option<VariableFixing>,
}

pub struct VariableFixing {
    pub value: f64,
    pub provenance: FixingProvenance,
}
```

The effective domain is the intersection of the declared domain and the fixing.

### 7.2 API

```rust
impl Model {
    pub fn fix(&mut self, variable: Variable, value: f64) -> Result<(), ModelError>;
    pub fn unfix(&mut self, variable: Variable) -> Result<(), ModelError>;
    pub fn fixing(&self, variable: Variable) -> Result<Option<&VariableFixing>, ModelError>;
    pub fn declared_bounds(&self, variable: Variable) -> Result<Bounds, ModelError>;
    pub fn effective_bounds(&self, variable: Variable) -> Result<Bounds, ModelError>;
}
```

`fix()` is a first-class canonical mutation, not merely an undocumented call to `set_variable_bounds()`.

### 7.3 Solver representation

Default compilation uses bound tightening:

```text
fix x = v  =>  lower(x) = upper(x) = v
```

An equality-row API remains separate:

```rust
model.add_constraint(x.eq(4.0).named("fixed_dispatch"))?;
```

This is chosen only when the modeler explicitly wants a row.

### 7.4 Validation

- values must be finite and inside declared bounds;
- integer values must be integral within the model integrality tolerance and are normalized;
- binary values must normalize to `0` or `1`;
- changing declared bounds while fixed is allowed only if the new bounds contain the fixed value;
- `unfix()` restores the current declared domain;
- IIS provenance distinguishes declared bounds from persistent fixings.

## 8. Primal assignments, starts, hints, and locks

### 8.1 Shared value container

```rust
pub struct PrimalAssignment {
    lineage: ModelLineageId,
    source_revision: Option<ModelRevision>,
    values: BTreeMap<Variable, f64>,
}

impl PrimalAssignment {
    pub fn new(model: &Model) -> Self;
    pub fn set(mut self, variable: Variable, value: f64) -> Result<Self, AssignmentError>;
    pub fn remove(mut self, variable: Variable) -> Self;
    pub fn value(&self, variable: Variable) -> Option<f64>;
    pub fn iter(&self) -> impl Iterator<Item = (Variable, f64)> + '_;
}
```

A `PrimalAssignment` makes no feasibility or optimality claim.

### 8.2 Distinct semantics

- **Persistent fixing:** canonical feasible-region restriction stored in `Model`.
- **Temporary fixing / solution lock:** solve-scoped feasible-region restriction.
- **MIP start:** coherent candidate incumbent, possibly partial and repairable.
- **Variable hint:** independent search guidance, not necessarily jointly feasible.
- **LP basis:** separate future warm-start artifact; not represented as a primal assignment.

ROML never silently converts one category into another.

### 8.3 Solution lock

```rust
pub struct SolutionLock {
    pub assignment: PrimalAssignment,
    pub selector: LockSelector,
    pub continuous: ContinuousLock,
}

pub enum LockSelector {
    AllAssigned,
    IntegerAssigned,
    BinaryAssigned,
    Variables(BTreeSet<Variable>),
    Except(BTreeSet<Variable>),
}

pub enum ContinuousLock {
    Exact,
    Within { absolute: f64 },
}
```

The default convenience is `SolutionLock::integer_variables(solution.primal_assignment())`.

### 8.4 Hints

```rust
pub struct VariableHints {
    entries: BTreeMap<Variable, VariableHint>,
}

pub struct VariableHint {
    pub value: f64,
    pub priority: HintPriority,
}
```

Hints are passed only to backends with native hint support unless an explicit user policy requests a conversion.

## 9. SolvePlan and reversible overlays

### 9.1 API

```rust
pub struct SolvePlan {
    pub options: SolveOptions,
    pub overlay: SolveOverlay,
    pub mip_starts: Vec<MipStart>,
    pub hints: VariableHints,
    pub objective_override: Option<ObjectivePolicy>,
    pub unsupported: UnsupportedFeaturePolicy,
}

impl SolvePlan {
    pub fn new() -> Self;
    pub fn options(self, options: SolveOptions) -> Self;
    pub fn lock(self, lock: SolutionLock) -> Self;
    pub fn fix(self, assignment: PrimalAssignment) -> Self;
    pub fn mip_start(self, start: MipStart) -> Self;
    pub fn hints(self, hints: VariableHints) -> Self;
    pub fn objectives(self, policy: ObjectivePolicy) -> Self;
}
```

Backends expose:

```rust
pub fn solve_plan(
    &mut self,
    model: &mut Model,
    plan: SolvePlan,
) -> Result<Solution, SolveError>;
```

Existing `solve()` and `solve_with()` construct simple plans.

### 9.2 Overlay lifecycle

```text
commit canonical model
-> compile/synchronize canonical revision
-> validate and compile SolvePlan
-> apply overlay
-> apply starts/hints
-> execute objective stages
-> extract results
-> remove overlay
-> verify backend canonical state
```

If overlay rollback fails or cannot be proven, the backend session becomes `RequiresRebuild`. The canonical model and journal are unchanged.

### 9.3 Effective plan report

Every solution records what happened:

```rust
pub struct EffectiveSolvePlan {
    pub applied_features: Vec<AppliedFeature>,
    pub adjustments: Vec<PlanAdjustment>,
    pub rejections: Vec<PlanRejection>,
    pub objective_stages: Vec<ObjectiveStageResult>,
}
```

## 10. Compiler and bridge system

### 10.1 Compiler session

`SolverSession<B>` owns a compiler session parameterized by backend capabilities and compilation policy.

```rust
pub struct CompilationSession {
    capabilities: BackendCapabilities,
    policy: CompilationPolicy,
    compiled_revision: ModelRevision,
    fingerprint: Option<CompilationFingerprint>,
}
```

### 10.2 Policy

```rust
pub enum CompilationPolicy {
    Auto,
    Portable,
    NativeRequired,
}
```

- `Auto`: prefer a validated native primitive, otherwise use a portable exact bridge.
- `Portable`: use deterministic ROML formulations suitable for cross-solver research.
- `NativeRequired`: reject any construct without native backend support.

Per-construct overrides may request a specific validated formulation.

### 10.3 Backend IR

```rust
pub struct BackendSnapshot {
    pub revision: ModelRevision,
    pub variables: Vec<CompiledVariable>,
    pub linear_rows: Vec<CompiledLinearRow>,
    pub native_constraints: Vec<BackendConstraint>,
    pub objectives: Vec<CompiledObjective>,
    pub origin_map: OriginMap,
    pub report: CompilationReport,
}

#[non_exhaustive]
pub enum BackendConstraint {
    Indicator(CompiledIndicator),
    Sos1(CompiledSos),
    Sos2(CompiledSos),
    PiecewiseLinear(CompiledPiecewiseLinear),
}
```

Future extensions may add quadratic rows/objectives, cones, and nonlinear evaluation graphs.

### 10.4 Backend synchronization contract

M3 intentionally amends the M2 backend contract:

```rust
pub enum BackendSynchronization {
    Delta(BackendDeltaBatch),
    Rebuild(BackendSnapshot),
}
```

Canonical `DeltaBatch` remains a model-level artifact. Backend sessions consume compiled deltas, not canonical semantic changes.

For M3 v1:

- primitive linear changes retain incremental compilation;
- parameter changes retain incremental compilation when the selected recipe is unchanged;
- adding/removing/changing a semantic construct may force a deterministic rebuild;
- incremental semantic relowering is introduced only after equivalence tests exist.

### 10.5 Capability registry

The current flat Boolean capability struct is replaced by typed feature support.

```rust
#[non_exhaustive]
pub enum BackendFeature {
    Lp,
    Mip,
    IncrementalBounds,
    IncrementalRows,
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

pub struct FeatureSupport {
    pub level: SupportLevel,
    pub limitations: FeatureLimitations,
}

pub enum SupportLevel {
    Native,
    Unsupported,
}
```

Bridge support is reported separately in `CompilationReport`.

## 11. Bound analysis and formulation safety

```rust
pub struct BoundAnalysis;

impl BoundAnalysis {
    pub fn scalar_bounds(
        model: &ModelSnapshot,
        function: &ScalarFunction,
    ) -> Result<Interval, BoundAnalysisError>;
}
```

M3 starts with deterministic interval propagation over declared/effective variable bounds and parameter values.

Rules:

- a Big-M bridge requires a finite proven value or an explicit user value;
- explicit M values are validated against available bounds when possible;
- the compilation report records the value and its derivation;
- failure returns `CompileError::UnboundedBigM` with the construct and missing bound source;
- M3 does not perform implicit auxiliary LP bound tightening;
- LP-based tightening is a later optional optimization.

## 12. IIS and conflict diagnostics

### 12.1 Optional backend trait

```rust
pub trait InfeasibilityAnalysisSession {
    fn analyze_infeasibility(
        &mut self,
        request: &InfeasibilityRequest,
    ) -> Result<BackendConflict, BackendError>;
}
```

This is not added to the required `BackendSession` trait.

### 12.2 User API

```rust
pub fn analyze_infeasibility(
    &mut self,
    model: &mut Model,
    options: IisOptions,
) -> Result<InfeasibilityReport, AnalysisError>;
```

The analysis runs against the exact compiled representation used for the model revision, then maps results through `OriginMap`.

### 12.3 Report

```rust
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
```

Conflict members distinguish:

- user constraint lower/upper/equality sides;
- declared variable lower/upper bounds;
- persistent fixings;
- solve-overlay locks;
- semantic constructs and generated roles.

The default report renderer produces concise text and Markdown. Structured output is available without requiring serialization; optional Serde support may be feature-gated later.

### 12.4 HiGHS gate

Implementation must audit the pinned bundled and minimum supported system HiGHS C headers. If the required IIS interface is not present in the currently pinned binding, the phase must either:

1. upgrade to the first qualified HiGHS/highs-sys version exposing the required official interface; or
2. expose typed `Unsupported` for versions without it.

A home-grown deletion filter is not presented as native IIS in M3.

## 13. Soft constraints and constraint slacks

### 13.1 API

```rust
let slack = model.soften(
    capacity,
    soft()
        .upper_side()
        .max_violation(20.0)
        .penalty(1_000.0)
        .on(cost_objective)
        .named("capacity_overage"),
)?;
```

```rust
pub struct SoftConstraintHandle {
    pub constraint: Constraint,
    pub lower_violation: Option<Variable>,
    pub upper_violation: Option<Variable>,
    pub construct: Construct,
}
```

The returned violation variables are canonical auxiliary variables with stable `Variable` handles and generated provenance. They may be inspected and used in later expressions. The high-level soft-constraint construct remains canonical and owns their semantics.

### 13.2 Semantics

For `a(x) <= u`, upper relaxation is:

```text
a(x) <= u + s_upper
0 <= s_upper <= max_upper
```

For `a(x) >= l`, lower relaxation is:

```text
a(x) + s_lower >= l
0 <= s_lower <= max_lower
```

Equality and ranged constraints use separate nonnegative lower and upper violation variables unless the user explicitly requests a signed correction.

### 13.3 Signed correction

Signed correction is a separate builder:

```rust
model.add_signed_correction(constraint, correction().bounds(-10.0, 10.0))?;
```

An L1 penalty is represented by positive/negative parts. ROML never linearly penalizes a free signed variable without explicit semantics.

### 13.4 Penalties

```rust
pub enum PenaltyTarget {
    Objective(Objective),
    LexicographicPriority(i32),
    None,
}
```

Weights may be parameter-dependent finite `ValueExpr`s. Minimize/maximize sign handling is internal and recorded.

### 13.5 Feasibility relaxation

Backend-native feasibility relaxation is a separate solve/analysis API. It does not mutate the canonical model and is not conflated with persistent soft constraints.

## 14. Objective policies and lexicographic solving

Objective expressions are canonical model state. The policy selecting and ordering objectives is also mathematical intent and may be stored in the model.

```rust
pub enum ObjectivePolicy {
    Single(Objective),
    Weighted(Vec<WeightedObjective>),
    Lexicographic(Vec<ObjectiveLevel>),
}

pub struct ObjectiveLevel {
    pub objective: Objective,
    pub absolute_tolerance: f64,
    pub relative_tolerance: f64,
}
```

API:

```rust
model.set_objective_policy(
    lexicographic()
        .first(service_violation)
        .then(total_cost, degradation().relative(1e-3))
        .then(switches, degradation().absolute(0.0)),
)?;
```

Execution:

- use native multiobjective support when the policy and backend semantics match;
- otherwise perform sequential solves with temporary objective-lock rows in the solve overlay;
- default stage policy requires an optimal result before descending to the next priority;
- an explicit `UseBestFeasible` mode permits continuation and records the qualification;
- solution metadata includes every stage result and every objective value.

## 15. Modeling constructs v1

### 15.1 Indicators

```rust
model.add_indicator(on, when_one(), production.le(100.0))?;
```

Compilation order under `Auto`:

1. native indicator;
2. exact SOS bridge where supported and selected;
3. bound-derived Big-M bridge;
4. typed compile error.

### 15.2 Reification and threshold detection

```rust
let high = model.reify(
    expression.ge(100.0),
    iff().separation(1e-6),
)?;
```

Continuous exact reification requires an explicit separation/tolerance. Integral expressions may infer a unit separation only when integrality is proven.

### 15.3 Min, max, absolute value, positive part, clamp

```rust
let peak = model.max_of([x, y, z], exact())?;
let epigraph = model.max_of([x, y, z], epigraph())?;
let deviation = model.abs(actual - target, exact())?;
let shortage = model.positive_part(demand - supply)?;
let clipped = model.clamp(x, lower, upper)?;
```

Exact equality and epigraph/hypograph semantics are separate. ROML does not infer exactness from objective context in M3.

### 15.4 Boolean and cardinality

```rust
model.implies(a, b)?;
model.iff(a, b)?;
model.any_of(flags)?;
model.all_of(flags)?;
model.exactly_one(flags)?;
model.at_most(k, flags)?;
model.at_least(k, flags)?;
```

### 15.5 Products

```rust
let y = model.product(binary_var, bounded_scalar_function)?;
```

M3 exact product support is limited to binary times bounded linear scalar function and binary times binary. Continuous-times-continuous is exposed only as an explicitly named relaxation in a later milestone.

### 15.6 Disjunctions

The internal construct model must permit future disjunctions, but a general disjunctive DSL is not part of M3 v1. Indicators and Boolean composition cover the immediate use cases.

## 16. Piecewise-linear functions

```rust
let cost = model.piecewise_linear(
    x,
    points,
    pwl()
        .epigraph()
        .extrapolation(Extrapolation::Reject)
        .named("production_cost"),
)?;
```

Validation:

- finite points;
- strictly increasing breakpoints;
- explicit extrapolation policy;
- continuity where required;
- convexity/concavity classified from segment slopes.

Representations:

- convex epigraph: supporting linear inequalities, no binaries;
- concave hypograph: supporting linear inequalities, no binaries;
- exact graph: native PWL, SOS2, or exact disjunctive formulation;
- nonconvex graph: native PWL, SOS2, or binary formulation;
- requested convex/concave relation inconsistent with points: validation error.

The compilation report states why binaries were or were not introduced.

## 17. Incremental semantics

### 17.1 Canonical changes

Add explicit canonical changes for:

- fixing set/removed;
- construct add/remove/update/activity;
- metadata changes that affect diagnostics only;
- objective-policy changes;
- soft-constraint attachment/removal;
- auxiliary-variable ownership.

### 17.2 Compilation invalidation

Each compiled recipe declares dependencies:

```rust
pub struct CompilationRecipe {
    pub construct: Construct,
    pub dependencies: RecipeDependencies,
    pub representation: RepresentationKind,
    pub fingerprint: RecipeFingerprint,
}
```

For M3:

- pure metadata changes do not resynchronize solver state;
- direct linear coefficient/bound/parameter changes use existing incremental paths;
- a fixing becomes an incremental effective-bound update where supported;
- solve overlays are not canonical revisions;
- semantic construct changes may force rebuild;
- parameter updates inside a stable bridge may emit compiled cell/bound deltas only after differential tests prove equivalence;
- recipe changes always force rebuild.

## 18. Error taxonomy

```rust
#[non_exhaustive]
pub enum ModelError {
    // existing variants
    AssignmentModelMismatch,
    FixedValueOutsideBounds,
    NonIntegralFixing,
    ConstructNotFound(Construct),
    InvalidConstruct(String),
    InvalidPiecewiseLinear(String),
    InvalidPenalty(String),
}

#[non_exhaustive]
pub enum CompileError {
    UnsupportedConstruct { construct: Construct, backend: String },
    NativeFeatureRequired { feature: BackendFeature },
    UnboundedBigM { construct: Construct, expression: String },
    InvalidExplicitBigM { construct: Construct, value: f64 },
    RelaxationWouldChangeSemantics { construct: Construct },
    OriginMappingFailure,
}

#[non_exhaustive]
pub enum PlanError {
    Assignment(AssignmentError),
    UnsupportedFeature(BackendFeature),
    OverlayConflict(String),
    InvalidObjectivePolicy(String),
}

#[non_exhaustive]
pub enum AnalysisError {
    ModelNotInfeasible,
    Unsupported(BackendFeature),
    StaleCompilation,
    Backend(BackendError),
    Mapping(String),
}
```

Errors identify the user entity/construct, selected representation, backend, and violated invariant.

## 19. Module layout

### Core canonical model

```text
src/
  identity.rs
  metadata.rs
  function/
    mod.rs
    scalar.rs
    set.rs
  construct/
    mod.rs
    indicator.rs
    reification.rs
    minmax.rs
    absolute.rs
    boolean.rs
    cardinality.rs
    product.rs
    piecewise_linear.rs
    soft.rs
  assignment.rs
  objective_policy.rs
```

### Compilation

```text
src/compiler/
  mod.rs
  session.rs
  capability.rs
  backend_ir.rs
  origin.rs
  report.rs
  bounds.rs
  bridge/
    mod.rs
    indicator.rs
    minmax.rs
    absolute.rs
    boolean.rs
    product.rs
    piecewise_linear.rs
    soft.rs
```

### Solve orchestration and analysis

```text
src/solver/
  plan.rs
  overlay.rs
  effective_plan.rs
  infeasibility.rs
  multiobjective.rs
```

Backends add focused modules for backend-IR translation, starts/hints, IIS, and native multiobjective support.

## 20. Testing strategy

### 20.1 Semantic tests

- construct validation and exactness;
- fixing/unfixing and declared/effective bounds;
- assignment lineage and stale-entity rejection;
- soft-constraint algebra;
- objective-policy validation;
- PWL convexity classification.

### 20.2 Compiler tests

For every construct and representation:

- deterministic compilation;
- native versus bridge semantic equivalence;
- generated-origin completeness;
- Big-M derivation correctness;
- no-binary convex PWL qualification;
- rebuild versus compiled incremental equivalence.

### 20.3 Solver tests

- overlay apply/rollback under success and injected failure;
- locks alter feasibility; starts/hints do not;
- unsupported features are reported;
- lexicographic native and sequential paths agree;
- IIS maps generated rows back to original constructs;
- soft constraints report violation values.

### 20.4 Property/differential tests

- random bounded indicator/min/max/abs instances compared to explicit reference formulations;
- random PWL points classified and compared to direct evaluation;
- random fix/unfix sequences preserve domain invariants;
- compiled delta sequence equals compiled rebuild;
- native and portable policies produce equivalent optimal values on qualified small models.

### 20.5 Qualification evidence

Each phase records:

- exact base/head SHA;
- focused and full commands;
- capability/version matrix;
- skipped checks;
- public API diff;
- package contents;
- independent review findings;
- residual risks.

## 21. Migration and compatibility

M3 is allowed to make reviewed pre-1.0 backend-contract changes because canonical semantic compilation cannot be represented cleanly through the current `ModelSnapshot`-direct session boundary.

Migration rules:

- ordinary M2 user code remains source-compatible unless a specific contradiction is proven;
- `solve()` and `solve_with()` remain;
- new functionality enters through additive APIs first;
- backend authors receive a migration guide from canonical `Synchronization` to `BackendSynchronization`;
- deprecated advanced protocol aliases remain for one documented compatibility window where mechanically possible;
- no eager public exposure of internal compiler stores.

## 22. NLP readiness boundary

M3 does not implement nonlinear expression tracing, automatic differentiation, Hessians, nonlinear callbacks, cones, or NLP solvers.

M3 must nevertheless avoid these dead ends:

- treating every constraint as a linear row;
- making `LinExpr` the permanent universal function type;
- making backend IR synonymous with sparse matrices;
- assuming all generated entities are linearization artifacts;
- assuming a solve has one objective pass;
- assuming infeasibility analysis always returns row IDs only;
- embedding Big-M policy into construct definitions.

A post-M3 NLP milestone should be able to add `ScalarFunction::Quadratic` and `ScalarFunction::Nonlinear`, extend backend capabilities/IR, and reuse identity, metadata, objective policies, solve plans, origin mapping, diagnostics, and compilation reporting.

## 23. Acceptance criteria

M3 is complete only when:

1. high-level constructs survive in canonical snapshots and revisions;
2. backend sessions consume compiled backend IR;
3. user variables, generated variables, and overlay entities have unambiguous identity and origin;
4. persistent fixing, temporary locking, MIP starts, and variable hints are semantically distinct and tested;
5. overlays are proven reversible or force backend rebuild;
6. HiGHS exposes qualified MIP-start behavior and version-aware IIS support;
7. IIS reports refer to original ROML names, sides, bounds, fixings, and constructs;
8. soft constraints support bounded nonnegative violations and explicit penalties;
9. lexicographic solving works through native or portable sequential execution;
10. indicators, Boolean/cardinality constructs, min/max, absolute value, supported products, and PWL have exact validated formulations;
11. convex PWL epigraph and concave PWL hypograph introduce no binaries;
12. every bridge has origin mapping and a formulation report;
13. no Big-M is generated without finite proof or explicit user input;
14. primitive incremental behavior remains qualified;
15. package, docs, public API, and fresh-consumer checks pass;
16. an NLP-readiness audit finds no linear-only architectural dead end.

## 24. Approved decisions

- Preserve high-level semantic constructs in the canonical model.
- Use bound tightening as the default implementation of variable fixing.
- Keep persistent model mutations separate from solve-scoped overlays.
- Use a canonical semantic IR plus a backend IR.
- Adopt a function-in-set seam with linear functions implemented first.
- Require typed capabilities and explicit bridge reporting.
- Make origin mapping mandatory.
- Keep IIS and feasibility relaxation separate.
- Store objective policy as mathematical model intent, with solve-time override support.
- Defer NLP implementation while preserving the extension boundary described above.
