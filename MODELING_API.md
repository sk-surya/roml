# ROML Modeling API Guide

This guide teaches the canonical ROML modeling path: build a named model with
validated definitions, add constraints and objectives with fluent expressions,
solve with HiGHS through the `Highs` façade, and re-solve incrementally after
parameter changes — without touching revisions, snapshots, deltas, or
synchronization.

It is written for humans and coding agents who want correct ROML code without
reverse-engineering the crate.

Every inline snippet below is compiled and run by
`roml-highs/tests/modeling_guide.rs`; the major workflows additionally link a
compiled example in `roml-highs/examples/`. Advanced escape hatches are
explicitly labeled **advanced** and live behind the `roml::advanced` namespace.

## Imports

```rust
use roml::prelude::*;
use roml_highs::Highs;
```

`roml::prelude::*` is the curated default surface (API-07.1): the model, entity
definitions, expression traits, solution/status types, and errors. `Highs` is
the golden-path solver façade.

> **Legacy root imports:** protocol and backend-extension types (`Change`,
> `CoeffId`, `DeltaBatch`, `ModelOp`, `ModelRevision`, `ModelSnapshot`,
> `AdapterCursor`, `BackendSession`, `Synchronization`, …) remain reachable at
> the crate root for migration-era compatibility, but are **not** the
> recommended surface. Reach them through `roml::advanced` when you need them,
> and import only `roml::prelude::*` for ordinary models.

## 1. Model and entity definitions

Create a model with `Model::new()` or `Model::named("…")`:

```rust
let mut model = Model::named("production");
```

Entities are created from **validated definition builders** (D7). Each creation
is fallible and validates domain invariants before mutating the model (D10).

| Builder | Meaning |
| --- | --- |
| `continuous()` | non-negative continuous variable |
| `integer()` | non-negative integer variable |
| `binary()` | variable fixed to the unit interval `[0, 1]` |
| `parameter(value)` | a mutable parameter with an initial value |

Each definition accepts `.bounds(lower, upper)`, `.lower_bound(…)`,
`.upper_bound(…)`, and `.named("…")`:

```rust
let x = model.add_variable(continuous().named("x"))?;
let y = model.add_variable(integer().bounds(0.0, 10.0).named("y"))?;
let z = model.add_variable(binary().named("z"))?;
let w = model.add_variable(continuous().lower_bound(1.0).named("w"))?;
let price = model.add_parameter(parameter(1.0).named("price"))?;
```

Invalid input is rejected with a typed `ModelError` — inverted bounds
(`ModelError::InvalidBounds`), non-finite bounds, or binary bounds outside
`[0, 1]` (`ModelError::InvalidBinaryBounds`) never mutate the model.

## 2. Expressions and constraints

Write linear algebra directly on entity handles — `+`, `-`, and `*` with
variables, parameters, and `f64` constants:

```rust
let _expr = 3.0 * x + y - 1.0;   // LinExpr
let _priced = price * x + y;     // parameters participate
```

Constraints use the fluent builders `.le(…)`, `.ge(…)`, `.eq(…)`, and
`.between(lower, upper)` (API-04.3). `add_constraint` is the canonical
constraint mutation (API-04.1):

```rust
model.add_constraint((x + y).le(4.0).named("capacity"))?;
model.add_constraint(x.ge(1.0))?;
model.add_constraint(y.between(0.0, 3.0))?;
model.add_constraint((2.0 * x - y).eq(1.0))?;
```

`constraint!(…)` remains available as a *pure* spec builder:

```rust
let spec = constraint!(x + y <= 4.0);
model.add_constraint(spec)?;
```

The effectful `constrain!` and `set_objective!` macros are deprecated
(migration-only); prefer the method forms above.

## 3. Objectives

`minimize(expr)` and `maximize(expr)` are the canonical single-objective
mutations (API-04.2). They return the objective's handle:

```rust
let obj = model.maximize(3.0 * x + y + 2.0)?;
assert_eq!(model.objective_constant(obj), Some(2.0));
assert_eq!(model.active_objective(), Some(obj));
```

Objective constants are stored on the objective and reported **exactly once**
in `Solution::objective_value()` (API-03.5) — you never add them back yourself.
The low-level multi-objective controls (`add_objective`, `set_active_objective`,
…) are **advanced**; ordinary models use `minimize`/`maximize`.

## 4. Names and diagnostics

Names are first-class model metadata (D6): retrievable, and shown in
`pprint()` output.

```rust
assert_eq!(model.variable_name(x)?, Some("x"));
assert_eq!(model.parameter_name(price)?, Some("price"));
println!("{}", model.pprint());
```

## 5. Solving with HiGHS

