---
gsd_phase_number: M1R-01
name: backend-contract-migration-closure
milestone: ROML-M1R
goal: Make the revisioned protocol the only supported semantic execution path.
dependencies: [M1R-00]
parallelism: one contract owner; tests/docs and compatibility analysis may run in parallel
---

# M1R-01 — Backend Contract Migration Closure

## Target public flow
```text
Model::snapshot/revision/journal
  -> BackendSession::synchronize(&Model)
  -> pending DeltaBatch sequence or rebuild snapshot
  -> ApplyReceipt + AdapterCursor/Health
  -> BackendSession::solve(&SolveRequest)
  -> SolveResult { requested, effective, termination, solution_view }
```

## Tasks
### 01.1 API inventory and compatibility decision
Inventory every public trait/type/method/export in `src/solver`, `src/sync`, crate root, prelude, README, MODELING_API, examples, and backend crates. Decide exact replacement/deprecation mapping for:
- `SolverAdapter`
- `SolverModelExt`
- `SolveOptions`
- `SolverStatus` / `SolverError`
- cloned `HashMap` solution access
- callbacks

### 01.2 Write failing contract tests
Mandatory tests must prove:
- backend failure cannot consume unacknowledged deltas;
- two sessions independently catch up;
- failed request validation consumes neither request nor model history;
- requested option is applied/adjusted/rejected;
- rebuild resets cursor and health deterministically;
- status preserves incumbent/proof/ambiguous states;
- legacy shim, if retained, delegates safely.

### 01.3 Finalize interfaces
Define exact interfaces in focused modules:
- `BackendAdapter` or `BackendSessionFactory`
- `BackendSession`
- `ApplyReceipt` / `ApplyOutcome`
- `AdapterCursor` / `AdapterHealth`
- `SolveRequest` / `EffectiveSolveConfig`
- `Termination` / `BackendError`
- borrowed/indexed `SolutionView`
- capability sets and callback taxonomy

The implementation plan must include concrete signatures before worker dispatch. Avoid one giant trait when lifecycle, synchronization, solve, and extraction can be separate bounded interfaces.

### 01.4 Synchronization integration
- Connect Model revision/journal/snapshot to session synchronization.
- Define acknowledgement and journal retention behavior.
- Define gap/rebuild behavior.
- Ensure transactions create one atomic revision boundary.
- Remove supported reliance on adjacency of legacy `Change` events.

### 01.5 Solve-policy separation
- Remove transient options from canonical `Model`.
- Make each `SolveRequest` immutable and reusable after failure.
- Return requested and effective configuration.
- Validate capabilities before mutation/solve where possible.

### 01.6 Legacy disposition
Choose one:
- remove legacy APIs before v0.1; or
- provide a deprecated adapter over the safe session contract.

Forbidden: retaining `drain_changes()` + best-effort option behavior as a supported path.

### 01.7 Public surface and documentation
Update crate exports, prelude, README, MODELING_API, examples, changelog, migration guide, and semver baseline. All examples compile and use the supported path.

## Verification
```bash
cargo test -p roml --all-targets
cargo test -p roml --test backend_contract
cargo test -p roml --test sync_characterization
cargo test -p roml --test status_negotiation_tests
cargo clippy -p roml --all-targets -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps
cargo semver-checks check-release -p roml
```
No required test remains ignored.

## Gate
- M1R-C1–C8 pass.
- Contract has independent architecture/API review.
- HiGHS/MOSEK/Xpress workers receive frozen interfaces and conformance adapter hooks.
- Contract edits after freeze require a recorded decision and coordinated rebase.
