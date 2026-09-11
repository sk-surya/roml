# MIR-03 Report — shared modeling IR (partial)

**Phase:** MIR-03. **Requirements:** IR-18…IR-23.
**Branch:** `phase-mir-03`. **Execution base:**
`main@43887eece93e77bcd2581bd94a79e084502c6e20`.

**Status: in progress.** The shared `roml::modeling` IR and its conservative
eligibility proof are implemented and tested at the IR/L2 boundary (IR-18…IR-22).
The phase **exit gate is not yet met**: the IR is not yet wired into the
model/compiler, so the flagship BESS formulation does not yet *automatically*
produce eligible blocks and `general_affine == 0` is not demonstrated.

## What landed

| Task | Requirement | Artifact |
|---|---|---|
| M3-1 | IR-18 | `src/modeling/view.rs`: `View<S>` (span + shape + signed strides + offset), `VarView`/`ParamView` with owner checks; O(1) `slice`/`reverse`/`transpose`; typed malformed-metadata errors; row-major traversal reused from `bulk::StridedMap` |
| M3-2 | IR-19 | `src/modeling/coeff.rs`: `NumView`, `CoeffView`/`ConstantView` (`One`/`Scalar`/`Dense`/`ScaledParam`, `Zero`), `Term`, `LinArray`; `scaled` folds the scale with **no numeric-buffer copy** (pointer-identity test) |
| M3-3 | IR-20 | `ParamView::mul_linarray`: fast IR only for `One`/`Scalar` term coefficients + `Zero`/`Scalar` constant; uncovered forms return `Ok(None)` for the general path |
| M3-4 | IR-21 | `src/modeling/eligibility.rs`: `try_param_block_layout` + `SinkCells`; conservative, metadata-only; counterexamples covered |
| M3-5 | IR-22 | `src/modeling/builder.rs`: `RowBatch`/`RowSink`/`RowBatchPlan` — disjoint constant+parametric rows commit as one packed batch; collisions fall back without duplicate physical cells |
| M3-6 | IR-23 | partially: the IR rejection set signals fallback (`None`/`General`/`Unsupported`); the model-level differential is not yet written |

## IR-21 counterexample coverage

| Counterexample | Result | Test |
|---|---|---|
| broadcast over rows | eligible | `broadcast_over_rows_is_eligible` |
| broadcast into one objective cell | ineligible | `broadcast_into_one_objective_cell_is_ineligible` |
| two params → one cell | ineligible | `two_params_one_cell_is_ineligible` |
| pre-occupied canonical cell | ineligible | `pre_occupied_cell_is_ineligible` |
| non-monotone parameter stride | fallback | `non_monotone_parameter_stride_falls_back` |
| interleaved disjoint cell indices | fallback (conservative) | `interleaved_cell_indices_fall_back_conservatively` |
| non-parametric term | no dependency block | `disjoint_contiguous_family_and_nonparametric_term` |

## Verification (head `a255cf8`)

```text
cargo clippy -p roml --all-targets -- -D warnings      clean
cargo nextest run -p roml                              1488 passed, 4 skipped
RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps  clean
```

22 in-crate `modeling::*` tests.

## Residual / not done (exit gate)

1. **Wire `roml::modeling` into the canonical model and compiler** so a BESS
   `rm.sum(price * (discharge - charge))` lowering automatically calls
   `try_param_block_layout`, stores `ParamDepBlock`s, and reports
   `general_affine == 0`.
2. **Model-level fallback differential (IR-23)** for every IR rejection:
   normalized canonical snapshot equality between the fast and general paths.
3. **Row-sink construction** from canonical cells (the `SinkCells`
   implementation currently lives in tests; production needs a model-backed
   impl).
4. `ParamView * LinArray` broadcasting beyond exact-shape match; nested/matmul
   topology metadata (deliberately out of the initial conservative subset).
5. `roml-mosek`/`roml-xpress` remain untestable locally (proprietary SDKs).

The draft PR (#63) stays open and unmerged until the exit gate is attempted and
reviewed.
