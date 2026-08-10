# Phase 31 — Objective Policies and Lexicographic Solves Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** provide canonical single/weighted/lexicographic objective policies and a deterministic portable staged executor whose temporary objective locks are exact, reversible, and fully reported.

**Architecture:** objective policy is canonical semantic state; execution policy is solve-scoped. The portable executor is normative and uses P27/P28 objective overrides and rollback-safe overlays. Native multiobjective is optional and supported only after semantic qualification.

**Requirements:** SM-11.1–SM-11.8, SM-07.7, P27 objective-lock debt closure, P30 penalty-priority integration.

## Global constraints

- No hidden solver-default multiobjective semantics.
- Every weight/tolerance finite; weights nonnegative.
- Objective sense normalization is explicit.
- Default lexicographic continuation requires optimal stage.
- `BestFeasible` continuation is explicit and appears in results.
- Stage artifacts never persist in canonical model or subsequent solves.
- Portable/native results share one structured contract.

## Target concepts

```rust
pub struct WeightedObjective {
    pub objective: Objective,
    pub weight: f64,
}

pub struct WeightedObjectives {
    pub objectives: Vec<WeightedObjective>,
}

pub struct LexicographicLevel {
    /// P30 PenaltyTarget::Priority(n) maps to this stable numeric priority.
    pub priority: u32,
    pub objectives: Vec<WeightedObjective>,
    pub absolute_tolerance: f64,
    pub relative_tolerance: f64,
}

pub struct LexicographicObjectives {
    pub levels: Vec<LexicographicLevel>,
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

pub struct ObjectiveValue {
    pub objective: Objective,
    pub value: f64,
}

pub struct ObjectiveLockReport {
    pub priority: u32,
    pub reference_value: f64,
    pub absolute_tolerance: f64,
    pub relative_tolerance: f64,
    pub allowed_degradation: f64,
}

pub struct ObjectiveStageResult {
    pub priority: u32,
    pub status: SolveStatus,
    pub objective_values: Vec<ObjectiveValue>,
    pub lock: Option<ObjectiveLockReport>,
    pub compilation_id: CompilationId,
}
```

Existing `Objective`, `SolveStatus`, and `CompilationId` are reused. If Task 31-00 finds established equivalent result/value types, use them rather than create duplicate wrappers; update the phase context before production code.

---

### Task 31-00: Characterize existing objective state and P27 lock debt

- [ ] Freeze current single-objective semantics, activation/replacement, objective constants, `SolvePlan::objective_override`, overlay objective locks, and solution metadata.
- [ ] Map every target concept above to existing reusable types or new P31-owned types; prohibit duplicate wrappers where an equivalent stable type already exists.
- [ ] Add target compile fixture for weighted and 2-level lexicographic policy.
- [ ] Reproduce P27 accepted debt: objective-lock degradation currently lacks real stage optimum semantics; write failing characterization for the intended P31 behavior.
- [ ] Record base API/evidence.
- [ ] Commit characterization only.

### Task 31-01: Canonical objective-policy model

**Files:** objective model module, snapshot/delta/compiler IR, tests.

- [ ] Define `ObjectivePolicy::{None,Single,Weighted,Lexicographic}` in canonical semantic state.
- [ ] Validate duplicate objective references, stale objectives, duplicate priorities, empty levels, weight/tolerance finiteness/nonnegativity, deterministic priority order.
- [ ] Weighted policy normalizes objective senses into one minimization or maximization convention before combination; document formula.
- [ ] Model mutations affecting active policy are atomic and revisioned.
- [ ] Clone/snapshot/delta/rebuild equivalence tests.
- [ ] Keep existing single-objective convenience API source-compatible by mapping it onto `Single`.
- [ ] Commit.

### Task 31-02: Backend IR and capability contract

- [ ] Extend backend objective-policy IR only as needed; do not pass canonical object references into backends.
- [ ] Add typed features for native weighted/lexicographic support if not already present.
- [ ] Portable support is separate from native support.
- [ ] Compile reports list objective policy, selected representation, normalized senses/weights, and rejection reasons.
- [ ] Primitive `Single` path remains equivalent to pre-P31 compilation.
- [ ] Commit.

### Task 31-03: Portable weighted objectives

- [ ] Add reference-formulation tests combining min and max objectives with positive/zero weights and nonzero constants.
- [ ] Prove normalized combined scalar objective matches direct evaluation.
- [ ] Parameter-dependent objective coefficients/weights update correctly if supported by semantic representation; otherwise reject parameterized weights explicitly rather than snapshotting stale values.
- [ ] HiGHS solve equivalence against manually combined objective.
- [ ] Commit.

