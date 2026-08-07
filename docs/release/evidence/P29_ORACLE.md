# Phase 29 Isolated Oracle Evidence

## Scope

- Slice: isolated LP feasibility-oracle contract and cache/recovery seam
- Base: `c489e04`
- Provider implementation: bundled HiGHS 1.15.0 isolated LP oracle with
  incremental restriction toggles; system-discovered native providers remain
  typed unsupported pending their version matrix

## RED characterization

`tests/infeasibility_oracle.rs` initially failed because there was no
`AnalysisSession`, no exact `RestrictionSelection::all`, and no isolated
oracle-session contract. The tests then established the smallest executable
behavior before backend integration.

## Implemented behavior

- `AnalysisSession` receives one exact `SemanticConflictUniverse` and an oracle
  whose `CompilationId` must match before construction.
- Selection checks reject stale compilation identity and atom ids before the
  oracle is called.
- Cache keys include exact compilation identity, universe atom identity,
  grouping, selected atoms, and numerical tolerance bits.
- Proven feasible, proven infeasible, and `Unknown` outcomes remain distinct;
  no status coercion is performed by the session.
- Backend oracle errors clear the cache and transition only the isolated
  analysis session to `RequiresRebuild`; the persistent solve session is not
  reused by this seam.
- A cache hit does not invoke the oracle again.
- The bundled HiGHS oracle builds one fresh analysis session from the exact
  `BackendSnapshot`, changes row/column bounds incrementally between checks,
  and reports only the three tri-state outcomes.

## Verification

```text
cargo fmt --all -- --check                                  PASS
cargo clippy -p roml --test infeasibility_oracle -- -D warnings PASS
cargo test -p roml --test infeasibility_oracle              PASS (3 tests)
```

## Qualification boundary

The solver-free core still does not pretend to be an LP optimizer. Real LP
oracle qualification is attached to the bundled HiGHS evidence; backend
authors must provide the same isolated factory contract before another native
provider can participate.
