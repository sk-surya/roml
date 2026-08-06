# Phase 29 Design Packet — Solver-Agnostic IIS and Conflict Analysis

**Date:** 2026-08-06  
**Status:** Owner direction; binding input to P29 discussion and planning  
**Target phase:** 29 — IIS/conflict analysis and origin-aware reports  
**Baseline:** `main@b04f0f516b159548327b868b5011fe2c24fe4207`

## 1. Objective

Phase 29 establishes infeasibility diagnosis as a solver-agnostic ROML capability rather than a thin wrapper around whichever conflict API a backend happens to expose.

P29 must deliver:

1. a ROML-owned LP IIS engine with competitive repeated-solve behavior;
2. optional backend-native IIS providers, beginning with version-qualified HiGHS support;
3. diagnosis in original ROML semantic terms, not merely backend row numbers;
4. precise, inspectable guarantees and partial-completion semantics; and
5. an additive extension path to MIP, quadratic, conic, nonlinear, and mixed-integer nonlinear models.

The immediate product scope is **LP infeasibility**. A model whose LP relaxation is feasible but whose MIP is infeasible is explicitly the next scope, not an implicit or falsely labeled P29 result.

## 2. Governing decisions and invariants

This packet refines, but does not reverse, the accepted M3 decisions:

- **D17:** IIS/conflict analysis and feasibility relaxation are separate APIs.
- **D18:** reports state analysis kind, scope, minimality, completion, backend identity, model identity, and exact compilation identity.
- **D19:** `Native` IIS means an official, audited backend API. A ROML algorithm is never labeled backend-native.
- **D28:** exact `CompilationId`, not a revision or digest alone, governs mapping and stale-state safety.
- **Design §23:** conflict analysis must not assume that conflicts contain only linear row IDs, and backend IR must not become synonymous with matrices.
- **P28:** unsupported behavior rejects by default; conversions and fallbacks are explicit and recorded.
- **P32 F4:** no false native claims.

Additional P29 invariants:

1. The public conflict unit is a **semantic restriction atom**, not a compiled row.
2. Every conflict member maps to an original ROML restriction or a named generated semantic construct.
3. Disabling one semantic atom removes exactly that restriction while preserving all other restrictions.
4. A complete ROML LP result is irreducible only with respect to the declared candidate universe, grouping, feasibility oracle, and recorded numerical tolerances.
5. P29 never claims minimum-cardinality or smallest IIS.
6. An `Unknown` oracle result may preserve a useful infeasible subsystem, but it prevents an irreducibility claim.
7. Native data is accepted only for the exact `CompilationId` it analyzed.
8. Analysis runs in an isolated analysis session by default and cannot poison the persistent solve session.
9. Feasibility relaxation remains a separate future/parallel facility; it is not used as a user-visible substitute for IIS.
10. The core `roml` crate remains solver-free.

## 3. Decisions answering the P29 discussion questions

### P29-D1 — Ship ROML portable LP IIS and native IIS as separate providers

P29 ships both:

- **ROML LP IIS:** solver-agnostic orchestration using a backend feasibility oracle and reversible semantic restriction masks;
- **native IIS:** an optional backend provider, initially HiGHS, available only when the audited API/version/model class is qualified.

The ROML path is not merely a best-effort deletion heuristic. When every oracle call returns a proven LP feasibility status and the final verification pass completes, it guarantees a **subset-minimal/irreducible infeasible subsystem relative to the declared semantic candidate universe**. It remains a `RomlPortable` provider, never `Native`.

Recommended modes:

```rust
#[non_exhaustive]
pub enum InfeasibilityMode {
    /// Prefer a qualified native seed, then perform ROML semantic reduction
    /// and verification. Fall back to ROML-only analysis when native support
    /// is unavailable.
    Auto,
    /// Use only the ROML solver-agnostic engine.
    RomlPortable,
    /// Require the official backend-native IIS/conflict API.
    NativeOnly,
    /// Require a native seed, then refine and verify it in semantic ROML
    /// terms.
    NativeThenRoml,
}
```

`Auto` is the recommended default. It makes native routines an acceleration and evidence source while retaining ROML's semantic grouping and guarantee discipline.

P29 model-class behavior:

| Model state | P29 behavior |
|---|---|
| Continuous linear model infeasible | Analyze as `OriginalLp` |
| MIP whose LP relaxation is infeasible | Analyze only when explicitly requested as `LpRelaxation`; report that scope |
| MIP whose LP relaxation is feasible but original MIP is infeasible | Typed `Unsupported` / `NoConflictInRequestedScope`; defer to the MIP-conflict phase |
| Feasible model | Return `NoConflict` rather than manufacturing a report |
| Unknown/numerical solve state | Return incomplete analysis with reason; never claim IIS |

### P29-D2 — Use a plan/executor entry point on `SolverSession`

The primary user-facing entry point follows the P28 orchestration pattern:

```rust
impl<B> SolverSession<B> {
    pub fn analyze_infeasibility(
        &mut self,
        model: &Model,
        plan: &InfeasibilityPlan,
    ) -> Result<InfeasibilityReport, InfeasibilityError>;
}
```

The ordinary solver facade may provide a convenience wrapper that constructs a default plan, but the capability is **not** a `Model` method. Infeasibility analysis requires backend capabilities, an exact compiled artifact, a repeated feasibility oracle, resource limits, and provider selection.

Low-level backend contracts remain optional and bounded:

```rust
pub trait InfeasibilityOracleFactory {
    type Oracle: FeasibilityOracle;

    fn spawn_infeasibility_oracle(
        &self,
        snapshot: &BackendSnapshot,
        universe: &CompiledRestrictionUniverse,
    ) -> Result<Self::Oracle, BackendError>;

    fn native_conflict(
        &self,
        _request: &NativeConflictRequest,
    ) -> Result<NativeConflict, BackendError> {
        Err(BackendError::unsupported("native IIS"))
    }
}
```

The default architecture creates an **isolated analysis session** from the already-produced `BackendSnapshot`. This permits aggressive temporary restriction toggling, basis reuse, and provider-specific options without mutating the live canonical solve session. A single analysis rebuild is acceptable; rebuilding for every candidate check is not.

### P29-D3 — Map every conflict member to semantic restriction atoms

The existing `OriginMap` proves compiled-entity provenance, but entity-level provenance is insufficient for IIS:

- one ranged row has independently removable lower and upper sides;
- a variable has independently removable lower and upper bounds;
- persistent fixing overlays declared bounds rather than replacing their provenance;
- solve locks and temporary fixings overlay the canonical effective bounds;
- one construct can generate multiple rows, variables, and bounds that must be enabled or disabled atomically;
- future integrality, cones, nonlinear domains, and complementarity restrictions are not row IDs.

P29 therefore adds a separate restriction-level map rather than overloading `EntityOrigin`:

```rust
pub struct SemanticConflictAtom {
    pub id: ConflictAtomId,
    pub origin: ConflictOrigin,
    pub kind: ConflictAtomKind,
    pub compiled_restrictions: Vec<CompiledRestrictionRef>,
    pub disable: RestrictionTogglePlan,
    pub restore: RestrictionTogglePlan,
    pub snapshot: ConflictMemberSnapshot,
}

#[non_exhaustive]
pub enum ConflictAtomKind {
    ConstraintSide { side: ConstraintSide },
    VariableBound { side: BoundSide },
    PersistentFixing,
    SolveLock,
    TemporaryFixing,
    Construct,
    // Future additions:
    Integrality,
    ConicDomain,
    NonlinearFunctionInSet,
    Complementarity,
}
```

Default semantic grouping:

- a one-sided constraint contributes one atom;
- equality is one semantic atom, even if represented by two compiled sides;
- a ranged constraint contributes two atoms, one per side;
- variable lower and upper bounds are separate atoms;
- a persistent fixing is one atom whose disable operation restores the declared bounds;
- a solve lock/temporary fixing is one atom whose disable operation restores canonical effective bounds;
- a semantic construct is one atom by default, grouping all generated restrictions atomically;
- optional advanced plans may request compiled-detail granularity for debugging, but such output is not the default user IIS.

The restriction map must preserve a **bound contribution stack**. For example:

```text
declared bounds
    -> persistent fixing
    -> solve-scoped temporary fixing / lock
```

Disabling a higher layer restores the next lower layer; it never blindly sets a bound to infinity.

