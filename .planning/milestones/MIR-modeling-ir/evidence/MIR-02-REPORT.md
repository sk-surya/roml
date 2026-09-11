# MIR-02 Report — Parametric Packed Construction and Block-Native Propagation

**Phase:** MIR-02
**Requirements:** IR-08 … IR-17
**Head:** `phase-mir-tranche1` (`4ed2332`, `047eee3`)
**Date:** 2026-09-11

## What landed

- `src/bulk.rs` — `StridedMap` (L2 strided ordinal map), `ParamDepLayout`
  and `ParamDepBlockWitness` witnesses.
- `src/model/coefficient.rs` — `StoredParamDepBlock` storage;
  `append_param_run_with_deps` (pre-mutation witness validation, no per-cell
  `param_positions`); `propagate_packed_span` (O(covered cells), no reverse
  index/overlay/`ValueExpr` work, skips dead/shadowed); `overlay_ids_for_param`;
  block-aware `for_param`; dependency blocks included in the invariant audit.
- `src/model/mod.rs` — `set_linear_objective_param_bulk_with_layout`,
  `add_linear_rows_param_bulk`, `canonicalize_param_row`,
  `set_parameters_bulk`, `apply_parameter_block`, `propagate_overlay_only`,
  `parameter_dependent_count`, `ModelError::NotPackable` /
  `InvalidParamDepLayout`.
- `src/model/transaction.rs` — block-retaining pending updates.
- `src/delta.rs` / `src/model/changelog.rs` — `ParameterValueChange`,
  `CoefficientPatch`, `ParametricRowBlock`;
  `Change::BulkParameterValues` / `BulkCoefficientPatch` /
  `BulkParametricRows`; `ModelOp::SetParametersBulk` /
  `SetCoefficientPatchBatch` / `AddParametricRows`; semantic function
  reconstruction covers parametric rows.
- Adapters — compiler expands packed patches/rows; reference backend applies
  packed ops; MOSEK/Xpress apply member/cell-wise and route packed block
  deltas through the per-change path.

## Requirement evidence

| ID | Evidence |
|---|---|
| IR-08 | `tests/mir02_parametric_rows.rs`: `param_rows_merge_same_param_duplicates`, `param_rows_distinct_params_not_packable_and_atomic` (typed `NotPackable`, zero constraints/cells/journal). Objective duplicate merge covered by `duplicate_same_param_scales_merge_into_one_cell`. |
| IR-09 | `set_linear_objective_param_bulk_with_layout` retrofits the packed objective without changing `set_linear_objective_param_bulk`; `layout_stores_blocks_without_param_positions`. |
| IR-10 | `forged_layout_is_rejected_atomically` (wrong scale, out-of-range cell offset) — typed `InvalidParamDepLayout`, no objective/cells. |
| IR-11 | `layout_stores_blocks_without_param_positions` (0 positions, 2 blocks); `block_dependencies_are_complete_for_every_param` (2 dependents/param); `block_model_passes_invariant_audit`. |
| IR-12 | `bulk_update_queues_rolls_back_and_commits`, `bulk_update_rejects_non_finite_atomically`. |
| IR-13 | `bulk_reprice_emits_one_parameter_change_and_one_patch_batch`: `param_position_lookups=0`, `overlay_lookups=0`, `value_expr_evals=0`, `coefficient_patch_batches=1`. |
| IR-14 | Same test asserts the delta is exactly one `SetParametersBulk` plus one `SetCoefficientPatchBatch`; `packed_delta_replays_without_the_live_model` applies the retained delta to a fresh `ReferenceBackend` and matches a snapshot rebuild. |
| IR-15 | `scalar_update_on_block_param_matches_general_path`. |
| IR-16 | `shadowed_cell_is_skipped_and_stays_correct` (snapshot equality with the general oracle). |
| IR-17 | `fresh_block_appends_after_prior_revision` (4 blocks after a second eligible objective; shared span still reprices). |

## Exact commands and results

