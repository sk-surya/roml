# Phase 30 — Soft Constraints and Feasibility Relaxation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** add mathematically explicit persistent soft constraints and a separate solve-scoped feasibility-relaxation workflow that composes with P29 IIS without confusing diagnosis with repair.

**Architecture:** persistent softening is canonical semantic state compiled through the existing construct/origin framework; feasibility relaxation is an isolated/overlay solve workflow over selected semantic restrictions. Portable ROML behavior is normative; native backend relaxations are optional qualified providers.

**Requirements:** SM-10.1–SM-10.9 plus existing SM-02.5/SM-04/SM-07 transaction invariants.

## Global constraints

- Persistent softening advances model revision; solve-scoped relaxation does not.
- Upper/lower violation variables are distinct stable semantic roles.
- No hidden objective mutation: penalties have explicit targets.
- Penalty weights are finite, nonnegative, parameter-aware.
- No L2/nonlinear relaxation in M3.
- IIS is a seed/scope helper, never a minimum-repair proof.
- Native relaxation cannot define ROML semantics.

## Target public concepts

```rust
pub struct SoftConstraint { /* stable construct handle */ }

pub struct SoftConstraintOptions {
    pub lower: ViolationPolicy,
    pub upper: ViolationPolicy,
    pub penalty: PenaltyPolicy,
}

pub enum PenaltyTarget {
    None,
    Objective(Objective),
    Priority(LexicographicPriority),
}

pub struct FeasibilityRelaxationPlan {
    pub scope: RelaxationScope,
    pub objective: RelaxationObjective,
    pub unsupported: UnsupportedFeaturePolicy,
}

pub struct FeasibilityRelaxationReport {
    pub outcome: RelaxationOutcome,
    pub members: Vec<RelaxedRestriction>,
    pub total_weighted_violation: f64,
    pub metadata: RelaxationMetadata,
}
```

Names are binding intent, not permission to ship decorative fields. Any field not governing execution is removed before merge.

---

### Task 30-00: Characterize existing construct/objective/overlay seams

**Files:** tests only + phase evidence.

- [ ] Freeze behavioral characterization for constraint identity, construct origins, parameter dependencies, objective replacement, and overlay rollback.
- [ ] Add compile-only target API fixture for one persistent soft upper constraint and one solve-scoped relaxation request.
- [ ] Confirm no existing public type ambiguously means both persistent softening and solve-scoped relaxation.
- [ ] Record exact base SHA and API inventory.
- [ ] Commit characterization only.

### Task 30-01: Canonical persistent soft-constraint contract

**Files:**
- Create focused `src/model/soft_constraint.rs` or follow current construct module layout.
- Modify construct kind/payload/snapshot/delta definitions.
- Tests: `tests/soft_constraints.rs`.

- [ ] Write failing tests for softening upper, lower, equality, ranged constraints through one builder call.
- [ ] Define stable `SoftConstraint`/payload roles referring to the original constraint; generated violation entities are compiler products, not user-created ordinary variables unless existing construct conventions require stable returned handles.
- [ ] Validate attempts to soften stale/inactive/unsupported constraints atomically.
- [ ] Preserve parameter dependencies from penalty expressions.
- [ ] Prove clone/snapshot/delta/remove behavior.
- [ ] Commit.

### Task 30-02: Exact portable algebra and origins

**Files:** compiler bridge recipe + bound analysis integration + tests.

Required formulas:

```text
upper f(x) <= u  => f(x) - v_up <= u,  v_up >= 0
lower f(x) >= l  => f(x) + v_lo >= l,  v_lo >= 0
equality          => both sides with distinct violations
ranged            => both sides with distinct violations
```

- [ ] Write direct algebra tests for all senses, including nonzero constants and parameterized coefficients.
- [ ] Add maximum-violation bounds and reject NaN/negative/non-finite maxima.
- [ ] Add deterministic generated roles `LowerViolation`, `UpperViolation`, and penalty contribution origins.
- [ ] Verify origin-map completeness and compilation report entries.
- [ ] Differential-solve fixtures against manually constructed reference formulations.
- [ ] Commit.

### Task 30-03: Penalty semantics

