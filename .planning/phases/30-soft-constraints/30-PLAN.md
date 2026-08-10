# Phase 30 — Soft Constraints and Feasibility Relaxation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** add mathematically explicit persistent soft constraints and a separate solve-scoped feasibility-relaxation workflow that composes with P29 IIS without confusing diagnosis, repair, provider choice, or cleanup failures.

**Architecture:** persistent softening is canonical semantic state compiled through the existing construct/origin framework; feasibility relaxation is an isolated/overlay solve workflow over a frozen set of semantic restriction kinds. Portable weighted-L1 behavior is normative. Native relaxation, if any, is a provider behind the same ROML contract.

**Requirements:** SM-10.1–SM-10.5, SM-10.7–SM-10.9; P30 closes the `None`/`Objective` portions of SM-10.6. P31 owns `ObjectivePolicy`, `ObjectivePriority`, and closes the priority-target portion of SM-10.6. Identity/overlay/error semantics are governed by `SHARED-CONTRACTS.md`.

## Global constraints

- P30 starts only after P36 is accepted/merged.
- Persistent softening advances canonical model revision; solve-scoped relaxation does not.
- Upper/lower violation variables are distinct stable semantic roles.
- No hidden objective mutation: penalties have explicit targets.
- Parameterized penalty weights are evaluated to finite nonnegative values before backend mutation.
- No priority target ships in P30; P31 adds the single shared `ObjectivePriority` integration.
- No L2/nonlinear relaxation in M3.
- IIS is a scope/diagnostic helper, never a minimum-repair proof.
- Native relaxation cannot define ROML semantics.
- Primary and rollback/rebuild failures are both preserved per shared contracts §2.

## Frozen P30 public concepts

Exact Rust module placement may follow current conventions, but semantic distinctions are binding.

```rust
pub struct SoftConstraint { /* opaque stable construct handle */ }

pub struct ViolationPolicy {
    pub max_violation: Option<f64>,
}

/// Existing ValueExpr/parameter-aware numeric expression is preferred over a
/// new one-parameter-only type.
pub struct PenaltyPolicy {
    pub weight: ValueExpr,
    pub target: PenaltyTarget,
}

pub enum PenaltyTarget {
    None,
    Objective(Objective),
    // P31 adds Priority(ObjectivePriority); P30 does not ship it early.
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
```

`UnsupportedFeaturePolicy` is **not** reused for provider selection; it governs a different SolvePlan concern. `RelaxationProviderPolicy` controls only native-vs-portable relaxation execution.

### Outcome semantics

- `OptimalRepair`: a relaxed solution is feasible for the relaxation model and weighted-L1 repair objective is proven optimal.
- `FeasibleRepair`: a relaxed feasible solution exists, but the repair objective is not proven optimal. The report must expose termination/bound/gap evidence; default P30 execution may choose to return this only under an explicit continuation policy.
- `NoRepairFound`: the permitted relaxation model is **proven infeasible** under the selected scope and finite violation caps. It is not used for limits, numerical failure, or missing provider.
- `Unknown(reason)`: solver termination gives neither an optimal/accepted feasible repair nor proof that no permitted repair exists.
- Operational failures (preflight, compile, backend mutation, extraction, rollback, verification, rebuild, I/O) are `Err(FeasibilityRelaxationError)` and are never encoded as `Unknown`/`NoRepairFound`.
- A mathematically successful solve followed by failed/uncertain rollback returns an operational error and marks `RequiresRebuild`; it is not returned as a reusable success report.

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
    pub provider: RelaxationProvider,
    pub termination: SolveStatus,
    pub numerics: RelaxationNumerics,
}

pub struct FeasibilityRelaxationPlan {
    pub scope: RelaxationScope,
    pub objective: RelaxationObjective,
    pub provider_policy: RelaxationProviderPolicy,
}

