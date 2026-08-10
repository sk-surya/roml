# Phase 30 — Soft Constraints and Feasibility Relaxation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** add exact persistent soft constraints and a separate solve-scoped weighted-L1 repair workflow with explicit provider, acceptance, outcome, provenance, numerical, and cleanup semantics.

**Requirements:** SM-10.1–SM-10.5, SM-10.7–SM-10.9 plus `None`/`Objective` portions of SM-10.6. P31 owns `ObjectivePolicy`, `ObjectivePriority`, and the priority-target portion of SM-10.6. `SHARED-CONTRACTS.md` governs identity/overlay/error semantics.

## Global constraints

- P30 starts only after P36 is accepted/merged and state authorizes P30.
- Persistent softening is canonical/revisioned; solve-scoped relaxation is not.
- Upper/lower violation roles are distinct with complete origins.
- Parameterized penalty weights resolve to finite nonnegative numbers before backend mutation.
- P30 ships no priority target; P31 adds one when priority execution exists.
- Portable weighted-L1 is normative; native is an optional provider.
- IIS scopes/diagnoses; it never proves minimum repair.
- Primary and cleanup/rebuild failures are both retained.

## Frozen public contracts

```rust
pub struct SoftConstraint { /* opaque stable construct handle */ }

pub struct ViolationPolicy {
    pub max_violation: Option<f64>,
}

pub struct PenaltyPolicy {
    pub weight: ValueExpr,
    pub target: PenaltyTarget,
}

pub enum PenaltyTarget {
    None,
    Objective(Objective),
    // P31 later adds Priority(ObjectivePriority).
}

pub enum RelaxationRestriction {
    ConstraintSide { constraint: Constraint, side: BoundSide },
    VariableBound { variable: Variable, side: BoundSide },
    PersistentFixing { variable: Variable },
}

pub enum RelaxationScope {
    AllEligible,
    Explicit(Vec<RelaxationRestriction>),
}

pub enum RelaxationObjective {
    WeightedL1,
}

pub enum RelaxationProviderPolicy {
    PortableOnly,
    PreferNative,
    NativeRequired,
}

pub enum RelaxationAcceptance {
    RequireOptimal,
    AcceptFeasible,
}

pub enum RelaxationExecutionProvider {
    PortableRoml,
    Native { backend: String, version: String },
}

pub enum RelaxationOutcome {
    OptimalRepair,
    FeasibleRepair,
    NoRepairFound,
    Unknown(RelaxationUnknownReason),
}

pub enum RelaxationUnknownReason {
    TimeLimit,
    IterationLimit,
    Numerical,
    Interrupted,
    Unclassified,
}

pub struct FeasibilityRelaxationPlan {
    pub scope: RelaxationScope,
    pub objective: RelaxationObjective,
    pub provider_policy: RelaxationProviderPolicy,
    pub acceptance: RelaxationAcceptance,
}
```

Defaults are frozen:

```text
scope           = AllEligible
objective       = WeightedL1
provider_policy = PortableOnly
acceptance      = RequireOptimal
```

`UnsupportedFeaturePolicy` is not reused for P30 provider selection.

## Outcome semantics

- `OptimalRepair`: relaxed feasible solution + repair objective proven optimal.
- `FeasibleRepair`: allowed **only** when `acceptance == AcceptFeasible`, a valid feasible relaxed solution exists, and optimality is not proven. Report must contain termination/bound/gap evidence available from the provider.
- Under `RequireOptimal`, a feasible incumbent without optimality proof is returned as `Unknown(limit/interrupted/unclassified as appropriate)`, not `FeasibleRepair`.
- `NoRepairFound`: permitted relaxation model is proven infeasible under selected scope and finite violation caps.
- `Unknown(reason)`: no accepted repair and no proof that no repair exists.
- Preflight/compile/apply/solve/extract/rollback/verification/rebuild failures are `Err(FeasibilityRelaxationError)`, never mathematical outcomes.
- Successful mathematics followed by failed/uncertain cleanup returns operational error and `RequiresRebuild`; no success report escapes.

## Required report schema

```rust
pub struct RelaxedRestriction {
    pub restriction: RelaxationRestriction,
    pub violation: f64,
    pub evaluated_weight: f64,
    pub weighted_violation: f64,
}

pub struct RelaxationNumerics {
    pub objective_value: Option<f64>,
    pub best_bound: Option<f64>,
    pub absolute_gap: Option<f64>,
    pub relative_gap: Option<f64>,
    pub feasibility_tolerance: Option<f64>,
    pub integrality_tolerance: Option<f64>,
}

pub struct RelaxationMetadata {
    pub model_lineage: ModelLineageId,
    pub model_instance: ModelInstanceId,
    pub model_revision: ModelRevision,
    pub base_compilation_id: CompilationId,
    pub relaxation_compilation_id: CompilationId,
    pub provider: RelaxationExecutionProvider,
    pub termination: SolveStatus,
    pub numerics: RelaxationNumerics,
}

pub struct FeasibilityRelaxationReport {
    pub outcome: RelaxationOutcome,
    pub members: Vec<RelaxedRestriction>,
    pub total_weighted_violation: f64,
    pub metadata: RelaxationMetadata,
}
```

Any field a provider cannot populate honestly must be omitted from the final public schema during Task 30-00 API freeze, not populated with fabricated values.

## Exact persistent algebra

```text
upper f(x) <= u -> f(x) - v_up <= u, v_up >= 0
lower f(x) >= l -> f(x) + v_lo >= l, v_lo >= 0
equality          -> distinct lower + upper violations
ranged            -> distinct lower + upper violations
```