```text
cargo fmt --all -- --check                                        clean
cargo clippy -p roml -p roml-highs -p roml-python --all-targets \
  --features roml-highs/bundled -- -D warnings                    clean
RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps            clean
cargo nextest run -p roml -p roml-highs --features roml-highs/bundled
  → 1530 passed, 4 skipped
python -m pytest python/tests --ignore=python/tests/typing
  → 148 passed, 1 skipped
scripts/check-quality-policy.sh origin/main                       pass
```

## Residual risk / not verified

- MIR-02 uses hand-constructed direct/core witnesses. The production
  sink-aware eligibility proof (`try_param_block_layout`) is MIR-03, so the
  high-level BESS formulation still routes through per-cell
  `param_positions` until MIR-03 calls the qualified objective path.
- `roml-mosek` / `roml-xpress` cannot be type-checked locally (proprietary
  SDK build scripts); their new arms are `rustfmt`-parsed. Backend CI with
  the SDKs must confirm.
- Row-block eligibility and the mixed constant+parametric row seam (IR-22)
  are MIR-03; MIR-02 rows use the per-cell reverse index.
- `overlay_lookups` on the scalar path counts all `for_param` entries
  (including packed positions); the eligible bulk path uses an overlay-only
  counter and reports 0. The MIR-00 baseline counter is unchanged.

## Contract MIR-03 may consume

`set_linear_objective_param_bulk_with_layout(..., &ParamDepLayout)` accepts a
sink-aware witness; core validates it post-canonicalization into a
`StoredParamDepBlock`. `set_parameters_bulk(span, values)` + `commit()` yields
one packed parameter-value change and one packed coefficient-patch batch
whose patches are self-contained.

---

# MIR-02 Remediation — owner review 2026-09-11

PR #58 was rejected on review. The following P1/P2 findings were remediated on
the same branch (`phase-mir-tranche1`); tranche 1 remains **unaccepted** and
awaits independent re-review. MIR-03 was not started.

## P1 — bulk update preserves the non-eligible packed fallback

`apply_parameter_block` now also propagates cells retained in `param_positions`
via `CoefficientIndex::propagate_packed_positions_span`, merging their changes
into the same packed coefficient-patch batch. Fully eligible families still
perform zero `param_positions` lookups.

RED→GREEN: `tests/mir02_remediation.rs::bulk_update_propagates_non_eligible_packed_positions`.

## P1 — dependency-layout ownership proof

`CoefficientIndex::validate_param_dep_blocks` and
`Model::validate_objective_dep_layout` now require every claimed canonical
position to resolve to the matching cell and be owned **exactly once**.
A cell covered twice (across witnesses or twice within one witness) is a typed
atomic `InvalidParamDepLayout`. Partial layouts track `param_positions` only for
uncovered cells; `param_positions_cells` reports the true uncovered count.

RED→GREEN: `empty_layout_keeps_every_cell_on_the_position_fallback`,
`partial_layout_uses_blocks_for_covered_and_positions_for_uncovered`,
`overlapping_witnesses_are_rejected_atomically`, plus in-crate
`duplicate_cell_positions_in_one_witness_are_rejected` and
`malformed_witness_map_metadata_is_rejected`.

## P1 — packed batching through Backend IR and HiGHS

`BackendOp::SetObjectiveCosts { objective, costs }` is introduced;
`CompilationSession` groups objective patches by compiled objective into one
packed op (constraint patches still expand per cell). HiGHS applies it with one
`Highs_changeColsCostByRange` (contiguous compiled columns) or
`Highs_changeColsCostBySet`, updating `obj_costs` equivalently to the scalar
path. An O(n²) per-patch `retain`+`sort` in the compiled-objective-coefficient
bookkeeping was replaced with one bulk update per objective.

Structural evidence: `tests/mir02_backend_batching.rs` asserts one
`SetObjectiveCosts` and zero `SetObjectiveCoefficient`/`SetLinearCoefficient`
ops for a direct eligible reprice; `roml-highs/tests/mir02_bess_batching.rs`
asserts one bulk native call (0.4 ms), zero scalar cost calls, and one canonical
patch batch. Debug counters: `roml_highs::cost_call_stats` and
`roml_highs::sync_stats`.

Raw timings (persistent HiGHS, 28,800 params / 57,600 objective cells):

