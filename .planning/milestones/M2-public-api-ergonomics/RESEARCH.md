# M2 Current-Main Research

**Inspected baseline:** `ac473911bc2239e940b8c2019dee3e01a445701e`

## R1 — The documented solve path is stale

`README.md` and `MODELING_API.md` show `roml_highs::HighsAdapter` and `solve_model(&mut model)`. Current `roml-highs/src/lib.rs` exports `HighsSession`, while `src/solver/session.rs` defines separate `synchronize` and `solve` operations. Repository search finds no production `HighsAdapter` or `solve_model` implementation.

**Impact:** the primary user story fails at copy/paste. P21 is required before cosmetic API cleanup.

## R2 — Core owns the data needed for orchestration

`Model::commit()` records a `DeltaBatch` through its private `SyncCoordinator`. `SyncCoordinator::batches_for_cursor` can select replayable deltas, and `Model::take_snapshot()` can produce a rebuild input. Because the coordinator is model-private, downstream backend crates cannot correctly orchestrate synchronization without either leaking internals or moving orchestration into core.

**Impact:** generic `SolverSession<B>` belongs in `roml`.

## R3 — The backend protocol is already suitable

`BackendSession` exposes `synchronize`, `solve`, and `close`; supplementary traits expose health, revision, metadata, callbacks, and solution views. HiGHS implements the contract and has conformance, differential, fault, and observable tests.

**Impact:** M2 should preserve the backend contract and add a façade rather than redesign it.

## R4 — Modeling entry points compete

Constraints currently include `constrain`, `constraint`, `constraint!`, `constrain!`, `add_constraint_expr`, and low-level coefficient APIs. Objectives include `minimize`, `maximize`, `set_objective`, `objective!`, `set_objective!`, `add_objective_spec`, `add_objective_expr`, and explicit activation.

**Impact:** choose one method-first path and classify the rest as optional builder or advanced escape hatch.

## R5 — Root/prelude exports mix audiences

`src/lib.rs` re-exports protocol types and includes `Change` and `CoeffId` in the prelude. These are valid for backend/framework authors but not normal model authors.

**Impact:** curate the prelude and group extension points.

## R6 — Result concepts overlap

`src/solution/mod.rs` defines user-oriented `Solution` with `SolverStatus`. `src/solver/request.rs` defines `SolveResult`, optional `SolveSolution`, and backend `TerminationStatus`.

**Impact:** keep the backend result as the protocol representation and normalize it into one user-facing solution/status pair.

## R7 — Names exist below the public model API

Variable, constraint, objective, and parameter stores contain optional names and `add_named` methods. Public `Model` creation methods do not expose them.

**Impact:** M2 can surface names without changing storage architecture.

## R8 — Validation differs by API

Bounds are constructible with public fields/unchecked `new`; `add_variable` is infallible; parameter creation and mutation use `debug_assert!` for finiteness; other mutations return `ModelError`.

**Impact:** definition builders and model mutations must validate in every build profile.

## R9 — Examples do not prove the product promise

`examples/simple_lp.rs` constructs but does not solve. `examples/parameter_update.rs` is a placeholder stating that backend-session migration is required.

**Impact:** P24 must compile and execute both end-to-end workflows.

## R10 — Internal correctness evidence must remain protected

The repository has substantial tests around canonical cells, all model-operation variants, revision chains, multi-cursor independence, fault injection, partial-apply recovery, HiGHS option negotiation, callbacks, duals, and reduced costs.

**Impact:** façade work must be additive over these invariants. Do not weaken tests to obtain ergonomic syntax.

## Existing files expected to change

Core:

- `src/lib.rs`
- `src/model/mod.rs`
- `src/model/variable.rs`
- `src/model/parameter.rs`
- `src/model/constraint.rs`
- `src/model/objective.rs`
- `src/expr/linear.rs`
- `src/solution/mod.rs`
- `src/solver/mod.rs`
- `src/solver/request.rs`
- new focused façade/definition modules

HiGHS:

- `roml-highs/src/lib.rs`
- possibly new `roml-highs/src/facade.rs`

Tests/docs:

- new compile API tests
- core integration tests
- HiGHS end-to-end tests
- `README.md`
- `MODELING_API.md`
- `examples/simple_lp.rs`
- `examples/parameter_update.rs`
- `CHANGELOG.md`
- new `MIGRATION.md`

## Evidence to capture in P20

- `cargo public-api -p roml`
- `cargo public-api -p roml-highs`
- rustdoc item inventory
- compile result for current README code
- compile-pass target quickstart fixture
- full baseline test counts
- repeated solve behavior and revision/health traces
- package file lists for both publishable crates