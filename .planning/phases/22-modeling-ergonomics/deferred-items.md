# Deferred Items — Phase 22 Modeling Ergonomics

Pre-existing observations discovered during P22 execution that are **outside this
phase's scope** (not caused by P22 changes). Logged per the executor scope
boundary so they are not lost. Do not fix as part of P22.

## 1. roml-mosek and roml-xpress do not compile against the P21+ solver API

- **Where:** `roml-mosek/src/adapter.rs`, `roml-mosek/src/lib.rs`,
  `roml-xpress/src/adapter.rs`, `roml-xpress/src/lib.rs`, and their
  `tests/integration.rs`.
- **Issue:** These crates import `roml::solver::SolverAdapter` and
  `roml::solver::SolverModelExt`, which were removed when P21 replaced the
  legacy adapter protocol with the `BackendSession` / `SolverSession<B>` model
  (commit `eb98a9a`, "remove legacy solver types"). The breakage predates P22
  (present at the P21 merge `f05e83d`); P22 changed only `src/model/*` and
  `src/expr/linear.rs`.
- **Scope note:** M2's baseline matrix (`.planning/milestones/M2-public-api-ergonomics/EXECUTION.md`)
  covers `roml` and `roml-highs` only. AGENTS.md keeps mosek/xpress
  "unpublished/experimental until independently qualified."
- **Evidence:** `cargo test --workspace --all-targets` reports E0432 for these
  two crates; `cargo test -p roml` and `cargo test -p roml-highs` are fully green.
- **Owner:** A later phase (surface curation / adapter migration) should migrate
  or gate these crates.

## 2. Raw `set_variable_bounds` (advanced) does not validate bounds

- **Where:** `Model::set_variable_bounds` (src/model/mod.rs).
- **Issue:** The advanced raw mutation accepts NaN/inverted `Bounds` directly
  (only `add_variable` / `add_integer` validate). D10/API-06.1 is satisfied on
  the canonical definition surface; the raw advanced path is not retrofitted.
- **Suggestion:** P23 surface curation may route `set_variable_bounds` through
  the same validation.

## 3. Missing `Sub<VarId> for VarId` expression operator

- **Where:** src/expr/linear.rs operator impls.
- **Issue:** `x - y` (two bare variable handles) does not compile; only
  `x + y`, `2.0 * x - y`, etc. compile. Pre-existing gap, not a D8-alias
  regression (aliases are plain type aliases, so operator availability is
  unchanged from before).
- **Suggestion:** Add `impl Sub<VarId> for VarId` (mirrors `Add<VarId>` for
  ergonomics) in a future phase if desired.

## 4. `add_constraint` / `add_objective` are not atomic when the expression
references a stale variable

- **Where:** `Model::add_constraint_spec_impl`, `Model::add_objective_expr`
  (insert the row/objective before expression compilation).
- **Issue:** If an expression references a removed variable, compilation fails
  after the empty row/objective was inserted, leaving a dangling row. API-06.5
  atomicity is proven for the definition builders (Task 1); the canonical
  expression path was not retrofitted because the plan's Task 1 atomicity scope
  is definition creation and a refactor risks the P21 suites.
- **Suggestion:** Pre-validate expression variables before inserting the row in
  a future hardening pass.

## 5. `add_constraint_coefficient` / `add_objective_coefficient` accept NaN/∞ constants

- **Where:** src/model/mod.rs coefficient operations (advanced).
- **Issue:** The raw advanced coefficient mutators do not reject non-finite
  constants; the D11 trio (`set_coefficient` / `add_to_coefficient` /
  `remove_coefficient_at`) does reject them. The canonical expr path folds
  constants into validated bounds, so the ordinary surface is covered.
- **Suggestion:** Extend non-finite rejection to the raw `*_coefficient`
  mutators in a future phase.