| Path | Before remediation | After remediation |
|---|---|---|
| canonical reprice (queue + commit) | — | 12 ms |
| incremental apply + solve (low-level session) | ~65 s | apply 51 ms + solve 42 ms |
| facade `Highs::solve` after reprice | ~65 s | 179 ms |
| scalar native cost calls | 57,600 | 0 |
| bulk native cost calls | 0 | 1 (0.4 ms) |
| pre-existing O(n²) compiler bookkeeping | ~65 s | eliminated |

The ~65 s before remediation comprised the O(n²) compiled-objective
bookkeeping plus per-cell native calls; both are gone. No MIR-08 latency
threshold is frozen.

## P1 — IR-17 with a real prior solve

`roml-highs/tests/mir02_bess_batching.rs::ir17_solve_then_append_then_shadow_matches_rebuild`
runs build → solve → append fresh packed cells → solve → shadow an existing
logical cell → solve, and asserts the incremental objective equals a fresh
snapshot rebuild. It also proves the appended and shadowed cells are handled by
the live session.

## P2 — symbolic reference state

`ReferenceBackend` patch batches update the existing cell's evaluated cache in
place, preserving the parameterized `ValueExpr`, and treat a missing target cell
as a typed apply error. `ReferenceBackend::symbolic_objective_cells` exposes the
symbolic shape.

RED→GREEN: `reference_replay_preserves_symbolic_patch_cells` (symbolic equality
between incremental replay and snapshot rebuild, not just numeric normalized
views).

## P2 — StridedMap hardening

Raw `StridedMap::new` is crate-private; `is_well_formed` rejects rank mismatch
and shape-product overflow; `get` uses checked arithmetic and the frozen
row-major convention (last dimension fastest — a real bug caught by the new
unit test). Zero strides remain legal; duplicate canonical cell positions are
rejected by dependency validation.

Unit tests: `strided_map_rejects_malformed_metadata`,
`strided_map_row_major_ordinals_and_zero_stride`.

## Verification (remediation head)

```text
cargo fmt --all -- --check                                        clean
cargo clippy -p roml -p roml-highs -p roml-python ... -D warnings  clean
RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps            clean
cargo nextest run -p roml -p roml-highs --features .../bundled     1543 passed, 4 skipped
python -m pytest python/tests --ignore=python/tests/typing        148 passed, 1 skipped
scripts/check-quality-policy.sh origin/main                       pass
git diff --check; cargo package --list -p roml                    clean
```

## Residual risk / not verified

- `roml-mosek` / `roml-xpress` still cannot be type-checked locally
  (proprietary SDKs); their `Change` arms are rustfmt-parsed only.
- The high-level BESS formulation does not automatically produce eligible
  blocks until MIR-03's sink-aware `try_param_block_layout`; the fixture uses a
  hand-verified layout.
- The compiled global `obj_costs` cache and the native cost vector are updated
  equivalently to the scalar path; full observational equivalence is covered by
  the rebuild-vs-incremental tests, not exhaustively re-proven here.

---

# Test expansion and code coverage (2026-09-11)

Requested after the remediation: broaden tests and measure coverage.

## New tests

| File | Tests | Focus |
|---|---|---|
| `tests/mir02_edge_cases.rs` | 28 | span/block API surfaces; empty/unchanged/length/override bulk updates; parametric-row shape/bounds/scale/stale-entity/zero-drop/multi-row/empty-row rejection; layout row-reference and out-of-range rejection; general-oracle differential; partial-layout scalar reprice; shadow/removal dependency iteration; diagnostics reset; block parameter naming; packed delta payload fields; parametric-row scalar reprice and reference replay; multi-block commits |
| `tests/mir02_remediation.rs` | 5 | the owner-review RED→GREEN regressions |
| `tests/mir02_backend_batching.rs` | 3 | packed objective-cost projection; mixed objective+constraint; two objectives → two packed ops |
| `roml-highs/tests/mir02_bess_batching.rs` | 4 | flagship bulk call; apply/solve split; IR-17 solve gate; gapped columns → set form |
| in-crate unit tests | 8 | StridedMap accessors/signed/empty; span+block accessors; transaction block-pending; coefficient validation negative/out-of-range/non-finite |

