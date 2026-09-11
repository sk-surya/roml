# MIR-03 Report — shared modeling IR, eligibility, and core lowering (final)

**Phase:** MIR-03. **Requirements:** IR-18…IR-23. **Disposition: PASS.**
**Branch:** `phase-mir-03`. **Execution base:**
`main@43887eece93e77bcd2581bd94a79e084502c6e20`.

MIR-03 delivers the shared model-owned strided array IR, the conservative
sink-aware eligibility proof, and the core lowering seams (automatic objective
eligibility and the mixed numeric+parametric row protocol), with the flagship
BESS objective satisfying its gates at the Rust/shared-IR level.

## Delivered

### IR / view foundation (IR-18)
- `roml::modeling::View<S>`: `span + shape + signed strides + offset`, with
  O(1) checked `slice`/`reverse`/`transpose`; malformed/overflowing metadata is
  a typed `ViewError`, never a wrap or panic. Row-major traversal reuses the
  MIR-02 `StridedMap`.
- `VarView`/`ParamView` carry `ModelInstanceId` ownership; construction is
  crate-private (trusted) and validates the mapped range against the span in
  O(rank). Cross-model composition is a typed error before member reconstruction
  (a `compile_fail` doctest proves no public safe path can forge an owner).

### Coefficient/constant IR (IR-19, IR-20)
- `NumView` (validated strided numeric view), `CoeffView`/`ConstantView`
  (`One`/`Scalar`/`Dense`/`ScaledParam`, `Zero`), `Term`, `LinArray`. Scalar
  scaling folds into the stored scale (zero-copy for `Dense`/`ScaledParam`).
- `ParamView * LinArray` stays on the fast IR only for `One`/`Scalar` term
  coefficients and a `Zero`/`Scalar` constant; uncovered forms return `Ok(None)`
  for the general symbolic path.

### Sink-aware eligibility (IR-21)
- `try_param_block_layout(sink, terms)` is metadata-only and conservative over
  `r -> (target(r), var_j(r))`: pairwise-disjoint variable spans; for runs longer
  than one ordinal, contiguous-dense variable/parameter views with positive
  parameter strides; a one-ordinal run admits broadcast. Canonical family order
  and p-base offsets are derived internally (no caller-supplied bases). Core
  revalidates the witness after canonicalization (unchanged MIR-02 defense).

### Core lowering (IR-22)
- **Automatic objective seam:** `Model::set_linear_objective_from_linarray`
  derives the layout automatically and commits through the existing MIR-02
  post-canonical validator.
- **Mixed-row protocol:** `MixedRowBlock` (one constraint allocation + bounds,
  numeric CSR, parametric CSR, cached values, derived `ParamDepLayout`) ->
  `Change::BulkMixedRows` -> `ModelOp::AddMixedRows` (one semantic op on one row
  set). The compiler emits each constraint exactly once with both coefficient
  streams combined; the reference backend mirrors it; mosek/xpress arms are
  added (rustfmt-parsed only). `Model::add_rows_from_plan` validates owner,
  bounds, finite values, live vars/params, **and the dependency witness** before
  allocating rows or journaling anything; replay consumes only the stored
  payload (no L1 re-planning).

## Flagship BESS objective (core level)

`model::mir03_tests::flagship_bess_objective_cardinality` builds 28,800 price
parameters driving 57,600 objective cells from trusted block spans:

- `param_dep_blocks >= 2`, `param_positions_cells == 0`, `general_affine == 0`;
- `num_coefficients() == 57,600`;
- after `set_parameters_bulk` + commit: `param_position_lookups == 0`,
  `overlay_lookups == 0`, `value_expr_evals == 0`,
  `coefficient_patch_batches == 1`;
- core post-canonical validation accepts the derived witness.

## IR-23 rejection corpus

Each class is proven end-to-end: shared-IR formulation -> eligibility/
`RowBlockPlan` rejects -> general path -> identical normalized mathematical
model, **before and after a bulk parameter update** (`model::mir03_ir23_tests`).

| Rejection class | Kind | Outcome |
|---|---|---|
| two distinct params reach one canonical cell | correctness | proof declines -> general path; model equal across reprice |
| zero-stride parameter over a multi-cell target | conservative | positions fallback; equal across reprice |
| reversed / negative parameter stride | conservative | no dependency block; equal across reprice |
| `Dense × ParamView` | conservative | `mul_linarray` declines |
| `ScaledParam × ParamView` (param x param) | conservative | `mul_linarray` declines |
| shape outside the exact-shape subset | conservative | `mul_linarray` declines |
| parameterized objective constant | correctness | typed `InvalidParamDepLayout`; no residue |
| mixed-row constant/parametric collision | correctness | `RowBlockPlan::General` |

Terminology:
- **Correctness rejection:** accepting this form into the current packed
  representation would be incorrect; it stays rejected unless the
  representation itself is deliberately extended (e.g. a future explicit
  parameterized-constant encoding).
- **Conservative rejection:** the form is mathematically packable but the
  initial proof deliberately declines it; these are an optimization backlog for
  MIR-04/06, not correctness gaps.

## Qualification tests (mixed-row protocol)

Green: one constraint allocation / one logical change; journal/delta replay
reproduces a clean backend rebuild; compiler rebuild equals incremental; partial
sync resumes from the acknowledged revision without loss or duplication; stale
variable rejects atomically; corrupt dependency witness rejects atomically
before allocation; constants folded into bounds survive; a bulk parameter update
after the row still uses the eligible dependency block (0 lookups, 1 patch
batch).

## Verification

```text
cargo fmt --all -- --check                              clean
cargo check -p roml --all-targets                       clean
cargo clippy -p roml --all-targets -- -D warnings       clean
RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps  clean
cargo nextest run -p roml                               1515 passed, 4 skipped
scripts/check-quality-policy.sh                         pass
git diff --check                                        clean
cargo package --list -p roml                            208 files
```

Exact-head CI (Policy, Quality, Coverage, Core, HiGHS, Python) is green.

## Review / remediation history (brief)

- The IR foundation was remediated across three owner review rounds: (1) the
  original six blockers (eligibility around `r -> (target(r), var_j(r))`,
  metadata-only proof, honest row targets, coefficient-metadata validation,
  checked transforms, `NumView` scope); (2) caller-supplied packed bases removed
  and the canonical layout derived internally, `RowBlockPlan` self-containment,
  the ownership trust boundary, span-range validation, and a checked-integer
  audit; (3) mixed-row constants, plan self-containment, and `VarId`
  generation identity. All remediation tests remain in the suite.

## Deferred by design

- **MIR-04/05 qualification debt:** once a public/general row-expression API
  exists, compare the same mixed numeric+parametric model through the
  user-facing symbolic path against `Model::add_rows_from_plan`. No artificial
  internal API was created to satisfy this now.
- **MIR-06 acceptance test:** wire the Python
  `rm.dot(price_grid, discharge - charge)` BESS formulation through the shared
  IR and the automatic objective seam.
- `roml-mosek` / `roml-xpress` `Change::BulkMixedRows` arms are rustfmt-parsed
  only (proprietary SDKs; cannot compile/test locally).