A raw native conflict is compiled-primitive evidence. Mapping it to semantic origins may merge multiple raw members. ROML must recheck and, when a semantic irreducibility claim is requested, re-reduce the mapped semantic set. A backend-minimal row conflict is not automatically a minimal semantic conflict after grouping.

Reports store names, sides, values, metadata, and provenance snapshots at analysis time, so they remain readable historical artifacts after the model changes. Any operation that re-resolves a report against a live model must validate model instance/revision and exact `CompilationId`; stale access returns a typed error.

### P29-D4 — Use one canonical report with dedicated renderers

`InfeasibilityReport` is the authoritative structured result. Text and Markdown are dedicated deterministic views:

```rust
pub struct TextInfeasibilityReport<'a>(pub &'a InfeasibilityReport);
pub struct MarkdownInfeasibilityReport<'a>(pub &'a InfeasibilityReport);
```

`Display for InfeasibilityReport` may provide a concise one-line summary, but it must not become the full stable rendering contract. Structured serialization can be added behind a feature after the Rust data model stabilizes.

The report records at least:

```rust
pub struct InfeasibilityReport {
    pub model_lineage: ModelLineageId,
    pub model_instance: ModelInstanceId,
    pub model_revision: ModelRevision,
    pub compilation_id: CompilationId,

    pub backend: BackendIdentity,
    pub provider_chain: Vec<AnalysisProviderRecord>,
    pub scope: InfeasibilityScope,
    pub candidate_universe: CandidateUniverseSummary,

    pub completion: AnalysisCompletion,
    pub guarantee: ConflictGuarantee,
    pub oracle_strength: FeasibilityProofStrength,
    pub numerical_policy: NumericalPolicyRecord,

    pub members: Vec<ConflictMember>,
    pub native_evidence: Option<NativeConflictEvidence>,
    pub statistics: InfeasibilityStatistics,
    pub warnings: Vec<AnalysisWarning>,
}
```

Guarantees are explicit:

```rust
#[non_exhaustive]
pub enum ConflictGuarantee {
    /// The returned members are infeasible, but minimality was not proven.
    InfeasibleSubsystem,
    /// Removing any one returned semantic atom makes the selected subsystem
    /// feasible under the recorded oracle and numerical policy.
    Irreducible {
        with_respect_to: CandidateUniverseSummary,
    },
    /// A native backend claim, preserved verbatim and not promoted to a ROML
    /// semantic irreducibility claim without ROML verification.
    NativeReported {
        backend_claim: String,
    },
    None,
}
```

Completion is separate from minimality:

```rust
#[non_exhaustive]
pub enum AnalysisCompletion {
    Complete,
    TimeLimit,
    OracleCallLimit,
    IterationLimit,
    Interrupted,
    NumericalFailure,
    BackendFailure,
}
```

A partial run returns the best validated infeasible subsystem found so far, plus unknown/possible-member information where available. It never silently upgrades the guarantee.

The report must state explicitly:

- whether the scope was original LP, LP relaxation, or another future model class;
- whether members are semantic atoms or compiled primitives;
- whether grouping occurred;
- whether the result is irreducible, merely infeasible, or only backend-reported;
- whether all one-member deletion checks completed;
- the feasibility tolerances and backend options used;
- that no minimum-cardinality guarantee is made;
- whether native evidence seeded the result;
- whether any oracle result was unknown or numerically ambiguous.

## 4. Capability model

`BackendFeature::Iis` continues to mean **official backend-native IIS/conflict support** under D19. Do not set `BackendFeature::Iis` to `SupportLevel::Bridge` merely because the ROML portable engine can run.

Native and portable availability can coexist, so a single-valued `FeatureSupport` entry is not sufficient to describe the full analysis stack. Add analysis-specific capability data:

```rust
pub struct InfeasibilityProviderCapabilities {
    pub native: Option<NativeIisCapability>,
    pub portable_lp: PortableLpCapability,
}

pub struct PortableLpCapability {
    pub available: bool,
    pub incremental_row_bounds: bool,
    pub incremental_variable_bounds: bool,
    pub basis_reuse: bool,
    pub dual_ray_seed: bool,
    pub isolated_session: bool,
}
```

The effective provider chain is recorded in the report. Examples:

```text
HiGHS native seed -> ROML semantic reducer -> ROML irreducibility verifier
ROML interval seed -> ROML semantic reducer -> ROML verifier
ROML full-universe seed -> ROML semantic reducer -> incomplete (time limit)
```

## 5. LP IIS framework

### 5.1 Candidate universe

The compiler builds a deterministic `SemanticConflictUniverse` from the exact `BackendSnapshot` and any solve overlay explicitly included by the request.

```rust
pub struct InfeasibilityPlan {
    pub mode: InfeasibilityMode,
    pub scope: InfeasibilityScope,
    pub grouping: ConflictGrouping,
    pub seed_policy: SeedPolicy,
    pub reduction: ReductionPolicy,
    pub budget: AnalysisBudget,
    pub numerical_policy: AnalysisNumericalPolicy,
}
```

The default P29 scope excludes objective functions because objectives do not affect ordinary feasibility. Future objective cutoffs and lexicographic locks are included only when present as explicit solve-overlay restrictions.

### 5.2 Feasibility oracle

The portable algorithm depends on a tri-state oracle:

```rust
pub trait FeasibilityOracle {
    fn compilation_id(&self) -> CompilationId;

    fn check(
        &mut self,
        selection: &ConflictSelection,
        budget: &OracleBudget,
    ) -> Result<FeasibilityOutcome, BackendError>;
}

#[non_exhaustive]
pub enum FeasibilityOutcome {
    ProvenFeasible(FeasibilityEvidence),
    ProvenInfeasible(InfeasibilityEvidence),
    Unknown(UnknownReason),
}
```

For LPs, `Proven` means solver-certified under the recorded numerical tolerances and status semantics; it does not claim exact-rational arithmetic.

The oracle implementation should:

1. validate the exact `CompilationId` before every mask transition;
2. translate selection changes into reversible row-side and variable-bound updates;
3. preserve compiled indices;
4. solve a zero-objective feasibility LP;
5. prefer incremental reoptimization and basis reuse where qualified;
6. map ambiguous, limit, and numerical statuses to `Unknown`, not infeasible;
7. recover or rebuild the isolated analysis session after uncertain mutation/rollback.

For LP backends, toggling row bounds to inactive infinities and restoring recorded sides is generally preferable to deleting and re-adding rows: indices remain stable and simplex bases are more reusable. The abstraction must still allow a backend to choose a different qualified mechanism.

### 5.3 Recommended algorithm

Use a hybrid strategy:

1. **cheap contradiction scan** for bound and row-activity contradictions;
2. **best available infeasible seed**, in priority order:
   - qualified native IIS/conflict result;
   - dual/Farkas certificate support, when exposed and validated;
   - elastic Phase-I support seed, when qualified;
   - full semantic candidate universe;
3. **adaptive chunk deletion** to remove large irrelevant groups cheaply;
4. **single-atom deletion polish** to prove irreducibility;
5. **fresh final verification** before setting the guarantee.

This design is preferred over a naive one-by-one scan of the entire model because native/certificate/elastic seeds can sharply reduce the starting set, and block deletion can remove irrelevant regions in few oracle calls. It is preferred as the initial implementation over making QuickXplain the only reducer because sequential mask changes and deletion passes provide predictable basis locality and simpler correctness evidence. The reducer interface should remain pluggable so a divide-and-conquer strategy can be benchmarked later.

Deterministic ordering should prioritize:

1. direct contradictions and native/certificate-supported atoms;
2. named user constraints and bounds;
3. fixings and locks;
4. grouped semantic constructs;
5. remaining atoms by stable declaration order.

Ordering is a performance policy, not a guarantee change.

### 5.4 Orchestrator pseudocode

