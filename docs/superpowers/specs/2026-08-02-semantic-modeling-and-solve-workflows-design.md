# ROML Semantic Modeling and Solve Workflows Design

**Date:** 2026-08-02  
**Status:** Approved architecture; implementation not started  
**Target:** M3 — Semantic Modeling and Solve Workflows  
**Baseline:** `main@d1f1ad38cec75abb671729df8efb87736861628c`

## 1. Objective

Extend ROML from an incremental linear/MILP modeling library into a semantic optimization modeling kernel that:

- preserves high-level mathematical intent in canonical model state;
- compiles that intent into backend-native or portable exact formulations;
- supports persistent fixing and solve-scoped solution reuse;
- distinguishes partial assignments, MIP starts, variable hints, and hard locks;
- supports IIS/conflict analysis with reports in original model terms;
- makes soft constraints and violation penalties ordinary modeling operations;
- supports weighted and lexicographic objectives;
- provides a bounded set of common MILP constructs;
- preserves incremental synchronization and bounded recovery; and
- permits later quadratic and nonlinear functions to extend, rather than replace, the architecture.

The design is **MILP-first, not MILP-only**. M3 does not implement NLP.

## 2. Binding invariants

1. `Model` remains solver-independent.
2. High-level constructs remain canonical entities and are not eagerly erased into rows.
3. Backend indices, native handles, selected Big-M values, and solve overlays never enter canonical state.
4. Unsupported features are rejected or exactly bridged; they are never silently ignored.
5. Exact constructs are never silently compiled as relaxations.
6. Big-M requires a finite proof or an explicit validated user value.
7. Every generated entity has an origin.
8. Solve-scoped overlays never advance canonical revision.
9. Failed overlay rollback forces backend rebuild before reuse.
10. Primitive compiled deltas and full compiled rebuilds are observationally equivalent.
11. Runtime/backend version limitations are explicit capabilities.
12. No publication, tag, or release is part of M3.

## 3. System architecture

ROML has two solver-neutral IR layers.

```text
Fluent user API
    -> canonical semantic model IR
    -> capability-aware compiler and bridge planner
    -> backend IR
    -> persistent backend session
    -> backend result
    -> origin-aware ROML solution/report
```

### 3.1 Canonical semantic model IR

Canonical state owns:

- user variables and declared domains;
- persistent fixings and fixing provenance;
- parameters and parameter-dependent values;
- scalar functions and sets;
- semantic constructs;
- objectives and objective policy;
- stable user entity IDs;
- names and user metadata;
- revisions and canonical changes.

It stores **what the modeler declared**, not how a particular solver represents it.

### 3.2 Backend IR

The compiler produces:

- dense compiled IDs distinct from user IDs;
- compiled variables, rows, objectives, and objective policy;
- normalized native primitives such as indicator, SOS1, SOS2, and PWL;
- generated auxiliaries;
- mandatory origin maps;
- formulation decisions and bound evidence;
- an exact opaque compilation identity;
- optional deterministic recipe/digest evidence for testing and caching.

Backends translate backend IR into official native APIs. They do not inspect mutable `Model` internals.

### 3.3 Why two IRs

Eager expansion would destroy semantic intent and prevent backend-aware formulation selection. Passing the full semantic model to every backend would couple backends to model internals and turn the backend API into an unbounded abstraction. Two IRs preserve intent while keeping solver integration bounded.

## 4. Identity model

Three identities serve different purposes.

```rust
pub struct ModelLineageId(u64);
pub struct ModelInstanceId(u64);
pub struct CompilationId(u64);
```

### 4.1 Model lineage

- New independent models receive distinct lineages.
- `Clone` preserves lineage.
- Assignments may be reused across clones in the same lineage when entity generations are still valid.
- Lineage does not claim that two clones have equal current state.

### 4.2 Model instance

- Every live `Model` object has a distinct `ModelInstanceId`.
- `Clone` allocates a new instance ID while preserving lineage and entity handles.
- Canonical state identity is `(ModelInstanceId, ModelRevision)`.
- Divergent clones with the same revision number are therefore never confused.

### 4.3 Compilation identity

- Every compiled backend artifact receives an opaque exact `CompilationId`.
- Backend results, overlays, conflict data, and origin maps carry this ID.
- Stale-state safety compares exact IDs, not hashes.
- A deterministic recipe fingerprint or representation digest may support cache/debug/test evidence but is never authoritative for correctness.

### 4.4 Entity identity

User entities retain current generation-safe handles. Compiler and overlay entities use separate IDs:

```rust
pub struct CompiledVariableId(u32);
pub struct CompiledConstraintId(u32);
pub struct CompiledObjectiveId(u32);
pub struct OverlayId(u64);
```

Generated entities never masquerade as user variables or constraints.

## 5. Metadata and provenance

Names remain the existing first-class entity name fields. Additional metadata is separate:

```rust
pub struct EntityMetadata {
    pub description: Option<String>,
    pub group: Option<String>,
    pub tags: Vec<String>,
    pub source: Option<ModelSource>,
}

pub struct ModelSource {
    pub module: Option<String>,
    pub file: Option<String>,
    pub line: Option<u32>,
    pub external_key: Option<String>,
}
```

Compiler origins are mandatory:

```rust
pub enum EntityOrigin {
    UserVariable(Variable),
    UserConstraint(Constraint),
    UserObjective(Objective),
    Construct { construct: Construct, role: GeneratedRole },
    SolveOverlay { overlay: OverlayId, role: GeneratedRole },
}
```

Origin mapping supports:

- IIS/conflict reports;
- formulation reports;
- generated-variable inspection;
- solver log interpretation;
- exported model sidecars;
- hiding compiler-only entities from ordinary solution iteration.

## 6. Function-in-set seam

Primitive constraints use a constrained function-in-set core:

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

M3 implements only `ScalarFunction::Linear`. Later milestones may add:

- `Quadratic(QuadExpr)`;
- `Nonlinear(NonlinearExpr)`;
- vector functions and conic sets;
- complementarity sets.

Existing `LinExpr`, `.le()`, `.ge()`, `.eq()`, and `.between()` remain the ordinary API. Users do not need to construct function/set enums directly.

## 7. Canonical semantic constructs

```rust
pub type Construct = ConstructId;

#[non_exhaustive]
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
```

Every construct has:

- stable generation-safe identity;
- activity state;
- metadata;
- referenced-entity validation;
- derived parameter dependencies;
- exact semantic type;
- optional per-construct formulation preference;
- canonical snapshot/delta representation.

One construct arena/store owns lifecycle. ROML does not add a separate side map for every feature.

## 8. Compiler and bridge system

### 8.1 Compilation policy

```rust
pub enum CompilationPolicy {
    Auto,
    Portable,
    NativeRequired,
}
```

- `Auto`: prefer a qualified exact native primitive, otherwise an exact portable bridge.
- `Portable`: force deterministic ROML formulations suitable for solver comparison.
- `NativeRequired`: reject when the backend lacks exact native support.

Per-construct preferences may narrow the global policy but cannot weaken exactness.

### 8.2 Backend capability model

```rust
#[non_exhaustive]
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

pub struct FeatureSupport {
    pub level: SupportLevel,
    pub limitations: FeatureLimitations,
}

pub enum SupportLevel {
    Native,
    Unsupported,
}
```

Native support and ROML bridge support are reported separately. Capability declarations may vary by backend version and model class.

### 8.3 Backend snapshot

```rust
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
}
```

Backend deltas identify exact source and target compilations:

```rust
pub struct BackendDeltaBatch {
    pub from_compilation: CompilationId,
    pub to_compilation: CompilationId,
    pub from_revision: ModelRevision,
    pub to_revision: ModelRevision,
    pub operations: Vec<BackendOp>,
}
```

M3 v1 keeps primitive linear changes incremental. Semantic construct changes may conservatively rebuild until recipe-level incremental equivalence is proven.

### 8.4 Objective policy in backend IR

```rust
pub enum CompiledObjectivePolicy {
    Single(CompiledObjectiveId),
    Weighted(Vec<CompiledWeightedObjective>),
    Lexicographic(Vec<CompiledObjectiveLevel>),
}
```

This prevents the backend snapshot from containing objectives without defining which optimization problem is active.

### 8.5 Bridge contract

Every bridge produces:

- compiled variables/rows/native primitives;
- mandatory origins;
- representation kind;
- parameter/domain dependencies;
- bound and Big-M evidence;
- a deterministic recipe descriptor;
- explicit failure when exact representation is unavailable.

No bridge may depend on an objective becoming tight unless the declared semantic relation is explicitly one-sided.

## 9. Bound analysis and Big-M safety

M3 implements deterministic interval propagation for linear scalar functions.

```rust
pub struct Interval {
    pub lower: f64,
    pub upper: f64,
}

pub struct BoundTrace {
    pub sources: Vec<BoundSource>,
    pub result: Interval,
}
```

Rules:

1. Big-M is derived for the exact one-sided implication being relaxed.
2. Every derived M records variable bounds and arithmetic used.
3. Explicit user M values are validated against known bounds when possible.
4. Missing finite proof returns `CompileError::UnboundedBigM`.
5. M3 does not silently solve auxiliary LPs for tighter bounds.
6. No default constant such as `1e6` exists.

## 10. Persistent fixing

Variable state separates declared domain and fixing:

```rust
pub struct VariableDomain {
    pub bounds: Bounds,
    pub var_type: VarType,
    pub semi: Option<SemiDomain>,
}

pub struct VariableFixing {
    pub value: f64,
    pub provenance: FixingProvenance,
}
```

Public operations:

```rust
model.fix(variable, value)?;
model.unfix(variable)?;
model.declared_bounds(variable)?;
model.effective_bounds(variable)?;
```

Compilation uses bound tightening:

```text
fix x = v  =>  lower(x) = upper(x) = v
```

An equality-row fixing is simply an explicit user constraint and remains distinct.

Validation:

- finite and inside declared bounds;
- integer/binary values normalized within named integrality tolerance;
- declared-bound changes excluding the fixing fail atomically;
- `unfix` restores current declared bounds;
- fixing provenance is visible to diagnostics.

## 11. Assignments, starts, hints, and locks

### 11.1 Primal assignment

```rust
pub struct PrimalAssignment {
    lineage: ModelLineageId,
    source_instance: Option<ModelInstanceId>,
    source_revision: Option<ModelRevision>,
    values: BTreeMap<Variable, f64>,
}
```

It is a partial value map and makes no feasibility or optimality claim.

### 11.2 Distinct semantics

- **Persistent fixing:** canonical feasible-region restriction.
- **Solution lock/temporary fixing:** solve-scoped feasible-region restriction.
- **MIP start:** coherent candidate incumbent, possibly partial and repairable.
- **Variable hint:** independent search guidance.
- **LP basis:** separate future artifact.

ROML never silently converts among them.

### 11.3 Solution lock

```rust
pub struct SolutionLock {
    pub assignment: PrimalAssignment,
    pub selector: LockSelector,
    pub continuous: ContinuousLock,
}
```

Selectors cover all assigned, integer assigned, binary assigned, explicit variables, and exclusions. Continuous locks support exact or absolute-band bounds.

### 11.4 Starts and hints

`MipStart` carries assignment, repair policy, and optional name. `VariableHints` carries independent values and priorities. Unsupported behavior rejects by default. Any conversion is explicit and recorded in effective solve metadata.

## 12. SolvePlan and reversible overlays

```rust
pub struct SolvePlan {
    pub options: SolveOptions,
    pub overlay: SolveOverlay,
    pub mip_starts: Vec<MipStart>,
    pub hints: VariableHints,
    pub objective_override: Option<ObjectivePolicy>,
    pub lex_stage_policy: LexStagePolicy,
    pub unsupported: UnsupportedFeaturePolicy,
}
```

Existing `solve()` and `solve_with()` construct simple plans.

Overlay lifecycle:

```text
commit canonical model
-> compile/synchronize canonical state
-> validate plan
-> compile overlay against exact CompilationId
-> apply overlay
-> apply starts/hints
-> execute objective stages
-> extract result tagged with CompilationId
-> rollback overlay
-> verify backend canonical state
```

A fallible rollback is explicit; it is not delegated solely to `Drop`. Uncertain rollback marks the session `RequiresRebuild`.

## 13. IIS and conflict diagnostics

Infeasibility analysis is an optional backend trait:

```rust
pub trait InfeasibilityAnalysisSession {
    fn analyze_infeasibility(
        &mut self,
        request: &InfeasibilityRequest,
    ) -> Result<BackendConflict, BackendError>;
}
```

The normalized report includes:

- model lineage;
- model instance and revision;
- exact compilation ID;
- backend identity/version;
- analysis kind;
- analysis scope;
- minimality claim;
- completion status;
- original constraint sides, variable bounds, fixings, locks, and constructs;
- statistics.

Reports render text, Markdown, and structured Rust data.

ROML distinguishes:

- native IIS/conflict analysis;
- portable heuristic/deletion analysis;
- feasibility relaxation.

A portable deletion filter is never labeled native IIS. HiGHS support is derived from exact official bundled/system APIs and version-qualified.

## 14. Soft constraints and feasibility relaxation

A one-call API attaches a semantic soft-constraint construct to an existing constraint and creates stable canonical violation variables.

```rust
let softened = model.soften(
    capacity,
    soft()
        .upper_side()
        .max_violation(20.0)
        .penalty(1_000.0)
        .on(cost_objective),
)?;
```

