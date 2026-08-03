# M3 Semantic Modeling and Solve Workflow Requirements

Requirement IDs are stable for M3. Every implementation PR must list the IDs it closes and the exact tests/evidence used.

## SM-01 — Canonical semantic IR

- **SM-01.1** Canonical model state stores linear function-in-set constraints and high-level constructs without eagerly erasing them into backend rows.
- **SM-01.2** `ScalarFunction` and `ScalarSet` are non-exhaustive extension points; M3 implements linear scalar functions only.
- **SM-01.3** Every construct has a stable handle, metadata, activity state, and parameter-dependency information.
- **SM-01.4** Canonical snapshots and deltas include semantic constructs and objective policies.
- **SM-01.5** Ordinary M2 `LinExpr` and constraint-builder APIs remain the canonical linear user path.
- **SM-01.6** No backend index, selected Big-M, native handle, or solve overlay is stored in canonical model state.

## SM-02 — Identity, lineage, metadata, and provenance

- **SM-02.1** Every independent model has an opaque `ModelLineageId`; clones preserve lineage.
- **SM-02.2** Reusable assignments validate lineage and entity generations.
- **SM-02.3** Variables, constraints, objectives, and constructs support names, descriptions, groups, tags, and optional source metadata.
- **SM-02.4** Compiler-generated and solve-overlay entities use distinct compiled IDs, never user entity handles.
- **SM-02.5** Every generated entity maps to a user entity, construct, or overlay role.
- **SM-02.6** Diagnostics distinguish declared bounds, persistent fixings, construct-derived bounds, and temporary locks.

## SM-03 — Compiler boundary and backend IR

- **SM-03.1** A capability-aware compiler transforms canonical snapshots/deltas into solver-neutral backend snapshots/deltas.
- **SM-03.2** Backends consume backend IR rather than mutable canonical `Model` state.
- **SM-03.3** Backend IR supports linear rows plus normalized indicator, SOS1, SOS2, and PWL primitives.
- **SM-03.4** `CompilationPolicy::{Auto, Portable, NativeRequired}` has documented deterministic semantics.
- **SM-03.5** Compilation produces an origin map, formulation report, recipe fingerprint, and generated-entity inventory.
- **SM-03.6** Recipe changes force deterministic rebuild; unchanged qualified recipes may produce compiled deltas.
- **SM-03.7** Primitive linear incremental behavior remains equivalent to compiled rebuild.
- **SM-03.8** Backend contract migration is documented and tested for the reference backend.

## SM-04 — Typed capabilities and effective support

- **SM-04.1** Backend feature support is queried through typed features rather than an expanding flat Boolean struct.
- **SM-04.2** Native backend support and ROML bridge support are reported separately.
- **SM-04.3** Capability declarations may vary by runtime/backend version and include limitations.
- **SM-04.4** Unsupported features are rejected unless an explicit exact bridge or user-selected conversion exists.
- **SM-04.5** Every solve records selected native features, bridges, adjustments, and rejections.

## SM-05 — Persistent variable fixing

- **SM-05.1** Variable state separates declared domain from optional persistent fixing.
- **SM-05.2** `Model::fix` and `Model::unfix` are typed atomic canonical mutations.
- **SM-05.3** The default compiled representation of fixing is equal lower/upper bounds.
- **SM-05.4** `unfix` restores the current declared bounds, not the bounds at the time of fixing.
- **SM-05.5** continuous, integer, and binary fixing validation is explicit and tolerance-aware.
- **SM-05.6** declared-bound changes that exclude an active fixing fail atomically.
- **SM-05.7** persistent fixing changes synchronize incrementally when the backend supports bound changes.

## SM-06 — Primal assignments and solution reuse

- **SM-06.1** `PrimalAssignment` stores a partial mapping without claiming feasibility or optimality.
- **SM-06.2** `Solution` can produce a lineage-bound primal assignment and selected subsets.
- **SM-06.3** solution locks, MIP starts, hints, and persistent fixings are distinct public types.
- **SM-06.4** solution-lock selectors support all assigned, integer assigned, binary assigned, explicit variables, and exclusions.
- **SM-06.5** continuous locks support exact or absolute-band semantics.
- **SM-06.6** stale or unrelated assignments fail before backend mutation.

## SM-07 — Solve plans and overlays

- **SM-07.1** `SolvePlan` combines solve options, overlays, starts, hints, objective overrides, and unsupported-feature policy.
- **SM-07.2** existing `solve` and `solve_with` remain convenience paths over `SolvePlan`.
- **SM-07.3** temporary fixings, solution locks, objective-lock rows, and cutoffs do not mutate the canonical model revision.
- **SM-07.4** overlay application and rollback are transactional from the caller's perspective.
- **SM-07.5** rollback uncertainty marks the backend `RequiresRebuild`.
- **SM-07.6** failure injection proves that no overlay leaks into a later solve.
- **SM-07.7** `Solution` metadata contains the effective solve plan and objective-stage results.

## SM-08 — MIP starts, hints, and warm-start semantics

- **SM-08.1** `MipStart` accepts full or partial primal assignments with explicit repair policy.
- **SM-08.2** multiple starts are supported only when declared by the backend; otherwise behavior follows explicit policy.
- **SM-08.3** `VariableHints` stores independent value/priority entries and never changes feasibility.
- **SM-08.4** unsupported starts and hints are rejected by default, never silently ignored.
- **SM-08.5** explicit conversion policies may convert hints to a start or a start to temporary fixing; conversions are recorded.
- **SM-08.6** LP basis warm starts remain a separate future artifact and are not conflated with primal assignments.
- **SM-08.7** HiGHS start behavior is qualified against pinned official APIs and supported versions.

