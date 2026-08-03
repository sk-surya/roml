# ROML Semantic Modeling and Solve Workflows Project

## Primary objective

Make ROML useful for serious research and production MILP modeling by adding high-value modeling and diagnostic workflows without turning the library into an ornamental DSL or coupling the canonical model to one solver.

## Product thesis

ROML's durable advantage should be:

```text
semantic mathematical intent
  -> dependency-aware canonical state
  -> capability-aware formulation selection
  -> recoverable persistent solver sessions
  -> origin-aware results and diagnostics
```

The library should let modelers express intent once, inspect how it was represented, and choose between backend-native performance and portable research formulations.

## Immediate deliverable

M3 delivers:

- canonical semantic constructs;
- a compiler boundary between canonical model state and backend state;
- typed backend capabilities and formulation reports;
- model lineage and provenance;
- persistent variable fixing by effective bound tightening;
- partial primal assignments, solution locking, MIP starts, and variable hints with distinct semantics;
- reversible solve overlays;
- native IIS/conflict analysis where supported and accurate reports in original model terms;
- bounded nonnegative constraint slacks, signed corrections, and explicit penalties;
- native or sequential lexicographic objective execution;
- indicators, reification, Boolean/cardinality constraints, min/max, absolute value, positive part, clamp, and exact supported products;
- piecewise-linear relations with convexity-aware formulations;
- a verified extension seam for later quadratic and nonlinear programming.

## User promise

A ROML modeler should be able to answer all of these questions from public APIs and reports:

1. What mathematical construct did I declare?
2. Which backend representation did ROML select?
3. Why was that representation valid?
4. Which auxiliary rows or variables were generated?
5. Which original construct caused an infeasibility conflict?
6. Did this solve use a fixing, a lock, a start, or a hint?
7. Which solve-scoped instructions were applied, adjusted, rejected, or bridged?
8. Which higher-priority objective values constrained later stages?
9. Did a PWL formulation introduce binaries, and why?
10. Can the backend be safely reused for the next solve?

## Scope boundaries

### In scope

- linear and mixed-integer canonical functions;
- semantic construct storage and revision tracking;
- solver-neutral backend IR;
- portable exact MILP bridges;
- selected normalized native backend primitives;
- origin mapping and compilation reports;
- solve overlays, starts, hints, and objective staging;
- IIS/conflict analysis and feasibility relaxation interfaces;
- HiGHS reference implementation;
- user-facing examples, docs, conformance, packaging, and fresh-consumer qualification.

### Explicitly out of scope

- general nonlinear expression tracing;
- automatic differentiation, gradients, Jacobians, or Hessians;
- quadratic objectives or constraints;
- conic modeling;
- complementarity/MPEC solving;
- stochastic-programming scenario trees;
- CP-SAT/global-constraint parity;
- automatic LP-based bound tightening;
- automatic selection among many advanced reformulations based on empirical performance;
- general disjunctive-programming DSL;
- stable cross-process model serialization;
- Python/Java/.NET APIs;
- publication or release.

## Engineering posture

M3 is not a feature dump. Every feature must have:

- one semantic definition;
- one public construction path;
- a typed validation contract;
- one or more explicit representations;
- a capability/bridge decision;
- origin mapping;
- deterministic rebuild behavior;
- focused semantic tests;
- backend conformance evidence;
- documentation that distinguishes guarantees from backend limitations.

Features that cannot meet this bar remain deferred.

## Architecture boundaries

### Canonical model

Owns identity, metadata, functions, sets, constructs, objective policies, declared domains, persistent fixings, parameters, revisions, and canonical deltas.

### Compiler

Owns capability negotiation, bridge selection, bound analysis, generated entities, origin maps, representation fingerprints, compiled snapshots/deltas, and reports.

### Solve orchestration

Owns canonical synchronization, solve overlays, starts, hints, objective staging, rollback, effective-plan reporting, and result projection.

### Backend

Owns translation from backend IR into an official solver API, native feature application, version-specific capability declarations, native diagnostics, lifecycle, and result extraction.

### Analysis/reporting

Owns normalized conflict semantics, mapping compiled findings to original entities, violation reporting, and human/structured output.

## Acceptance criteria

1. Existing M2 golden-path LP/MILP examples remain source-compatible and green.
2. Canonical snapshots contain semantic constructs rather than only their expanded rows.
3. A compiler produces deterministic backend IR and complete origin maps.
4. A backend session never receives mutable canonical model internals.
5. Persistent fix/unfix restores declared bounds correctly and synchronizes incrementally where supported.
6. Solution locks are temporary and cannot leak into subsequent solves.
7. MIP starts and hints do not alter the feasible region.
8. Unsupported starts/hints are not silently ignored.
9. HiGHS IIS support is version-qualified and reports original ROML names and provenance.
10. Soft constraints produce correct violation algebra, values, and objective penalties.
11. Lexicographic native and portable execution agree on qualified test models.
12. Indicators and other exact constructs never use unproven Big-M values.
13. Convex PWL epigraphs and concave PWL hypographs avoid binaries.
14. Exact nonconvex PWL graphs use a qualified native/SOS2/binary formulation.
15. Every generated backend entity has an origin.
16. Randomized compiled-delta execution remains equivalent to compiled rebuild.
17. Public API, rustdoc, migration, package, and fresh-consumer checks pass.
18. An independent review verifies that adding quadratic/nonlinear scalar functions would extend rather than replace the architecture.

## Success metrics

- Zero silent feature degradation.
- Zero generated entities without origin records.
- Zero Big-M bridges without finite proof or explicit user value.
- Zero overlay-leak failures in injected-failure tests.
- Exact semantic agreement between portable and native formulations on the qualification corpus.
- Existing primitive parameter-update benchmarks regress by no more than the threshold defined in P34.
- New users can build, solve, lock, soften, diagnose, and lexicographically optimize from compiled public examples alone.
