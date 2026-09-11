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