## SM-09 — IIS and conflict analysis

- **SM-09.1** infeasibility analysis is an optional backend capability, not a required `BackendSession` method.
- **SM-09.2** analysis preserves kind, scope, minimality claim, completion status, backend identity, and model revision.
- **SM-09.3** conflict members distinguish constraint sides, variable bounds, persistent fixings, temporary locks, and constructs.
- **SM-09.4** compiled conflict findings map through the origin map to original ROML terms.
- **SM-09.5** reports render text, Markdown, and structured Rust data.
- **SM-09.6** a request on a model not proven infeasible returns a typed error unless the backend explicitly supports direct analysis.
- **SM-09.7** HiGHS IIS support is version-qualified from official headers/APIs; unsupported versions return typed `Unsupported`.
- **SM-09.8** a portable deletion filter is not labeled native IIS.

## SM-10 — Soft constraints and feasibility relaxation

- **SM-10.1** a user can soften an existing constraint through one builder call.
- **SM-10.2** upper, lower, equality, and ranged relaxation semantics are mathematically correct.
- **SM-10.3** nonnegative lower/upper violation variables have stable handles and generated provenance.
- **SM-10.4** maximum violation bounds are finite/validated when supplied.
- **SM-10.5** signed correction is an explicit separate API.
- **SM-10.6** penalties target an objective, a lexicographic priority, or no objective; sign handling is correct for objective sense.
- **SM-10.7** penalty weights may depend on parameters and remain finite.
- **SM-10.8** solution APIs report lower, upper, and total violations in original constraint terms.
- **SM-10.9** backend-native feasibility relaxation is solve-scoped and separate from persistent soft constraints.

## SM-11 — Objective policies and lexicographic solves

- **SM-11.1** model state supports single, weighted, and lexicographic objective policies.
- **SM-11.2** lexicographic levels store absolute and relative degradation tolerances.
- **SM-11.3** native multiobjective execution is used only when backend semantics match the declared policy.
- **SM-11.4** portable fallback executes sequential solves with temporary objective-lock constraints.
- **SM-11.5** default stage policy requires an optimal stage before descending.
- **SM-11.6** explicit best-feasible continuation records the qualification.
- **SM-11.7** solution results expose values for every objective and every stage.
- **SM-11.8** native and portable execution agree on the qualification corpus.

## SM-12 — Common modeling constructs

- **SM-12.1** indicators preserve one-way implication semantics and compile natively or through exact bridges.
- **SM-12.2** reification/threshold detection requires explicit continuous separation unless integrality proves a discrete complement.
- **SM-12.3** exact min/max is distinct from epigraph/hypograph relations.
- **SM-12.4** absolute value, positive part, and clamp have explicit exact semantics.
- **SM-12.5** Boolean implication, equivalence, any/all, and cardinality helpers have exact linear formulations.
- **SM-12.6** exact product support is limited to binary-binary and binary times bounded linear scalar function.
- **SM-12.7** continuous-times-continuous products are not mislabeled exact.
- **SM-12.8** construct APIs return stable handles/results and expose formulation diagnostics.

## SM-13 — Big-M and bound analysis

- **SM-13.1** deterministic interval analysis computes bounds for linear scalar functions.
- **SM-13.2** a Big-M bridge requires a finite derived value or explicit user value.
- **SM-13.3** explicit M values are validated against known bounds where possible.
- **SM-13.4** compilation errors identify the construct and missing/unbounded expression.
- **SM-13.5** compilation reports record M values, derivations, and bound sources.
- **SM-13.6** M3 does not silently run auxiliary optimization problems for bound tightening.

## SM-14 — Piecewise-linear functions

- **SM-14.1** PWL points are finite, strictly ordered, and have explicit extrapolation policy.
- **SM-14.2** convexity/concavity classification is deterministic from segment slopes.
- **SM-14.3** convex epigraph and concave hypograph formulations introduce no binaries.
- **SM-14.4** exact graph formulations use qualified native PWL, SOS2, or exact binary representation.
- **SM-14.5** nonconvex exact graphs never fall back to a convex relaxation.
- **SM-14.6** representation choice and introduced binaries/auxiliaries appear in the compilation report.
- **SM-14.7** randomized PWL evaluations agree with compiled formulations over the tested domain.

## SM-15 — Qualification, compatibility, and NLP readiness

- **SM-15.1** M2 golden-path source compatibility is preserved unless a reviewed contradiction is documented.
- **SM-15.2** all backend-contract changes have migration documentation and conformance tests.
- **SM-15.3** focused, full, cross-platform, rustdoc, public-API, package, and fresh-consumer checks are recorded.
- **SM-15.4** portable/native formulation equivalence has deterministic corpus evidence.
- **SM-15.5** primitive parameter-update and solve performance remain within P34 regression thresholds.
- **SM-15.6** public documentation distinguishes semantic guarantees, native support, bridge support, and version limitations.
- **SM-15.7** an independent review verifies future quadratic/nonlinear functions can extend `ScalarFunction`, backend IR, capabilities, and compiler recipes without replacing identity, provenance, solve plans, objective policies, or reporting.
- **SM-15.8** no crate publication, tag, or release occurs without a separate explicit owner authorization.
