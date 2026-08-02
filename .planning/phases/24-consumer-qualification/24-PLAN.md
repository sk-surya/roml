# Phase 24 — Documentation and Consumer Qualification

> **For agentic workers:** this is the integration gate. Do not introduce new API concepts; fix inconsistencies in the accepted P21–P23 design.

**Goal:** prove the public API is understandable and usable from packed crates on clean consumers.

**Requirements:** API-09, API-10, and final verification of API-01 through API-08.

## Files

Modify:

- `README.md`
- `MODELING_API.md`
- `examples/simple_lp.rs`
- `examples/parameter_update.rs`
- `CHANGELOG.md`
- crate/module rustdoc

Create:

- `examples/simple_mip.rs`
- `examples/solve_options.rs`
- `examples/sparse_build.rs`
- `docs/release/evidence/M2_PUBLIC_API.md`
- temporary fresh-consumer projects outside the repository during verification

## Task 1 — Rewrite README around the golden path

README order:

1. one-sentence differentiator;
2. install commands;
3. complete HiGHS solve example;
4. incremental parameter update example;
5. explanation of automatic synchronization;
6. core/backend crate topology;
7. advanced API link;
8. status/support statement.

- [ ] Make the primary code block compile as a doctest or extracted test fixture.
- [ ] Do not show advanced protocol types above the advanced section.
- [ ] Keep pre-1.0 claims precise.
- [ ] Commit as `docs: rewrite README for user-facing API`.

## Task 2 — Rewrite modeling guide

Required chapters:

- model and entity definitions;
- expressions and constraints;
- objectives;
- names and diagnostics;
- solving with HiGHS;
- solve options and effective configuration;
- solution/status semantics;
- parameters and repeated solves;
- sparse construction;
- advanced sessions and synchronization;
- migration notes and common errors.

- [ ] Ensure every snippet is compiled or linked to a compiled example.
- [ ] State implicit commit/rebuild behavior and one-retry limit.
- [ ] Explain mathematical termination versus operational errors.
- [ ] Commit as `docs: rewrite modeling API guide`.

## Task 3 — Replace and expand examples

- [ ] `simple_lp.rs`: build and solve with `Highs`.
- [ ] `simple_mip.rs`: binary/integer definitions and MIP status.
- [ ] `parameter_update.rs`: two solves with one façade and changed objective/value.
- [ ] `solve_options.rs`: duration, gap, output, and effective configuration.
- [ ] `sparse_build.rs`: explicit cell semantics without `CoeffId` in ordinary code.
- [ ] Add CI execution where native availability permits; compile all examples everywhere supported.
- [ ] Commit as `docs: add complete public API examples`.

## Task 4 — Rustdoc closure

- [ ] Enable/retain missing-doc enforcement according to repository policy.
- [ ] Document every public constructor, mutation, error, status, metadata field, and advanced namespace.
- [ ] Add `# Errors` and `# Panics` sections; normal invalid input must not panic.
- [ ] Run doctests and deny rustdoc warnings.
- [ ] Commit as `docs: complete M2 rustdoc`.

## Task 5 — Packed fresh consumers

Create temporary projects outside the workspace:

### Core-only consumer

- depend on the generated `roml` package;
- build a named model;
- run without a C/C++ compiler or solver library requirement where environment allows verification.

### Default HiGHS consumer

- depend on generated `roml` and `roml-highs` packages;
- compile and run the README quickstart;
- verify repeated parameter solve.

### System HiGHS consumer

- test documented feature selection and discovery on supported Linux/macOS environments;
- verify actionable failure diagnostics when the system library is absent.

- [ ] Record exact archive paths, normalized manifests, commands, and outputs.
- [ ] Ensure no workspace path dependency masks missing versions/files.
- [ ] Commit only evidence and any required package fixes, not temporary consumer directories.

## Task 6 — Full qualification and review

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --all-features
cargo test --doc --workspace
cargo package --list -p roml
cargo package --list -p roml-highs
cargo package -p roml --locked
cargo package -p roml-highs --locked
cargo deny check
```

- [ ] Run public API and semver reports.
- [ ] Run all fresh consumers.
- [ ] Obtain independent API, protocol, and documentation review.
- [ ] Resolve every blocking finding.
- [ ] Write `M2_PUBLIC_API.md` with the traceability table and residual risks.
- [ ] Update M2 `STATE.md` with final SHA and closed IDs.
- [ ] Commit as `docs: record M2 public API qualification`.

## Gate

P24 and M2 pass only when every requirement has evidence, docs compile, packed consumers succeed, existing correctness tests remain green, and independent review has no unresolved blocker.