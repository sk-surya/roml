# Phase 29 Contract Characterization

## Provenance

- Phase: P29 — IIS/conflict analysis and origin-aware reports
- Base: `d26728ee6fc800d906a6133fdca70b64658e0ae2`
- Worktree: `phase-roml-P29-iis-conflicts`
- Scope: LP infeasibility contract only; no native implementation in this slice
- Binding owner direction: fetched `origin/docs/p29-iis-design-packet`, packet from PR #38

## Routing gate

The root GSD projection now identifies M3 as active. The canonical phase directory is
`.planning/phases/29-iis-conflict-analysis/`; the previously empty duplicate
`29-iis-conflict-reports` directory was removed after confirming it contained no files.
`node /home/skrishnan/.codex/gsd-core/bin/gsd-tools.cjs query init.phase-op 29`
resolves one directory with eight plans and the accepted context.

## Untouched baseline

Run in the clean Phase 29 worktree before implementation:

```text
cargo fmt --all -- --check                         PASS
cargo check -p roml --all-targets                   PASS
cargo clippy -p roml --all-targets -- -D warnings  PASS
cargo test -p roml --all-targets                   PASS (243 library tests plus integration suites)
RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps PASS
cargo package --list -p roml                      PASS
```

The baseline was run at the exact base above. Backend-native checks were not part of
the solver-free core baseline.

## RED characterization

`tests/iis_contract.rs` was first added against the owner-approved contract. Before
the declarations existed, compilation failed for the missing `InfeasibilityMode`,
`InfeasibilityPlan`, `InfeasibilityScope`, `FeasibilityOutcome`, and
`classify_feasibility` symbols. This established that the test was exercising the new
contract rather than existing behavior.

## Contract now frozen

- `SolverSession::analyze_infeasibility(&Model, &InfeasibilityPlan)` is the additive
  orchestration seam; there is no `Model` IIS method.
- `InfeasibilityMode` has distinct `Auto`, `RomlPortable`, `NativeOnly`, and
  `NativeThenRoml` variants.
- `OriginalLp` and explicit `LpRelaxation` are distinct scopes.
- `classify_feasibility` maps only `Optimal`/`Feasible` to proven feasible and
  `Infeasible` to proven infeasible. Ambiguous, unbounded, limit, interrupted,
  numerical, error, and unknown statuses map to `Unknown`.
- `InfeasibilityError::Unsupported` is the interim behavior and does not mutate the
  model or persistent backend session.
- The report contract carries mandatory `CompilationId`, lineage, instance, revision,
  provider, scope, completion, guarantee, semantic members, native evidence,
  statistics, and warnings. No minimum-cardinality guarantee exists.

## Focused verification

```text
cargo fmt --all                         PASS
cargo clippy -p roml --test iis_contract -- -D warnings PASS
cargo test -p roml --test iis_contract PASS (4 tests)
cargo test -p roml --test solver_facade PASS (19 tests)
```

Native qualification, semantic atom mapping, isolated oracle behavior, reducer
verification, renderers, and bundled/system HiGHS checks are intentionally deferred
to the later serial slices.
