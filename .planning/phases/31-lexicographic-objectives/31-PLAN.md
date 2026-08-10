# Phase 31 — Objective Policies and Lexicographic Solves Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** provide the single canonical objective-policy model and a deterministic portable weighted/lexicographic executor whose stage locks, continuation, cleanup, and reports are exact and backend-independent.

**Architecture:** P31 is the **sole owner** of canonical `ObjectivePolicy` and the shared `ObjectivePriority`. Policy is canonical model intent; execution/provider choice is solve-scoped. The portable sequential executor is normative and uses P27/P28 rollback-safe overlays. Native multiobjective is optional only after semantic qualification.

**Requirements:** SM-11.1–SM-11.8, SM-07.7 objective-stage closure, P27 objective-lock debt, and the priority-target portion of SM-10.6. Identity/overlay/error and lock-formula semantics are frozen in `SHARED-CONTRACTS.md`.

## Global constraints

- P31 begins only after P30 is accepted/merged.
- No second `ObjectivePolicy` type or second priority newtype exists anywhere in M3.
- `ObjectivePriority(0)` is highest/earliest; ascending numeric order defines stages.
- Every weight/tolerance is finite; weights/tolerances are nonnegative.
- Default continuation requires an optimal stage.
- Stage artifacts never persist in canonical state or following solves.
- Portable/native results share one structured result contract.
- Native provider semantics never weaken the portable contract.
- Primary + cleanup/rebuild failures are both preserved per shared contracts §2.

## Frozen public schemas

```rust
pub struct ObjectivePriority(u32);

pub struct WeightedObjective {
    pub objective: Objective,
    pub weight: f64,
}

pub struct WeightedObjectiveLevel {
    pub priority: ObjectivePriority,
    pub objectives: Vec<WeightedObjective>,
    pub absolute_tolerance: f64,
    pub relative_tolerance: f64,
}

pub struct WeightedObjectives {
    pub objectives: Vec<WeightedObjective>,
}

pub struct LexicographicObjectives {
    pub levels: Vec<WeightedObjectiveLevel>,
}

pub enum ObjectivePolicy {
    None,
    Single(Objective),
    Weighted(WeightedObjectives),
    Lexicographic(LexicographicObjectives),
}

pub enum StageContinuation {
    RequireOptimal,
    BestFeasible,
}

pub enum ObjectiveProviderPolicy {
    PortableOnly,
    PreferNative,
    NativeRequired,
}

pub struct ObjectiveValue {
    pub objective: Objective,
    pub value: f64,
}

pub enum StageContinuationDecision {
    ContinueOptimal,
    ContinueBestFeasible,
    StopNotOptimal,
    StopNoFeasiblePoint,
    StopUnknown,
}

pub struct ObjectiveLockReport {
    pub priority: ObjectivePriority,
    pub stage_sense: ObjectiveSense,
    pub reference_value: f64,
    pub absolute_tolerance: f64,
    pub relative_tolerance: f64,
    pub relative_scale: f64,
    pub allowed_degradation: f64,
    pub bound: f64,
}

pub struct ObjectiveStageResult {
    pub priority: ObjectivePriority,
    pub status: SolveStatus,
    pub continuation: StageContinuationDecision,
    pub objective_values: Vec<ObjectiveValue>,
    pub scalar_stage_value: Option<f64>,
    pub lock: Option<ObjectiveLockReport>,
    pub provider: ObjectiveExecutionProvider,
    pub compilation_id: CompilationId,
}

pub struct MultiObjectiveResult {
    pub final_solution: Solution,
    pub stages: Vec<ObjectiveStageResult>,
    pub provider: ObjectiveExecutionProvider,
}
```

Exact placement inside `Solution`/metadata can reuse existing structures, but the information above is mandatory and may not be split into competing result types.

## Objective-sense normalization

Each weighted scalar stage is normalized deterministically. One acceptable canonical convention is minimization:

```text
normalized term = +w * f(x) for MIN objective
normalized term = -w * f(x) for MAX objective
```

The stage scalar objective `g(x)` is the sum of normalized terms including each objective constant exactly once. The compiler/report records the normalization. All stage-lock math applies to this scalar stage objective or equivalently maps the bound back with its declared stage sense; tests must prove equivalence.

## Frozen degradation formula

From shared contracts §5:

```text
scale = abs(z*)
delta = abs_tol + rel_tol * scale
```

Minimization stage:

```text
f(x) <= z* + delta
```

Maximization stage:

```text
f(x) >= z* - delta
```

At `z*=0`, relative tolerance contributes zero. Negative optima use positive magnitude `|z*|`. No `max(1, |z*|)` or backend-default scale is permitted.

---

## Task 31-00: Characterize and freeze objective/priority ownership

- [ ] Freeze existing active-objective state, objective constants, objective override, P27 objective locks, P28 SolvePlan/effective-plan reporting, and P30 penalty types.
- [ ] Add compile-target fixture for weighted policy and two lexicographic levels using `ObjectivePriority`.
- [ ] Add negative API guard against a second priority alias/newtype and a second objective-policy owner.
- [ ] Reproduce P27 placeholder lock debt with failing test against the frozen `|z*|` formula.
- [ ] Record exact base/API inventory and leaf requirements, including P31 ownership of priority portion of SM-10.6.
- [ ] Commit characterization only.

## Task 31-01: Canonical `ObjectivePolicy` + `ObjectivePriority`

**Files:** objective model module, snapshot/delta/compiler IR, focused tests.

- [ ] Implement validated `ObjectivePriority`; 0 highest, ascending order.
- [ ] Implement `ObjectivePolicy::{None,Single,Weighted,Lexicographic}` as the sole canonical policy.
- [ ] Validate stale objectives, duplicate objective terms in one level, duplicate priorities, empty weighted set/level, and nonfinite/negative weights/tolerances atomically.
- [ ] Define whether duplicate objectives across different priorities are allowed; default is **allowed** because the same metric may legitimately refine at multiple stages, but each level owns its own weight/tolerance.
- [ ] Preserve current single-objective convenience API by mapping onto `Single` without changing golden-path source use.
- [ ] Prove clone/snapshot/delta/rebuild and revision semantics.
- [ ] Commit.

## Task 31-02: Backend IR, reports, provider policy

- [ ] Extend compiled objective-policy IR only with normalized solver-neutral data; never pass canonical handles directly to backend.
- [ ] Add typed native weighted/lexicographic features only if a provider can actually use them.
- [ ] Add solve-scoped `ObjectiveProviderPolicy::{PortableOnly,PreferNative,NativeRequired}` rather than overloading generic unsupported-feature policy.
- [ ] Compilation/effective-plan report records policy, normalized terms, priorities/tolerances, provider selection, and rejection/fallback reason.
- [ ] Primitive `Single` compilation/solve remains observationally equivalent to pre-P31 behavior.
- [ ] Commit.

## Task 31-03: Portable weighted scalar policy

- [ ] Reference tests combine min/max objectives, zero/positive weights, constants, and objectives whose optimal values are negative/zero/positive.
- [ ] Prove direct evaluation of original objectives equals normalized scalar expression transformation.
- [ ] If P31 allows parameterized objective weights, evaluate and validate them before backend mutation using the same pattern as P30 penalty weights; otherwise keep weights plain `f64` and reject any attempt to encode symbolic weights.
- [ ] HiGHS equivalence against a manually combined scalar objective.
- [ ] Commit.

## Task 31-04: Exact objective lock construction

- [ ] Table tests for min/max and `z* < 0`, `z*=0`, `z*>0`; abs-only, rel-only, both-zero, and mixed tolerances.
- [ ] Assert `relative_scale == abs(z*)` and exact derived bound in `ObjectiveLockReport`.
- [ ] Test objective constants exactly once.
- [ ] Test weighted multiobjective level lock against independently built scalar row.
- [ ] Replace P27 placeholder reference optimum with actual stage value.
- [ ] Generated lock origins include `ObjectivePriority`, normalized scalar-stage identity, and exact compilation identity.
- [ ] Commit.

## Task 31-05: Portable lexicographic executor

Algorithm:

```text
validate canonical policy + solve execution policy
synchronize exact base compilation
for levels sorted by ObjectivePriority ascending:
    apply temporary normalized objective override
    solve stage
    extract status + all objective values + scalar stage value
    classify continuation
    if another stage may run:
        create exact degradation lock from frozen formula
finally:
    rollback all stage artifacts
    verify base or require rebuild
return MultiObjectiveResult only after cleanup verification
```