`roml_highs::Highs` is the user-facing façade (D3): `new`, `solve`, and
`solve_with`. `solve` commits pending model mutations, synchronizes the solver,
runs the solve, and returns a normalized `Solution` (API-01.2):

```rust
let mut highs = Highs::new()?;
let solution = highs.solve(&mut model)?;
```

The complete end-to-end example is
[`roml-highs/examples/simple_lp.rs`](roml-highs/examples/simple_lp.rs), with a
mixed-integer variant in
[`roml-highs/examples/simple_mip.rs`](roml-highs/examples/simple_mip.rs).

## 6. Solve options and effective configuration

`solve_with(model, options)` passes an ergonomically built `SolveOptions`
(API-01.3). Options are validated **before** synchronization, so a failed
validation leaves the model and backend state unchanged:

```rust
use std::time::Duration;

let options = SolveOptions::new()
    .time_limit(Duration::from_secs(60))
    .relative_gap(0.1)
    .threads(1)
    .output(false);

let solution = highs.solve_with(&mut model, options)?;
```

The returned solution's `metadata().effective_configuration` records what the
backend actually applied — including any adjustments and rejections from option
negotiation (API-03.4):

```rust
let effective = &solution.metadata().effective_configuration;
assert_eq!(effective.mip_rel_gap, Some(0.1));
assert_eq!(effective.threads, Some(1));
```

See [`roml-highs/examples/solve_options.rs`](roml-highs/examples/solve_options.rs)
for a runnable version that prints the effective configuration.

## 7. Solution and status semantics

A solved model returns one `Solution` with a single `SolveStatus` (D4,
API-03). Distinguish **mathematical termination** from **operational failure**:

- A mathematical outcome — optimal, feasible, infeasible, unbounded, limit,
  interrupted, numerical — returns `Ok(Solution)`. Infeasible/unbounded
  solutions carry **no primal values**.
- An inability to perform or interpret the solve — invalid options, a failed
  synchronization, a backend/license failure, or an uninterpretable native
  status — returns `Err(SolveError)`.

```rust
if let Ok(solution) = highs.solve(&mut model) {
    match solution.status() {
        SolveStatus::Optimal => {
            let xv = solution.value(x);
            let objective = solution.objective_value();
            // duals / reduced costs where available:
            // solution.dual(con), solution.reduced_cost(var)
        }
        SolveStatus::Infeasible => { /* no primal values */ }
        _ => { /* other mathematical outcomes */ }
    }
}
```

The fixture `guide_status_semantics_math_vs_operational` in
`roml-highs/tests/modeling_guide.rs` pins both sides: an invalid `relative_gap`
returns `Err(SolveError::InvalidOptions)`, and an infeasible model returns
`Ok(Solution)` with status `Infeasible` and no values.

## 8. Parameters and repeated solves

Model coefficients may depend on parameters. Change a parameter with the
fallible `set_parameter` (stale parameter handles are rejected, API-06.3) and
re-solve **on the same `Highs` instance** — the update is applied
incrementally as a delta:

```rust
let first = highs.solve(&mut model)?;
assert_eq!(first.objective_value(), Some(4.0));

model.set_parameter(price, 3.0)?;

let second = highs.solve(&mut model)?;
assert_eq!(second.objective_value(), Some(12.0));
```

Do **not** recreate the solver between solves — that forces a full rebuild.
See [`roml-highs/examples/parameter_update.rs`](roml-highs/examples/parameter_update.rs).

### Automatic synchronization

`solve` performs an implicit `commit` of pending mutations, then synchronizes
(D5/D2):

- **Delta path.** Supported changes project as an incremental delta batch
  (parameter updates, bound edits, coefficient changes) without rebuilding
  solver state.
- **Rebuild path.** If a change cannot be expressed as a delta, the backend is
  rebuilt from a deterministic model snapshot.
- **One retry.** At most **one** automatic rebuild retry happens per solve
  attempt (API-02.3); terminal failures (including license errors) never retry.
- **Failure safety.** A failed synchronization or solve never loses model
  operations and never reports a stale solution as current (API-01.5).

You may call `model.commit()` explicitly to establish a revision boundary, but
the ordinary path never requires it.

## 9. Sparse construction

When a model is assembled from sparse matrix data, populate rows
coefficient-by-coefficient. The cell APIs name matrix coordinates — a
`CoefficientTarget::Constraint(con)` plus a variable — never a `CoeffId`
(D11). These are **advanced** escape hatches under `roml::advanced`:

- `set_coefficient(target, var, value)` — replace the canonical cell;
- `add_to_coefficient(target, var, value)` — algebraically add to it;
- `remove_coefficient_at(target, var)` — remove it by coordinate.