Full suite: **1578 tests, 4 skipped** (`cargo nextest run -p roml -p roml-highs --features roml-highs/bundled`).

## Coverage

`cargo llvm-cov nextest -p roml -p roml-highs --features roml-highs/bundled`:

```text
OVERALL 27759/32532 = 85.33% lines   (CI gate is 75%)
src/bulk.rs                       97.30%  (252/259)
src/diagnostics.rs               100.00%  (3/3)
src/model/coefficient.rs          85.42%  (1535/1797)
src/model/mod.rs                  92.31%  (3219/3487)
src/model/transaction.rs          95.24%  (80/84)
src/compiler/session.rs           74.68%  (1044/1398)
src/compiler/backend_ir.rs        85.75%  (728/849)
src/solver/reference.rs           82.54%  (851/1031)
src/delta.rs                      97.82%  (314/321)
src/model/changelog.rs            94.92%  (56/59)
src/model/variable.rs             94.30%  (182/193)
src/model/parameter.rs            98.36%  (120/122)
roml-highs/src/compiler.rs        75.35%  (538/714)
roml-highs/src/session.rs         88.42%  (1756/1986)
```

The newly added code is well covered (`bulk.rs` 97.3%, `transaction.rs`
95.2%, `diagnostics.rs` 100%, `delta.rs` 97.8%). The remaining uncovered lines
are defensive branches: dependency-map `continue`/error guards for metadata
that valid witnesses never produce, `BackendOp::SetObjectiveCosts` reference
validation error arms, and pre-existing uncovered paths in
`src/compiler/session.rs` (bridge/construct compilation) and
`roml-highs/src/compiler.rs` (objective-policy forms). No MIR success path is
uncovered.

Coverage is a local measurement; CI's `cargo llvm-cov --fail-under-lines 75`
remains the gate.

## Coverage iteration 2 (compiled-path and negative coverage)

Added `tests/mir02_compiled_path.rs` (incremental `CompilationSession` ->
`BackendOp` -> compiled `ReferenceBackend` replay equals a compiled rebuild for
variable blocks, parametric rows, packed objective costs and bulk repricing;
plus unknown-entity typed rejections), fully-shadowed and unrelated-span
reprice cases, merged-scale overflow, `ModelError` Display, a multi-run target
directory unit test, and `python/tests/test_mir_diagnostics.py`.

Final line coverage (`cargo llvm-cov nextest -p roml -p roml-highs --features
roml-highs/bundled`, 1588 Rust tests):

```text
OVERALL 27912/32559 = 85.73%   (baseline 84.96, iteration 1 85.33)
src/bulk.rs                  100.00%   src/delta.rs                 97.82%
src/diagnostics.rs           100.00%   src/model/changelog.rs       94.92%
src/model/transaction.rs     100.00%   src/compiler/session.rs      79.33%
src/model/variable.rs        100.00%   src/compiler/backend_ir.rs   85.75%
src/id/arena.rs              100.00%   src/solver/reference.rs      84.97%
src/model/parameter.rs        98.36%   roml-highs/src/compiler.rs   75.35%
src/model/coefficient.rs      86.03%   roml-highs/src/session.rs    88.42%
src/model/mod.rs              92.46%
```

Every MIR success path is covered. The remaining uncovered lines are:

- defensive guards no valid witness can reach (map `continue` arms, negative
  offsets, out-of-range positions) — negative tests cover the rejections that
  are reachable;
- error branches for unknown compiled entities in the compiler/reference/HiGHS
  that the compiler and validators prevent from being constructed in-process;
- pre-existing uncovered code in `src/compiler/session.rs` (construct bridge
  compilation) and `roml-highs/src/compiler.rs` (objective-policy forms), and
  `Model::parameter_block::remove` (`unimplemented!()`).

### Capability → test map

