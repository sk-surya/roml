# ROML M2 Migration Guide

This guide documents the pre-1.0 API transition established by the M2 public
API ergonomics milestone (phases P21–P24). The canonical method-first surface
(D1/D7/D8/D11) replaces the older raw constructors, effectful macros, and
legacy aliases. Every deprecated entry point below lists its replacement and a
migration section; deprecated APIs remain functional and tested for the
pre-1.0 deprecation window (API-08.3).

Deprecation notes on the code point to the sections here (`MIGRATION.md ->
<Section>`).

## Deprecation summary

| Deprecated item | Replacement | Section |
|---|---|---|
| `Model::add_var()` | `model.add_variable(continuous())` | [Variable and parameter creation](#variable-and-parameter-creation) |
| `Model::add_binary()` | `model.add_variable(binary())` | [Variable and parameter creation](#variable-and-parameter-creation) |
| `Model::add_integer(bounds)` | `model.add_variable(integer().bounds(lower, upper))` | [Variable and parameter creation](#variable-and-parameter-creation) |
| `Model::add_parameter(f64)` | `model.add_parameter(parameter(value))` (input shape preserved via `Into<ParameterDef>`) | [Variable and parameter creation](#variable-and-parameter-creation) |
| `Model::constrain(spec)` | `model.add_constraint(spec)` | [Constraints](#constraints) |
| `Model::constraint(spec)` | `model.add_constraint(spec)` | [Constraints](#constraints) |
| `constrain!(model, ...)` | `model.add_constraint(constraint!(...))` or `model.add_constraint((expr).le/ge/eq/between(...))` | [Constraints](#constraints) |
| `Model::set_objective(spec)` | `model.maximize(expr)` / `model.minimize(expr)` | [Objectives](#objectives) |
| `set_objective!(model, ...)` | `model.maximize(expr)` / `model.minimize(expr)` | [Objectives](#objectives) |
| `Model::drain_changes()` | `model.commit()` (revisioned sync handled by the façade) | [HiGHS solve path](#highs-solve-path) |
| Protocol/backend types in `roml::prelude` | `roml::advanced` | [Imports and prelude changes](#imports-and-prelude-changes) |

Pure builders `constraint!` and `objective!` are NOT deprecated; they remain
optional syntax sugar for building specs (D1/API-04.4).

## Variable and parameter creation

**Before**

```rust
let x = model.add_var();                       // continuous, non-negative
let y = model.add_binary();                    // binary [0, 1]
let z = model.add_integer(Bounds::new(0, 10)); // integer, unbounded by default
let price = model.add_parameter(1.0);
```

**After** — validated definition builders (D7). Fallible (`Result`) so invalid
bounds or non-finite values are rejected before mutation (D10/API-06).

```rust
let x = model.add_variable(continuous().named("x"))?;
let y = model.add_variable(binary())?;
let z = model.add_variable(integer().bounds(0.0, 10.0).named("z"))?;
let price = model.add_parameter(parameter(1.0).named("price"))?;
```

Notes:

- `continuous()` defaults to `[0, +inf)`; override with `.bounds(lower, upper)`,
  `.lower_bound(v)`, or `.upper_bound(v)`.
- `add_parameter(f64)` keeps compiling through the `Into<ParameterDef>` bridge,
  but the named `parameter(value)` form is recommended.
- Handles are the semantic aliases `Variable`, `Constraint`, `Objective`,
  `Parameter` (plain type aliases of the raw IDs — D8).

## Constraints

**Before**

```rust
model.constrain((x + y).le(4.0))?;
model.constraint(x.ge(1.0))?;
constrain!(model, x + y <= 4.0)?;
```

**After** — the canonical mutation is `Model::add_constraint(spec)` (API-04.1):

```rust
model.add_constraint((x + y).le(4.0))?;
model.add_constraint(x.ge(1.0))?;
model.add_constraint((x + y).le(4.0).named("capacity"))?;
// Equivalent with the pure builder:
model.add_constraint(constraint!(x + y <= 4.0))?;
```

Notes:

- Fluent builders `.le`, `.ge`, `.eq`, `.between` remain canonical (API-04.3).
- `add_constraint` accepts anything `Into<ConstraintSpec>`, including raw
  `ConstraintBounds` (input-shape compatibility bridge).
- NaN or inverted constraint bounds are now rejected before mutation (P23).

## Objectives

**Before**

```rust
model.set_objective(ObjectiveSpec::new(Sense::Maximize, x + 2.0 * y))?;
set_objective!(model, maximize: x + 2.0 * y + 3.0)?;
```

**After** — canonical single-objective mutations (API-04.2):

```rust
model.maximize(x + 2.0 * y + 3.0)?;
model.minimize(3.0 * x + y)?;
```

Notes:

- Objective constants are stored and reported exactly once (API-03.5).
- Multi-objective control (`add_objective_named`, `set_active_objective`,
  `set_objective_expr`) is an advanced escape hatch; the ordinary path is
  single-objective.

## HiGHS solve path

**Before** — manual adapters and destructive changelog drains.

```rust
// (pre-P21) create an adapter, drain changes, apply, solve...
let mut adapter = HighsAdapter::new()?;
adapter.apply_changes(model.drain_changes())?;
```

**After** — the `roml_highs::Highs` façade owns commit, synchronization,
delta replay, and snapshot rebuild (D2/D3/D5).

```rust
use roml_highs::Highs;

let mut highs = Highs::new()?;
let solution = highs.solve(&mut model)?;   // commits + synchronizes + solves
```

Notes:

- `Model::drain_changes()` is deprecated; `commit()` returns the new
  `ModelRevision` and the façade synchronizes automatically. At most one
  automatic snapshot-rebuild retry per solve attempt (API-02.3).
- Unsupported model changes surface as `Err(SolveError::Synchronization(_))`
  after the rebuild retry — never a stale or fabricated result (API-01.5).

## Parameter update and re-solve

**Before**

```rust
model.set_parameter(price, 3.0);
model.commit()?;
// ... re-run the adapter manually
```

**After** — `set_parameter` is fallible (stale IDs and non-finite values are
rejected, API-06.3); the next `solve` applies the change as a delta.

```rust
model.set_parameter(price, 3.0)?;
let solution = highs.solve(&mut model)?;   // one Highs instance, reused
```

## Solve options

**Before**

```rust
// (pre-P21) one-shot SolveOptions stored on the model
model.solve_options = Some(SolveOptions { time_limit: 60.0, threads: 4 });
```

**After** — per-solve ergonomic builder, validated before synchronization.

```rust
use std::time::Duration;

let solution = highs.solve_with(
    &mut model,
    SolveOptions::new()
        .time_limit(Duration::from_secs(60))
        .threads(4)
        .relative_gap(0.01)
        .output(false)
        .random_seed(7)
        .backend_option("presolve", "off"),
)?;
```

Notes:

- Options never leak across solves: each request is reset to backend defaults
  before the explicit values are applied.
- Negative/non-finite gaps and non-positive thread counts fail validation
  before any backend mutation (`SolveError::InvalidOptions`).

## Solution and status access

**Before** — split `SolveResult`/`TerminationStatus`/`SolverStatus` access.

**After** — one golden-path `Solution` with `SolveStatus` (API-03).

```rust
let solution = highs.solve(&mut model)?;
assert!(solution.status().is_optimal());        // SolveStatus
if let Some(v) = solution.value(x) { /* primal */ }
let objective = solution.objective_value();      // Option<f64>
let meta = solution.metadata();                  // backend, revision, effective config
```

Notes:

- A mathematical termination (infeasible, unbounded, limit) returns
  `Ok(Solution)` with the corresponding `SolveStatus`; an inability to perform
  or interpret the solve returns `Err(SolveError)` (API-03.3).
- `SolverStatus` and `SolverError` remain as compatibility aliases on the
  root; the golden path uses `SolveStatus` and `SolveError`.

## Advanced backend sessions

The backend contract, revisions, snapshots, deltas, cursors, capabilities,
callbacks, raw IDs, and expression internals are grouped under
`roml::advanced` (API-07.3, D9) and are absent from the default prelude
(API-07.2).

**Before** — import protocol types from the prelude.

```rust
use roml::prelude::*;
// DeltaBatch, ModelRevision, BackendSession, AdapterCursor, ... all in scope
```

**After** — import from `roml::advanced` explicitly.

```rust
use roml::advanced::{BackendSession, DeltaBatch, ModelRevision, Synchronization};
```

Notes:

- `BackendSession` is frozen for M2 (API-08.4); changes require an ADR
  amendment. `roml::solver::reference::ReferenceBackend` and
  `roml::solver::conformance` are the executable spec for backend authors.
- Raw IDs (`VarId`, `ConId`, `ObjId`, `ParamId`, `CoeffId`) live in
  `roml::advanced` and `roml::id`; ordinary code uses the semantic aliases.
- `IdArena` (raw arena internals) is crate-private (API-07.4).

## Coefficient-cell operations

Sparse coefficient cells are addressed by coordinate, not by `CoeffId` (D11).
The trio is `set_coefficient` (replace), `add_to_coefficient` (algebraic add),
and `remove_coefficient_at` (remove).

```rust
use roml::advanced::CoefficientTarget;

// Replace the canonical (constraint, variable) cell value.
model.set_coefficient(CoefficientTarget::Constraint(con), x, 2.5)?;
// Algebraically add to it.
model.add_to_coefficient(CoefficientTarget::Constraint(con), x, 1.5)?;
// Remove it by coordinate.
model.remove_coefficient_at(CoefficientTarget::Constraint(con), x)?;
```

Notes:

- Non-finite coefficient values are rejected before mutation (P23).
- `CoeffId` remains available in `roml::advanced` for framework authors; the
  ordinary surface reasons about matrix cells.

## Imports and prelude changes

The curated default prelude (P23) is limited to common model, expression,
definition, solver, solution, and error types (API-07.1).

**Before** (P20-era prelude):

```rust
use roml::prelude::*; // also brought Change, DeltaBatch, ModelRevision, ... into scope
```

**After** (P23 prelude):

```rust
use roml::prelude::*; // Model, aliases, definitions, LinExpr/specs, SolveOptions,
                      // Solution, SolveStatus, SolveError, ModelError, Bounds, ...
use roml::advanced::*; // only when implementing backends or using protocol types
```

The prelude re-exports: `Model`, `ModelError`, `Variable`/`Constraint`/
`Objective`/`Parameter`, `VariableDef`/`ParameterDef`, `continuous`/`integer`/
`binary`/`parameter`, `LinExpr`, `ConstraintSpec`, `ObjectiveSpec`,
`ConstraintExprExt`, `ObjectiveExprExt`, `Bounds`, `Sense`, `VarType`,
`SolveOptions`, `Solution`, `SolveStatus`, `SolveError`, and the pure
`constraint!` builder.

Protocol/backend types absent from the prelude (API-07.2): `Change`,
`CoeffId`, `DeltaBatch`, `ModelOp`, `ModelRevision`, `ModelSnapshot`,
`AdapterCursor`, `AdapterHealth`, `Synchronization`, `BackendSession`,
`SyncReceipt`. All remain reachable at the crate root and under
`roml::advanced`.
