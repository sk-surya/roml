# ROML Modeling API Guide

This document is the canonical, agent-friendly reference for writing ROML models, incremental updates, and solver workflows.

It is written for coding agents and humans who need to generate correct ROML code quickly without reverse-engineering the crate.

## Purpose

Use this guide when you need to:

- Build a linear or mixed-integer model from scratch
- Update an existing model incrementally
- Express parameterized coefficients and batch parameter updates
- Solve with `roml-highs`
- Switch between concise macro syntax and explicit low-level APIs

This guide is intentionally opinionated. It shows the preferred API first and the lower-level escape hatches second.

## Canonical Imports

Use these imports for most generated code:

```rust
use roml::prelude::*;
use roml::{constrain, constraint, objective, set_objective};
use roml_highs::HighsAdapter;
```

Notes:

- `roml::prelude::*` brings in `Model`, bounds/types, fluent expression traits, and `SolverModelExt`.
- `constrain!` and `set_objective!` are effectful macros that mutate the model directly.
- `constraint!` and `objective!` are pure builder macros that return `ConstraintSpec` and `ObjectiveSpec`.
- If you do not want macros, use `model.constrain(...)`, `model.maximize(...)`, and `model.minimize(...)` directly.

## Mental Model

ROML has four layers that matter when generating code:

1. `Model`
   Owns variables, constraints, objectives, parameters, coefficients, and the solver changelog.
2. `LinExpr`
   Temporary linear expression builder used for constraints and objectives.
3. `ValueExpr`
   Persistent expression used for parameter-dependent coefficient values.
4. `SolverAdapter` / `SolverModelExt`
   Applies model changes to a concrete solver and solves incrementally.

Important semantics:

- Variables, constraints, objectives, parameters, and coefficients all have stable typed IDs.
- Parameter updates are queued by `set_parameter` and only take effect on `commit()`.
- `drain_changes()` auto-commits pending parameter changes and logs a warning if you forgot to commit.
- Objective constants are stored on the objective itself. If you use the high-level objective APIs, you do not need to carry the constant separately.
- `solve_model()` drains pending changes, applies them, solves, and returns a `Solution` with the stored objective constant already folded into `solution.objective_value()`.

## Recommended API By Task

### 1. Create Variables

Preferred:

```rust
let x = model.add_var();
let y = model.add_var();
let z = model.add_binary();
let k = model.add_integer(Bounds::new(0.0, 10.0));
```

Use `add_variable(bounds, var_type)` only when you need explicit control.

### 2. Add Constraints

Preferred, effectful macro form:

```rust
constrain!(model, x + y <= 4.0)?;
constrain!(model, x >= 1.0)?;
constrain!(model, between: 0.0, z, 1.0)?;
```

Preferred, method form:

```rust
model.constrain((x + y).le(4.0))?;
model.constrain(x.ge(1.0))?;
model.constrain(z.between(0.0, 1.0))?;
```

Low-level form:

```rust
let con = model.add_constraint(ConstraintBounds::le(4.0));
model.add_coeff(con, x, 1.0)?;
model.add_coeff(con, y, 1.0)?;
```

Use the low-level form only when you need coefficient-by-coefficient control.

### 3. Set the Objective

Preferred, effectful macro form:

```rust
let obj = set_objective!(model, maximize: 3.0 * x + 2.0 * y + 5.0)?;
assert_eq!(model.objective_constant(obj), Some(5.0));
```

Preferred, method form:

```rust
let obj = model.maximize(3.0 * x + 2.0 * y + 5.0)?;
assert_eq!(model.objective_constant(obj), Some(5.0));
```

Equivalent minimize form:

```rust
let obj = model.minimize(x + 4.0 * y)?;
```

Pure builder form:

```rust
let spec = objective!(maximize: 3.0 * x + 2.0 * y + 5.0);
let obj = model.set_objective(spec)?;
```

Low-level form:

```rust
let (obj, offset) = model.add_objective_expr(3.0 * x + 2.0 * y + 5.0, Sense::Maximize)?;
model.set_active_objective(obj)?;
assert_eq!(offset, 5.0);
```

Use the low-level form only when you explicitly want separate objective creation and activation.

## Canonical Build-And-Solve Workflow

This is the default pattern that agents should generate unless there is a reason not to.

```rust
use roml::prelude::*;
use roml::{constrain, set_objective};
use roml_highs::HighsAdapter;

fn build_and_solve() -> Result<(), Box<dyn std::error::Error>> {
    let mut model = Model::new();

    let x = model.add_var();
    let y = model.add_var();

    constrain!(model, x + y <= 4.0)?;
    constrain!(model, x <= 3.0)?;
    constrain!(model, y <= 3.0)?;

    let obj = set_objective!(model, maximize: x + y + 2.0)?;

    let mut adapter = HighsAdapter::new();
    let solution = adapter.solve_model(&mut model)?;

    assert!(solution.is_optimal());
    assert_eq!(solution.objective_id(), Some(obj));
    assert_eq!(model.objective_constant(obj), Some(2.0));
    Ok(())
}
```