| Capability | Tests |
|---|---|
| diagnostics counters / reset / Python hook | `mir00_baseline_characterization`, `diagnostics_reset_clears_all_counters`, `test_mir_diagnostics.py` |
| opaque spans / trusted block allocation | `bulk.rs` compile_fail doctest + unit tests, `mir01_block_allocation` |
| variable packed Change/ModelOp (canonical + compiled) | `mir01_block_allocation`, `mir02_compiled_path` |
| parameter block creation semantics | `mir01_block_allocation` |
| per-member staleness | `variable.rs` unit tests, `mir01_block_allocation` |
| layout storage without positions | `layout_stores_blocks_without_param_positions` |
| ownership/overlap/forgery rejection | `mir02_remediation`, `mir02_edge_cases`, coefficient unit tests |
| partial-layout mixed blocks+positions | `partial_layout_*`, `shadowed_block_cell_keeps_overlay_dependency`, `removed_overlay_cell_*` |
| objective/rows canonicalization | `mir02_parametric_blocks`, `mir02_parametric_rows`, `mir02_edge_cases` |
| bulk transaction queue/commit/rollback/scalar-override | `bulk_update_*`, `two_block_updates_*`, `scalar_pending_write_*` |
| block propagation counters and self-contained delta | `bulk_reprice_emits_*`, `packed_delta_payloads_carry_expected_fields`, `packed_delta_replays_*` |
| backend/HiGHS batching (range + set forms) | `mir02_backend_batching`, `mir02_bess_batching` |
| IR-17 real solve sequence | `ir17_solve_then_append_then_shadow_matches_rebuild` |
| symbolic reference state | `reference_patch_preserves_symbolic_expression_and_updates_cache`, `reference_replay_preserves_symbolic_patch_cells` |
| StridedMap metadata / ordinal convention | `bulk.rs` unit tests |

## Coverage iteration 3 (existing/under-tested code)

Added tests for pre-existing, safety-relevant code rather than only MIR code:

- `src/model/validation.rs`: `FiniteScalar`/`BoundValue`/`Tolerance` accessors,
  `From` conversions, `Display`, arithmetic operators, and
  `fixing_within_declared` (71.4% -> 94.9%).
- `src/compiler/mod.rs`: every `CompileError` `Display` arm, including the
  construct/Big-M/PWL/identity variants (35.3% -> 95.7%).
- `src/model/coefficient.rs`: `append_constant_block` canonicalization
  (unsorted duplicates + near-zero drop), `add` combining a constant into a
  packed parametric cell with identity-preserving shadowing, and `for_var`
  packed scans.

Final line coverage (`cargo llvm-cov nextest -p roml -p roml-highs
--features roml-highs/bundled`, 1596 Rust tests): **86.08%** overall
(baseline 84.96). Key files: `bulk.rs` 100%, `diagnostics.rs` 100%,
`transaction.rs` 100%, `variable.rs` 100%, `arena.rs` 100%, `validation.rs`
94.9%, `compiler/mod.rs` 95.7%, `delta.rs` 97.8%, `coefficient.rs` 88.2%,
`model/mod.rs` 92.5%, `session.rs` 79.3%, `backend_ir.rs` 85.8%,
`reference.rs` 85.0%.

The remaining sub-80% files (`src/compiler/session.rs` construct-bridge
compilation, `roml-highs/src/compiler.rs` objective-policy forms,
`roml-highs/src/{iis,native_iis}.rs`, `src/io/mps/write/*`,
`src/solver/{relaxation,infeasibility}.rs`) are pre-existing feature areas
with existing suites that do not exercise every branch; they are unrelated to
MIR and are not expanded here.

## Coverage iteration 4 (pre-existing, non-MIR subsystems)

Requested: test the pre-existing low-coverage areas and close the gap.

### Findings

- **MPS `write/mod.rs` dead code.** ~320 source lines (~210 executable) of
  `#[allow(dead_code)]` "reference" encoders (`encode_columns`,
  `encode_row_bounds`, `encode_bounds`, `encode_continuous_bounds`,
  `encode_integer_bounds`, `encode_binary_bounds`, `bound`) were explicitly
  superseded by the active `bounds::*` pipeline and had no callers. Tests
  cannot reach private dead code; it was removed (the active `finite_value`
  helper it shared is retained).
