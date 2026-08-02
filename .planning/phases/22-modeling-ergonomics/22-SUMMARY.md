---
phase: 22-modeling-ergonomics
plan: 22
subsystem: api
tags: [rust, milp, modeling-api, entity-names, sparse-coefficients, validation, diagnostics]

# Dependency graph
requires:
  - phase: 21-solver-facade
    provides: fallible entry points, generic add_constraint(spec), semantic aliases, Model::named, add_empty_constraint primitive
provides:
  - validated variable/parameter definitions with single-side bound overrides and atomic rejection
  - name getters with typed stale-ID errors for all four entity types
  - canonical add_constraint / minimize / maximize paths proven end-to-end
  - D11 sparse cell-coordinate trio (set_coefficient / add_to_coefficient / remove_coefficient_at)
  - name-aware pprint diagnostics with stable debug-handle fallback
affects: [23-surface-curation, 24-consumer-qualification]

# Actuals (#2632) — chars/4 over the realized diff (added content only).
actuals:
  tokens: 11304
  tasks: 6
  commits: 7

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Definition builders (continuous/integer/binary/parameter) consumed by fallible entry points with single-side bound overrides"
    - "Name-as-diagnostic metadata: queryable getters, duplicates allowed, snapshot-excluded (D6)"
    - "Coordinate-based sparse cell mutation preserving the one-cell-per-(target,var) invariant"
    - "Name-aware formatting with stable debug-handle fallback"

key-files:
  created:
    - tests/modeling_ergonomics.rs
    - tests/named_entities.rs
  modified:
    - src/model/mod.rs
    - src/model/variable.rs
    - src/model/coefficient.rs
    - src/expr/linear.rs

key-decisions:
  - "Name getters return Result<Option<&str>, ModelError>: typed stale-ID errors (D10/API-06.3), Ok(None) for valid unnamed, Ok(Some) for named"
  - "Binary bounds validated within [0,1] in add_variable with a dedicated InvalidBinaryBounds error (API-06.4/06.6)"
  - "ObjectiveSpec gains a name field + .named(); the spec path honors it without complicating minimize/maximize (API-05.4)"
  - "add_empty_constraint(bounds) is the public advanced bounds-only primitive; the spec and expr paths route through an internal named primitive"
  - "D11 trio semantics: set replaces and drops prior parameter deps, add accumulates, remove_coefficient_at is idempotent on missing cells"
  - "pprint prefers names with index fallback; names are diagnostics, not unique keys"

patterns-established:
  - "Pattern: validated definition builders keep construction policy separate from handles (D7)"
  - "Pattern: advanced raw mutation is public but explicitly separated from the canonical spec path"
  - "Pattern: diagnostics render entity names and fall back to stable x[N]/p[N]/c[N]/obj[N] handles"

requirements-completed: [API-04, API-05, API-06]