```text
function analyze_infeasibility(model, plan):
    validate(plan)

    compiled = compile_exact(model, plan.scope)
    universe = build_semantic_conflict_universe(
        compiled.snapshot,
        plan.scope,
        plan.grouping,
    )

    oracle = backend.spawn_isolated_infeasibility_oracle(
        compiled.snapshot,
        universe.compiled_restrictions,
    )

    assert oracle.compilation_id == compiled.compilation_id

    base = oracle.check(universe.all, plan.budget.base_check)
    match base:
        ProvenFeasible:
            return NoConflict(report_identity(compiled, plan))
        Unknown(reason):
            return IncompleteNoConflictProof(reason, report_identity(...))
        ProvenInfeasible:
            continue

    seed_candidates = [
        interval_activity_seed(universe),
        native_seed_if_allowed(oracle, universe, plan),
        dual_ray_seed_if_available(oracle, universe, plan),
        elastic_phase_one_seed_if_available(oracle, universe, plan),
        universe.all,
    ]

    seed = first candidate in seed_candidates such that:
        candidate is non-empty AND
        oracle.check(candidate) == ProvenInfeasible

    reduced = hybrid_reduce(oracle, seed, plan.reduction, plan.budget)
    verified = verify_final_conflict(oracle, reduced, plan.budget)

    raw_report = build_structured_report(
        compiled,
        universe,
        seed provenance,
        reduced,
        verified,
        oracle statistics,
    )

    return map_members_to_original_semantics(raw_report)
```

### 5.5 Hybrid reducer pseudocode

```text
function hybrid_reduce(oracle, candidate, policy, budget):
    require oracle.check(candidate) == ProvenInfeasible

    C = candidate
    complete = true

    block_size = largest_power_of_two_at_most(max(1, size(C) / 2))

    while block_size > 1 AND budget remains:
        removed_any = false

        for block in deterministic_blocks(C, block_size):
            trial = C minus block
            if trial is empty:
                continue

            outcome = oracle.check(trial)
            match outcome:
                ProvenInfeasible:
                    C = trial
                    removed_any = true
                ProvenFeasible:
                    keep block in C
                Unknown:
                    keep block in C
                    complete = false

        if not removed_any:
            block_size = block_size / 2

    # Exact deletion polish. Restart after removals if implementation ordering
    # or grouped toggles can expose newly removable atoms; otherwise one pass
    # is sufficient because infeasibility is monotone under adding restrictions.
    for atom in deterministic_atoms(C):
        if budget exhausted:
            complete = false
            break

        trial = C minus {atom}
        outcome = oracle.check(trial)

        match outcome:
            ProvenInfeasible:
                C = trial
            ProvenFeasible:
                keep atom in C
            Unknown:
                keep atom in C
                complete = false

    return ReducedConflict(
        members = C,
        reduction_complete = complete,
    )
```

### 5.6 Final verification pseudocode

```text
function verify_final_conflict(oracle, reduced, budget):
    if oracle.check(reduced.members) != ProvenInfeasible:
        return AnalysisFailure("final subsystem not proven infeasible")

    for atom in reduced.members:
        outcome = oracle.check(reduced.members minus {atom})
        match outcome:
            ProvenFeasible:
                continue
            ProvenInfeasible:
                return ReductionBugOrStaleState(atom)
            Unknown(reason):
                return VerifiedInfeasibleButNotIrreducible(reason)

    return IrreducibleUnderRecordedOracle
```

The final pass is intentionally redundant. It detects stale masks, restoration defects, provider mapping mistakes, and reducer bugs before ROML emits a strong claim.

## 6. Native HiGHS provider

Implement native integration in `roml-highs/src/iis.rs` and expose it only through official generated `highs-sys` bindings.

Binding and version rules:

1. Audit the exact pinned `highs-sys 1.15.0` bindings and the corresponding official HiGHS header/source.
2. Audit each supported system HiGHS version separately.
3. Compile-gate references to IIS symbols when a system library/header does not provide them. A runtime version check alone cannot prevent a link-time missing-symbol failure.
4. Never add handwritten `extern "C"` declarations.
5. Never infer structure layout, status constants, or strategy values.
6. Do not hardcode numeric `iis_strategy` bit meanings from unversioned documentation; use audited named constants or a version-specific adapter with tests.
7. Return typed `Unsupported` when the symbol, model class, or version has not been qualified.
8. Preserve row/column indices, row/column bound-side statuses, and all native possible/member/excluded states exposed by the audited API.
9. Tag native output with the exact `CompilationId` and reject mismatched mapping.
10. Record whether HiGHS analyzed the original LP, a QP, or the LP relaxation of a MIP. P29 may expose only the qualified LP behavior.

HiGHS native output is one of:

- a direct `NativeOnly` result carrying the backend's stated guarantee; or
- a seed for the ROML semantic reducer and verifier.

For `Auto`, prefer the second behavior. It protects the user-facing semantic guarantee from native row-level grouping effects and backend-version differences.

The HiGHS C API currently exposes IIS retrieval for rows and columns/bounds, while official HiGHS documentation also describes the IIS implementation as evolving and currently focused on LP behavior. Therefore P29 must qualify exact versions and benchmark rather than assuming parity with commercial solver conflict refiners.

Official evidence to audit during implementation:

- [HiGHS C API — `Highs_getIis`](https://ergo-code.github.io/HiGHS/stable/interfaces/c_api/)
- [HiGHS advanced features — IIS](https://ergo-code.github.io/HiGHS/stable/guide/advanced/)
- [HiGHS options — IIS strategies](https://ergo-code.github.io/HiGHS/dev/options/definitions/)
- [`highs-sys` 1.15.0 generated bindings](https://docs.rs/highs-sys/1.15.0/highs_sys/)

## 7. Report member shape

Each user-visible conflict member contains original semantic data and optional compiled evidence:

```rust
pub struct ConflictMember {
    pub atom_id: ConflictAtomId,
    pub origin: ConflictOrigin,
    pub display_name: String,
    pub kind: ConflictMemberKind,
    pub metadata: Option<EntityMetadataSnapshot>,
    pub source: Option<ModelSource>,
    pub declaration: ConflictDeclarationSnapshot,
    pub native_membership: Option<NativeMembershipStatus>,
    pub compiled_evidence: Vec<CompiledRestrictionEvidence>,
}

#[non_exhaustive]
pub enum ConflictDeclarationSnapshot {
    ConstraintSide {
        constraint: Constraint,
        side: ConstraintSide,
        bound: f64,
        function_summary: String,
    },
    VariableBound {
        variable: Variable,
        side: BoundSide,
        value: f64,
        provenance: BoundProvenance,
    },
    PersistentFixing {
        variable: Variable,
        value: f64,
        declared_bounds: Bounds,
    },
    SolveLock {
        variable: Variable,
        locked_bounds: Bounds,
        restored_bounds: Bounds,
        overlay: OverlayId,
    },
    Construct {
        construct: Construct,
        construct_kind: String,
        generated_roles: Vec<GeneratedRole>,
    },
}
```

Default text and Markdown reports foreground original names, sides, values, fixing/lock provenance, construct type, and model source. Compiled IDs and backend membership codes appear in a technical evidence section rather than replacing semantic information.

## 8. Error and recovery semantics

Introduce a focused error family:

```rust
#[non_exhaustive]
pub enum InfeasibilityError {
    ModelNotProvenInfeasible,
    UnsupportedScope { requested: InfeasibilityScope },
    NativeUnsupported { backend: BackendIdentity, reason: String },
    StaleCompilation { expected: CompilationId, actual: CompilationId },
    IncompleteOriginMapping { missing: Vec<CompiledRestrictionRef> },
    InvalidRestrictionToggle { atom: ConflictAtomId, reason: String },
    OracleUnknown { reason: UnknownReason },
    AnalysisSessionRequiresRebuild,
    Backend(BackendError),
}
```

Operational rules:

- plan validation and universe completeness run before native mutation;
- every restriction-mask transition is transactional or explicitly recoverable;
- uncertain rollback marks only the isolated analysis session `RequiresRebuild`;
- rebuilding preserves the same source model identity but creates a new analysis backend state as required by existing `CompilationId` semantics; the report must use the identity of the exact analyzed compiled state;
- a native provider failure in `Auto` may fall back to ROML only when no partial native mutation threatens the ROML oracle session;
- `NativeOnly` never silently falls back;
- timeout and cancellation return partial structured evidence when an infeasible subsystem has already been validated.

## 9. Performance design

The performance target is not merely fewer Rust operations; it is fewer expensive solver factorizations and less model reconstruction.

Required mechanisms:

1. one isolated backend build per analysis attempt under normal operation;
2. stable compiled indices throughout restriction toggling;
3. incremental row-bound and variable-bound updates;
4. simplex basis reuse where qualified;
5. backend-appropriate reoptimization, typically dual simplex after bound/row-side changes;
6. cached outcomes only within one exact `CompilationId`, candidate-universe identity, numerical policy, and restriction selection;
7. native/certificate/elastic seeds before full-universe reduction when available;
8. adaptive block deletion before single-member polish;
9. deterministic ordering for reproducibility;
10. explicit counters for oracle solves, LP iterations, rebuilds, mask operations, cache hits, seed size, and final size.

Do not make a universal claim that one reducer dominates all models. Implement the strategy behind an internal reducer interface and benchmark against:

- naive full-universe one-at-a-time deletion;
- native HiGHS result alone;
- native-seeded ROML reduction;
- ROML full-universe adaptive deletion;
- a later divide-and-conquer/QuickXplain-style reducer if evidence warrants it.

A performance optimization is accepted only if final semantic verification and report guarantees remain unchanged.

## 10. Tests and acceptance evidence

### 10.1 Correctness corpus

Include deterministic cases for:

- direct contradictory variable bounds;
- two-row and multi-row LP IISs;
- redundant constraints outside the IIS;
- equality and ranged-row side membership;
- lower-bound-only and upper-bound-only membership;
- persistent fixing versus declared bound provenance;
- solve lock and temporary fixing provenance;
- one semantic construct generating several compiled restrictions;
- two distinct IISs, proving ROML returns one IIS without claiming the smallest or all IISs;
- stale `CompilationId` mapping rejection;
- feasible model and unknown numerical status;
- time/oracle-call-limited partial results.

For every complete ROML result, tests must independently verify:

1. all returned members together are infeasible; and
2. removing each returned member makes the subsystem feasible under the same recorded oracle policy.

### 10.2 Differential and mutation tests

- Compare native HiGHS evidence with ROML portable results without requiring identical member sets when multiple IISs exist.
- Map a native compiled conflict to semantic atoms, then verify semantic irreducibility independently.
- Inject missing origin entries and malformed toggle plans; fail before analysis.
- Inject rollback failure; require isolated-session rebuild.
- Mutate bound-layer restoration logic; tests must catch fixing/lock provenance errors.
- Mutate completion/guarantee promotion; tests must prevent an incomplete run from claiming irreducibility.

### 10.3 Version qualification

- bundled audited HiGHS version: native IIS compile/link/run test;
- each supported system HiGHS version: explicit capability result;
- old/unqualified system version: build must not reference an unavailable IIS symbol and runtime must report typed `Unsupported`;
- record actual library version and build mode in evidence.

### 10.4 Performance benchmarks

Use planted-conflict LP families with increasing irrelevant-constraint counts and conflict sizes. Record:

- wall time;
- oracle calls;
- cumulative LP iterations;
- factorizations if available;
- rebuild count;
- initial/seed/final candidate sizes;
- basis reuse count;
- peak memory.

Acceptance requires evidence that the default hybrid is not pathologically worse than naive deletion on the benchmark corpus and materially improves at least the large sparse-conflict cases. Do not encode a marketing threshold before baseline measurements exist.

## 11. Implementation slices for P29

### Slice 0 — Contract and characterization

- audit current compiler/session/origin contracts;
- pin official HiGHS IIS API/version evidence;
- add failing API/guarantee tests;
- freeze public vocabulary: scope, provider, semantic atom, completion, guarantee.

### Slice 1 — Semantic restriction universe

- add `ConflictAtomId`, semantic atom types, compiled restriction refs, and restriction-level origin map;
- compile row sides, variable bounds, fixings, locks, and constructs into reversible atom toggles;
- prove completeness and exact disable/restore semantics.

### Slice 2 — Isolated LP feasibility oracle

- spawn an analysis session from `BackendSnapshot`;
- implement transactional mask changes, zero-objective feasibility solves, status triage, and basis reuse;
- add stale-compilation and rollback-rebuild tests.

### Slice 3 — ROML reducer and verifier

- cheap contradiction seed;
- full-universe seed fallback;
- adaptive block deletion;
- exact deletion polish;
- mandatory final verifier;
- budgets, interruption, partial completion, statistics.

### Slice 4 — Reports and renderers

- canonical structured report;
- deterministic text and Markdown views;
- original-name/provenance snapshots;
- concise `Display` summary;
- public exports at crate root and `roml::advanced` according to P28 conventions.

### Slice 5 — HiGHS native provider

- audited build/version gates;
- native row/column/bound extraction;
- native-only mode;
- native-seeded semantic reduction;
- bundled/system qualification matrix.

### Slice 6 — Performance and release evidence

- planted IIS benchmark corpus;
- native/portable differential suite;
- coverage and mutation targets;
- docs and examples;
- phase verification, independent review, and evidence packet.

Do not combine the MIP-only conflict algorithm with P29 merely because the API has future variants. Land the extension seam and stop after qualified LP behavior.

## 12. Next phase — LP-feasible but MIP-infeasible

The next conflict phase should reuse the P29 architecture while changing the oracle and candidate semantics.

Required additions:

- `OriginalMip` scope with a complete MIP feasibility oracle;
- explicit report distinction between original-MIP and LP-relaxation conflicts;
- native MIP conflict/IIS providers where officially qualified;
- MIP warm starts and incumbent/bound reuse for repeated feasibility checks;
- optional semantic **integrality atoms** that relax selected integer variables to continuous values for explanation;
- classical MIP-conflict mode where integrality remains ambient model structure;
- propagation, native conflict, branch-and-bound, and elastic seeds;
- exact deletion verification only when each MIP feasibility result is conclusive.

A complete MIP IIS can be much more expensive than LP IIS. Budgets and incomplete guarantees are therefore part of the core P29 report contract rather than retrofits.

## 13. Nonlinear and broader future compatibility

The public abstraction must remain function-in-set and restriction-oriented:

```text
semantic restriction atom
    -> compiled restriction(s)
    -> provider-specific feasibility oracle
    -> conflict evidence
    -> original semantic report
```

Future model classes add atom kinds and proof-strength variants; they do not replace the report or origin architecture.

### Convex QP/conic

- use backend-native conflicts or globally valid infeasibility certificates when available;
- preserve quadratic/conic function and set identity in conflict members;
- never flatten the public result into anonymous linearized rows.

### Smooth constrained NLP

- a local restoration failure is not a proof of global infeasibility;
- reports must distinguish `GloballyProvenInfeasible`, `LocallyInfeasibleCandidate`, `RestorationFailed`, and `Unknown`;
- only globally conclusive oracle results may support an IIS/irreducibility claim;
- local diagnostic subsets may be useful but must use different terminology.

### Nonconvex NLP and MINLP

- true IIS claims require a globally valid infeasibility oracle or an officially qualified native conflict claim;
- local solver termination cannot be promoted to IIS;
- spatial branch-and-bound/native global solver evidence can plug into the same provider contract;
- integrality, nonlinear domains, and generated reformulations retain separate semantic origins.

### Unconstrained optimization

Ordinary unconstrained optimization has no constraint infeasibility. Contradictory variable domains or model validation failures should be diagnosed before solve and may reuse member/report presentation without being mislabeled an IIS.

## 14. Explicit non-goals for P29

P29 does not:

- enumerate all IISs;
- compute a guaranteed minimum-cardinality IIS;
- implement feasibility relaxation;
- diagnose integrality-only MIP infeasibility;
- claim exact-rational proof from floating-point LP solves;
- expose raw backend row IDs as the ordinary report;
- mutate canonical model state during analysis;
- make HiGHS native IIS availability a requirement for ROML IIS;
- implement nonlinear feasibility oracles;
- publish, tag, or release crates.

## 15. Planning gate

The P29 plan is acceptable only if it preserves all of the following:

- ROML portable LP IIS is first-class and solver-agnostic;
- native HiGHS IIS is optional, audited, version-gated, and separately labeled;
- the user entry point is plan-driven session orchestration;
- semantic restriction atoms and exact compilation identity govern mapping;
- complete reports prove semantic irreducibility under recorded tolerances;
- incomplete runs cannot claim minimality;
- the algorithm uses an isolated incremental feasibility oracle, seed reduction, adaptive deletion, and final verification;
- LP scope lands before MIP-only and nonlinear scope;
- tests prove disable/restore semantics, origin completeness, and the guarantee itself.

If an implementation detail conflicts with these constraints, amend this packet explicitly rather than silently changing the model.