```rust
use roml::advanced::{CoefficientTarget, ConstraintBounds};

let con = model.add_constraint(ConstraintSpec::new(
    LinExpr::new(),
    ConstraintBounds::le(8.0),
))?;
model.set_coefficient(CoefficientTarget::Constraint(con), x, 2.0)?;
model.add_to_coefficient(CoefficientTarget::Constraint(con), x, 1.0)?;
model.set_coefficient(CoefficientTarget::Constraint(con), y, 3.0)?;
model.remove_coefficient_at(CoefficientTarget::Constraint(con), y)?;
```

See [`roml-highs/examples/sparse_build.rs`](roml-highs/examples/sparse_build.rs)
for a runnable sparse LP.

## 10. Advanced sessions and synchronization

Framework and backend authors extend ROML through `roml::advanced` and
`roml::backend`: the frozen `BackendSession` contract, revisions, snapshots,
delta batches, adapter cursors, capabilities, callbacks, and the raw entity
IDs. Ordinary models never need these. The `Highs` façade owns all
synchronization; if you are implementing a new solver backend, the
`ReferenceBackend` and the conformance suite are the executable specification.

## 11. Solve plans and warm starts

The solve-attempt contract (P28): instead of composing a solve from scattered
options, one [`SolvePlan`] declares the entire attempt — options, a reversible
overlay, MIP warm starts, variable hints, an optional objective override, and
the unsupported-feature policy:

```text
SolvePlan {
    options: SolveOptions,
    overlay: SolveOverlay,
    mip_starts: Vec<MipStart>,
    hints: VariableHints,
    objective_override: Option<ObjectivePolicy>,
    lex_stage_policy: LexStagePolicy,
    unsupported: UnsupportedFeaturePolicy,
}
```

`SolvePlan::new(options)` builds the empty plan; executing it is exactly
`solve`/`solve_with` (the equivalence is one code path, not a coincidence).
`Highs::solve_plan(model, plan)` runs the full lifecycle: validate the plan
(lineages, entities, finite values, conflicts, duplicates) → resolve
starts/hints against the backend's typed capabilities and the policy →
synchronize → apply the overlay → apply qualified starts/hints → solve →
enforce the exact `CompilationId` gate → roll back the overlay → record the
effective plan.

**Warm starts.** A [`MipStart`] is a full or partial primal assignment with an
explicit [`RepairPolicy`]; its [`PrimalAssignment`] carries the model's
lineage/instance/revision as provenance. A start is a search hint: it can
never change the proven optimum. A backend that does not qualify starts
**rejects by default** — nothing is silently ignored; an explicit conversion
policy (`ConvertStartToTemporaryFixing`, `ConvertHintToStart`) applies the
conversion and records it.

**Effective-plan reporting.** Every real solve's metadata carries the
`EffectiveSolvePlan`: `applied_features` (what the backend executed),
`adjustments` (explicit conversions), `rejections` (unconvertible requests),
and `objective_stages` (empty until P31). `metadata().compilation_id` is the
exact identity of the compiled state that was solved.

The full program is compiled and run as
[`roml-highs/examples/warm_start_mip.rs`](roml-highs/examples/warm_start_mip.rs):
`solve` cold, re-solve with a `MipStart` carrying a feasible but SUBOPTIMAL
assignment (so the hint is genuine — it must not fix the model to its
values), and read the applied features and compilation identity from the
metadata while the solver recovers the proven optimum.

## 12. Reversible solve overlays

Overlays (P27) express *solve-scoped* restrictions without touching the
canonical model: temporary fixings, solution locks, objective-lock rows, and
cutoffs apply for one solve attempt and are rolled back (and verified) after
it. The canonical model is never mutated by an overlay.

```text
SolveOverlay::new(
    temporary_fixings: BTreeMap<Variable, f64>,  // y := 4.0 for this solve
    locks: Vec<SolutionLock>,                    // lock an assignment's values
    objective_locks: Vec<ObjectiveLock>,         // stage-optimum degradation rows (P31)
    cutoffs: Vec<ObjectiveCutoff>,               // bound the objective for this solve
)
```

A [`SolutionLock`] pins selected variables of a [`PrimalAssignment`]
(`LockSelector::AllAssigned` / `IntegerAssigned` / `BinaryAssigned`, with
`ContinuousLock::Exact` or an absolute band). Solve with
`Highs::solve_with_overlay(model, options, &overlay, objective_override)`; the
result's `metadata().overlay_id` identifies the applied overlay, and a
subsequent plain solve is provably unaffected.

The full program is compiled and run as
[`roml-highs/examples/overlay_solve.rs`](roml-highs/examples/overlay_solve.rs):
solve a production mix, re-solve with `y` temporarily fixed and `x` locked to
its baseline, then verify the overlay is fully rolled back.

