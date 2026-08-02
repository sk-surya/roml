# M2 Public API Requirements

Requirement IDs are stable for this milestone. Every implementation PR must list the IDs it closes and the exact tests or evidence used.

## API-01 — End-to-end solve façade

- **API-01.1** `roml-highs` exposes `Highs::new() -> Result<Highs, HighsError>`.
- **API-01.2** `Highs::solve(&mut self, model: &mut Model) -> Result<Solution, SolveError>` performs commit, synchronization, solve, and result normalization.
- **API-01.3** `Highs::solve_with(&mut self, model: &mut Model, options: SolveOptions) -> Result<Solution, SolveError>` preserves explicit option negotiation and effective configuration.
- **API-01.4** Reusing one `Highs` instance across solves uses deltas when valid and a snapshot rebuild when required.
- **API-01.5** A failed synchronization or solve never loses model operations and never reports a stale solution as current.

## API-02 — Solver-neutral orchestration

- **API-02.1** Generic synchronization/solve orchestration lives in `roml`, not in each backend crate.
- **API-02.2** The orchestration layer operates on the frozen backend session contract and does not expose model internals to downstream crates.
- **API-02.3** Rebuild fallback is deterministic and bounded: at most one automatic rebuild retry per solve attempt.
- **API-02.4** Backend errors retain backend identity, operation, category, and health effect.

## API-03 — Unified solution semantics

- **API-03.1** Golden-path callers receive one `Solution` type.
- **API-03.2** Golden-path callers inspect one `SolveStatus` type that preserves optimal, feasible, infeasible, unbounded, limit, interrupted, numerical, license, and backend-failure distinctions where applicable.
- **API-03.3** A mathematical termination such as infeasible returns `Ok(Solution)` with no primal values; inability to perform or interpret the solve returns `Err(SolveError)`.
- **API-03.4** `Solution` exposes variable values, objective identity/value, duals, reduced costs, effective options, backend metadata, and model revision where available.
- **API-03.5** Objective constants are included exactly once in reported objective values.

## API-04 — Canonical modeling style

- **API-04.1** `model.add_constraint(spec)` is the canonical constraint mutation.
- **API-04.2** `model.minimize(expr)` and `model.maximize(expr)` are the canonical single-objective mutations.
- **API-04.3** Expression methods `.le`, `.ge`, `.eq`, and `.between` remain the canonical constraint builders.
- **API-04.4** `constraint!` may remain as an optional pure builder macro; effectful `constrain!` and `set_objective!` are not part of the recommended surface.
- **API-04.5** Low-level sparse APIs distinguish replace, algebraic add, and remove-by-cell semantics.

## API-05 — Entity definitions and naming

- **API-05.1** Variable creation supports validated continuous, integer, and binary definitions with optional names.
- **API-05.2** Parameter creation supports a finite initial value and optional name.
- **API-05.3** Constraint specifications support optional names.
- **API-05.4** Objectives support optional names without complicating the ordinary `minimize`/`maximize` path.
- **API-05.5** Names are retrievable and appear in diagnostics and model formatting where available.
- **API-05.6** Public semantic aliases `Variable`, `Constraint`, `Objective`, and `Parameter` are available while raw `*Id` names remain in the advanced API.

## API-06 — Validation and errors

- **API-06.1** Invalid bounds are rejected before model mutation.
- **API-06.2** NaN and non-finite parameter/coefficient values are rejected in debug and release builds.
- **API-06.3** Parameter mutation is fallible and rejects stale IDs.
- **API-06.4** Builders validate domain-specific invariants, including binary bounds and integer bounds.
- **API-06.5** Public mutations are atomic from the caller's perspective on validation failure.
- **API-06.6** Error messages identify the entity or option and the violated invariant.

## API-07 — Public surface curation

- **API-07.1** The default prelude is limited to common model, expression, definition, solver, solution, and error types.
- **API-07.2** `Change`, `CoeffId`, `DeltaBatch`, `ModelOp`, `ModelRevision`, `ModelSnapshot`, `AdapterCursor`, `AdapterHealth`, `Synchronization`, `BackendSession`, and `SyncReceipt` are absent from the default prelude.
- **API-07.3** Backend extension points are grouped under `roml::advanced` or `roml::backend` with explicit stability documentation.
- **API-07.4** Implementation stores and raw arena details remain private.
- **API-07.5** `cargo public-api` output is reviewed and stored as milestone evidence.

## API-08 — Compatibility and migration

- **API-08.1** Replacement APIs land and compile before old conveniences are deprecated.
- **API-08.2** Pre-1.0 breaking changes are documented in `MIGRATION.md` and `CHANGELOG.md`.
- **API-08.3** Deprecated APIs include actionable replacement notes and remain tested for the chosen deprecation window.
- **API-08.4** Backend contract changes are forbidden in M2 unless an executable contradiction makes the façade impossible; such a change requires an ADR amendment.

## API-09 — Documentation and examples

- **API-09.1** README contains a compiled HiGHS LP/MILP solve example using the golden path.
- **API-09.2** A compiled incremental parameter example demonstrates two solves with one `Highs` instance.
- **API-09.3** `MODELING_API.md` teaches the canonical path first and labels advanced escape hatches.
- **API-09.4** Rustdoc covers errors, status semantics, synchronization behavior, and solution availability.
- **API-09.5** Examples use no machine-specific paths or commercial solver dependencies.

## API-10 — Qualification

- **API-10.1** Existing core and HiGHS test suites remain green.
- **API-10.2** Compile-pass tests cover the canonical API; compile-fail tests reject invalid or ambiguous usage where practical.
- **API-10.3** Fresh consumers build against packaged `roml` and `roml-highs` archives.
- **API-10.4** Core-only consumers require no C/C++ compiler or solver library.
- **API-10.5** HiGHS consumers work with the documented default feature and system-discovery mode where supported.
- **API-10.6** Independent review confirms API coherence, protocol preservation, error semantics, and documentation accuracy.