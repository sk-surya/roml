# MIR-03 Report — shared modeling IR (foundation, final pre-integration pass)

**Phase:** MIR-03. **Requirements:** IR-18…IR-23.
**Branch:** `phase-mir-03`. **Execution base:**
`main@43887eece93e77bcd2581bd94a79e084502c6e20`.

**Status: architecture-review candidate.** The isolated IR foundation has been
remediated twice. **No model/compiler wiring has been attempted.** The exit gate
(BESS automatic eligibility, `general_affine == 0`) is not met.

## Final pass (review of `96bb1ed`)

1. **No caller-supplied packed bases.** `TargetRun` no longer carries `bases`.
   `try_param_block_layout` derives canonical family ordering (by first canonical
   variable ordinal) and cumulative p-base offsets internally; the RED
   regression `canonical_order_follows_variable_order_not_source_order` supplies
   the terms in reverse source order and asserts the derived offsets follow
   canonical `VarId` order. Core post-canonical validation is unchanged (defense
   in depth).
2. **Complete mixed-row plan.** `RowBatchPlan::Planned(RowBlockPlan)` replaces
   the parametric-only result and owns the row topology (`LocalRow`), the numeric
   packed stream (`NumericCell`), the parametric `ParamDepLayout`, and the
   derived canonical offsets — one future core mixed-row commit needs no raw
   `Term` re-reading and no independent collision rediscovery. Tests A–E:
   mixed row plan; multiple rows with each target allocated once; constant+
   parametric collision → `General`; two-parametric collision → `General`;
   numeric-only batch stays on the numeric stream. (The core commit itself is
   pending integration, so IR-22 is not claimed complete.)
3. **Ownership trust boundary closed.** `VarView::new`/`ParamView::new` are
   `pub(crate)`; `SinkMap`/`TargetRun`/`try_param_block_layout`/`RowBatch`/
   `RowBlockPlan` are crate-private L1→L2 planning internals. A `compile_fail`
   doctest on `VarView` proves no public safe path can pair a span with a forged
   owner.
4. **Span-range validation.** `VarView`/`ParamView` construction proves from
   metadata (O(rank) `mapped_range`) that every mapped member offset is within
   the span; metadata-only `slice`/`reverse`/`transpose` preserve that. Tests:
   offset beyond span, positive-stride past end, negative below zero, valid
   reversed view.
5. **Checked-integer audit.** `StridedMap::get` coordinates, `is_contiguous_dense`
   dimensions, eligibility `run.start`/length conversions, `NumView`
   `mapped_range` and buffer-length comparisons, and `View::slice`/`reverse` all
   use checked `usize -> isize`/`usize -> u32` conversions and arithmetic.
   Malformed/unrepresentable metadata is a typed error or fallback; boundary
   regressions use `usize::MAX` dimensions without allocating buffers.

## Interfaces (crate-private until MIR-04)

```text
SinkMap { shape, runs: Vec<TargetRun> }
TargetRun { target, objective, start, len }        // no bases
try_param_block_layout(&SinkMap, &[Term]) -> Option<ParamDepLayout>  // derived cell_offset
RowBatch { new, len, is_empty, push(LocalRow, LinArray), plan() }
RowBatchPlan::{ Planned(RowBlockPlan), General }
RowBlockPlan { owner(), rows(): &[LocalRow], numeric(): &[NumericCell], parametric(): &ParamDepLayout }
```

## IR-21 regressions

| Case | Expectation | Test |
|---|---|---|
| BESS two-term objective | eligible, two families | `bess_objective_two_terms_produce_two_families` |
| canonical vs source order | derived offsets follow `VarId` order | `canonical_order_follows_variable_order_not_source_order` |
| broadcast over rows | eligible, honest `row=Some` | `broadcast_over_rows_uses_honest_row_targets` |
| broadcast into one objective cell | ineligible | `broadcast_into_one_objective_cell_is_ineligible` |
| overlapping two-term spans | ineligible | `overlapping_two_term_spans_are_ineligible` |
| non-monotone parameter stride | fallback | `non_monotone_parameter_stride_falls_back` |
| zero stride over multi-ordinal run | fallback | `zero_stride_over_a_multi_ordinal_run_falls_back` |
| malformed sink cover / duplicate rows | typed rejection | `sink_map_rejects_bad_run_covers_and_duplicate_rows` |
| dimensions above `isize::MAX` | no accidental eligibility | `unrepresentable_dimensions_never_prove_eligibility` |

## Verification

```text
cargo fmt --all -- --check                              clean
cargo clippy -p roml --all-targets -- -D warnings       clean
cargo nextest run -p roml                               1498 passed, 4 skipped
RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps  clean
cargo test -p roml --doc                                compile_fail regression passes
```

## Residual / deliberately not done

1. Model/compiler wiring; BESS automatic eligibility; `general_affine == 0`.
2. Core post-canonical revalidation and the one-commit mixed-row core path.
3. IR-23 model-level fallback differential.
4. `roml-mosek`/`roml-xpress` remain untestable locally (proprietary SDKs).

## Integration progress (post A/B/C, authorized)

The three gating issues (constants, plan self-containment, `VarId` identity)
are fixed at `bca2c53`. Integration has begun:

- **Automatic objective seam (`7d5ee50`).**
  `Model::set_linear_objective_from_linarray(sense, &LinArray)` derives the L2
  dependency layout via `try_param_block_layout` (no hand-supplied witness),
  materializes packed cells in canonical family order, and commits through the
  existing MIR-02 post-canonical validation. Non-eligible input falls back to
  the general path (no layout), still correct.
- **IR-23 objective differential (`747716d`).** The automatic (packed) and
  general (no-layout) objective paths produce equal canonical snapshots before
  and after a bulk reprice.
- **IR-31 flagship cardinality (`747716d`+).** `model::mir03_tests::
  flagship_bess_objective_cardinality` builds 28,800 price parameters driving
  57,600 objective cells from trusted block spans and asserts, automatically:
  - `param_dep_blocks >= 2`, `param_positions_cells == 0`, `general_affine == 0`;
  - `num_coefficients() == 57,600`;
  - after `set_parameters_bulk` + commit: `param_position_lookups == 0`,
    `overlay_lookups == 0`, `value_expr_evals == 0`,
    `coefficient_patch_batches == 1`.
  Runs in ~42 ms; core post-canonical validation accepts the derived witness.

Verification: `cargo clippy -p roml --all-targets -- -D warnings` clean;
`cargo nextest run -p roml` **1503 passed, 4 skipped**; rustdoc clean.

### Still remaining (MIR-03 exit gate)

1. The one-commit mixed constant+parametric **row** seam (a model commit API
   consuming `RowBlockPlan`); `RowBatch`/`RowBlockPlan` exist but are not yet
   wired to a model entry point.
2. IR-23 differential across **every** rejection class (currently the eligible
   objective only).
3. Wiring the **Python** BESS formulation (`rm.dot(price_grid, discharge -
   charge)`) through the automatic proof; the flagship counters above are at the
   Rust core level.
4. Full exact-head qualification matrix and evidence/state finalization.