- **Real MPS writer bug found.** The active `bounds::encode_integer` did not
  raise the INTORG default upper before lowering-from-above: an integer with
  lower > 1 (e.g. `[2, 8]`) emitted `LI` leaving a transient empty domain that
  the reader rejected. Fixed to mirror the (removed) reference encoder, locked
  by a direct record case and an end-to-end round-trip.
- **Coverage artifact.** The `write/{bounds,format,objective,projection}.rs`
  files are compiled twice (library + `#[path]` test copies), so the focused
  `mps_write_*` tests exercise the test copies; the library copies need
  end-to-end `MpsWriter` tests. Added those.

### Tests added

- `tests/mps_write_edge.rs`: ranged rows, mixed continuous/integer/binary/free
  domains, extreme-magnitude scientific formatting, objective-less models, free
  bounds (round-tripped), unrepresentable free rows, overflowing range widths.
- `tests/mps_reader_errors.rs`: missing ENDATA, COLUMNS/RHS ordering, duplicate
  sections, data after ENDATA, unbalanced/nested INTORG/INTEND,
  OBJSENSE-without-payload — each asserting its typed `MpsErrorKind`.
- `solver::session` trait-default tests with a stub backend; `roml-highs`
  MIP-start mapping error tests.

### Coverage

Overall **87.10%** (baseline 84.96, prior iteration 86.08). `src/io/mps/write/mod.rs`
removed from the low list; `src/io/mps/state.rs` and
`src/io/mps/write/objective.rs` improved.

Suite: **1615 Rust tests**, 151 Python tests.

### Remaining low-coverage (pre-existing, unrelated to MIR)

`src/solver/relaxation.rs` (72%), `src/solver/infeasibility.rs` (74%),
`roml-highs/src/{iis,native_iis}.rs` (~71%), `src/io/mps/write/projection.rs`
(73%), `src/compiler/session.rs` (79%), `roml-highs/src/compiler.rs` (75%),
`src/compiler/bridge/{soft_constraint,indicator}.rs` (~75%). These are complex
solver-internal subsystems whose main paths already have dedicated suites; the
remaining lines are error/edge branches requiring per-subsystem fixtures.

## Coverage iteration 5 (solver-internal subsystems)

Continued into the solver-internal pre-existing areas with reachable tests:

- **Feasibility relaxation** (`src/solver/relaxation.rs`): eight explicit-scope
  preflight rejections (empty scope; unknown/inactive/non-finite constraint
  side; unknown/non-finite variable bound; unfixed/unknown persistent fixing).
- **Infeasibility** (`src/solver/infeasibility.rs`): `completion_for_analysis`
  for every `UnknownReason` and no-outcome/budget cases; `InfeasibilityError`
  Display for every variant.
- **HiGHS IIS boundary** (`roml-highs/tests/iis.rs`): `OriginalLp` with a
  discrete variable rejects; a non-finite plan feasibility tolerance rejects.
- **Backend session traits** (`src/solver/session.rs`): overlay apply/rollback/
  verify, native-conflict and infeasibility-oracle defaults all reject with
  typed `Unsupported` (constructed snapshot/overlay/universe in-crate).
- **MPS projection**: an active semantic construct is rejected as
  `Unrepresentable` with the construct named.

Coverage: **87.44%** overall (baseline 84.96). `src/solver/session.rs` raised
from 16% to >78%, `infeasibility.rs` off the low list, `relaxation.rs` 72→75%.

### Remaining uncovered (defensive/native branches)

The residual low-coverage files require native-failure injection or trigger
defensive branches unreachable through the public API:
`roml-highs/src/{iis,native_iis}.rs` (native status/mapping error branches),
`src/compiler/bridge/{soft_constraint,indicator}.rs` (compile-time branches the
model builders already prevent), `src/io/mps/write/projection.rs`
(stale/absent-entity guards), `roml-highs/src/{compiler,lifecycle}.rs`
(backend error branches), and `src/solver/relaxation.rs` (native-provider and
cleanup paths). These are not MIR surface and do not affect MIR acceptance.

## Coverage iteration 6 (genuine closure of every sub-78% file)

