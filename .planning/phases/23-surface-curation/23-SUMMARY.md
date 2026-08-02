---
phase: 23-surface-curation
plan: 23
subsystem: api
tags: [rust, public-api, prelude, deprecation, validation, migration, pre-1.0]

# Dependency graph
requires:
  - phase: 22-modeling-ergonomics
    provides: canonical add_constraint/minimize/maximize, D7 definitions, D11 sparse trio, semantic aliases, name getters
  - phase: 21-solver-facade
    provides: SolverSession<B>, SolveOptions, Solution/SolveStatus/SolveError, roml_highs::Highs
provides:
  - curated default prelude (API-07.1/07.2) with compile_fail negative inventory
  - roml::advanced backend-extension namespace with stability docs (API-07.3/07.4)
  - release-safe validation for all public model mutations (API-06)
  - pre-1.0 deprecations for effectful macros and raw constructors with tested compatibility window (API-08.3)
  - MIGRATION.md + CHANGELOG.md entries (API-08.2)
  - cargo public-api evidence and P20-baseline comparison (API-07.5)
affects: [24-consumer-qualification, MODELING_API.md rewrite, packaging hygiene]

# Actuals (#2632) — pairs with the plan's `estimate` to calibrate future estimates.
actuals:
  tokens: 229918   # chars/4 over the realized diff (includes generated public-api evidence)
  tasks: 6
  commits: 10      # task commits; final docs commit is separate

# Tech tracking
tech-stack:
  added: [cargo-public-api 0.52.0 (evidence tool)]
  patterns:
    - compile_fail doctest as prelude negative-inventory guard
    - roml::advanced grouping module with explicit stability/semver docs
    - #[allow(deprecated)] compatibility-window testing pattern

key-files:
  created:
    - src/advanced.rs
    - tests/prelude_contract.rs
    - tests/advanced_surface.rs
    - tests/validation_consistency.rs
    - tests/compatibility_api.rs
    - MIGRATION.md
    - docs/release/M2_P23_PUBLIC_API_REVIEW.md
    - docs/release/evidence/M2_P23_public_api_roml.txt
    - docs/release/evidence/M2_P23_public_api_roml_highs.txt
  modified:
    - src/lib.rs
    - src/model/mod.rs
    - src/expr/linear.rs
    - src/id/mod.rs
    - src/id/arena.rs
    - tests/modeling_ergonomics.rs, tests/definition_builders.rs, tests/solver_facade.rs,
      tests/named_entities.rs, tests/public_api_compile.rs, tests/end_to_end_equivalence.rs,
      tests/model_characterization.rs, tests/macro_api.rs, tests/objective_constant_delta.rs,
      tests/delta_content_verification.rs, roml-highs/tests/repeated_session_baseline.rs,
      roml-highs/tests/contract_tests.rs, examples/simple_lp.rs, CHANGELOG.md

key-decisions:
  - "Protocol/backend types leave the default prelude but remain public at the root and under roml::advanced (D9 'remain public but recede'); root exports kept so the P20/P21/P22 suites stay green."
  - "add_parameter(f64) cannot carry #[deprecated]: it shares the generic add_parameter<P: Into<ParameterDef>> with the canonical parameter(value) form (Rust forbids deprecated trait impls; the P20 Into bridge preserves the call shape). Documented in MIGRATION.md instead of a code deprecation."
  - "Incidental test usages of deprecated APIs migrated to canonical; characterization/compatibility suites keep exercising the deprecated surface under #[allow(deprecated)] (API-08.3)."
  - "VarId - VarId operator added (deferred item 3) mirroring x + y."
  - "NaN term coefficients are validated in validate_expression_entities before row insertion, because LinExpr::simplify silently drops NaN terms (atomicity preserved)."

patterns-established:
  - "Deprecation-window testing: a dedicated tests/compatibility_api.rs pins every deprecated entry point; incidental usages migrate to canonical."
  - "Prelude negative inventory enforced by a compile_fail doctest on the prelude module docs."
  - "Advanced surface proven sufficient by a backend-author compile test (tests/advanced_surface.rs)."

requirements-completed: [API-06, API-07, API-08]