### Task 31-04: Derive exact degradation-lock semantics

Define one documented scale rule valid around zero. Recommended lock tolerance magnitude:

```text
allowed_degradation = abs_tol + rel_tol * max(1, abs(z_star))
```

Then for a minimization stage:

```text
f(x) <= z_star + allowed_degradation
```

For maximization:

```text
f(x) >= z_star - allowed_degradation
```

If the existing approved design specifies another scale, the implementation must choose one explicitly and update this plan through review before coding.

- [ ] Table tests for min/max with `z*` positive, zero, negative; abs-only, rel-only, both-zero tolerances.
- [ ] Test objective constants are included exactly once.
- [ ] Test multi-objective weighted stage lock against direct scalar expression.
- [ ] Replace P27 placeholder/zero-reference behavior with stage-result-based lock construction.
- [ ] Ensure lock origins identify priority and objective policy.
- [ ] Commit.

### Task 31-05: Portable lexicographic executor

**Files:** solver orchestration module + core fault backend + HiGHS tests.

Algorithm:

```text
validate policy + plan against model state
sync/compile exact base
for each level in deterministic priority order:
    apply temporary objective override
    solve exactly once for stage attempt
    capture status/objective values/CompilationId
    apply continuation rule
    if another level remains:
        derive and apply objective lock
finally:
    rollback all stage artifacts
    verify base state or mark RequiresRebuild
return aggregate stage report
```

- [ ] Write 2-level and 3-level reference examples with known unique outcome.
- [ ] Write `RequireOptimal` test where stage hits time/iteration limit: later stages must not run.
- [ ] Write `BestFeasible` test where feasible incumbent continues and result records qualification.
- [ ] Add failure injection before/after every stage apply/solve/extract/lock/rollback boundary.
- [ ] Prove no artifact leaks into following ordinary solve.
- [ ] Commit.

### Task 31-06: Result/report contract

- [ ] `Solution`/metadata expose all canonical objective values at final solution and each `ObjectiveStageResult`.
- [ ] Record stage status, continuation decision, optimum/incumbent used for lock, abs/rel tolerance, derived lock, provider, exact CompilationId.
- [ ] Text/Markdown diagnostics remain deterministic.
- [ ] Stale stage artifacts cannot be applied to another model instance/compilation.
- [ ] Commit.

### Task 31-07: P30 penalty-priority integration

- [ ] Map `PenaltyTarget::Priority(u32)` from P30 to the matching P31 `LexicographicLevel::priority`; missing priorities reject before backend mutation.
- [ ] Build integration example: priority 0 minimize weighted violations, priority 1 maximize economics, priority 2 minimize deviation/churn.
- [ ] Compare result against manually staged reference solves.
- [ ] Verify zero-violation optimum does not permit economics stage to reintroduce violation beyond declared tolerance.
- [ ] Commit.

### Task 31-08: Native HiGHS multiobjective audit

- [ ] Audit official pinned HiGHS APIs/version semantics for multiple objectives, priorities, weights, tolerances, status/continuation.
- [ ] If complete semantic mapping exists, implement native provider and differential corpus against portable executor.
- [ ] If incomplete/absent, declare typed Unsupported/Bridge and keep portable executor as production path.
- [ ] Do not hold P31 for a native path.
- [ ] Commit audit/evidence.

### Task 31-09: Qualification and closure

- [ ] Randomized small multiobjective corpus compared with an independent sequential reference implementation.
- [ ] Native/portable comparison when native exists.
- [ ] P30 integrated production-like examples.
- [ ] Performance evidence: number of solves, lock rows, rebuild/delta behavior; no unexplained regressions.
- [ ] Core/HiGHS/MSRV/fmt/clippy/rustdoc/package/coverage/quality/policy green.
- [ ] Independent review focuses on lock formulas, continuation semantics, rollback, objective constants/senses, and public guarantee language.
- [ ] Evidence `docs/release/evidence/P31_OBJECTIVE_POLICIES.md`.
- [ ] Owner merge; activate P34 afterward.

## P31 stop conditions

Stop if objective locks cannot be represented transactionally, native behavior would require weakening portable semantics, relative-tolerance behavior remains ambiguous around zero, or a failed stage can leak objective/lock state.