# Coverage metadata (#1602) — deterministic UAT routing.
coverage:
  - id: D1
    description: "Validated variable and parameter definitions: defaults, single-side bound overrides, atomic rejection of inverted/NaN/invalid-infinity/binary-out-of-range bounds and non-finite parameters"
    requirement: API-05
    verification:
      - kind: unit
        ref: "tests/modeling_ergonomics.rs#continuous_default_is_non_negative_unbounded"
        status: pass
      - kind: unit
        ref: "tests/modeling_ergonomics.rs#bounds_and_single_side_builders_override_defaults"
        status: pass
      - kind: unit
        ref: "tests/modeling_ergonomics.rs#rejects_binary_bounds_outside_unit_interval"
        status: pass
      - kind: unit
        ref: "tests/modeling_ergonomics.rs#failed_creation_is_atomic"
        status: pass
      - kind: unit
        ref: "tests/modeling_ergonomics.rs#parameter_definition_defaults_and_validation"
        status: pass
    human_judgment: false
  - id: D2
    description: "Semantic aliases work unchanged with expression operators; name getters for all entity types return typed stale-ID errors, Ok(None) for unnamed, and survive clone/mutation; duplicate names are diagnostics"
    requirement: API-05
    verification:
      - kind: unit
        ref: "tests/named_entities.rs#aliases_support_expression_operators_unchanged"
        status: pass
      - kind: unit
        ref: "tests/named_entities.rs#named_creation_for_all_entity_types"
        status: pass
      - kind: unit
        ref: "tests/named_entities.rs#name_getters_reject_stale_ids_with_typed_errors"
        status: pass
      - kind: unit
        ref: "tests/named_entities.rs#names_survive_clone_and_ordinary_mutation"
        status: pass
    human_judgment: false
  - id: D3
    description: "Canonical add_constraint path: .le/.eq/.ge/.between, expression-constant adjustment, parameter coefficients, named specs, raw-bounds bridge, and public advanced add_empty_constraint"
    requirement: API-04
    verification:
      - kind: unit
        ref: "tests/modeling_ergonomics.rs#canonical_add_constraint_le"
        status: pass
      - kind: unit
        ref: "tests/modeling_ergonomics.rs#constraint_expression_constant_adjusts_bounds"
        status: pass
      - kind: unit
        ref: "tests/modeling_ergonomics.rs#constraint_parameter_coefficients_are_canonical"
        status: pass
      - kind: unit
        ref: "tests/modeling_ergonomics.rs#advanced_add_empty_constraint_creates_bounds_only_row"
        status: pass
    human_judgment: false
  - id: D4
    description: "Canonical objective path: minimize/maximize activate exactly once, constants retained, later objective replaces active, parameter coefficients, named variants (spec path + advanced add_objective_named) with switching"
    requirement: API-04
    verification:
      - kind: unit
        ref: "tests/modeling_ergonomics.rs#minimize_activates_exactly_once"
        status: pass
      - kind: unit
        ref: "tests/modeling_ergonomics.rs#subsequent_objective_replaces_active"
        status: pass
      - kind: unit
        ref: "tests/modeling_ergonomics.rs#objective_parameter_coefficients_are_canonical"
        status: pass
      - kind: unit
        ref: "tests/modeling_ergonomics.rs#advanced_named_objective_creation_and_switching"
        status: pass
    human_judgment: false
  - id: D5
    description: "D11 sparse cell-coordinate trio: set replaces, add_to accumulates keeping one canonical cell, remove_coefficient_at removes by coordinate (idempotent); stale entities and non-finite values rejected"
    requirement: API-04
    verification:
      - kind: unit
        ref: "tests/modeling_ergonomics.rs#set_coefficient_replaces_cell_value"
        status: pass
      - kind: unit
        ref: "tests/modeling_ergonomics.rs#add_to_coefficient_accumulates_and_keeps_one_cell"
        status: pass
      - kind: unit
        ref: "tests/modeling_ergonomics.rs#remove_coefficient_at_removes_by_coordinate"
        status: pass
      - kind: unit
        ref: "tests/modeling_ergonomics.rs#sparse_ops_reject_stale_entities_and_non_finite_values"
        status: pass
    human_judgment: false
  - id: D6
    description: "Named diagnostics: pprint prefers names for variables/parameters/constraints/objectives and expression terms with debug-handle fallback, never panicking on removed entities"
    requirement: API-05
    verification:
      - kind: unit
        ref: "tests/named_entities.rs#pprint_prefers_names"
        status: pass
      - kind: unit
        ref: "tests/named_entities.rs#pprint_never_panics_on_removed_entities"
        status: pass
    human_judgment: false

# Metrics
duration: 46min
completed: 2026-08-02
status: complete
---

# Phase 22: Modeling Ergonomics Summary

**Validated definition builders, typed name getters, canonical constraint/objective paths, the D11 sparse cell trio, and name-aware pprint diagnostics over the P21 modeling surface**

## Performance

- **Duration:** ~46 min
- **Started:** 2026-08-02T19:20:00Z (approx)
- **Completed:** 2026-08-02T20:06:09Z
- **Tasks:** 6
- **Files modified:** 6 (2 created, 4 modified)

