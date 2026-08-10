# M3 — Semantic Modeling and Solve Workflows

> **Historical/superseded routing notice:** This milestone README records the original M3 planning contract and product context. It is not an active routing or production-authorization authority. Current execution is governed by root `.planning/STATE.md`, milestone `STATE.md`, and `COMPLETION-ROADMAP.md`: `PR #45 merge -> P36 -> P30 -> P31 -> P34`. While PR #45 is unmerged, production authorization is false.

**Branch:** `docs/m3-semantic-modeling-workflows`  
**Baseline:** `main@d1f1ad38cec75abb671729df8efb87736861628c`  
**Planning date:** 2026-08-02  
**Status:** PLANNED — design approved; implementation not started

M3 turns ROML from a qualified incremental linear/MILP modeling library into a semantic optimization modeling kernel. High-level constructs remain in canonical model state and compile into backend-native or portable formulations. The milestone adds solution reuse, infeasibility diagnostics, soft constraints, lexicographic solving, and a bounded set of common modeling constructs while leaving a deliberate extension seam for future quadratic and nonlinear programming.

## Outcome

A modeler can express and solve workflows such as:

```rust
use roml::prelude::*;
use roml_highs::Highs;

let mut model = Model::named("dispatch");
let on = model.add_variable(binary().named("on"))?;
let generation = model.add_variable(
    continuous().bounds(0.0, 100.0).named("generation"),
)?;

let capacity = model.add_constraint(
    generation.le(80.0).named("capacity"),
)?;
model.add_indicator(on, when_one(), generation.ge(10.0))?;

let cost = model.add_objective(minimize(25.0 * generation).named("cost"))?;

let violation = model.soften(
    capacity,
    soft()
        .upper_side()
        .max_violation(20.0)
        .penalty(1_000.0)
        .on(cost)
        .named("capacity_overage"),
)?;

let service = model.add_objective(minimize(violation.total()).named("service"))?;
model.set_objective_policy(ObjectivePolicy::Lexicographic(vec![
    ObjectiveLevel {
        objective: service,
        absolute_tolerance: 1e-4,
        relative_tolerance: 0.0,
    },
    ObjectiveLevel {
        objective: cost,
        absolute_tolerance: 1e-6,
        relative_tolerance: 0.0,
    },
]))?;

let mut highs = Highs::new()?;
let first = highs.solve(&mut model)?;

let lock = SolutionLock {
    assignment: first.primal_assignment(),
    selector: LockSelector::IntegerAssigned,
    continuous: ContinuousLock::Exact,
};
let second = highs.solve_plan(
    &mut model,
    SolvePlan {
        overlay: /* solve-scoped overlay carrying the lock; overlay contents follow design §12 */,
        ..SolvePlan::default()
    },
)?;
```

If the model is infeasible, the same façade can request an origin-aware report:

```rust
let report = highs.analyze_infeasibility(&mut model, &InfeasibilityRequest::default())?;
println!("{}", report.render_text()?);
```

## Packet index

1. `PROJECT.md` — objective, scope, product promise, and acceptance criteria.
2. `REQUIREMENTS.md` — stable M3 requirement IDs.
3. `DECISIONS.md` — approved architecture and API decisions.
4. `ROADMAP.md` — ten-phase dependency graph and gates.
5. `STATE.md` — execution ledger, current gate, and owner decisions.
6. `RESEARCH.md` — current-main findings and consequences.
7. `TRACEABILITY.md` — requirement-to-phase/evidence map.
8. `RISKS.md` — failure modes, mitigations, and reversal triggers.
9. `EXECUTION.md` — branch, review, testing, evidence, and integration protocol.
10. `docs/superpowers/specs/2026-08-02-semantic-modeling-and-solve-workflows-design.md` — approved programming design.
11. `docs/superpowers/plans/2026-08-02-semantic-modeling-and-solve-workflows.md` — task-level implementation plan.

## Phase map

```text
P25 Semantic IR, identity, metadata
  -> P26 Compiler boundary, backend IR, origin maps
       -> P27 Fixing, assignments, solution locks, overlays
            -> P28 MIP starts, hints, effective solve plans
                 -> P29 IIS/conflict analysis and reports
                 -> P30 Soft constraints and feasibility relaxation
                 -> P31 Objective policies and lexicographic solves
       -> P32 Common semantic constructs
            -> P33 Piecewise-linear functions and bound analysis
                 -> P34 Qualification, docs, migration, NLP-readiness audit
```

The paragraph above describes the original milestone dependency view and is retained for historical context only. It is superseded for active execution by the completion program: `PR #45 merge -> P36 -> P30 -> P31 -> P34`. Research/review may proceed ahead, but production implementation remains serialized and no phase is active while PR #45 is unmerged.

## Execution rule

- Keep one implementation phase active by default.
- Current completion-program rule: while PR #45 is unmerged, zero production phases are active; after merge, only P36 may activate.
- At most one additional review/fix branch may be active.
- Do not implement modeling conveniences before P26 freezes the semantic-to-backend compilation contract.
- Do not add native solver code from memory; derive every symbol and capability from pinned official headers or APIs.
- Do not qualify a bridge until exact semantic equivalence and origin completeness are tested.
- Do not publish crates or create a release as part of M3.

## Stopping condition

M3 is complete when all requirements in `REQUIREMENTS.md` are closed with tests and evidence, all ten phase gates pass, the public API and package contents are reviewed, HiGHS workflows are qualified on supported versions, and an independent NLP-readiness review finds no linear-only architectural dead end.
