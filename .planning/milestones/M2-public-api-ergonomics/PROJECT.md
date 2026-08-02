# ROML Public API Ergonomics Project

## Primary objective

Turn ROML's qualified incremental modeling machinery into a small, obvious external API that is pleasant for ordinary Rust users while preserving advanced backend extension points.

## Immediate deliverable

A coherent model-build-solve-update workflow with:

- one canonical modeling style;
- one public solver façade;
- one public solution/status model;
- first-class names for model entities;
- typed validation in debug and release builds;
- advanced protocol types moved out of the default path;
- copy-pasteable examples that compile and solve.

## Current problem

The repository has strong internal capabilities but an incomplete external loop:

- documentation references `HighsAdapter::solve_model`, while the backend exports `HighsSession` and requires separate synchronization and solve operations;
- model revisions and journal data are internally owned, so external callers cannot naturally orchestrate incremental synchronization;
- constraints and objectives each have several equivalent methods and macros with no single golden path;
- root exports and the prelude mix ordinary modeling concepts with backend protocol internals;
- `Solution`/`SolverStatus` overlap with `SolveResult`/`SolveSolution`/`TerminationStatus`;
- names exist inside stores but are not exposed through normal `Model` methods;
- validation and fallibility differ across bounds, parameters, variables, and coefficients;
- the parameter-update example is a placeholder rather than a working user workflow.

## Product promise

ROML should feel like a Rust-native optimization library, not a backend protocol toolkit.

A user should reason in this order:

```text
Model -> variables/parameters -> constraints -> objective -> solver -> solution
```

The library should internally handle:

```text
pending mutations -> atomic revision -> delta or snapshot -> backend synchronization
-> solve option negotiation -> result normalization
```

## Binding constraints

- Core remains solver-free and contains no native HiGHS dependency.
- Existing canonical-cell, revision, journal, snapshot, recovery, and backend-session invariants remain intact.
- No silent fallback for unsupported solve options.
- No silent invalid-ID or non-finite-value handling.
- HiGHS remains the reference end-to-end backend.
- MOSEK and Xpress are not required to complete M2, but the generic façade must not prevent later adapters.
- MSRV remains Rust 1.85 unless changed by a separate owner-approved decision.
- No crate publication, tag, or release is part of this milestone.

## Canonical API shape

### Model construction

```rust
let mut model = Model::named("dispatch");

let generation = model.add_variable(
    continuous().bounds(0.0, 100.0).named("generation"),
)?;
let committed = model.add_variable(binary().named("committed"))?;
let price = model.add_parameter(parameter(20.0).named("price"))?;

model.add_constraint(
    generation.ge(10.0).named("minimum_generation"),
)?;
model.add_constraint(
    (generation - 100.0 * committed)
        .le(0.0)
        .named("commitment_link"),
)?;
model.maximize(price * generation - 100.0 * committed)?;
```

### Solve and incremental update

```rust
let mut highs = Highs::new()?;
let first = highs.solve(&mut model)?;

model.set_parameter(price, 30.0)?;
let second = highs.solve(&mut model)?;
```

### Explicit solve policy

```rust
let options = SolveOptions::new()
    .time_limit(std::time::Duration::from_secs(30))
    .relative_gap(1e-4)
    .threads(8)
    .output(false);

let solution = highs.solve_with(&mut model, options)?;
```

## Scope boundaries

### In scope

- generic model-to-backend orchestration;
- `roml_highs::Highs` façade;
- normalized `Solution`, `SolveStatus`, and metadata;
- named variable, parameter, constraint, and objective APIs;
- method-first modeling API;
- optional pure `constraint!` builder macro;
- prelude/root export curation;
- typed validation and consistent errors;
- migration/deprecation path;
- working README, guide, examples, doctests, and fresh consumers.

### Explicitly out of scope

- indexed variable containers or tensor-shaped decision variables;
- nonlinear, conic, stochastic, or neural-network modeling;
- serialization format design;
- LP/MPS import/export;
- IIS extraction;
- basis/warm-start public API;
- callback simplification;
- multi-objective policy beyond preserving the advanced existing capability;
- commercial backend qualification;
- performance redesign of the core revision protocol.

## Acceptance criteria

1. The README's primary HiGHS example compiles and solves in CI.
2. A parameter update followed by `solve` reuses the same backend façade and produces the updated answer.
3. Ordinary user code imports no revision, snapshot, delta, cursor, synchronization, or backend-session type.
4. The prelude contains only common modeling, solving, and error concepts.
5. There is one public solution type and one public status type in the golden path.
6. Every model entity can be named at creation or through a documented mutation.
7. Invalid bounds, non-finite values, stale IDs, and invalid solve options return typed errors in release builds.
8. Existing backend conformance, differential, callback, and recovery tests remain green.
9. Fresh projects consume packed `roml` and `roml-highs` archives without workspace path leakage.
10. Public API inventory and migration documentation show no accidental stable exposure.