## 13. Semantic constructs

The construct library (P32) captures high-level semantics exactly and compiles
them through the portable bridge — every generated row and auxiliary carries a
construct origin, and each builder returns a stable [`Construct`] handle (and,
where applicable, the result-variable handle):

| Builder | Semantics |
| --- | --- |
| `Model::add_indicator(activator, direction, relation, preference)` | the relation holds when the binary activator is 1 (`WhenOne`) or 0 (`WhenZero`) |
| `Model::add_boolean(…)` | Boolean combinations of binary conditions |
| `Model::add_cardinality(…)` | at-most/at-least/exact counts over a variable set |
| `Model::add_minmax(operands, sense, relation, preference)` | exact `output = min/max(…)`, or the one-sided epigraph/hypograph (zero binaries) |
| `Model::add_absolute_value(expression, variant, preference)` | exact absolute value, positive part, or clamp |
| `Model::add_binary_times_linear(binary, expression, preference)` | exact `output = binary × expression` |

The relation determines the formulation (D24): a `Max`+`Epigraph` or
`Min`+`Hypograph` compiles to zero-binary one-sided rows, an `Exact`
min/max to a bounded selector formulation with binaries. Exactness is always
the user's explicit choice — it is never inferred from objective context.

The full program is compiled and run as
[`roml-highs/examples/constructs.rs`](roml-highs/examples/constructs.rs):
an indicator coupling a binary to a constraint, a binary-times-linear product,
an absolute-value epigraph, and a min/max epigraph, all binding exactly at the
optimum.

## 14. Piecewise-linear functions

`Model::add_piecewise_linear(argument, points, relation, extrapolation,
preference)` models `output` against a piecewise-linear function of a scalar
linear argument (P33):

- `points` are finite, strictly increasing breakpoints with `ValueExpr` values
  (parameter-dependent values are allowed);
- `relation` is `Epigraph` (`output >= f(argument)`), `Hypograph`
  (`output <= f(argument)`), or `ExactGraph` (`output = f(argument)`);
- `extrapolation` is `Constant` (clamp) or `Linear` (continue the end segment
  slope).

Curvature is classified deterministically from segment slopes. A convex
epigraph / concave hypograph compiles to supporting-inequality rows with
**zero binaries**; an exact or nonconvex graph compiles to the deterministic
exact segment-binary representation — never a convex relaxation. No Big-M is
introduced anywhere; the compilation report records the argument interval,
bound sources, curvature, representation, and binary-avoidance reasons.

Two typed guards keep the compiled model honest: `CompileError::
ExtrapolationConflict` rejects a declaration whose bound-derived argument
interval can leave the breakpoint range (Constant one-sided relations; the
exact graph under either policy), and `PwlEvalError` replaces panics on the
payload's semantic operations for parameterized point values (use the `_with`
resolver variants: `evaluate_with`, `classify_curvature_with`,
`segment_slopes_with`).

The full program is compiled and run as
[`roml-highs/examples/pwl_production_planning.rs`](roml-highs/examples/pwl_production_planning.rs):
a convex tiered cost modeled as an epigraph PWL (zero binaries) plus a
min/max capacity construct, with the reported cost asserted equal to the exact
PWL function.

## 15. Migration notes and common errors

The P21–P23 redesign replaced the legacy adapter/macro surface. Migration
notes, including the pre-1.0 breaking changes and the deprecated-surface
disposition, are in [`MIGRATION.md`](MIGRATION.md) and
[`CHANGELOG.md`](CHANGELOG.md). Common errors, all returned as typed values:

| Symptom | Cause | Fix |
| --- | --- | --- |
| `Err(ModelError::InvalidBounds)` | inverted or non-finite bounds | fix the `.bounds(…)` / `.lower_bound(…)` values |
| `Err(ModelError::InvalidBinaryBounds)` | binary bounds outside `[0, 1]` | use `binary()` without overrides, or fix bounds |
| `Err(ModelError::*NotFound(…))` | a stale entity handle | re-query the handle after removal/recreation |
| `Err(ModelError::NonFiniteValue(…))` | NaN/∞ coefficient or bound | validate numeric inputs before mutation |
| `Err(SolveError::InvalidOptions(…))` | a negative gap or non-positive thread count | fix the `SolveOptions` value; the model and backend are unchanged |
| `Err(SolveError::License(…))` | backend license failure (terminal) | address licensing; a retry will not help |
| `Ok(Solution)` with status `Infeasible` / `Unbounded` | mathematical termination | inspect `solution.status()`; primal values are absent |

A model with no active objective is a degenerate solve: HiGHS optimizes the
empty objective and reports `Optimal` with objective `0.0`. Give every model an
active objective via `minimize`/`maximize` before solving.