## Incremental Workflow Pattern

Reuse the same `Model` and the same solver adapter across solves.

That is the point of the changelog and adapter design.

```rust
use roml::prelude::*;
use roml::{constrain, set_objective};
use roml_highs::HighsAdapter;

fn incremental_workflow() -> Result<(), Box<dyn std::error::Error>> {
    let mut model = Model::new();
    let x = model.add_var();

    constrain!(model, x >= 1.0)?;
    let _obj = set_objective!(model, minimize: x)?;

    let mut adapter = HighsAdapter::new();

    let first = adapter.solve_model(&mut model)?;
    assert_eq!(first.value(x), Some(1.0));

    model.set_variable_bounds(x, Bounds::new(5.0, f64::INFINITY))?;

    let second = adapter.solve_model(&mut model)?;
    assert_eq!(second.value(x), Some(5.0));

    Ok(())
}
```

Agent rule:

- Do not recreate the solver adapter between solves unless you intentionally want a full rebuild.

## Parameterized Coefficients And Transactions

Parameters are for batched, incremental coefficient updates.

Use `ValueExpr` when a coefficient depends on one or more parameters.

```rust
use roml::prelude::*;
use roml_highs::HighsAdapter;

fn parameterized_workflow() -> Result<(), Box<dyn std::error::Error>> {
    let mut model = Model::new();

    let x = model.add_var();
    let p = model.add_parameter(1.0);

    let obj = model.add_objective(Sense::Minimize);
    model.set_active_objective(obj)?;
    model.add_objective_coefficient(obj, x, p)?;

    let mut adapter = HighsAdapter::new();

    let first = adapter.solve_model(&mut model)?;
    assert_eq!(first.value(x), Some(0.0));

    model.set_parameter(p, 3.0);
    model.commit();

    let second = adapter.solve_model(&mut model)?;
    assert_eq!(second.objective_id(), Some(obj));

    Ok(())
}
```

Transaction rules:

- `set_parameter(param, value)` only queues the update.
- `commit()` applies all queued parameter changes together.
- `rollback()` drops pending parameter changes.
- Agents should call `commit()` explicitly when they intend batched updates. Do not rely on `drain_changes()` auto-commit unless you are intentionally using that fallback.

## When To Use `ValueExpr`

Use `ValueExpr` when a coefficient depends on parameters.

Examples:

```rust
model.add_constraint_coefficient(con, x, p)?;
model.add_constraint_coefficient(con, x, ValueExpr::param(p1) * ValueExpr::param(p2))?;
model.add_objective_coefficient(obj, y, ValueExpr::constant(2.0) / ValueExpr::param(scale))?;
```

Do not use `ValueExpr` just to write ordinary constant linear expressions. For ordinary constraints and objectives, prefer `LinExpr`, fluent methods, and macros.

## Recommended Syntax Choices For Agents

Use these defaults unless the user asks for a different style:

1. Import `roml::prelude::*`.
2. Use `constrain!(model, ...)` for constraints.
3. Use `set_objective!(model, ...)` for the active objective.
4. Use `model.maximize(...)` or `model.minimize(...)` when you want method syntax instead of macros.
5. Use `adapter.solve_model(&mut model)` for solve calls.
6. Use low-level coefficient APIs only when building sparse structures programmatically or handling parameterized coefficients explicitly.

This produces short, readable code without hiding the model operations.

## Objective Constants

Objective constants behave differently from constraint constants.

- Constraint constants are absorbed into the constraint bounds when you call `add_constraint_expr` or `model.constrain(...)`.
- Objective constants are stored on the objective and returned by `model.objective_constant(obj)`.
- `model.objective_expression(obj)` reconstructs the full objective, including the constant.
- `adapter.solve_model(&mut model)` includes that constant in `solution.objective_value()`.

Example:

```rust
let obj = model.maximize(2.0 * x + 3.0)?;
assert_eq!(model.objective_constant(obj), Some(3.0));

let expr = model.objective_expression(obj)?;
assert_eq!(expr.get_constant(), 3.0);
```

## Pure Builder Macros vs Effectful Macros

Use pure builder macros when you want a spec value:

```rust
let cap = constraint!(x + y <= 4.0);
let goal = objective!(maximize: x + 2.0 * y);

model.constrain(cap)?;
model.set_objective(goal)?;
```

Use effectful macros when you want concise direct mutation:

```rust
let con = constrain!(model, x + y <= 4.0)?;
let obj = set_objective!(model, maximize: x + 2.0 * y)?;
```

Agent rule:

- Prefer effectful macros for concise generated code.
- Prefer pure builder macros when specs need to be stored, reused, passed around, or conditionally applied.

## Explicit Low-Level APIs

Use these only when the higher-level APIs are too restrictive:

- `add_constraint(bounds)`
- `add_constraint_coefficient(con, var, value_expr)`
- `add_coeff(con, var, value)`
- `add_objective(sense)`
- `add_objective_coefficient(obj, var, value_expr)`
- `add_objective_coeff(obj, var, value)`
- `set_active_objective(obj)`
- `set_objective_expr(obj, expr)`
- `remove_variable(var)`
- `remove_constraint(con)`
- `remove_objective(obj)`
- `remove_coefficient(coeff)`