coverage:
  - id: D1
    description: "Curated default prelude limited to common model/expression/definition/solver/solution/error types; protocol types absent"
    requirement: API-07
    verification:
      - kind: integration
        ref: "tests/prelude_contract.rs#prelude_supports_full_ordinary_workflow"
        status: pass
      - kind: integration
        ref: "tests/prelude_contract.rs#prelude_covers_solve_and_solution_vocabulary"
        status: pass
      - kind: other
        ref: "src/lib.rs prelude compile_fail doctest (API-07.2 negative inventory)"
        status: pass
      - kind: other
        ref: "docs/release/M2_P23_PUBLIC_API_REVIEW.md (public-api grep: 0 matches for the 11 types under roml::prelude)"
        status: pass
    human_judgment: false
  - id: D2
    description: "roml::advanced namespace groups backend contract/revisions/snapshots/deltas/cursors/capabilities/callbacks/raw IDs with stability docs; arena internals private"
    requirement: API-07
    verification:
      - kind: integration
        ref: "tests/advanced_surface.rs#backend_contract_implementable_from_advanced"
        status: pass
      - kind: integration
        ref: "tests/advanced_surface.rs#advanced_exposes_protocol_and_id_vocabulary"
        status: pass
      - kind: other
        ref: "cargo public-api output: IdArena absent, 60 advanced symbols present"
        status: pass
    human_judgment: false
  - id: D3
    description: "Release-safe validation: set_variable_bounds/set_constraint_bounds/set_semicontinuous/raw *_coefficient reject NaN/inverted/non-finite; failed mutations are atomic; VarId-VarId operator"
    requirement: API-06
    verification:
      - kind: integration
        ref: "tests/validation_consistency.rs (13 tests, debug + release)"
        status: pass
      - kind: other
        ref: "cargo test --release -p roml --test validation_consistency (13 passed)"
        status: pass
      - kind: other
        ref: "cargo test --release -p roml --test modeling_ergonomics (38 passed)"
        status: pass
    human_judgment: false
  - id: D4
    description: "Deprecations with actionable replacement notes; deprecated APIs remain tested for the pre-1.0 window"
    requirement: API-08
    verification:
      - kind: integration
        ref: "tests/compatibility_api.rs (5 tests)"
        status: pass
      - kind: integration
        ref: "tests/public_api_compile.rs (current-surface guard, allows deprecated)"
        status: pass
    human_judgment: false
  - id: D5
    description: "MIGRATION.md with before/after for all plan-listed areas + CHANGELOG.md unreleased entries; every deprecation note links to a migration section"
    requirement: API-08
    verification:
      - kind: other
        ref: "MIGRATION.md (10 sections) + CHANGELOG.md Unreleased Deprecated/Changed"
        status: pass
    human_judgment: false
  - id: D6
    description: "cargo public-api evidence stored and compared to the P20 baseline; semver-checks skip recorded"
    requirement: API-07
    verification:
      - kind: other
        ref: "docs/release/M2_P23_PUBLIC_API_REVIEW.md (removals classified, all intentional pre-1.0 breaks with migration coverage)"
        status: pass
    human_judgment: false
  - id: D7
    description: "Independent API coherence review (API-10.6) of protocol preservation, error semantics, prelude/advanced split"
    verification: []
    human_judgment: true
    rationale: "Requires a separate human reviewer; not performable by the executing agent. Flagged in docs/release/M2_P23_PUBLIC_API_REVIEW.md as recommended before merge."

# Metrics
duration: 84min
completed: 2026-08-02
status: complete
---

# Phase 23: Surface Curation Summary

**Curated default prelude, `roml::advanced` backend namespace, release-safe validation, and pre-1.0 deprecations for effectful macros and raw constructors, all with MIGRATION.md + CHANGELOG entries and a public-API review against the P20 baseline**

## Performance

- **Duration:** 84 min
- **Started:** 2026-08-02T19:30:00Z
- **Completed:** 2026-08-02T20:57:13Z
- **Tasks:** 6
- **Commits:** 10 (task commits; final docs commit separate)

## Accomplishments

- **Curated prelude** — `roml::prelude` now exports only common model/expression/definition/solver/solution/error types; the 11 API-07.2 protocol types are absent, guarded by a `compile_fail` doctest negative inventory and verified in the public-api output.
- **`roml::advanced` namespace** — the backend contract, revisions, snapshots, deltas, cursors, capabilities, callbacks, and raw IDs are grouped under one documented namespace with stability/semver docs; `IdArena` made crate-private; a backend-author compile test proves the contract is implementable from `roml::advanced` alone.
- **Release-safe validation** — `set_variable_bounds`, `set_constraint_bounds`, `set_semicontinuous`, and the raw `*_coefficient` mutators now reject NaN/inverted/non-finite inputs with typed errors in all build profiles; NaN term coefficients are rejected before row insertion (they would otherwise be silently dropped by `simplify`); failed mutations leave counts/changelog/revision unchanged. `VarId - VarId` operator added.
- **Deprecations with tested compatibility** — `Model::constrain`/`constraint`, effectful `constrain!`/`set_objective!`, `Model::set_objective`, and `add_var`/`add_binary`/`add_integer` are deprecated with actionable notes; `tests/compatibility_api.rs` proves the whole deprecated surface still runs; pure `constraint!`/`objective!` builders remain.
- **Migration + changelog** — `MIGRATION.md` with before/after for all ten plan-listed areas; every deprecation note links to its section; `CHANGELOG.md` Unreleased Deprecated/Changed entries.
- **Public API review** — `cargo public-api 0.52.0` inventories stored and diffed against P20; all removals classified as intentional pre-1.0 breaks with migration coverage; `cargo-semver-checks` skip recorded (not installed / no pre-1.0 baseline).