Semantics:

```text
upper: a(x) - s_upper <= upper_bound
lower: a(x) + s_lower >= lower_bound
range: lower_bound - s_lower <= a(x) <= upper_bound + s_upper
s_lower, s_upper >= 0
```

Equality and ranged constraints normally use separate nonnegative violations. Signed correction is a separate API; L1 penalty uses positive and negative parts.

Penalty semantics always minimize weighted violation. The compiler handles sign when attaching to maximize objectives. Penalties may target an objective, a lexicographic priority, or no objective.

Backend-native feasibility relaxation remains solve-scoped and does not mutate the canonical model or create persistent soft handles.

## 15. Objective policies

```rust
pub enum ObjectivePolicy {
    Single(Objective),
    Weighted(Vec<WeightedObjective>),
    Lexicographic(Vec<ObjectiveLevel>),
}
```

### 15.1 Weighted policy

Weights are finite and nonnegative. Each objective is normalized to minimization according to its own sense:

```text
minimize objective f  -> +f
maximize objective f  -> -f
```

The weighted policy minimizes the weighted normalized sum. This makes mixed original senses unambiguous.

### 15.2 Lexicographic policy

Each level has absolute and relative degradation tolerances. Portable execution performs sequential solves with temporary objective-lock rows.

For a minimization optimum `z`:

```text
f(x) <= z + absolute_tolerance + relative_tolerance * |z|
```

For a maximization optimum `z`:

```text
f(x) >= z - absolute_tolerance - relative_tolerance * |z|
```

Default policy requires an optimal stage before descending. Explicit best-feasible continuation records the qualification.

Native multiobjective execution is used only when backend semantics match exactly; otherwise ROML uses the portable path.

## 16. Common modeling constructs

### 16.1 Indicators

```rust
model.add_indicator(on, when_one(), production.le(100.0))?;
```

Compilation order under `Auto`:

1. qualified native indicator;
2. exact portable bridge using finite bound evidence;
3. typed compile error.

### 16.2 Reification

```rust
let high = model.reify(expression.ge(100.0), iff().separation(1e-6))?;
```

Continuous exact reification requires explicit separation. Unit separation may be inferred only when the expression is proven integer-valued.

### 16.3 Min, max, absolute value, positive part, clamp

Exact equality and epigraph/hypograph semantics are distinct. ROML does not infer exactness from objective context.

- max epigraph: linear, no binaries;
- min hypograph: linear, no binaries;
- exact bounded min/max: native or selector formulation;
- absolute value/positive part/clamp: exact semantic constructs with bounded exact bridges.

### 16.4 Boolean and cardinality

ROML supports implication, equivalence, any/all, exactly-one, at-most, and at-least over binary variables through exact linear formulations.

### 16.5 Products

M3 exact support is limited to:

- binary times binary;
- binary times bounded linear scalar function.

Continuous-times-continuous equality is not exposed as exact MILP. Future relaxations must be explicitly named as relaxations.

## 17. Piecewise-linear functions

PWL declarations specify:

- finite strictly increasing breakpoints;
- relation: epigraph, hypograph, or exact graph;
- extrapolation policy;
- optional formulation preference.

Curvature is classified from segment slopes.

Compilation:

- convex epigraph: supporting linear inequalities, zero binaries;
- concave hypograph: supporting linear inequalities, zero binaries;
- exact graph: qualified native PWL, SOS2, or exact segment-binary formulation;
- nonconvex exact graph: never a convex relaxation.

The compilation report states curvature, selected representation, generated counts, and why binaries were introduced or avoided.

## 18. Incremental semantics

Canonical changes include construct lifecycle, objective policy, fixings, metadata, and auxiliary ownership.

M3 policy:

- metadata-only changes do not affect backend state;
- primitive linear coefficient/bound/parameter changes retain incremental paths;
- fixing becomes an effective bound delta where supported;
- solve overlays are not revisions;
- semantic construct changes may rebuild;
- parameter updates inside a bridge emit compiled deltas only after recipe stability and differential equivalence are proven;
- any uncertainty selects rebuild.

## 19. Failure semantics

New error families distinguish:

- model validation;
- assignment lineage/entity mismatch;
- compilation/bridge failure;
- unsupported native feature;
- unbounded or invalid Big-M;
- solve-plan conflict;
- stale compilation identity;
- backend operational failure;
- analysis mapping failure.

Errors identify the user entity/construct, backend, selected representation, and violated invariant where applicable.

## 20. Module boundaries