pub struct FeasibilityRelaxationReport {
    pub outcome: RelaxationOutcome,
    pub members: Vec<RelaxedRestriction>,
    pub total_weighted_violation: f64,
    pub metadata: RelaxationMetadata,
}
```

No report field is optional merely for appearance; each must be populated or omitted from the final API if the selected provider cannot produce it honestly.

---

## Task 30-00: Characterize seams and freeze leaf contracts

**Files:** tests + `.planning/phases/30-soft-constraints/30-CONTEXT.md`.

- [ ] Freeze current construct origins, effective/declared bounds, persistent fixing, objective replacement, parameter evaluation, overlay rollback/rebuild, and P29 restriction member shapes.
- [ ] Map each frozen concept above to exact existing type names; do not create aliases just to match this plan.
- [ ] Add compile-target fixture for one persistent soft upper constraint and one portable relaxation request.
- [ ] Add negative compile/API guard proving no P30 `ObjectivePolicy`, priority newtype, or generic `UnsupportedFeaturePolicy` provider field is introduced.
- [ ] Record exact base SHA/API inventory and leaf requirement ownership, including SM-10.6 split with P31.
- [ ] Commit characterization only.

## Task 30-01: Canonical persistent soft constraints

**Files:** focused model/construct module + `tests/soft_constraints.rs`.

- [ ] Red tests for upper, lower, equality, and ranged softening through one builder call.
- [ ] Preserve original constraint as reporting anchor; create stable lower/upper violation handles/roles with generated provenance according to accepted D15.
- [ ] Validate stale/inactive/unsupported constraint inputs atomically.
- [ ] Store penalty weight as existing parameter-aware numeric expression; capture dependencies canonically.
- [ ] Prove clone/snapshot/delta/remove behavior and revision advancement.
- [ ] Commit.

## Task 30-02: Exact algebra, caps, origins

Required formulas:

```text
upper f(x) <= u  => f(x) - v_up <= u,  v_up >= 0
lower f(x) >= l  => f(x) + v_lo >= l,  v_lo >= 0
equality          => both sides with distinct violations
ranged            => both sides with distinct violations
```

- [ ] Direct algebra tests include nonzero constants and parameterized functions.
- [ ] Validate finite nonnegative `max_violation`; cap the generated violation variable directly.
- [ ] Complete deterministic origin roles for every generated row/variable/coefficient.
- [ ] Differential solve against hand-built reference formulations for all row senses.
- [ ] Commit.

## Task 30-03: Penalty evaluation and objective targeting

- [ ] Test min and max objectives: positive violation must worsen the selected objective after sense normalization.
- [ ] Test constant/zero/parameterized weights and parameter updates.
- [ ] Before compilation/backend mutation, evaluate every active weight against the exact model revision; reject negative, NaN, ±inf, stale/unbound evaluation.
- [ ] Record evaluated penalty weights in compilation/relaxation evidence so P31 never executes a symbolic/unchecked priority penalty.
- [ ] Implement only `PenaltyTarget::{None,Objective}` in P30.
- [ ] Verify parameter delta changes penalty coefficient mathematically and rebuild/delta equivalence remains valid.
- [ ] Commit.

## Task 30-04: Solution violation accessors

- [ ] Expose raw lower/upper/total violation by `SoftConstraint` and original constraint identity.
- [ ] Keep raw primal violation separate from tolerance-adjusted diagnostic presentation.
- [ ] Define behavior for missing values/non-feasible solution statuses according to existing `Solution` rules.
- [ ] Add deterministic named rendering.
- [ ] Commit.

## Task 30-05: Signed correction separate API

- [ ] Define signed correction through positive/negative parts and explicit objective semantics.
- [ ] Prove by tests that it is not inferred from or conflated with ordinary one/two-sided softening.
- [ ] Keep it out of defaults.
- [ ] Commit.

## Task 30-06: Portable solve-scoped weighted-L1 relaxation

**Files:** create focused solver workflow module; core reference/fault tests + HiGHS integration.

Eligible P30 restriction kinds:

```text
ConstraintSide     original active primitive linear constraint lower/upper side
VariableBound      original declared variable lower/upper side
PersistentFixing   canonical fixing restriction as one two-sided repair atom
```

Not eligible in P30:

```text
TemporaryLock
SolveOverlay-only restriction
Grouped high-level semantic Construct
compiler-generated-only row/bound
unknown/native-only member
```

- [ ] Build explicit portable relaxation model/overlay from exact base state; preserve canonical revision.
- [ ] For persistent fixing, use one semantic member with positive/negative deviation and weighted-L1 magnitude; do not misreport it as a declared bound.
- [ ] Populate base + relaxation compilation IDs and exact provider/termination/numerical metadata.
- [ ] Implement outcome classification exactly as frozen above.
- [ ] Inject failures at preflight, compile, apply, solve, extraction, rollback, rollback verification, and rebuild.
- [ ] Assert primary + cleanup/rebuild failures survive composition and uncertain state becomes `RequiresRebuild`.
- [ ] Commit.

## Task 30-07: Exact P29 IIS -> relaxation mapping

**Mapping table:**

| P29 member/origin | P30 action |
|---|---|
| original primitive constraint lower/upper side | map to `ConstraintSide` |
| imported MPS row side resolved to original constraint | map to `ConstraintSide`; preserve source provenance in rendered report |
| declared variable lower/upper bound | map to `VariableBound` |
| imported MPS explicit/synthetic bound resolved to declared variable side | map to `VariableBound`; preserve explicit/synthetic source origin |
| persistent fixing atom | map to `PersistentFixing` |
| temporary solution lock / overlay fixing | **reject `UnsupportedRelaxationOrigin`** |
| grouped semantic construct | **reject `UnsupportedRelaxationOrigin` in P30** |
| compiled-only/generated member lacking original supported semantic atom | **reject** |
| stale model/revision/CompilationId | **reject stale analysis before mutation** |

- [ ] Conversion is all-or-error: never silently drop unsupported IIS members.
- [ ] Preserve report identity checks and exact current state/compilation agreement.
- [ ] Document that IIS scoping can reduce search space but does not prove minimum-weight or minimum-cardinality global repair relative to the unrestricted model.
- [ ] End-to-end fixture: imported infeasible MPS -> P29 IIS -> mapping -> weighted-L1 relaxation -> provenance-aware report.
- [ ] Commit.

## Task 30-08: Native provider audit

- [ ] Audit pinned official HiGHS APIs for feasibility relaxation semantics.
- [ ] `PortableOnly` always uses ROML path.
- [ ] `PreferNative` uses native only after exact semantic qualification; otherwise portable with explicit provider metadata.
- [ ] `NativeRequired` returns typed unsupported before mutation if no qualified native provider exists.
- [ ] If native is qualified, differential-test member violations, objective, outcome, and numerical metadata against portable path.
- [ ] Do not delay P30 if native remains unsupported.
- [ ] Commit audit/evidence.

## Task 30-09: Qualification and closure

- [ ] Reference-formulation corpus for all senses/caps/weight forms and both objective senses.
- [ ] P29->P30 imported-MPS fixtures including explicit/synthetic bound provenance and unsupported-origin rejection.
- [ ] Fault matrix proves no leak and exact composite errors.
- [ ] Record solve/oracle counts, objective/numerical metadata, and representative overhead telemetry.
- [ ] Core/HiGHS/MSRV/fmt/clippy/rustdoc/package/coverage/quality/policy green on exact head.
- [ ] Independent OR-formulation review explicitly checks signs, caps, equality/ranged algebra, outcome guarantees, IIS mapping, and error composition.
- [ ] Zero unresolved P0/P1.
- [ ] Evidence `docs/release/evidence/P30_SOFT_CONSTRAINTS_RELAXATION.md` includes leaf requirement rows and residuals.
- [ ] Owner merge; P31 activates only after merge.

## P30 positive closure predicate

P30 is complete iff every P30-owned SM-10 leaf/sub-clause has evidence, portable weighted-L1 semantics and outcome classification pass reference/differential/fault tests, every supported P29 origin maps exactly and unsupported origins reject, parameterized weights are resolved before mutation, no cleanup failure is lost, exact-head CI/review are green, and owner merge is complete.

## Stop conditions

Stop and return to design review if persistent softening requires solve-scoped state in canonical model, relaxation rollback cannot be proven, outcome classification would conflate solver unknown with no repair, an IIS member would need silent dropping, objective penalty sign depends on backend convention, or native semantics cannot map exactly to the portable report contract.
