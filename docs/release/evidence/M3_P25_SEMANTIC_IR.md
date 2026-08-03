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
| 3 | `c19c608` | `feat(model): add linear function-in-set semantics` |
| 4 | `79c3d9a` | `feat(model): add canonical construct lifecycle` |

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

### Task 4 — canonical construct lifecycle

- `src/construct/mod.rs` — `pub type Construct = ConstructId`; `#[non_exhaustive] ConstructKind` (P25 carries only the `#[doc(hidden)] Fixture(FixturePayload)` variant; design §7 declares the `Indicator`/`Reification`/`MinMax`/`AbsoluteValue`/`Boolean`/`Cardinality`/`BinaryProduct`/`PiecewiseLinear`/`SoftConstraint` extension surface, landing with the per-construct modules in P30/P32/P33); `ConstructEntry { id, kind, active }`; `FormulationPreference { Auto, Portable, NativeRequired }`; the generation-safe `ConstructStore` (checked atomic ids, removal invalidates ids, stale ids rejected with typed errors).
- `Model` — `add_construct_fixture`, `construct`, `set_construct_active`, `remove_construct`, `num_constructs`, `construct_parameter_dependencies`; the construct arena is cloned with the model (same ids + activity).
- `src/model/changelog.rs` — self-contained `Change::ConstructAdded/ConstructRemoved/ConstructActivityChanged`.
- `src/delta.rs` — `ModelOp::AddConstruct/RemoveConstruct/SetConstructActive`; `DeltaBatch.constructs` reconstructed from ops.
- `src/snapshot.rs` — `ModelSnapshot.constructs` populated by `Model::take_snapshot` from the arena.
- `EntityRef::Construct` is now usable (construct metadata round-trips; invariant checks reference live constructs).
- `ModelError::ConstructNotFound` and `ModelError::IdentityOverflow` added (typed errors).

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

### Task 4 — canonical construct lifecycle

`tests/semantic_ir.rs` construct section (7 tests, total 15 in the file). RED recorded first: the initial run failed to compile (missing `roml::construct` module, `add_construct_fixture`/`construct`/`set_construct_active`/`remove_construct`/`num_constructs`, `ModelSnapshot.constructs`/`DeltaBatch.constructs`, `ModelError::ConstructNotFound`), confirming the tests target not-yet-implemented behavior.

```text
running 15 tests
test construct_metadata_usable_via_entity_ref ... ok
test construct_activity_toggling_reflected_in_snapshot ... ok
test construct_clone_preserves_ids_and_activity ... ok
test construct_snapshot_and_delta_round_trip ... ok
test construct_add_returns_stable_id_and_payload_round_trips ... ok
test construct_remove_invalidates_id_and_stale_ids_rejected ... ok
test construct_store_survives_rebuild ... ok
test delta_carries_semantic_function_entries ... ok
test model_invariants_verify_legacy_fields_against_semantic_view ... ok
test snapshot_carries_semantic_function_entries ... ok
test function_constraint_is_constructible_from_spec ... ok
test ge_eq_between_convert_to_canonical_sets ... ok
test into_scalar_function_converts_lin_expr ... ok
test le_converts_to_linear_function_and_less_equal_set ... ok
test ordinary_builder_round_trips_through_coefficient_index ... ok

test result: ok. 15 passed; 0 failed; 0 ignored
```

Every construct fixture survives add / clone (same ids + activity) / snapshot / delta / activity toggle / remove (stale id rejected with typed `ConstructNotFound`) / rebuild (fresh model restored from snapshot carries equal construct content), SM-01.3/SM-01.6.

## Native/backend evidence

P25 introduces no native/backend surface. Construct `ModelOp` variants are explicit no-op arms in the ReferenceBackend and HiGHS projection (SM-01.6: no backend index/handle enters canonical state; M3 v1 does not compile constructs).

## Failure/recovery evidence

- Stale construct ids are rejected with typed `ModelError::ConstructNotFound` (never silently ignored, D10): `construct`, `set_construct_active`, and `remove_construct` on a removed id all fail; `remove_construct` of an already-removed id fails rather than no-op.
- Metadata on a removed construct is caught by `validate_invariants` (dead-construct reference violation).
- `cargo test -p roml --all-targets` and `cargo test -p roml-highs --all-targets` pass after the `ModelSnapshot`/`DeltaBatch` field additions (the ~34 roml-highs test snapshot literals were updated mechanically; no behavior changed).

## Public API and packaging

Post-P25 `cargo public-api -p roml` capture: `docs/release/evidence/M3_P25_public_api_roml_final.txt` (12302 lines; baseline 10737). Diff: **+1571 added / −6 removed** (the six "removed" lines are `Model::clone`/`Model::default` moving from derived to manual impls — the methods remain public with identical signatures, confirmed by `tests/public_api_compile.rs` and the full suite; SM-15.1 M2 surface preserved).

New public types (root + module paths): `ModelLineageId`, `ModelInstanceId`, `ConstructId`, `IdentityOverflow`, `EntityMetadata`, `EntityRef`, `ModelSource`, `ScalarFunction`, `ScalarSet`, `FunctionConstraint`, `FunctionEntry`, `IntoScalarFunction`, `Construct`, `ConstructKind`, `ConstructEntry`, `FormulationPreference`. New methods on `Model`: `lineage`, `instance`, `set_metadata`, `metadata`, `remove_metadata`, `constraint_function`, `add_construct_fixture`, `construct`, `set_construct_active`, `remove_construct`, `num_constructs`, `construct_parameter_dependencies`. `ModelSnapshot`/`DeltaBatch` gain `functions` and `constructs` fields. `ModelError` gains `ConstructNotFound` and `IdentityOverflow`.

