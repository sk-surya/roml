# ROML M3 Completion and M4 Foundation Design

**Status:** owner-approved architecture translated into program design
**Planning base:** `main` after P35 merge and P29 design-record merge
**Execution order:** P36 -> P30 -> P31 -> P34 -> M4 planning

## 1. Objective

Close the current MILP milestone as a coherent research/production system rather than continuing horizontal feature accumulation. The next work must convert the capabilities already built into closed loops:

1. external model -> ROML -> external model;
2. infeasible model -> semantic IIS -> controlled repair;
3. multi-criteria model -> deterministic staged solve;
4. full M3 -> qualified public capability set;
5. only then extend the semantic function/compiler architecture toward quadratic and nonlinear optimization.

The program optimizes for semantic correctness, provenance, differential evidence, and production failure semantics. It explicitly rejects feature-count-driven expansion.

## 2. Program sequence

```text
P35 MPS import + corpus qualification [COMPLETE]
        |
        v
P36 MPS deterministic write-back + round-trip qualification
        |
        v
P30 Soft constraints + solve-scoped feasibility relaxation
        |
        v
P31 Objective policies + portable lexicographic orchestration
        |
        v
P34 M3 integration / qualification / docs / NLP-readiness
        |
        v
M4 Quadratic + nonlinear semantic foundation [PLAN ONLY after P34]
```

P30 and P31 are architecturally independent after P28, but they will execute sequentially here because review/integration capacity is the active bottleneck. P31 consumes P30's objective-priority integration and P34 qualifies their interaction.

## 3. Binding program invariants

### 3.1 Semantic authority

Canonical ROML state is the semantic authority. File formats, backend objects, native helper APIs, and compiled representations are projections. None may silently redefine model semantics.

### 3.2 Exact-state authority

`ModelLineageId`, `ModelInstanceId`, revision, and exact opaque `CompilationId` retain their established meanings. Deterministic hashes are evidence/cache keys only.

### 3.3 Native versus portable

Backend-native functionality is used only when its documented semantics match ROML's declared contract. Otherwise ROML uses a proven portable formulation/executor or returns typed `Unsupported`. Native support is an optimization/capability, never the definition of a ROML feature.

### 3.4 No silent loss

Serialization, softening, relaxation, objective orchestration, and future nonlinear compilation must either preserve declared semantics or fail with a typed, actionable error. There is no best-effort mode hidden behind defaults.

### 3.5 Transactional solve-scoped state

Feasibility relaxation, lexicographic objective locks, starts, hints, temporary fixings, and other solve-scoped artifacts use the existing overlay/session transaction semantics. They never become accidental canonical mutations.

### 3.6 Origin preservation

Generated variables, rows, bounds, objective locks, relaxation artifacts, and imported/exported source mappings retain enough provenance to render explanations in original semantic terms.

### 3.7 One active implementation phase

Only one of P36/P30/P31/P34 is active at a time. A later phase may be researched or reviewed, but implementation does not start until the prior phase is accepted and merged.

## 4. P36 — deterministic MPS write-back

### 4.1 Goal

Any linear LP/MILP representable by the supported P35 semantic surface can be deterministically serialized to free MPS, read independently by HiGHS, and round-tripped without changing the normalized mathematical model or bounded solve result.

### 4.2 Architecture

Add a writer beside the existing reader:

```text
Model / canonical snapshot
        |
        v
MpsWriteProjection
  - representability validation
  - deterministic names/order
  - objective/vector selection
  - canonical coefficient stream
        |
        v
MpsWriter<W: Write>
        |
        +--> bytes / file
        |
        +--> HiGHS readModel differential oracle
        |
        +--> ROML MpsReader semantic round trip
```

The writer serializes the mathematical model, not source-file layout. P35 source ordering, comments, duplicate records, and original fixed-column layout are not preserved.

### 4.3 V1 dialect

Default and only required P36 output is deterministic free MPS for linear LP/MILP:

- `NAME`
- `OBJSENSE`
- `OBJNAME`
- `ROWS`
- `COLUMNS`
- `RHS`
- `RANGES`
- `BOUNDS`
- `INTORG` / `INTEND` markers when appropriate
- `ENDATA`

Quadratic, conic, SOS, indicator, PWL, and vendor-specific semantic extensions are not emitted in P36. High-level ROML constructs must already have a mathematically equivalent linear/MILP canonical/backend projection if the writer is asked to export the compiled formulation; otherwise semantic-model export returns typed `Unrepresentable`.

### 4.4 Two explicit export targets

Do not overload one API with ambiguous semantics:

```rust
pub enum MpsWriteTarget {
    SemanticModel,
    CompiledLinearFormulation,
}
```

