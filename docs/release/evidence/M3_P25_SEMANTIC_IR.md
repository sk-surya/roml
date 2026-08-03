# P25 Evidence — Canonical Semantic IR, Identities, and Metadata

**Phase:** 25-semantic-ir-foundation
**Plan:** `25-PLAN.md`
**Requirements:** SM-01.1, SM-01.2, SM-01.3, SM-01.4, SM-01.5, SM-01.6, SM-02.1, SM-02.2, SM-02.3, SM-02.5 (foundations), SM-02.7, SM-15.1 (foundations)
**Branch:** `phase-roml-P25-semantic-ir-foundation`

This document records the P25 baseline, implementation evidence, public API diff, and reviewer dispositions per `EXECUTION.md` § "Evidence file structure".

## Scope and requirements

P25 establishes canonical semantic state before adding workflows: opaque identity (lineage/instance/construct), entity metadata, linear function-in-set constraints, and the generation-safe construct arena. It is a serial chain (Task 1 → Task 2 → Task 3 → Task 4) with no intra-phase parallelism.

## Baseline and environment

| Item | Value |
|---|---|
| Base commit (`git rev-parse HEAD`) | `7b124ad164ebd42259c2541cf8484826d6eecfba` |
| Branch | `phase-roml-P25-semantic-ir-foundation` |
| `rustc --version` | `rustc 1.97.1 (8bab26f4f 2026-07-14)` |
| `cargo --version` | `cargo 1.97.1 (c980f4866 2026-06-30)` |
| `rustc -vV` host | `aarch64-apple-darwin` (LLVM 22.1.6) |
| OS | macOS `Darwin forge 25.4.0` Darwin Kernel 25.4.0 arm64 |
| `cargo public-api --version` | `cargo-public-api 0.52.0` |
| HiGHS build | bundled via `highs-sys 1.15.0` (cmake); no system HiGHS |

All commands below ran on the platform above with the toolchain above at the base commit, on the untouched tree (before any P25 source modification).

### Untouched baseline matrix — `roml` (Task 1)

