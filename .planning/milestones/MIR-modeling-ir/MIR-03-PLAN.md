# MIR-03 Execution Plan — shared modeling IR + conservative layout proof

**Phase:** MIR-03. **Requirements:** IR-18…IR-23. **Depends on:** MIR-02 (merged).
**Execution base:** `main@43887eece93e77bcd2581bd94a79e084502c6e20`.
**Exit gate:** the BESS parametric objective automatically produces eligible
`ParamDepBlock`s through metadata proof, with `general_affine == 0` on the
flagship formulation.

Read `DESIGN.md` §1–§7 and load `skills/roml-ir-invariants/SKILL.md` before
editing. Correctness/fallback dominate: a false-positive eligibility is a
correctness defect; a conservative fallback is a performance finding.

## Tasks (TDD, one commit each)

- **M3-1 — L1 view metadata IR (IR-18).** *This slice.*
  `src/modeling/view.rs`: `View<S>` = span + shape + signed strides + offset;
  `VarView`/`ParamView` = owner + `View<Span>`; slicing/transpose/reversal edit
  metadata only; row-major ordinals reuse the MIR-02 `StridedMap`; cross-model
  composition is a typed error before member reconstruction.
  Tests: `modeling::view::tests::{view_maps_row_major_ordinals,
  slice_transpose_and_reverse_are_metadata_only, malformed_metadata_is_a_typed_error,
  symbolic_views_reconstruct_members_and_reject_cross_model}`.

- **M3-2 — coefficient/constant IR + zero-copy scale (IR-19).**
  Contract `One`, `Scalar`, `Dense { scale, values: NumView }`,
  `ScaledParam { scale, params }`; `ConstantView` equivalent with `Zero`.
  `α * Dense` folds into `scale` without copying the numeric buffer
  (allocation/counter assertion); `Term { vars, coeff }` and `LinArray`.
  - M3-2a `NumView` (shared/owned numeric buffer + strided view).
  - M3-2b `α*Dense`/`α*ScaledParam` scale folding; covered families materialize
    no `Vec<Affine>`/per-cell `ValueExpr`.

- **M3-3 — conservative `ParamView * LinArray` (IR-20).** Fast subset only when
  every variable term coefficient is `One`/`Scalar` and the constant is
  `Zero`/`Scalar`; `Dense × ParamView`, `ScaledParam × ParamView` and other
  uncovered forms fall to the general symbolic path.
  Tests: fast/fallback differential corpus.

- **M3-4 — sink-aware `try_param_block_layout` (IR-21).**
  `fn try_param_block_layout(sink: &SinkMap, terms: &[Term]) -> Option<ParamDepLayout>`
  proving, from metadata only: injective `(target, var)` cells; no second
  contribution to the same canonical cell from other terms in the same sink;
  post-canonical strided storage witness. Core still revalidates after
  canonicalization (IR-12).
  Counterexamples (must be covered): broadcast-over-rows **eligible**;
  broadcast-into-objective **ineligible**; non-monotone stride fallback;
  interleaved disjoint views handled; two params → one cell **ineligible**.

- **M3-5 — mixed constant+parametric CSR builder (IR-22).** Rows with disjoint
  canonical cells commit in one batch; collisions into one canonical cell fall
  back/generalize without duplicate physical cells.

- **M3-6 — general fallback corpus (IR-23).** Normalized canonical snapshot
  differential across the whole IR rejection set.

## Diagnostics to keep honest

Lowering: `numeric_bulk`, `parametric_bulk`, `general_affine`,
`param_dep_blocks`, `param_positions_cells`, `rule_rows_accumulated`,
`rule_bulk_commits`. Flagship: 28,800 price params → 57,600 objective cells;
`general_affine == 0` is the MIR-03 exit metric.

## Evidence

Per-task tests under `tests/mir03_*` + in-crate; a `evidence/MIR-03-REPORT.md`
once the exit gate is attempted. No MIR-04 L1 ergonomics in this phase.