## Accomplishments

- Validated `VariableDef`/`ParameterDef` builders with `lower_bound`/`upper_bound`
  single-side overrides and a dedicated `InvalidBinaryBounds` error; failed
  creation is atomic (counts, changelog, revision unchanged).
- Four name getters (`variable_name`, `parameter_name`, `constraint_name`,
  `objective_name`) returning `Result<Option<&str>, ModelError>` — typed
  stale-ID errors per D10/API-06.3; names survive clone and ordinary mutation.
- Canonical `add_constraint` / `minimize` / `maximize` paths proven across the
  full matrix (`.le/.eq/.ge/.between`, constant folding, parameter coefficients,
  named variants); `add_empty_constraint(bounds)` is now a public advanced
  bounds-only primitive.
- D11 sparse trio (`set_coefficient`, `add_to_coefficient`,
  `remove_coefficient_at`) with exact replace/add/remove-by-coordinate
  semantics; canonical cell count stays one for repeated additions.
- `pprint` prefers entity names and renders named variables in reconstructed
  expressions, falling back to stable `x[N]`/`p[N]`/`c[N]`/`obj[N]` handles;
  never panics on removed/stale entities.

## Task Commits

Each task was committed atomically with the plan's specified message:

1. **Task 1: Add validated definitions** - `53715a8` (feat)
2. **Task 2: Expose semantic aliases and names** - `c30173d` (feat)
3. **Task 3: Canonical constraint path** - `9501a26` (feat)
4. **Task 4: Canonical objective path** - `c74cc21` (feat)
5. **Task 5: Clarify sparse coefficient mutation semantics** - `6053bc7` (feat)
6. **Task 6: Improve named model diagnostics** - `5e84994` (feat)

**Plan metadata:** `docs(22)` summary commit (this file).

## Files Created/Modified

- `tests/modeling_ergonomics.rs` - Golden-path behavioral tests (definitions,
  constraint/objective matrices, D11 sparse trio).
- `tests/named_entities.rs` - Alias, name, lifecycle, and diagnostic formatting tests.
- `src/model/mod.rs` - `InvalidBinaryBounds`; binary validation in `add_variable`;
  four name getters; `add_objective_internal`/`add_objective_named`;
  public `add_empty_constraint` + internal named primitive; D11 trio;
  name-aware `pprint`, `entity_label`, `var_label`, `format_lin_expr(model, expr)`.
- `src/model/variable.rs` - `VariableDef::lower_bound` / `upper_bound`.
- `src/model/coefficient.rs` - `CoefficientIndex::set_expr` (expression replace
  maintaining the parameter dependency index).
- `src/expr/linear.rs` - `ObjectiveSpec` name field + `.named()`; `add_objective_spec`
  honors names; `add_objective_expr`/`add_constraint_expr` route through the
  primitives.

## Decisions Made

- Name getters use the accepted consistency rule: `Result<Option<&str>,
  ModelError>` — typed stale-ID errors for removed IDs, `Ok(None)` for valid
  unnamed entities (D10/API-06.3).
- Binary bounds are validated to lie within `[0,1]` at `add_variable` with a
  distinct `InvalidBinaryBounds` error that names the violated invariant
  (API-06.4/06.6); subsets of `[0,1]` are accepted.
- `ObjectiveSpec` gained a name field and `.named()` builder honored by the spec
  path (`set_objective` / `add_objective_spec`), so named objectives do not
  complicate the ordinary `minimize`/`maximize` path (API-05.4).
- `add_empty_constraint(bounds)` is the public advanced bounds-only row
  primitive (per the P20 disposition); the spec path routes through an internal
  `add_empty_constraint_internal(bounds, name)`.
- D11 semantics: `set_coefficient` replaces (dropping prior parameter deps via
  `CoefficientIndex::set_expr`), `add_to_coefficient` algebraically accumulates
  (reusing the canonical-cell combine), `remove_coefficient_at` removes by
  coordinate and is idempotent on a missing cell.