## Task Commits

1. **Task 1: Define the minimal prelude** — `48397f5` (refactor: curate default roml prelude)
2. **Task 2: Group advanced/backend concepts** — `d7869e6` (refactor: group backend extension API)
3. **Task 3: Make validation consistent** — `8c1144f` (fix: enforce public validation in all profiles)
4. **Task 4: Deprecate duplicate entry points** (committed per logical group)
   - `6d814a0` (refactor: deprecate Model::constrain and Model::constraint aliases)
   - `68d3860` (refactor: deprecate effectful constrain! and set_objective! macros)
   - `1e463b7` (refactor: deprecate Model::set_objective convenience)
   - `36b6ce6` (refactor: deprecate raw constructor wrappers)
5. **Task 5: Write migration guide** — `308951a` (docs: add M2 API migration guide)
6. **Task 6: Public API and semver review** — `8985a10` (docs: record M2 public API review)
- **Follow-up fix** — `cf916f9` (fix: satisfy rustdoc -D warnings on prelude doc link)

## Files Created/Modified

- `src/advanced.rs` — new `roml::advanced` namespace (stability + semver docs, grouped re-exports).
- `src/lib.rs` — curated prelude + `compile_fail` negative-inventory doctest; deprecated `constrain!`/`set_objective!` macros.
- `src/model/mod.rs` — validation on `set_variable_bounds`/`set_constraint_bounds`/`set_semicontinuous`/raw `*_coefficient`; NaN term-coefficient rejection in `validate_expression_entities`; deprecated `add_var`/`add_binary`/`add_integer`; `validate_constraint_bounds` helper.
- `src/expr/linear.rs` — `VarId - VarId` operator; NaN constraint-bound rejection in `add_constraint_expr`; deprecated `constrain`/`constraint`/`set_objective`.
- `src/id/mod.rs`, `src/id/arena.rs` — `IdArena` crate-private (API-07.4).
- `tests/prelude_contract.rs`, `tests/advanced_surface.rs`, `tests/validation_consistency.rs`, `tests/compatibility_api.rs` — new P23 suites.
- `MIGRATION.md` — full before/after migration guide.
- `docs/release/M2_P23_PUBLIC_API_REVIEW.md` + `docs/release/evidence/M2_P23_public_api_{roml,roml_highs}.txt` — public API evidence.
- `CHANGELOG.md` — Unreleased Deprecated/Changed entries.
- Test/example files — migrated incidental deprecated usages to canonical; added `#[allow(deprecated)]` where suites intentionally exercise the deprecated surface.

## Decisions Made

- Protocol/backend types "remain public but recede" (D9): removed from the prelude, grouped under `roml::advanced`, root exports kept so every existing suite stays green.
- `add_parameter(f64)` documented rather than `#[deprecated]`-marked: it shares the generic `add_parameter<P: Into<ParameterDef>>` with the canonical `parameter(value)` form, and `#[deprecated]` is not allowed on trait impls; the P20 `Into` bridge preserves the call shape.
- Deprecation-window strategy: a dedicated compatibility suite keeps the old surface tested; incidental usages across the P20/P21/P22 suites migrate to canonical; characterization guards keep exercising the old surface under `#[allow(deprecated)]`.
- NaN term coefficients validated pre-insertion (not in `simplify`) to preserve atomicity — `simplify` silently drops NaN terms.
- `cargo-semver-checks` skipped per plan ("if configured"): not installed and no pre-1.0 baseline; reason recorded in evidence rather than claimed passing.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] NaN term coefficients were silently dropped by `simplify`**
- **Found during:** Task 3 (validation)
- **Issue:** `LinExpr::simplify` filters terms with `|v| v.abs() >= EPSILON`; a NaN coefficient fails that comparison and was silently discarded, while `validate_expression_entities` only checked the expression constant — so `add_constraint_expr` with a NaN term produced an empty row instead of an error.
- **Fix:** `validate_expression_entities` now evaluates and rejects non-finite term coefficients before any row insertion (atomic).
- **Files modified:** `src/model/mod.rs`
- **Verification:** `tests/validation_consistency.rs#expr_with_nan_coefficient_is_rejected_atomically`
- **Committed in:** `8c1144f` (Task 3)