- `SemanticModel` writes only concepts directly representable in standard linear MPS from canonical model state. Unsupported semantic constructs return a typed representability error.
- `CompiledLinearFormulation` is an advanced path that exports an exact compiled linear/MILP snapshot plus generated names/origin metadata. It requires an exact compilation artifact and is clearly labeled as formulation export, not source-model export.

P36 acceptance requires `SemanticModel`; compiled-formulation export may be implemented if the current backend IR makes it small and exact, but must not delay P36 unless needed for corpus coverage.

### 4.5 Determinism

For a fixed canonical model state and write options, bytes are deterministic across repeated runs on the same ROML version:

- variables sorted by stable semantic ID unless explicit deterministic name policy says otherwise;
- constraints sorted by stable semantic ID;
- each COLUMNS coefficient emitted once from the canonical cell;
- one deterministic RHS/RANGES/BOUNDS vector name;
- deterministic contiguous integer-marker regions;
- locale-independent floating formatting;
- `-0.0` normalized to `0`;
- non-finite coefficients/offsets rejected before any partial output is committed.

The public `write_path` implementation writes to a temporary sibling and atomically replaces only after successful serialization where the platform permits; stream writes return an error after partial bytes are possible and document that distinction.

### 4.6 Round-trip gates

Four independent gates:

1. **ROML semantic round trip**: `Model -> MPS -> MpsReader -> normalized model`.
2. **HiGHS structure**: `Model -> MPS -> Highs_readModel` versus direct ROML->HiGHS projection.
3. **Solve equivalence**: bounded selected models agree on termination class and objective under declared tolerances.
4. **Corpus transcode**: each supported Netlib model `external MPS -> ROML -> deterministic MPS -> ROML/HiGHS` preserves normalized structure; all 94 P35-supported Netlib files are attempted and every exclusion is explicitly classified.

No byte-for-byte equivalence to the input MPS is required.

## 5. P30 — soft constraints and feasibility relaxation

### 5.1 Goal

Turn infeasibility diagnosis into a repair workflow while maintaining a hard distinction between persistent model semantics and solve-scoped analysis.

```text
solve -> infeasible -> IIS -> choose relaxable semantic restrictions
     -> feasibility relaxation -> violation report -> optional model edit
```

### 5.2 Persistent soft constraints

`Model::soften(...)` creates a canonical semantic construct. It is a model change and advances revision. The original constraint identity remains the reporting anchor.

For a linear function `f(x)`:

- upper `f(x) <= u`: `f(x) <= u + v_up`, `v_up >= 0`;
- lower `f(x) >= l`: `f(x) >= l - v_lo`, `v_lo >= 0`;
- equality `f(x) = b`: lower and upper violations are distinct stable auxiliaries;
- ranged `l <= f(x) <= u`: lower and upper violations remain distinct;
- maximum violation, when supplied, is finite, nonnegative, and enforced as an auxiliary bound.

A signed correction API is separate and uses positive/negative parts. It is never inferred from ordinary softening.

### 5.3 Penalty policy

Penalty is not embedded as a magical objective mutation. Define explicit semantic policy:

```rust
pub enum PenaltyTarget {
    None,
    Objective(Objective),
    Priority(LexicographicPriority),
}
```

Weights are finite, nonnegative, may depend on parameters, and are normalized against objective sense by the objective-policy compiler/executor. P30 may support `Objective` and `None` immediately; `Priority` becomes active with P31 without changing stored soft-constraint semantics.

### 5.4 Solve-scoped feasibility relaxation

Add an analysis/solve-plan operation distinct from persistent soft constraints. It selects relaxable semantic atoms or original constraints/bounds, constructs temporary violation artifacts through an isolated/overlay session, solves a declared relaxation objective, maps results back to original restrictions, and rolls back.

The default portable relaxation is ROML-owned. A qualified native HiGHS relaxation path may accelerate it only if official semantics can be mapped exactly. Native and portable reports use the same structured result contract.

### 5.5 Relaxation objectives

Initial portable set:

- weighted L1 magnitude: minimize `sum(w_i * v_i)`;
- weighted violation-count approximation/exact MILP only when explicit binary indicators and valid finite activation bounds are available; this is optional and cannot block P30;
- no implicit L2/nonlinear relaxation in M3.

### 5.6 IIS integration

P29 and P30 remain separate capabilities, but provide a convenience composition layer:

```rust
report.relaxation_scope()
```

or equivalent that produces a selection of original relaxable restrictions without asserting that relaxing only IIS members is globally optimal. The API must state that an IIS is a diagnostic seed, not a proof of minimum repair.

## 6. P31 — objective policies and lexicographic solves

### 6.1 Goal

Make multi-criteria optimization explicit and deterministic without depending on solver-specific multiobjective semantics.

### 6.2 Canonical policy

