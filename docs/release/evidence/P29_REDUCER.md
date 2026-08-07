# Phase 29 Portable Reducer Evidence

## Scope

- Slice: deterministic semantic reduction and mandatory fresh verifier
- Base: `dce62f6`
- Provider: solver-agnostic `AnalysisSession` oracle seam
- Cardinality: no optimization and no minimum-cardinality vocabulary

## RED characterization

`tests/iis_reducer.rs` first failed because no reducer module or reduction result
types existed. The fixture oracle proves infeasibility only when semantic atoms
0 and 1 are both selected, making atoms 2 and 3 removable witnesses.

## Implemented behavior

- Initial proven-feasible checks return `NoConflict`.
- Initial `Unknown` checks return `NoConflictProof`.
- Proven infeasible seeds use deterministic chunk deletion followed by exact
  single-atom polish.
- The reducer performs a fresh final selected-set check and fresh checks for
  every single-atom deletion before `Irreducible` is returned.
- Any non-proven deletion check or failed final verification downgrades the
  result to `InfeasibleSubsystem`; no incomplete run can claim irreducibility.
- Statistics retain oracle calls, iterations, chunk attempts, and fresh
  verification checks.

## Verification

```text
cargo fmt --all -- --check                             PASS
cargo clippy -p roml --test iis_reducer -- -D warnings PASS
cargo test -p roml --test iis_reducer                  PASS (2 tests)
```

Integration with the public `SolverSession` report orchestration and native
seed providers remains a later slice. Multiple-IIS enumeration and
minimum-cardinality search are explicitly out of scope.