**2. [Rule 1 - Bug] `#[deprecated]` on a trait impl is not allowed**
- **Found during:** Task 4 (deprecations)
- **Issue:** Deprecating the `add_parameter(f64)` path via `#[deprecated]` on `impl From<f64> for ParameterDef` fails to compile — the attribute is rejected on trait impl blocks by current rustc.
- **Fix:** Documented the f64-form preference in MIGRATION.md instead; `add_var`/`add_binary`/`add_integer` (distinct inherent methods) carry real `#[deprecated]`.
- **Files modified:** `src/model/mod.rs`, `MIGRATION.md`, `CHANGELOG.md`
- **Verification:** `cargo check` clean; MIGRATION.md section "Variable and parameter creation".
- **Committed in:** `36b6ce6` (Task 4 Group D)

**3. [Rule 1 - Bug] Rustdoc `-D warnings` failed on a redundant explicit link**
- **Found during:** verification matrix (Task 6)
- **Issue:** The prelude doc link `[advanced](crate::advanced)` tripped `rustdoc::redundant_explicit_links`.
- **Fix:** Use the inferred link ``[`advanced`]``.
- **Files modified:** `src/lib.rs`
- **Verification:** `RUSTDOCFLAGS='-D warnings' cargo doc` clean.
- **Committed in:** `cf916f9`

**4. [Documented change] `nan_constraint_bounds_accepted` characterization updated**
- **Found during:** Task 3 (validation)
- **Issue:** `tests/model_characterization.rs` pinned the pre-P23 defect that NaN constraint bounds were accepted; Task 3 fixes exactly this.
- **Fix:** Updated to `nan_constraint_bounds_rejected` asserting `NonFiniteValue("constraint bound")` and no mutation.
- **Files modified:** `tests/model_characterization.rs`
- **Committed in:** `8c1144f` (Task 3)

**5. [Deferred, recorded] `Model::deltas_since(rev)` test-only helper not removed**
- **Found during:** Task 6 (public API review)
- **Issue:** The P20 disposition classifies `deltas_since` as "internal exposure to remove", but the P21/P22 suites use it heavily.
- **Fix:** Deferred to P24 so the suites stay green; classified in the review doc as a deferred removal rather than silently removed.
- **Files modified:** `docs/release/M2_P23_PUBLIC_API_REVIEW.md`
- **Committed in:** `8985a10` (Task 6)

---

**Total deviations:** 5 (3 auto-fixed bugs, 1 documented characterization update, 1 deferred removal)
**Impact on plan:** All auto-fixes were necessary for correctness/atomicity and the migration story. No scope creep; `deltas_since` deferral is explicitly recorded.

## Issues Encountered

- `#[deprecated]` cannot be applied to trait impls (rustc rejects it), which forced the documented-not-annotated approach for `add_parameter(f64)`.
- Deprecating `Model::constrain`/`Model::set_objective` required `#[allow(deprecated)]` at their internal call sites (`constraint`, `minimize`, `maximize`) and across the P20/P21/P22 suites; handled with file-level `#![allow(deprecated)]` on characterization/compatibility suites and per-function allows elsewhere.
- The P23 public-api output (10 737 lines) is ~45% larger than P20 because `roml::advanced` re-export metadata duplicates entries; this is expected and classified in the review doc.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- **Ready for P24 (consumer qualification):** the default surface is small and intentional; advanced authors have a documented path (`roml::advanced`); all breaking/deprecated changes have tested replacements and `MIGRATION.md` entries; the public API review and evidence are in place.
- **Recommended before P23 merge:** independent API coherence review (API-10.6), flagged in `docs/release/M2_P23_PUBLIC_API_REVIEW.md`.
- **Carried forward for P24:** `MODELING_API.md` final rewrite (P23 only staged the migration structure); `Model::deltas_since` removal; packaging hygiene for `roml` (include filter); the mosek/xpress compile gap (out of M2 scope).

---
*Phase: 23-surface-curation*
*Completed: 2026-08-02*

## Self-Check: PASSED

- All 9 created files verified present on disk.
- All 10 task commits verified in `git log`.
- Verification matrix: fmt clean; clippy `-D warnings` clean; roml 551/0; roml-highs 89/0; release modeling_ergonomics 38/0; release validation_consistency 13/0; rustdoc `-D warnings` clean; `cargo public-api` for both crates.
