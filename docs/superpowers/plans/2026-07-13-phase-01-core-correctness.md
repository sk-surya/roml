# Phase 01 Plan — Canonical Model Correctness

> Use TDD for every semantic change. Do not touch native adapter internals except to keep the workspace compiling behind temporary gates.

**Goal:** make the solver-independent model mathematically canonical, validated, and invariant-checked.

**Requirements:** R2.*, R8.3.

## Task 1.1 — Freeze current behavior with characterization tests

**Files:** `tests/model_characterization.rs`, existing model/expr/value tests.

Add tests for:

- variable/constraint/objective/parameter creation and removal;
- active/inactive transitions;
- duplicate terms targeting the same cell;
- parameter propagation through nested expressions;
- stale IDs and cross-entity invalid references;
- invalid bounds, NaN, infinity, and division by zero;
- objective constants and switching;
- deletion cascades and emitted changes.

Mark tests that demonstrate known incorrect behavior with a clear ignored/P0 annotation only until the corresponding task fixes them. Do not assert the bug as desired behavior.

## Task 1.2 — Introduce typed validation

**Files:** create `src/model/validation.rs`; revise `src/model/mod.rs`, `variable.rs`, `constraint.rs`, `parameter.rs`, `coefficient.rs`, `objective.rs`, `value_expr/mod.rs`, error definitions.

1. Write failing tests for each invalid input.
2. Define validation helpers/types:
   - finite scalar/coefficient/parameter;
   - bound scalar allowing signed infinity but not NaN;
   - non-negative finite tolerance;
   - expression evaluation error.
3. Change public mutators to return typed `Result` where failure is possible.
4. Make stale/unknown IDs explicit errors; never silently ignore.
5. Define binary-variable bound policy and test it.
6. Ensure transactions validate final staged state, not only intermediate order.

**Acceptance:** no invalid numeric state enters canonical storage; every rejected mutation leaves model/revision unchanged.

## Task 1.3 — Canonicalize coefficient cells

**Files:** refactor `src/model/coefficient.rs`, `src/expr/linear.rs`, `src/value_expr/mod.rs`, model methods and tests.

1. Add `CellKey { target, variable }` internally.
2. Replace multiple coefficient records per cell with one canonical cell record, or add a compilation layer that guarantees uniqueness before storage.
3. Implement deterministic symbolic addition/normalization sufficient to combine constants and parameter expressions.
4. Recompute one dependency set for the aggregate expression.
5. On update/removal, update reverse parameter indices atomically.
6. Define zero normalization; test cancellation and reintroduction.
7. Preserve public ergonomic expression operators while removing last-write-wins projection.

**Mandatory red tests:**

```text
p*x + q*x -> (p+q)*x
2*x + p*x + 3*x -> (5+p)*x
p*x - p*x -> no structural cell (or documented zero cell)
remove contributor/update p -> mathematically correct remaining value
```

## Task 1.4 — Close referential integrity and deletion semantics

**Files:** model stores and model orchestration tests.

1. Define deletion policy for variables with coefficient references, constraints/objectives with cells, and parameters with dependents.
2. Prefer atomic cascade with complete emitted operations or typed refusal; do not leave dangling secondary indices.
3. Add `Model::validate_invariants()` behind test/debug visibility.
4. Validate:
   - every cell references live entities;
   - every reverse index matches forward storage;
   - exactly one active objective;
   - cached values equal expression evaluation;
   - store counts/IDs are coherent;
   - no duplicate cell keys.
5. Call invariant validation after every operation in generated tests.

## Task 1.5 — Correct constants/defaults/objective semantics

**Files:** `src/model/mod.rs`, objective/solution tests.

1. Add a failing regression test for `ModelConstants::default` recursion.
2. Remove the inherent recursive method or invoke trait default explicitly.
3. Define objective constant storage and reported objective value contract.
4. Define solver-neutral infinity/tolerance policy; avoid hiding backend-specific limits in core constants.
5. Test objective activation with zero variables and empty objectives.

## Task 1.6 — Property and model-based tests

**Files:** `proptest`-based tests under `tests/property_model.rs` or an equivalent deterministic generator; add dependency as dev-only.

Generate legal and illegal mutation sequences with fixed replay seeds. After each legal step:

- run invariants;
- compare cached cell values with independent expression evaluation;
- compare secondary index queries with a simple reference map;
- verify failed operations are atomic;
- verify remove/re-add IDs cannot alias stale handles.

Persist minimal failing seeds as regression tests.

## Task 1.7 — API and migration cleanup

1. Narrow public visibility of internal stores/data where feasible.
2. Add accessors/views for legitimate inspection.
3. Update README/examples/docs and CHANGELOG for result-returning mutators and coefficient normalization.
4. Run `cargo-semver-checks` as informational because no release baseline exists.

## Verification

```bash
cargo fmt --all -- --check
cargo clippy -p roml --all-targets -- -D warnings
cargo test -p roml --all-targets
RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps
cargo test -p roml --test property_model -- --nocapture
```

Run property tests with an elevated case count in CI nightly/advisory.

**Phase gate:** all P1 characterization defects are replaced by correct tests; no duplicate canonical cells; invalid mutations are typed and atomic; invariant/property suites pass.