| Command | Exit | Result |
|---|---|---|
| `cargo fmt --all -- --check` | 0 | formatting clean |
| `cargo check -p roml --all-targets` | 0 | clean |
| `cargo clippy -p roml --all-targets -- -D warnings` | 0 | clean, warnings denied |
| `cargo test -p roml --all-targets` | 0 | **553 passed; 0 failed; 2 ignored** |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps` | 0 | docs generated, no warnings |
| `cargo package --list -p roml` | 0 | 70 files (include-filtered; see `M3_P25_package_roml.txt`) |

### Untouched baseline matrix — `roml-highs` (Task 1)

| Command | Exit | Result |
|---|---|---|
| `cargo check -p roml-highs --all-targets` | 0 | clean |
| `cargo clippy -p roml-highs --all-targets -- -D warnings` | 0 | clean, warnings denied |
| `cargo test -p roml-highs --all-targets` | 0 | **100 passed; 0 failed; 0 ignored** |
| `RUSTDOCFLAGS='-D warnings' cargo doc -p roml-highs --no-deps` | 0 | docs generated, no warnings |
| `cargo package --list -p roml-highs` | 0 | 32 files (see `M3_P25_package_roml_highs.txt`) |

### Public API / package capture (Task 1)

- `docs/release/evidence/M3_P25_public_api_roml.txt` — `cargo public-api -p roml` output, normalized per M2 convention (repository absolute paths replaced with the `$REPO` token). `10737` lines.
- `docs/release/evidence/M3_P25_public_api_roml_highs.txt` — `cargo public-api -p roml-highs` output, normalized. `106` lines.
- `docs/release/evidence/M3_P25_package_roml.txt` — `cargo package --list -p roml` (70 files).
- `docs/release/evidence/M3_P25_package_roml_highs.txt` — `cargo package --list -p roml-highs` (32 files).

## Commit trail

| Task | Commit | Message |
|---|---|---|
| 1 | `8ebbf8a` | `test(m3): capture semantic modeling baseline` |
| 2 | `217aa0c` | `feat(model): add lineage instance and metadata` |
| 3 | `(pending — filled at commit)` | `feat(model): add linear function-in-set semantics` |

## Public interfaces

### Task 3 — function-in-set canonical constraints

- `src/function/scalar.rs` — `#[non_exhaustive] ScalarFunction { Linear(LinExpr) }` (design §6, SM-01.2).
- `src/function/set.rs` — `#[non_exhaustive] ScalarSet { LessEqual, GreaterEqual, EqualTo, Interval { lower, upper } }`.
- `src/function/mod.rs` — `FunctionConstraint { function, set }`, `IntoScalarFunction` trait (`impl for LinExpr`), `FunctionEntry { constraint, function, set }`.
- `src/model/constraint.rs` — `From<ConstraintBounds> for ScalarSet`; `ConstraintSpec::into_function_constraint()` / `From<ConstraintSpec> for FunctionConstraint` (the canonical `.le/.ge/.eq/.between` conversion path).
- `src/expr/linear.rs` — added `PartialEq` to `TermCoeff`, `Term`, `LinExpr` (required by `ScalarFunction`'s `PartialEq` per design §6; additive, no behavior change).
- `Model::constraint_function(con)` — deterministic reconstruction from the coefficient index (single coefficient authority, SM-01.1).
- `ModelSnapshot` and `DeltaBatch` — new `functions: Vec<FunctionEntry>` carried as a derived semantic view; reconstructed from the legacy constraint/cell fields with invariant checks (SM-01.4).

### Task 2 — identity, metadata, and Model lineage/instance

- `src/identity.rs` — `ModelLineageId(u64)`, `ModelInstanceId(u64)`, `ConstructId(u64)` (opaque; `Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord`), allocated by checked per-family atomic counters with zero reserved; overflow returns typed `IdentityOverflow` (design §4).
- `src/metadata.rs` — `ModelSource { module, file, line, external_key }`, `EntityMetadata { description, group, tags, source }` (each `Clone, Debug, Default, PartialEq, Eq`), `EntityRef { Variable, Constraint, Objective, Parameter, Construct }` (`Clone, Copy, Debug, PartialEq, Eq, Hash`; the `Construct` variant becomes usable when the Task 4 arena lands) (design §5).
- `Model` — manual `Default` (allocates fresh lineage + instance) and `Clone` (preserves lineage, allocates new instance) replacing `#[derive(Default, Clone)]`; public `lineage()`, `instance()`, `set_metadata()`, `metadata()`, `remove_metadata()`; metadata store keyed by `EntityRef`, canonical but non-solver-affecting (revision does not advance).
- `SolveMetadata` — adds `model_lineage: ModelLineageId` and `model_instance: ModelInstanceId`; `Default` allocates fresh ids (SM-02.7).

## Focused verification

### Task 1 — characterization on the untouched tree

`tests/m3_baseline_characterization.rs` (6 tests) covers: fluent linear modeling (`Model::new` + `add_variable` + `add_constraint` + `maximize`), deterministic snapshot round-trip, parameter update (`set_parameter`), objective constant propagation, solution metadata, and one-rebuild-retry behavior.

```text
running 6 tests
test solution_metadata_round_trip ... ok
test objective_constant_propagation ... ok
test parameter_update_propagates_to_coefficients ... ok
test deterministic_snapshot_round_trip ... ok
test fluent_linear_modeling ... ok
test one_rebuild_retry_recovers_post_update_state ... ok

test result: ok. 6 passed; 0 failed; 0 ignored
```

Command: `cargo test -p roml --test m3_baseline_characterization -- --nocapture` (exit 0).

This is characterization, not a red/green feature test — it passes on the untouched tree and must keep passing as P25 extends canonical state (SM-01.5 / SM-15.1).

### Task 2 — lineage, instance identity, and metadata

`tests/lineage_metadata.rs` (5 tests). RED recorded first: the initial run failed to compile (missing `ModelLineageId`/`EntityRef`/`EntityMetadata` types and `Model::lineage`/`Model::instance`/metadata accessors), confirming the test targets not-yet-implemented behavior.

```text
running 5 tests
test independent_models_never_share_lineage_or_instance ... ok
test clone_preserves_lineage_but_allocates_new_instance ... ok
test solve_metadata_records_every_state_id ... ok
test lineage_and_instance_ids_are_unique_across_many_models ... ok
test metadata_round_trips_per_entity ... ok

test result: ok. 5 passed; 0 failed; 0 ignored
```

Command: `cargo test -p roml --test lineage_metadata` (exit 0). Full suite `cargo test -p roml --all-targets` (exit 0) and `cargo clippy -p roml --all-targets -- -D warnings` (exit 0) both pass.

- SM-02.1: independent models never share lineage; clones preserve lineage.
- SM-02.7: every live model has a distinct instance; clone allocates a new instance while preserving lineage.
- SM-02.3: metadata (description/group/tags/source) round-trips per entity; metadata changes do not advance the revision.
- SM-02.2 foundation: lineage is the reuse-compatibility identity.

### Task 3 — function-in-set canonical constraints

`tests/semantic_ir.rs` (8 tests). RED recorded first: the initial run failed to compile (missing `ScalarFunction`/`ScalarSet`/`FunctionConstraint`/`IntoScalarFunction`, missing `into_function_constraint`/`into_scalar_function`/`constraint_function`, missing `functions` fields), confirming the test targets not-yet-implemented behavior.

```text
running 8 tests
test ge_eq_between_convert_to_canonical_sets ... ok
test function_constraint_is_constructible_from_spec ... ok
test le_converts_to_linear_function_and_less_equal_set ... ok
test into_scalar_function_converts_lin_expr ... ok
test ordinary_builder_round_trips_through_coefficient_index ... ok
test delta_carries_semantic_function_entries ... ok
test model_invariants_verify_legacy_fields_against_semantic_view ... ok
test snapshot_carries_semantic_function_entries ... ok

test result: ok. 8 passed; 0 failed; 0 ignored
```

Commands (all exit 0):
- `cargo test -p roml --test semantic_ir`
- `cargo test -p roml --test m3_baseline_characterization` (SM-01.5 preserved)
- `cargo test -p roml --all-targets` (578 passed; 0 failed; 0 warnings)
- `cargo clippy -p roml --all-targets -- -D warnings`
- `cargo check -p roml-highs --all-targets` / `cargo test -p roml-highs --all-targets` (100 passed) — M2 backend surface stays green (SM-15.1)

The `ModelSnapshot`/`DeltaBatch` `functions` field addition required a mechanical `functions: vec![]` update to 34 `ModelSnapshot` struct literals in `roml-highs/tests/{behavior_tests,solve_observables_tests,contract_tests}.rs` (Rule 3: directly caused by the Task 3 field addition; no roml-highs behavior changed).

- SM-01.1: linear function-in-set constraints stored canonically; the coefficient index stays the single authority; `constraint_function` reconstructs deterministically.
- SM-01.2: `ScalarFunction`/`ScalarSet` are `#[non_exhaustive]`; M3 implements linear only.
- SM-01.4: snapshots and deltas carry reconstructed semantic function/set entries; every transitional legacy field is invariant-checked.
- SM-01.5: ordinary `LinExpr` and builder APIs remain the canonical linear path (characterization still green).

## Native/backend evidence

<!-- P25 touches no native surface; recorded here when applicable. -->

## Failure/recovery evidence

<!-- Filled per task. -->

## Public API and packaging

<!-- Filled at the phase boundary after Task 4. -->

## Deviations and decisions

<!-- Filled per task. -->

## Reviewer findings

<!-- Filled at the P25 phase boundary after Task 4 (independent review). -->

## Residual risks

<!-- Filled per task. -->

## Gate result

<!-- Filled at the P25 phase boundary. -->
