# Phase 29 Semantic Restriction Universe Evidence

## Scope and base

- Base after Slice 1: `88efce8`
- Worktree: `phase-roml-P29-iis-conflicts`
- Scope: exact compiled LP restriction atoms and reversible bound layers
- Native provider: not used in this slice

## RED tests

`tests/restriction_universe.rs` initially failed because the exact
`SemanticConflictUniverse::from_snapshot` constructor and restriction-level
vocabulary did not exist. The test fixture is an origin-complete compiled
snapshot containing one ranged row and one bounded variable.

## Implemented invariants

- Finite lower and upper sides become separate semantic atoms in deterministic
  row-then-variable declaration order.
- Each atom carries a stable `ConflictAtomId`, semantic kind, original origin,
  compiled side reference, disable/restore plans, and immutable name/value
  snapshot.
- `RestrictionOriginMap` records the exact `CompilationId` and rejects map or
  atom access under another compilation identity before lookup.
- `BoundContributionStack` keeps declared, persistent-fixing, solve-lock, and
  temporary-fixing layers. Disabling a higher layer reveals the exact lower
  predecessor; it never relaxes to infinity.
- Origin-incomplete rows/variables reject with `InvalidUniverse` before a
  reducer or backend mutation.
- Generated construct and solve-overlay origins are retained as explicit
  semantic categories; overlay-origin restrictions are rejected until an
  explicit overlay analysis scope is implemented.

## Verification

```text
cargo fmt --all -- --check                                  PASS
cargo clippy -p roml --test restriction_universe -- -D warnings PASS
cargo test -p roml --test restriction_universe              PASS (3 tests)
```

The full phase test matrix and bound-stack property/mutation corpus remain
later Slice 2 follow-up work before phase completion.
