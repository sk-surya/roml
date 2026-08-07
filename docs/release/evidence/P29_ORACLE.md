# Phase 29 Isolated Oracle Evidence

## Scope

- Slice: isolated LP feasibility-oracle contract and cache/recovery seam
- Base: `c489e04`
- Provider implementation: solver-agnostic orchestration only; native backend
  factory and real LP solve qualification remain later work

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

## Verification

```text
cargo fmt --all -- --check                                  PASS
cargo clippy -p roml --test infeasibility_oracle -- -D warnings PASS
cargo test -p roml --test infeasibility_oracle              PASS (3 tests)
```

## Remaining gate

The next oracle implementation task must connect this seam to an isolated
backend factory that builds once from the exact `BackendSnapshot`, applies
transactional restriction masks, and proves rollback/rebuild behavior. The
solver-free `ReferenceBackend` remains a projection backend and is not being
misrepresented as an LP optimizer.