```text
canonical model:
  src/identity.rs
  src/metadata.rs
  src/function/**
  src/construct/**
  src/assignment.rs
  src/objective_policy.rs
  src/model/**
  src/snapshot.rs
  src/delta.rs

compiler:
  src/compiler/backend_ir.rs
  src/compiler/capability.rs
  src/compiler/session.rs
  src/compiler/origin.rs
  src/compiler/report.rs
  src/compiler/bounds.rs
  src/compiler/bridge/**

solve orchestration and analysis:
  src/solver/plan.rs
  src/solver/overlay.rs
  src/solver/effective_plan.rs
  src/solver/infeasibility.rs
  src/solver/feasibility_relaxation.rs
  src/solver/multiobjective.rs

HiGHS native integration:
  roml-highs/src/compiler.rs
  roml-highs/src/start.rs
  roml-highs/src/iis.rs
  roml-highs/src/relaxation.rs
  roml-highs/src/multiobjective.rs
```

## 21. Testing strategy

### Semantic tests

- construct validation and exactness;
- lineage/instance/compilation identity;
- fixing/unfixing state machine;
- assignment compatibility;
- soft-constraint algebra;
- objective policy validation;
- PWL curvature and evaluation.

### Compiler tests

- deterministic compilation;
- exact compilation identity propagation;
- origin completeness;
- native versus portable semantic equivalence;
- Big-M derivation;
- zero-binary convex PWL qualification;
- compiled delta versus rebuild equivalence.

### Solver tests

- overlay apply/rollback under injected failure;
- locks alter feasibility while starts/hints do not;
- unsupported-feature reporting;
- lexicographic native/portable agreement;
- IIS mapping to original constructs;
- soft violation values.

### Property and differential tests

- random bounded construct instances versus explicit reference formulations;
- random PWL points versus direct evaluation;
- random fix/bound/unfix sequences;
- fixed-seed compiled delta/rebuild comparison;
- portable/native formulation corpus.

## 22. Compatibility and migration

M3 intentionally amends the advanced backend synchronization contract because semantic compilation cannot cleanly pass through the current canonical `ModelSnapshot` boundary.

Rules:

- ordinary M2 modeling and solve APIs remain source-compatible unless an executable contradiction is approved;
- `solve()` and `solve_with()` remain;
- compiler internals are not added to the ordinary prelude;
- backend authors receive a migration guide;
- deprecated advanced aliases remain only where mechanically safe;
- commercial backends remain separate qualification tracks.

## 23. NLP readiness boundary

M3 does not implement nonlinear ASTs, expression tracing, automatic differentiation, gradients, Jacobians, Hessians, cones, or NLP solvers.

M3 must avoid these dead ends:

- treating every constraint as a sparse linear row;
- making `LinExpr` the permanent universal function;
- making backend IR synonymous with matrices;
- assuming every generated entity is a linearization artifact;
- assuming one objective pass per solve;
- assuming conflict analysis returns only row IDs;
- embedding Big-M policy in construct semantics.

A later NLP milestone should add function variants and backend primitives while reusing identity, metadata, constructs, objective policy, solve plans, origins, diagnostics, and compilation reports.

## 24. Acceptance criteria

M3 is complete only when:

1. semantic constructs survive canonical snapshots and revisions;
2. backends consume backend IR;
3. lineage, instance, and exact compilation identities are qualified;
4. all generated entities have origins;
5. fixings, locks, starts, and hints remain semantically distinct;
6. overlays cannot leak;
7. HiGHS start and IIS support are version-qualified;
8. IIS reports use original ROML names/provenance and precise guarantees;
9. soft constraints and solve-scoped relaxation are distinct and correct;
10. lexicographic native and portable paths agree on the corpus;
11. common constructs have exact validated formulations;
12. no Big-M lacks finite proof or explicit validated input;
13. convex PWL epigraph and concave PWL hypograph introduce zero binaries;
14. primitive incremental behavior remains qualified;
15. docs, public API, package, and fresh-consumer checks pass;
16. the NLP extension review requires additive changes rather than architectural replacement.

## 25. Approved decisions

- Preserve high-level constructs canonically.
- Use canonical semantic IR plus backend IR.
- Use function-in-set as the nonlinear-ready seam.
- Separate lineage, model instance, and compilation identity.
- Use bound tightening for variable fixing.
- Keep persistent mutations separate from solve overlays.
- Distinguish assignments, starts, hints, and locks.
- Use typed version-aware capabilities.
- Require exact bridges, origins, and formulation reports.
- Never invent Big-M.
- Keep IIS separate from feasibility relaxation.
- Store objective policy as mathematical intent.
- Defer NLP implementation while preserving its extension boundary.