- `pprint` prefers names in headers and expression terms with stable index
  fallback; names remain diagnostics, not unique keys (D6 non-goal preserved:
  snapshots are numeric reconstruction paths and exclude names).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Binary-bounds validation was absent**
- **Found during:** Task 1 (validated definitions)
- **Issue:** `add_variable(binary().bounds(0.0, 5.0))` was accepted even though
  API-06.4 requires binary bounds within `[0,1]`. The plan explicitly called for
  checking and adding this validation.
- **Fix:** Added `ModelError::InvalidBinaryBounds` and a `[0,1]` check in
  `add_variable`; bounds strictly outside `[0,1]` are rejected, subsets accepted.
- **Files modified:** src/model/mod.rs, tests/modeling_ergonomics.rs
- **Verification:** `rejects_binary_bounds_outside_unit_interval` passes.
- **Committed in:** `53715a8` (Task 1 commit)

**2. [Rule 2 - Missing Critical] `lower_bound`/`upper_bound` builders absent**
- **Found during:** Task 1 (plan interface lists them; gap analysis claimed present)
- **Issue:** The plan interface specifies `VariableDef::lower_bound` and
  `VariableDef::upper_bound`, but only `bounds`/`named` existed.
- **Fix:** Added single-side bound overrides preserving the other default side.
- **Files modified:** src/model/variable.rs, tests/modeling_ergonomics.rs
- **Verification:** `bounds_and_single_side_builders_override_defaults` passes.
- **Committed in:** `53715a8` (Task 1 commit)

**3. [Rule 3 - Blocking] `ObjectiveSpec` lacked name support for Task 2's
"named creation for all entity types" test**
- **Found during:** Task 2 (semantic aliases and names)
- **Issue:** Named objective creation required a name-bearing spec; `ObjectiveSpec`
  had no name field and `add_objective_spec` discarded it.
- **Fix:** Added the name field + `.named()`, routed `add_objective_spec` /
  `add_objective_expr` through a new `add_objective_internal(sense, name)`
  primitive, and added `add_objective_named` (advanced) in Task 4.
- **Files modified:** src/expr/linear.rs, src/model/mod.rs
- **Verification:** `named_creation_for_all_entity_types`,
  `named_objective_via_spec_path`, `advanced_named_objective_creation_and_switching` pass.
- **Committed in:** `c30173d` (Task 2), `c74cc21` (Task 4)

**4. [Rule 3 - Blocking] `add_empty_constraint` was `pub(crate)` and took a
two-arg `(bounds, name)` signature**
- **Found during:** Task 3 (canonical constraint path)
- **Issue:** The plan/disposition require a public advanced bounds-only
  primitive `add_empty_constraint(bounds)`.
- **Fix:** Made `add_empty_constraint(bounds)` public (advanced) and split the
  internal named form into `add_empty_constraint_internal(bounds, name)`;
  `add_constraint_spec_impl` and `add_constraint_expr` route through it.
- **Files modified:** src/model/mod.rs, src/expr/linear.rs
- **Verification:** `advanced_add_empty_constraint_creates_bounds_only_row` passes.
- **Committed in:** `9501a26` (Task 3 commit)

**5. [Rule 1 - Bug] Test asserted a wrong expected value for `upper_bound`**
- **Found during:** Task 1 (RED phase)
- **Issue:** `continuous().upper_bound(7.0)` preserves the `0.0` lower default,
  yielding `[0.0, 7.0]`; the first test draft asserted `[0.0, +inf)`.
- **Fix:** Corrected the test expectation to `Bounds::new(0.0, 7.0)`. The
  implementation was already correct.
- **Files modified:** tests/modeling_ergonomics.rs
- **Verification:** `bounds_and_single_side_builders_override_defaults` passes.
- **Committed in:** `53715a8` (Task 1 commit)

---

**Total deviations:** 5 auto-fixed (2 missing critical, 2 blocking, 1 bug)
**Impact on plan:** All fixes were required by the plan's own interface list or
API-06 requirements; no scope creep. No Rule 4 (architectural) deviations.