Package list unchanged in composition: `cargo package --list -p roml` still 70 files (`src/construct/**`, `src/function/**`, `src/identity.rs`, `src/metadata.rs` added under the existing `include` filter).

## Deviations and decisions

### Auto-fixed issues (Rules 1–3)

1. **[Rule 3] `SolveMetadata` call sites** (Task 2) — `src/solver/facade.rs` and `tests/status_mapping.rs` constructed `SolveMetadata` without the new lineage/instance fields; fixed with `..SolveMetadata::default()`.
2. **[Rule 3] `ModelSnapshot`/`DeltaBatch` field additions ripple** (Tasks 3, 4) — adding `functions`/`constructs` fields broke struct literals in `src/solver/conformance.rs` and ~34 `roml-highs/tests/{behavior_tests,solve_observables_tests,contract_tests}.rs` snapshot fixtures; all updated with `functions: vec![]`/`constructs: vec![]`. No roml-highs behavior changed.
3. **[Rule 3] `ModelOp` construct variants** (Task 4) — adding `AddConstruct`/`RemoveConstruct`/`SetConstructActive` broke the exhaustive matches in `src/solver/reference.rs` and `roml-highs/src/projection.rs`; added explicit no-op arms (SM-01.6).

### Design interpretations

1. **`ConstructKind` in P25 carries only the fixture variant.** Design §7 declares nine payload variants (`Indicator`, ..., `SoftConstraint`) whose payload types are defined by the per-construct modules (P30/P32/P33). P25 declares the `#[non_exhaustive]` boundary and the design §7 extension surface in rustdoc but stores only the private `Fixture(FixturePayload)` variant — honoring the P25 scope note ("must not pre-implement their formulations") and the acceptance criterion (`#[non_exhaustive] ConstructKind`).
2. **`LinExpr`/`Term`/`TermCoeff` gained `PartialEq`** (Task 3) — required by `ScalarFunction`'s `PartialEq` per design §6; additive, no behavior change.
3. **Delta `functions`/`constructs` are reconstructed views** (not stored second authorities) — the coefficient index and construct arena remain the single authorities (SM-01.1); `DeltaBatch::new` and the snapshot projection derive the semantic entries deterministically, and every transitional legacy field is invariant-checked.

## Reviewer findings

Independent review at the P25 phase boundary (gsd-code-review, standard depth, 24 files; two-pass protocol per EXECUTION.md).

- **P0/P1 (critical):** none remain. Two criticals found and fixed with TDD:
  - CR-01 delta reconstructed `set` used pre-adjustment `AddConstraint` bounds, diverging from the canonical folded bounds on the constant-folding path (`(x + 3).le(5)`); fixed in `5c748e1` (last same-batch `SetConstraintBounds` wins) with tests `delta_set_reflects_bounds_folded_from_expression_constant` (+ge/between variants).
  - CR-02 real solve path never bound the solved model's lineage/instance into `SolveMetadata` (`..SolveMetadata::default()` allocated fresh unrelated ids per solve); fixed in `37e67f0` (ids threaded from `SolverSession::solve_with`) with test `real_solve_binds_model_lineage_and_instance_into_metadata`.
- **P2 (warnings):** all 6 fixed with TDD in `5c748e1`, `a845968`, `839361f`, `4335e2d`: WR-01 deterministic term order (sorted by var in `constraint_expression`, snapshot, delta — test `constraint_expression_term_order_matches_snapshot`); WR-02 tautological `debug_assert_eq` removed, real cross-check extended to function equality in `take_snapshot`; WR-03 saturating `fetch_update` (no wrap/reuse after overflow — seam test `family_allocate_saturates_on_overflow_without_reissuing`); WR-04 panic boundary documented; WR-05 `set_metadata` liveness validation + cascade removal in `remove_variable`/`remove_constraint`/`remove_objective`; WR-06 `remove_construct` cascades metadata (test `construct_remove_cascades_metadata_and_invariants_pass`).
- **Info:** all 3 fixed — IN-01 dead `affects_solver` removed; IN-02 testable overflow seam; IN-03 rebuild test strengthened (by-id full-entry assertions).
- **Dispositions:** all findings accepted and fixed; no deferrals. Re-verification pass confirmed every finding RESOLVED against the current code; `cargo test -p roml --all-targets` 594 pass, clippy `-D warnings` clean.

## Residual risks

- `cargo test -p roml --all-targets` reports **585 passed; 0 failed; 0 ignored**; `roml-highs` **100 passed; 0 failed; 0 ignored**.
- Construct `ModelOp` ops are no-ops in the ReferenceBackend/HiGHS adapters — constructs are canonical entities only; compilation to backend rows is deferred to the compiler phase (P26+). SM-01.6 holds: no backend index/handle/Big-M/overlay in canonical state.
- `cargo package --list -p roml` captures the include-filtered 70-file list; the `roml-mosek`/`roml-xpress` adapters remain known-broken against the current facade (pre-existing, out of scope — matching M2 convention).
- The public-API capture files are normalized (`$REPO` token); raw `cargo public-api` output may differ in absolute paths.

## Gate result

<!-- Filled at the P25 phase boundary after Task 4 (independent review). -->
