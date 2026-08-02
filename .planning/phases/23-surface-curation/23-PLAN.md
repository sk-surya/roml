# Phase 23 — Surface Curation, Validation, and Migration

> **For agentic workers:** replacement APIs must be green before any deprecation. Preserve advanced backend capabilities through explicit namespaces.

**Goal:** reduce the default API to intentional user concepts and provide a mechanical migration path from current pre-1.0 entry points.

**Requirements:** API-06, API-07, API-08.

## Planned file structure

Create:

- `src/advanced.rs` or `src/backend/mod.rs` — grouped extension re-exports.
- `tests/prelude_contract.rs` — intended prelude compile checks.
- `tests/compatibility_api.rs` — deprecation compatibility coverage.
- `MIGRATION.md`

Modify:

- `src/lib.rs`
- module visibility/re-exports across core
- `CHANGELOG.md`
- `MODELING_API.md` only for migration structure; final rewrite is P24.

## Task 1 — Define the minimal prelude

Target prelude:

```text
Model
Variable, Constraint, Objective, Parameter
VariableDef, ParameterDef
continuous, integer, binary, parameter
LinExpr, ConstraintSpec, ConstraintExprExt
Bounds/ConstraintBounds only if still needed by ordinary code
SolveOptions, Solution, SolveStatus, SolveError
ModelError
constraint! (optional)
```

- [ ] Add compile tests importing only `roml::prelude::*` for ordinary modeling.
- [ ] Add negative inventory assertions or public-API review checks showing protocol internals are absent.
- [ ] Implement the prelude without glob-re-exporting advanced modules.
- [ ] Commit as `refactor: curate default roml prelude`.

## Task 2 — Group advanced/backend concepts

- [ ] Re-export backend contract, revisions, snapshots, deltas, cursors, capabilities, callbacks, and raw IDs from one documented advanced namespace.
- [ ] Add a backend-author compile example.
- [ ] State stability and semver expectations in rustdoc.
- [ ] Keep implementation stores and arena internals private.
- [ ] Commit as `refactor: group backend extension API`.

## Task 3 — Make validation consistent

- [ ] Replace remaining debug-only input checks with typed errors.
- [ ] Audit every public model mutation and solve option builder for NaN, infinities, inverted bounds, stale IDs, zero/negative threads, and invalid gaps/durations.
- [ ] Add release-profile tests or run targeted tests under `cargo test --release`.
- [ ] Verify failed mutations do not alter counts, revision, journal, or backend state.
- [ ] Commit as `fix: enforce public validation in all profiles`.

## Task 4 — Deprecate duplicate entry points

After replacements pass:

- [ ] Deprecate `Model::constraint` alias in favor of `add_constraint`.
- [ ] Deprecate effectful `constrain!` in favor of `model.add_constraint(constraint!(...))` or fluent specs.
- [ ] Deprecate `set_objective!` and `objective!` where they add no retained pure-builder value according to P20 disposition.
- [ ] Deprecate raw constructor signatures replaced by definitions, with exact notes.
- [ ] Keep tests proving old supported behavior still works during the deprecation window.
- [ ] Commit each logical group separately.

## Task 5 — Write migration guide

`MIGRATION.md` must include before/after examples for:

- variable and parameter creation;
- constraints;
- objectives;
- HiGHS solve path;
- parameter update/re-solve;
- solve options;
- solution/status access;
- advanced backend sessions;
- coefficient-cell operations;
- import/prelude changes.

- [ ] Link every deprecation message to a migration section.
- [ ] Update `CHANGELOG.md` under an unreleased heading.
- [ ] Commit as `docs: add M2 API migration guide`.

## Task 6 — Public API and semver review

- [ ] Run `cargo public-api` for core and HiGHS.
- [ ] Compare to P20 inventory and classify every addition/removal.
- [ ] Run `cargo-semver-checks` against the chosen pre-1.0 baseline if configured.
- [ ] Record intentional breakage and migration coverage.
- [ ] Obtain independent API review.
- [ ] Commit evidence as `docs: record M2 public API review`.

## Verification

```bash
cargo fmt --all -- --check
cargo clippy -p roml -p roml-highs --all-targets -- -D warnings
cargo test -p roml --all-targets
cargo test -p roml-highs --all-targets
cargo test --release -p roml --test modeling_ergonomics
RUSTDOCFLAGS='-D warnings' cargo doc -p roml -p roml-highs --no-deps
cargo public-api -p roml
cargo public-api -p roml-highs
```

## Gate

P23 passes when the default surface is small and intentional, advanced authors retain a documented path, all validation is release-safe, and every breaking/deprecated change has a tested replacement and migration entry.