- [ ] Known 2-level and 3-level fixtures with unique final result.
- [ ] `RequireOptimal`: time/iteration/unknown stage does not descend.
- [ ] `BestFeasible`: only a valid feasible incumbent may descend; stage result records `ContinueBestFeasible` and lock reference uses that incumbent value, never an unavailable bound.
- [ ] No-feasible/unknown status is distinguished from operational error.
- [ ] Inject failures at every apply/solve/extract/lock/rollback/verification/rebuild boundary.
- [ ] Successful mathematics + failed rollback returns composite operational error; no result leaks.
- [ ] Following ordinary solve sees no stage artifacts.
- [ ] Commit.

## Task 31-06: Final stage/result/metadata contract

- [ ] Populate every mandatory `ObjectiveStageResult` field honestly for portable execution.
- [ ] Final solution exposes all canonical objective values at the final point.
- [ ] Stage compilation IDs are exact; stale stage/lock artifacts reject on reuse.
- [ ] Text/Markdown structured rendering is deterministic and does not imply optimality for `BestFeasible` stages.
- [ ] Numerical/provider limitations are explicit in effective-plan metadata.
- [ ] Commit.

## Task 31-07: Activate P30 priority penalties

P31 adds the one new P30 enum variant:

```rust
PenaltyTarget::Priority(ObjectivePriority)
```

- [ ] No separate P30/P31 priority integer or alias is introduced.
- [ ] At execution, evaluate all parameterized P30 penalty weights to finite nonnegative numeric values **before** constructing the priority stage and before backend mutation.
- [ ] Missing referenced priority rejects atomically.
- [ ] Build integration example: priority 0 minimize weighted violation, priority 1 maximize economics, priority 2 minimize deviation/churn.
- [ ] Compare against an independent manually staged reference implementation.
- [ ] Verify zero-violation optimum with `z*=0` cannot reintroduce violation through a relative-only tolerance; only declared absolute tolerance can allow degradation.
- [ ] Commit.

## Task 31-08: Native multiobjective audit/provider

- [ ] Audit official pinned HiGHS API/version semantics for objectives, priorities, weights, abs/rel degradation, continuation/status, and objective constants.
- [ ] `PortableOnly` always uses sequential ROML execution.
- [ ] `PreferNative` selects native only if the entire frozen contract—including `|z*|` scale—is equivalent; otherwise portable with explicit reason.
- [ ] `NativeRequired` rejects before mutation when equivalence is not qualified.
- [ ] If native is qualified, run native/portable differential corpus across positive/zero/negative optima and mixed senses.
- [ ] Native absence does not block P31 portable completion.
- [ ] Commit audit/evidence.

## Task 31-09: Qualification and closure

- [ ] Independent sequential reference corpus for small weighted/lexicographic models.
- [ ] Full P30 priority integration corpus with parameterized penalty weights evaluated before stage execution.
- [ ] Fault matrix proves no objective/lock leak and preserves primary + rollback/rebuild failures.
- [ ] Record solve count, lock count, delta/rebuild behavior, provider/version, and objective numerical observations.
- [ ] Core/HiGHS/MSRV/fmt/clippy/rustdoc/package/coverage/quality/policy green on exact head.
- [ ] Independent review explicitly checks single ownership, priority type, sense normalization, `|z*|` formula, zero/negative optima, continuation, result schema, P30 integration, and cleanup composition.
- [ ] Zero unresolved P0/P1.
- [ ] Evidence `docs/release/evidence/P31_OBJECTIVE_POLICIES.md` includes leaf SM-11 rows + priority sub-clause of SM-10.6.
- [ ] Owner merge; P34 activates only afterward.

## P31 positive closure predicate

P31 is complete iff one canonical `ObjectivePolicy` and one `ObjectivePriority` exist, all SM-11 leaves and priority-target SM-10.6 evidence pass, portable weighted/lexicographic execution agrees with independent reference models, every lock uses the frozen `|z*|` formula, parameterized P30 penalties are numerically resolved before priority execution, fault cleanup is exact, exact-head CI/review are green, and owner merge is complete.

## Stop conditions

Stop and return to written-spec review if objective locks cannot be represented transactionally, the priority type fragments, native semantics require a different degradation formula, stage outcome/continuation cannot be represented without guarantee inflation, parameterized penalties remain symbolic at priority execution, or any failed stage can leak objective/lock state.