The iteration-5 residual list was closed genuinely rather than skipped: each
uncovered branch was either exercised through a real test seam (public API,
in-crate fault injection, or a small testability refactor) or, where the branch
is provably unreachable through the model builders, left documented. No test
was deleted and no error was weakened.

- **`roml-highs/src/native_iis.rs`** → 94.9% (was 71.5): `native_conflict`
  split into a session-taking helper so the version-qualification and
  request-identity guards are testable; all `bound_sides` / `native_bound` /
  `native_membership` constants and unknowns, the `checked_count` /
  `checked_index` negative/out-of-range rejections, and native variable-bound
  IIS lower/upper-conflict integration tests covering the column mapping.
- **`roml-highs/src/iis.rs`** → 83.2% (was 70.6): in-crate oracle tests for the
  stale-compilation-id and foreign-atom selection rejections, a rejected
  negative feasibility-tolerance budget, and a budgeted feasible check.
- **`roml-highs/src/compiler.rs`** → 80.4% (was 75.4): a public-session
  incremental-removal test builds/solves, removes a variable/constraint/
  objective, and asserts incremental/rebuild equivalence, exercising the
  `RemoveVariable` / `RemoveLinearRow` / `RemoveObjective` backend branches.
- **`src/compiler/bridge/soft_constraint.rs`** → 93.0% production (was 73.7):
  in-crate `BridgeContext` tests for absent/inactive original constraint,
  non-finite/negative violation cap, non-finite/negative weight, and a missing
  weight parameter (all typed rejections).
- **`src/compiler/bridge/indicator.rs`** → 99.1% production (was 76.1):
  `one_sided_implications` for every `ScalarSet` kind and a missing parameter;
  `indicator_bounds` for all direction/side combinations.
- **`roml-highs/src/lifecycle.rs`** → 87.5% production (was 77.8):
  `new_unchecked` construction and a Drop callback-state cleanup test that
  registers a real `CallbackState` and drops.
- **`src/io/mps/write/projection.rs`** → 86.8% production (was 74.8): unit
  tests for `next_generated_name` occupancy, ordered dependency traversal
  across every `ValueExpr` arm, `checked_finite` normalization/rejection, every
  model-scoped error constructor, and an objective/constraint name-collision
  integration test honoring `PreserveOrGenerate` vs `StrictPreserve`.
- **`src/io/mps/mod.rs`** → 96.5% (was 75.1): a public reader-surface test file
  for path I/O errors, source-span validation/display, section and error-kind
  display for every variant, and source-map span resolution.
- **`src/solver/relaxation.rs`** → 88.1% production (was 75.2):
  `report_members` fault-injection tests (unknown/duplicate/non-integral
  candidates, non-relaxed base violation, missing constraint/variable/fixing,
  non-finite weight, soft cap exceeded), `unknown_reason` mapping,
  `compile_portable_overlay` all-eligible collection plus stale-entity and
  missing-fixing rejections, and an all-eligible empty-scope rejection. The
  unused `_typed_ids` shim was removed as dead code.

Coverage: **88.86%** overall (iteration 5: 87.44%; original baseline: 84.96%).
No file with ≥40 executable lines is below 78%. Production-only coverage
(excluding in-crate `#[cfg(test)]` modules) for the iteration-6 files is
projection 86.8%, relaxation 88.1%, indicator 99.1%, soft_constraint 93.0%,
lifecycle 87.5%. Five files remain ≥78% (relaxation previously 75.2%), and the
remaining uncovered lines are the documented native-failure/defensive guards
(invalid `Highs_create` handle, 64-bit `HighsInt` build, `Highs_*` status
failures, and model-builder-prevented invalid states).

Verification at the iteration-6 head: `cargo fmt --all -- --check`,
`cargo check -p roml --all-targets`, `cargo clippy -p roml -p roml-highs
-p roml-python --all-targets -- -D warnings`, `RUSTDOCFLAGS='-D warnings'
cargo doc -p roml --no-deps`, `scripts/check-quality-policy.sh`,
`git diff --check`, `cargo package --list -p roml` (203 files), `nextest` for
`roml` + `roml-highs --features roml-highs/bundled` (**1684 passed, 4
skipped**), and the Python suite (**151 passed, 1 skipped**).