```rust
pub enum ObjectivePolicy {
    None,
    Single(Objective),
    Weighted(WeightedObjectives),
    Lexicographic(LexicographicObjectives),
}
```

Weighted objectives normalize each objective sense before applying finite nonnegative weights. Lexicographic levels carry absolute and relative degradation tolerances and deterministic ordering.

### 6.3 Portable executor is normative

The portable algorithm is the semantic reference:

```text
for stage in priority order:
    apply objective override
    solve
    require stage qualification according to continuation policy
    capture objective values + exact CompilationId
    add temporary objective lock derived from stage result and tolerances
continue
finally rollback every stage artifact
```

Default continuation requires an optimal stage. Explicit `BestFeasible` continuation is separately named and recorded.

### 6.4 Correct lock formulas

The implementation must derive degradation locks from objective sense and the actual stage optimum; formulas must be valid for positive, zero, and negative optima. Relative tolerance is based on a documented scale rule rather than naive multiplication that changes direction around zero.

### 6.5 Native multiobjective

Use only if a backend's ordering, weight, tolerance, continuation, and status semantics match ROML. Otherwise support level is `Unsupported` or ROML portable. Native/portable equivalence is a P31 gate when any native implementation is declared.

### 6.6 P30 composition

Soft-constraint penalties can occupy a lexicographic priority without custom executor paths. Example production pattern:

```text
priority 0: minimize violations
priority 1: maximize economics
priority 2: minimize deviation/churn
```

This interaction receives direct integration tests before P31 acceptance.

## 7. P34 — M3 closure

### 7.1 Goal

Stop adding M3 features and prove the combined system is coherent, usable, performant, and extensible.

### 7.2 Required integrated workflows

At minimum:

1. build parameterized LP -> incremental solves -> export MPS -> HiGHS read;
2. import infeasible MPS -> P29 IIS -> P30 feasibility relaxation -> source-aware report;
3. MILP with starts + overlay + semantic constructs + P31 lexicographic objective;
4. PWL/construct bridge -> formulation report -> solve -> deterministic export classification;
5. failure injection across compile/apply/solve/rollback/analysis stages;
6. fresh consumer using packed `roml` + `roml-highs` only.

### 7.3 Qualification

P34 owns:

- complete SM requirement traceability;
- OS/MSRV/backend-version CI matrix;
- public API diff and semver review;
- package/fresh-consumer checks;
- documentation/examples/migration consistency;
- primitive incremental performance regression gate already specified by M3;
- targeted P29/P30/P31 analysis/orchestration benchmarks;
- native/portable equivalence evidence;
- stale-state/rollback failure matrix;
- no P0/P1 findings at merge.

### 7.4 Stop condition

After P34 is accepted, M3 is closed. New MILP convenience features are parked unless they repair a demonstrated usability/correctness gap. The next milestone starts from an explicit semantic-extension design review.

## 8. M4 — quadratic and nonlinear semantic foundation

M4 is deliberately **not** an implementation continuation of P34. P34 first certifies that the existing extension seams are real.

### 8.1 Initial architectural direction

Extend rather than replace:

```rust
pub enum ScalarFunction {
    Linear(LinearFunction),
    Quadratic(QuadraticFunction),
    // Later: expression graph / callback-backed differentiable function
}
```

The next design must address:

- canonical quadratic term representation and duplicate-cell authority;
- convexity metadata versus proof;
- QP/QCQP objective/constraint sets;
- backend IR extensions for quadratic primitives;
- native versus portable policy (no fake linearization);
- derivative/evaluation interfaces for future NLP;
- exact identity/provenance across nonlinear compilation;
- local versus global solve/feasibility claims;
- nonlinear infeasibility diagnostics that never label local restoration failure as IIS;
- MINLP as a later composition, not an initial M4 requirement.

### 8.2 Proposed M4 sequence

```text
M4-P0  quadratic semantic IR + evaluator
M4-P1  QP objective compilation + HiGHS/qualified backend
M4-P2  quadratic constraints + convexity/capability contract
M4-P3  nonlinear expression/evaluation interface design
M4-P4  first smooth NLP backend qualification
M4-P5  nonlinear diagnostics / warm starts / integration
```

Exact numbering is intentionally deferred until P34's NLP-readiness review validates the extension seams.

## 9. Explicit non-goals of this program

Before P34 closure, do not start:

- LP-format parser/writer solely for format breadth;
- JSON/YAML model serialization;
- SMPS;
- nonlinear or quadratic production implementation;
- generalized distributed solve orchestration;
- new commercial-solver adapters solely for feature parity;
- minimum-cardinality IIS research;
- release/publication work not already covered by separate owner gates.

## 10. Program acceptance

This design is complete when the repository contains implementation-ready phase plans, one active phase (P36), explicit deferred work, and GSD routing that cannot accidentally start P30/P31/P34 out of order.