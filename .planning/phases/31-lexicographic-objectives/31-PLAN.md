# Phase 31 — Objective Policies and Lexicographic Solves Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** provide one canonical objective-policy model and a deterministic portable weighted/lexicographic executor whose stage locks, continuation, cleanup, and reporting are exact.

**Requirements:** SM-11.1–SM-11.8, SM-07.7 objective-stage closure, P27 objective-lock debt, and priority-target SM-10.6. `SHARED-CONTRACTS.md` governs identity/overlay/error/lock semantics.

## Global constraints

- P31 starts only after P30 is accepted/merged and state authorizes P31.
- P31 is sole owner of `ObjectivePolicy` and `ObjectivePriority`.
- `ObjectivePriority(0)` is highest; levels execute in ascending numeric order.
- **P31 objective weights are plain finite nonnegative `f64` in M3.** Parameterized objective weights are deferred; P30 penalty weights remain parameterized and are numerically resolved before priority execution.
- Default stage continuation is `RequireOptimal`.
- Stage artifacts never persist.
- Portable sequential execution is normative; native is optional only on exact semantic equivalence.
- Primary and cleanup/rebuild failures are both retained.

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

pub enum ObjectiveExecutionProvider {
    PortableSequential,
    Native { backend: String, version: String },
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
    pub reference_value: f64,
    pub absolute_tolerance: f64,
    pub relative_tolerance: f64,
    pub relative_scale: f64,
    pub allowed_degradation: f64,
    pub normalized_upper_bound: f64,
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

Exact placement may reuse current `Solution`/metadata structures, but the information above is mandatory and may not split into competing result models.

## Weighted-stage normalization

M3 uses one canonical minimization normalization:

```text
normalized term = +w * f(x) for original MIN objective
normalized term = -w * f(x) for original MAX objective
```

All P31 `WeightedObjective.weight` values are finite nonnegative `f64`. The normalized scalar stage `g(x)` includes every referenced objective constant exactly once.

Because `g` is always minimized, P31 stage locks are represented canonically as:

```text
z* = solved normalized scalar stage value
scale = abs(z*)
delta = abs_tol + rel_tol * scale
g(x) <= z* + delta
```

This is exactly the shared `|z*|` formula expressed after sense normalization. At `z*=0`, relative tolerance contributes zero. Negative values use positive magnitude. No `max(1, |z*|)` scale exists.

Native providers may internally use other sense forms only if their externally observed semantics are proven equivalent to this normalized contract.

---

## Task 31-00 — characterize and freeze ownership/API

- [ ] Freeze current objectives/constants/override/P27 locks/P28 effective-plan reporting/P30 penalty types.
- [ ] Compile-target weighted + two-level lexicographic policy using `ObjectivePriority`.
- [ ] Negative API guard: no second objective policy/priority type; no symbolic/parameterized P31 objective weight type.
- [ ] Reproduce P27 lock debt against `|z*|` formula.
- [ ] Record exact base/API inventory and priority-target SM-10.6 ownership.

## Task 31-01 — canonical policy / priority model

- [ ] Implement `ObjectivePriority`, `ObjectivePolicy::{None,Single,Weighted,Lexicographic}`.
- [ ] Validate stale refs, duplicate objectives within a level, duplicate priorities, empty sets/levels, nonfinite/negative weights/tolerances atomically.
- [ ] Same objective may appear at different priorities; each level owns its own weight/tolerances.
- [ ] Existing single-objective convenience API maps to `Single` without golden-path break.
- [ ] Clone/snapshot/delta/rebuild/revision semantics proven.

## Task 31-02 — backend IR / reporting / provider policy

- [ ] Solver-neutral normalized policy IR only; no backend receives canonical object handles directly.
- [ ] Add native weighted/lexicographic capabilities only if executable.
- [ ] `ObjectiveProviderPolicy::{PortableOnly,PreferNative,NativeRequired}` is solve-scoped and separate from generic unsupported-feature policy.
- [ ] Effective plan/report records normalized terms, priorities/tolerances, provider and fallback/rejection reason.
- [ ] Primitive `Single` remains observationally equivalent to pre-P31 behavior.

## Task 31-03 — portable weighted policy

- [ ] Independent references combine min/max objectives, zero/positive weights, constants, and negative/zero/positive objective values.
- [ ] Reject any non-finite/negative weight before canonical mutation/compilation.
- [ ] Parameterized objective weights are explicitly unsupported in M3 rather than represented by a second expression type.
- [ ] HiGHS equivalence against manually combined scalar objective.

## Task 31-04 — exact lock construction

- [ ] Table-test `z*<0`, `z*=0`, `z*>0`; abs-only, rel-only, zero, mixed tolerances.
- [ ] Assert `relative_scale == abs(z*)` and `normalized_upper_bound == z* + delta`.
- [ ] Test objective constants once and weighted-level lock versus independent scalar row.
- [ ] Replace P27 placeholder reference optimum with actual stage value.
- [ ] Generated lock origins include priority, normalized stage identity, exact CompilationId.

## Task 31-05 — portable lexicographic executor

```text
validate policy + provider/continuation
synchronize exact base
for priorities ascending:
  apply normalized objective override
  solve
  extract status + objective vector + scalar stage value
  classify continuation
  if next stage allowed: add exact normalized degradation lock
finally rollback all temporary artifacts and verify base
return result only after cleanup verification
```

- [ ] Known 2/3-level unique-result fixtures.
- [ ] `RequireOptimal`: any nonoptimal stage stops descent.
- [ ] `BestFeasible`: descend only from a valid feasible incumbent; lock uses its scalar incumbent value; record `ContinueBestFeasible` without implying optimality.
- [ ] No-feasible/Unknown outcomes are separate from operational errors.
- [ ] Fault injection every apply/solve/extract/lock/rollback/verify/rebuild boundary.
- [ ] Successful math + failed cleanup => composite error and no `MultiObjectiveResult`.
- [ ] Ordinary solve afterward proves no leaked stage state.

## Task 31-06 — stage/result metadata

- [ ] Populate every mandatory stage field honestly for portable provider.
- [ ] Final solution exposes all canonical objective values at final point.
- [ ] Exact stage CompilationIds; stale stage/lock artifacts reject.
- [ ] Deterministic rendering never overstates `BestFeasible` optimality.

## Task 31-07 — activate P30 priority penalties

P31 adds exactly:

```rust
PenaltyTarget::Priority(ObjectivePriority)
```

- [ ] No alternate numeric priority type.
- [ ] Evaluate every P30 parameterized penalty weight to a finite nonnegative numeric value **before** constructing priority stage and before backend mutation.
- [ ] Missing referenced priority rejects atomically.
- [ ] Integration reference: priority 0 minimize weighted violation; priority 1 maximize economics; priority 2 minimize deviation/churn.
- [ ] Zero-violation `z*=0` with relative-only tolerance cannot reintroduce violation; only absolute tolerance can.

## Task 31-08 — native provider audit

- [ ] Audit official pinned HiGHS priorities/weights/tolerances/status/constants semantics.
- [ ] `PortableOnly` always portable.
- [ ] `PreferNative` selects native only when entire normalized contract—including `|z*|` scale—is equivalent; else portable with explicit reason.
- [ ] `NativeRequired` rejects before mutation if unqualified.
- [ ] Qualified native path differentially covers mixed senses and positive/zero/negative optima.
- [ ] Native absence does not block P31 portable completion.

## Task 31-09 — qualification / closure

- [ ] Independent weighted/lexicographic reference corpus.
- [ ] Full P30 priority integration with parameterized penalty resolution before stage execution.
- [ ] Fault matrix proves no objective/lock leakage and no lost primary/cleanup errors.
- [ ] Core/HiGHS/MSRV/fmt/clippy/rustdoc/package/coverage/quality/policy green exact head.
- [ ] Independent review checks sole ownership, numeric weight contract, normalization, `|z*|`, continuation, result schema, P30 integration, cleanup.
- [ ] Leaf evidence `P31_OBJECTIVE_POLICIES.md`; zero unresolved P0/P1; owner merge.

## Positive closure predicate

P31 is complete iff one canonical `ObjectivePolicy` and one `ObjectivePriority` exist; all P31 objective weights satisfy the frozen numeric contract; all SM-11 leaves and priority SM-10.6 have evidence; portable execution agrees with independent reference models; every lock uses normalized `|z*|` math; P30 parameterized penalties resolve before priority construction; cleanup is exact; exact-head mandatory CI/review pass; and owner merge completes.

## Stop conditions

Stop for design review if objective locks cannot be transactional, priority fragments, native semantics require different degradation guarantees, stage outcomes cannot be reported honestly, P30 penalties remain symbolic at execution, or failed stages can leak state.