**Files:** soft payload/objective-policy compiler tests.

- [ ] Write min-objective and max-objective sign tests; penalty must always make violation worse according to the selected objective semantics.
- [ ] Write tests for zero weight, positive weight, parameterized weight updates, invalid negative/non-finite weight.
- [ ] Implement `PenaltyTarget::None` and `PenaltyTarget::Objective` without depending on P31 runtime.
- [ ] Store `Priority` semantically if needed for forward compatibility, but until P31 is present, attempting to solve an active priority-targeted penalty must return typed Unsupported rather than ignore it.
- [ ] Verify parameter delta updates the compiled penalty cells without semantic drift.
- [ ] Commit.

### Task 30-04: Solution violation accessors

**Files:** solution/report modules and tests.

- [ ] Expose violation by `SoftConstraint` and original constraint identity: lower, upper, total.
- [ ] Define tolerance treatment explicitly; raw primal violation and tolerance-adjusted diagnostic view must not be conflated.
- [ ] Handle missing/non-optimal solutions according to existing `Solution` access semantics.
- [ ] Add named diagnostic rendering.
- [ ] Commit.

### Task 30-05: Signed correction as separate semantic API

- [ ] Define signed correction with positive/negative parts and explicit algebra.
- [ ] Add tests proving it is not equivalent to ordinary one-sided softening.
- [ ] Keep it out of convenience defaults.
- [ ] Commit.

### Task 30-06: Solve-scoped portable feasibility-relaxation contract

**Files:**
- Create `src/solver/feasibility_relaxation.rs` (or established solver workflow module).
- Tests: core fault backend + HiGHS integration.

- [ ] Define `RelaxationScope` over original constraint sides/variable bounds/fixings/locks as appropriate for M3.
- [ ] Define tri-state/typed outcome and completion metadata; preserve model lineage/instance/revision/CompilationId/provider/numerical policy.
- [ ] Default objective: weighted L1 sum of nonnegative violation magnitudes.
- [ ] Build an isolated analysis model or temporary overlay from the exact compiled base; do not mutate canonical revision.
- [ ] Always attempt rollback/rebuild recovery on failure according to P27/P29 semantics.
- [ ] Add failure injection at preflight, compile, apply, solve, extract, rollback.
- [ ] Commit.

### Task 30-07: IIS composition

- [ ] Add a conversion from a P29 report to an explicit `RelaxationScope` or helper function without coupling P29 algorithm to P30.
- [ ] Reject stale report/model identity mismatch before backend mutation.
- [ ] Document and test that restricting relaxation to an IIS does not claim globally minimum repair.
- [ ] Add example: infeasible imported MPS -> IIS -> relax report.
- [ ] Commit.

### Task 30-08: Optional native HiGHS feasibility-relaxation audit

- [ ] Audit pinned official HiGHS header/source for any feasibility-relaxation API and exact semantics.
- [ ] If semantics are qualified, add typed native provider and native/portable differential fixtures.
- [ ] If not qualified, record `Unsupported` with exact version limitation; do not delay portable P30.
- [ ] Never simulate a native claim.
- [ ] Commit audit/evidence.

### Task 30-09: Qualification and closure

- [ ] Randomized/manual-reference formulation equivalence for each sense/penalty case.
- [ ] P29->P30 integrated fixtures including imported MPS provenance.
- [ ] Performance baseline: oracle/solve counts and overhead for representative relaxation sizes; record, do not optimize without evidence.
- [ ] Core/HiGHS/MSRV/fmt/clippy/rustdoc/package/coverage/quality/policy green.
- [ ] Independent OR-formulation review specifically checks signs, bounds, equality/ranged algebra, and guarantee language.
- [ ] No P0/P1 findings.
- [ ] Evidence `docs/release/evidence/P30_SOFT_CONSTRAINTS_RELAXATION.md`.
- [ ] Owner merge; activate P31 only afterward.

## P30 stop conditions

Stop and redesign if persistent softening requires solve-scoped state in canonical model, relaxation rollback cannot be proven, objective penalty sign depends on backend convention, or native behavior cannot be mapped to the portable report contract.