Finite `max_violation` must be nonnegative and caps the generated violation variable. Signed correction remains a separate API using positive/negative parts.

## Exact P29 IIS -> P30 map

| P29 member/origin | P30 action |
|---|---|
| original primitive constraint lower/upper | `ConstraintSide` |
| imported MPS row side resolved to original constraint | `ConstraintSide`, retain source provenance |
| declared variable lower/upper bound | `VariableBound` |
| imported explicit/synthetic bound resolved to declared side | `VariableBound`, retain source provenance |
| persistent fixing atom | `PersistentFixing` |
| temporary lock / overlay-only fixing | reject `UnsupportedRelaxationOrigin` |
| grouped semantic construct | reject `UnsupportedRelaxationOrigin` in P30 |
| compiler-generated-only member | reject |
| stale model/revision/CompilationId | reject before mutation |

Mapping is all-or-error. No IIS member is silently dropped.

---

## Task 30-00 — characterize and freeze API

- [ ] Freeze current construct/origin, declared/effective bounds, fixing, parameter evaluation, objectives, overlays, and P29 member shapes.
- [ ] Map conceptual types above onto exact existing names; avoid duplicate wrappers.
- [ ] Add compile-target fixture for persistent softening + portable relaxation.
- [ ] Negative API guards: no P30 `ObjectivePolicy`, priority type/field, or generic unsupported-provider field.
- [ ] Verify default provider/acceptance values and all outcome distinctions.
- [ ] Record exact base/API inventory and SM-10.6 split.

## Task 30-01 — canonical persistent soft constraints

- [ ] TDD upper/lower/equality/ranged softening.
- [ ] Stable violation handles/roles + original constraint reporting anchor.
- [ ] Atomic rejection for stale/inactive/unsupported inputs.
- [ ] Penalty dependencies canonical; clone/snapshot/delta/remove/revision behavior proven.

## Task 30-02 — exact algebra, caps, origins

- [ ] Independent reference-formulation tests for all row senses, constants, and parameterized functions.
- [ ] Validate finite nonnegative caps and complete generated origins.
- [ ] HiGHS/reference differential equivalence.

## Task 30-03 — penalty evaluation and objective targeting

- [ ] Min/max objective sign tests, zero/positive/parameterized weights.
- [ ] Evaluate weights against exact model revision before compilation/mutation; reject negative/NaN/inf/stale/unbound values.
- [ ] Record evaluated weights in evidence/metadata.
- [ ] Implement only `PenaltyTarget::{None,Objective}`.
- [ ] Parameter updates preserve delta/rebuild mathematical equivalence.

## Task 30-04 — violation accessors + signed correction

- [ ] Solution accessors for raw lower/upper/total violations by soft constraint/original constraint.
- [ ] Separate raw value from tolerance-adjusted presentation.
- [ ] Implement signed correction as a separate explicit API; never infer it from softening.

## Task 30-05 — portable solve-scoped weighted-L1 provider

- [ ] Build isolated/overlay repair model from exact base state without canonical revision change.
- [ ] Restriction support exactly matches mapping table.
- [ ] Persistent fixing uses one semantic repair atom with positive/negative deviation magnitude.
- [ ] Populate exact base/relaxation CompilationIds, provider, termination, numerics.
- [ ] Implement `RequireOptimal`/`AcceptFeasible` classification exactly.
- [ ] Fault injection at preflight/compile/apply/solve/extract/rollback/verify/rebuild; preserve primary + cleanup failures.

## Task 30-06 — P29 composition

- [ ] Implement all-or-error IIS conversion table.
- [ ] Exact instance/revision/CompilationId freshness checks before mutation.
- [ ] Imported MPS -> P29 IIS -> P30 relaxation -> source-aware report fixture.
- [ ] Document that IIS scoping does not prove global minimum-weight/cardinality repair.

## Task 30-07 — native provider audit

- [ ] Audit pinned official HiGHS relaxation semantics.
- [ ] `PortableOnly` always portable.
- [ ] `PreferNative` uses native only on exact contract equivalence, else portable with reason.
- [ ] `NativeRequired` rejects before mutation when unqualified.
- [ ] Qualified native path differentially compares members/objective/outcome/numerics to portable.
- [ ] Native absence does not block portable P30 completion.

## Task 30-08 — qualification / closure

- [ ] Full reference corpus across senses/caps/weights/objective senses and both acceptance policies.
- [ ] P29 supported/unsupported-origin corpus with imported explicit/synthetic bounds.
- [ ] Fault matrix proves no leaks/composite error loss.
- [ ] Core/HiGHS/MSRV/fmt/clippy/rustdoc/package/coverage/quality/policy green exact head.
- [ ] Independent OR review checks algebra/signs/caps/outcomes/IIS mapping/cleanup.
- [ ] Leaf evidence `P30_SOFT_CONSTRAINTS_RELAXATION.md`; zero unresolved P0/P1; owner merge.

## Positive closure predicate

P30 is complete iff every P30-owned SM-10 leaf/sub-clause has evidence; persistent algebra and portable weighted-L1 provider agree with independent references; both acceptance policies classify outcomes exactly; all supported P29 origins map and unsupported origins reject; parameter weights resolve before mutation; no cleanup failure is lost; exact-head mandatory CI/review pass; and owner merge completes.

## Stop conditions

Stop for design review if solve-scoped state must enter canonical softening, rollback cannot be proven, `Unknown` and `NoRepairFound` cannot remain distinct, unsupported IIS members would need silent omission, penalty sign depends on backend convention, or native provider semantics cannot map exactly.
