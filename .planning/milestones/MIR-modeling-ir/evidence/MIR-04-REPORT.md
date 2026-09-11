# MIR-04 Report — Rust Level-1 ergonomics + label boundary (final)

**Phase:** MIR-04. **Requirements:** IR-24, IR-25. **Disposition: PASS
(Rust/shared-IR level).** **Branch:** `phase-mir-04`. **Execution base:**
`main@d0de5a6969f2cefe790412bab8a7622f97ad75ce`.

MIR-04 delivers the Rust Level-1 array surface over the MIR-03 ordinal IR:
structured handles, expression composition, cell-wise and reduction rows, array
objectives, checked boundary labels, a normalized ordinal-IR fingerprint,
native examples with no raw ids, and solution read-back.

## Delivered

### M4-1 Array handles (IR-24)
- `Model::var(name, shape).bounds(lo, hi).kind(..).build() -> VarArray` and
  `Model::param(name, shape, values) -> ParamArray` over the MIR-03 trusted
  `VarView`/`ParamView` (owner + span + strided shape + boundary name).
- `Shape` from `usize`/`[usize; N]`/`Vec<usize>`/`&[usize]`; metadata-only
  `slice(axis, start, len)`, `reverse(axis)`, `transpose(a, b)`; typed
  `ModelError::InvalidArrayShape` / `MismatchedBulkLengths` rejections.
- One packed variable block per array; per-element names never materialized.

### M4-2 Expression algebra (IR-24)
- `VarArray::expr()` (unit coefficients); `ParamArray::try_mul(&LinArray)`
  (conservative fast IR or explicit `None`); `LinArray::{try_add, try_sub,
  scaled}`.
- **Cell-wise rows:** `LinArray::{le, ge, eq, le_each, ge_each, eq_each}` ->
  `RowSpec` -> `Model::add_row` (one constraint per cell).
- **Reduction rows:** `LinArray::{rows_eq, rows_le, rows_ge}` ->
  `RowBlockSpec` -> `Model::add_rows` (one constraint per leading-axis entry,
  summing that entry's coefficients).
- **Array objectives:** `Model::maximize_array` / `minimize_array`. Purely
  parametric arrays commit through the MIR-03 automatic-eligibility seam;
  numeric/mixed arrays commit through the general objective path. Row paths use
  the packed mixed-row seam when the numeric form is covered, else the general
  symbolic path.
- Numeric cell streams are canonicalized (sorted/merged/zero-dropped) before
  the packed append, so `append_canonical_block`'s invariant holds even for
  overlapping views (e.g. `energy[:, 1:]` and `energy[:, :-1]`).

### M4-3 Label boundary (IR-24)
- `Labeled<A: Shaped>` with `Axis { name, labels }`; `new` validates rank and
  axis width; `align`/`align_axis` return typed `LabelError`s.
- `inner`/`into_inner` yield the ordinal array: labels never enter
  `LinArray`/`ValueExpr` and relabeling cannot change the IR.

### M4-4 Normalized ordinal-IR fingerprint (IR-25)
- `Model::normalized_ordinal_fingerprint()` and
  `ModelSnapshot::normalized_ordinal_fingerprint()`: deterministic FNV-1a
  64-bit over variables (bounds/type/fixing), constraints (bounds), objectives
  (sense/constant), and cells sorted by (target kind, target ordinal, var
  ordinal).
- Absolute ids/generations, owners, names, labels, parameter values, and the
  revision are excluded; a structural difference changes the value.

### M4-5 Examples + gate (IR-24)
- `examples/l1_bess.rs`, `examples/l1_transportation.rs`, and
  `examples/l1_min_cost_flow.rs` build their models with no raw ids or manual
  linear expressions; all run on the packed path (`general_affine == 0`,
  `param_cells == 0`, `param_blocks >= 1`).
- `tests/mir04_example_gate.rs` reads the example sources and fails on
  `VarId`/`LinExpr`/`ValueExpr`/raw mutators.

### M4-6 Solution read-back (IR-24)
- `Solution::{array_values, try_array_values, array_value}` read a `VarArray`
  in row-major order; missing values are `None`/typed `SolutionReadError`,
  never silent zeros.

## Exit gate (IR-24)

Native Rust BESS, transportation, and min-cost-flow examples contain no raw
`VarId`/manual `LinExpr` in ordinary model code, enforced by a source gate
(`tests/mir04_example_gate.rs`). Label-typed formulations produce equal
normalized ordinal-IR fingerprints (`tests/mir04_labels.rs`,
`tests/mir04_fingerprint.rs`).

## Differential evidence

- Cell-wise BESS balance block (L1 handles) is snapshot-identical to the raw
  `add_linear_rows_bulk` construction (`model::mir04_row_seam_tests`).
- A conservatively-declined multi-cell parametric block is snapshot-identical
  to the raw per-cell general construction.
- Leading-axis transportation sums (L1) are snapshot-identical to the raw bulk
  construction; a numeric array objective is snapshot-identical to the raw
  objective-coefficient construction (`model::mir04_reduction_tests`).
- Purely parametric BESS objective still satisfies `param_dep_blocks >= 2`,
  `param_positions_cells == 0`, `general_affine == 0`, and reprice with 0
  lookups / 1 patch batch (`tests/mir04_expr.rs`, MIR-03 flagship).

## Verification

```text
cargo fmt --all -- --check                              clean
cargo check -p roml --all-targets                       clean
cargo clippy -p roml --all-targets -- -D warnings       clean
RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps  clean
cargo nextest run -p roml                               1530 passed, 4 skipped
scripts/check-quality-policy.sh                         pass
git diff --check                                        clean
cargo package --list -p roml                            216 files
```

## Residual risk / deferred

- `roml-highs/examples/bess_mpc.rs` remains the raw-API performance benchmark.
  Its mixed-length objective (a numeric terminal term plus a shorter parametric
  term array) is not expressible through the single-shape `LinArray` yet; the
  ordinary L1 BESS example satisfies IR-24. Tracked for MIR-05/06.
- Multi-cell-per-row **parametric** reductions and parameterized objective
  constants (`ScaledParam` constants) are conservative rejections that fall
  back to the general symbolic path; extending the packed representation is an
  optimization backlog, not a correctness gap.
- `roml-mosek`/`roml-xpress` remain rustfmt-parsed only for the new seams
  (proprietary SDKs).
- Python is intentionally untouched (MIR-06).