Typical reasons to use the low-level API:

- Building coefficients in loops from sparse matrix data
- Updating one coefficient at a time
- Managing multiple inactive objectives explicitly
- Delaying objective activation
- Reconstructing or inspecting specific coefficient IDs

## Common Incremental Operations

### Change Bounds

```rust
model.set_variable_bounds(x, Bounds::new(5.0, 10.0))?;
model.set_constraint_bounds(con, ConstraintBounds::range(0.0, 20.0))?;
```

### Toggle Activity

```rust
model.set_variable_active(x, false)?;
model.set_constraint_active(con, false)?;
```

### Remove Entities

```rust
model.remove_constraint(con)?;
model.remove_variable(x)?;
model.remove_objective(obj)?;
```

Removing a variable, constraint, or objective also removes its attached coefficients and logs all resulting changes.

### Switch Objectives

If you want multiple objectives in the model and explicit switching:

```rust
let profit = model.add_objective(Sense::Maximize);
model.set_objective_expr(profit, 3.0 * x + y)?;

let cost = model.add_objective(Sense::Minimize);
model.set_objective_expr(cost, x + 2.0 * y)?;

model.set_active_objective(profit)?;
model.set_active_objective(cost)?;
```

If you only want the currently active objective, use `set_objective!`, `model.maximize(...)`, or `model.minimize(...)` instead.

## Solution Access

After `solve_model`, use the returned `Solution` directly:

```rust
let solution = adapter.solve_model(&mut model)?;

if solution.is_optimal() {
    let x_val = solution.value_or_zero(x);
    let objective = solution.objective_value();
    let dual = solution.dual(con);
    let reduced_cost = solution.reduced_cost(x);
}
```

Use `model.constraint_slack(con, &solution)` or related model-side introspection when you need algebraic checks against the solution.

## What Agents Should Avoid

Avoid these patterns unless the user explicitly asks for them:

- Building simple constraints with `add_constraint` + repeated `add_coeff` when `constrain!` or `model.constrain(...)` would be clearer
- Using `add_objective_spec(...); set_active_objective(...)` for ordinary single-objective cases
- Forgetting to call `commit()` after batched parameter changes
- Recreating `HighsAdapter` between every solve in an incremental workflow
- Carrying the objective constant separately after using `set_objective!`, `model.maximize(...)`, or `model.minimize(...)`

## Preferred Code Templates

### Small LP

```rust
use roml::prelude::*;
use roml::{constrain, set_objective};
use roml_highs::HighsAdapter;

let mut model = Model::new();
let x = model.add_var();
let y = model.add_var();

constrain!(model, x + y <= 4.0)?;
constrain!(model, x <= 3.0)?;
constrain!(model, y <= 3.0)?;

let obj = set_objective!(model, maximize: x + y)?;

let mut adapter = HighsAdapter::new();
let solution = adapter.solve_model(&mut model)?;
assert_eq!(solution.objective_id(), Some(obj));
```

### Mixed-Integer Model

```rust
let x = model.add_binary();
let y = model.add_binary();
let z = model.add_binary();

constrain!(model, 2.0 * x + 3.0 * y + 2.0 * z <= 5.0)?;
let _obj = set_objective!(model, maximize: 5.0 * x + 4.0 * y + 3.0 * z)?;
```

### Programmatic Sparse Constraint Build

```rust
let con = model.add_constraint(ConstraintBounds::le(rhs));
for (var, coeff) in sparse_terms {
    model.add_coeff(con, var, coeff)?;
}
```

### Parameter Batch Update

```rust
for (param, value) in updates {
    model.set_parameter(param, value);
}
model.commit();

let solution = adapter.solve_model(&mut model)?;
```

## HiGHS Runtime Notes

For `roml-highs`, the build and runtime environment must be able to find HiGHS.

Common setup variables:

- `HIGHS_ROOT`
- `HIGHS_LIB_DIR`
- `HIGHS_SOURCE_DIR`

On macOS, if you are linking against a local shared-library build of HiGHS, test execution may also need:

```bash
DYLD_LIBRARY_PATH=/path/to/highs/lib
```

Example:

```bash
HIGHS_LIB_DIR=/Users/skrishnan/repos/HiGHS/build/lib \
DYLD_LIBRARY_PATH=/Users/skrishnan/repos/HiGHS/build/lib \
cargo test -p roml-highs
```

## One-Line Guidance For Coding Agents

Default to this style:

```rust
use roml::prelude::*;
use roml::{constrain, set_objective};
```

Then:

- create variables with `add_var`, `add_binary`, or `add_integer`
- add constraints with `constrain!(model, ...)`
- set the active objective with `set_objective!(model, ...)`
- batch parameter changes with `set_parameter` + `commit`
- solve incrementally with one reused adapter via `solve_model`

If you follow those rules, the generated ROML code will usually be short, readable, and correct.