## Issues Encountered

- **Transient clippy/compile race:** one `cargo clippy --all-targets -D warnings`
  run raced with an in-flight `cargo fmt` on a test file and reported an
  `unused_variables` error; a clean re-run passed. Also an unused `con` binding
  in `pprint_prefers_names` (renamed `_con`).
- **`VarId - VarId` operator missing:** `(x - y)` does not compile (pre-existing
  gap; only `x + y`, `2.0 * x - y`, etc. work). The Task 3 equality test uses
  `2.0 * x - y`. Logged as a deferred item.
- **Reconstructed-expression term order is nondeterministic:** `constraint_expression`
  / `objective_expression` iterate a `HashSet`-backed index, so term order in
  `pprint` is not insertion order. The `pprint_prefers_names` test asserts on
  line contents rather than exact order.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- **P23 (surface curation):** ready to deprecate `constrain`/`constraint`,
  effectful macros, and builder wrappers now that the canonical replacements
  compile with coverage (`add_constraint`, `minimize`, `maximize`,
  definition-builder forms). The D11 trio and `add_empty_constraint` are
  advanced-surface anchors. Deferred items 2-5 in `deferred-items.md` are
  candidate P23 validation/ergonomics follow-ups.
- **P24 (consumer qualification):** representative LP, MILP, parameterized,
  named, and sparse examples now have one coherent method-first style.
- **Concern:** `roml-mosek` / `roml-xpress` remain un-compilable against the
  P21+ solver API (pre-existing; logged as deferred item 1). They are out of M2
  scope and experimental per AGENTS.md.

---
*Phase: 22-modeling-ergonomics*
*Completed: 2026-08-02*

## Review Cycle (PR #22, 2026-08-02)

Independent review returned 2 blocking findings, both resolved:

1. **`set_coefficient` compared cached evaluated values instead of expression semantics** — replacing a parameter-dependent cell (`p * x`, p = 2) with the constant 2 hit the cached-value early-return, leaving the parameter dependency in place; a later `p` update silently changed the "replaced" coefficient. Fix: the no-op check now only fires for a prior **constant** expression equal to the requested value; any parameter-dependent cell is replaced (dependency dropped). Test: `set_coefficient_replaces_parameter_dependent_expression` (replacement survives a parameter update).
2. **API-06.5 claimed satisfied while `add_constraint(spec)`/`add_objective` were non-atomic** — the row/objective was inserted before expression compilation, so a stale variable/parameter left a dangling row + changelog event. Fix (in P22, not deferred): `Model::validate_expression_entities` pre-validates every referenced variable, parameter, and the expression constant's finiteness BEFORE any insertion, applied to `add_constraint_spec_impl`, `add_objective_spec`, `add_objective_expr`, and `add_constraint_expr`. Six atomicity tests cover the matrix (stale variable/parameter × constraint/objective + `add_constraint_expr`). Deferred item 4 marked resolved.

Post-fix matrix: roml **528 passed / 0 failed**; roml-highs 89/0; clippy/rustdoc/fmt/doctests clean.

## Self-Check: PASSED

- `22-SUMMARY.md` exists at `.planning/phases/22-modeling-ergonomics/22-SUMMARY.md`.
- Task commits verified: `53715a8`, `c30173d`, `9501a26`, `c74cc21`, `6053bc7`, `5e84994`.
- Test files exist: `tests/modeling_ergonomics.rs`, `tests/named_entities.rs`.
- Verification matrix green: fmt clean; clippy `-D warnings` clean for `roml` and
  `roml-highs`; `cargo test -p roml --all-targets` 528 passed / 0 failed
  (522 at phase end + 6 review-round tests);
  `cargo test -p roml-highs --all-targets` 89 passed / 0 failed;
  `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps` clean
  (and for `roml-highs`). Pre-existing `roml-mosek`/`roml-xpress` E0432 failures
  are logged in `deferred-items.md` (out of M2 scope).
