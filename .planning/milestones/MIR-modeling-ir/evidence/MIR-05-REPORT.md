# MIR-05 Report — rule builders (IR-26)

**Phase:** MIR-05. **Requirement:** IR-26. **Disposition: PASS (Rust/shared-IR
level).** **Branch:** `phase-mir-05`. **Execution base:** `main@052091f`.

MIR-05 delivers closure rule builders as syntax over the frozen MIR-04 L1. Rule
callbacks construct row expressions once per index into an in-memory CSR batch
and commit the whole component in one packed operation; there is no per-index
core insertion.

## Delivered

### `RuleBatch` (M5-1)
- `roml::modeling::RuleBatch` is a model-owned accumulator over the existing
  `RowBatch`. `add_row`/`add_eq`/`add_le`/`add_ge` assign a local row ordinal and
  push `(LocalRow, LinArray)`; nothing touches the model.
- `into_plan() -> Option<RowBlockPlan>` yields the single self-contained plan,
  or `None` when the batch is not packable.
- `VarArray::row(i)` / `ParamArray::row(i)` give a per-index coefficient array
  (leading-axis slice + contiguous reshape); `From<VarArray> for LinArray`
  (and `&VarArray`) keeps rule call sites free of raw ids.

### `Model::add_rules` (M5-2)
- Runs a closure once per index to *construct* row expressions, then commits the
  whole batch with **one** `add_rows_from_plan` call (one `Change::BulkMixedRows`
  / `ModelOp::AddMixedRows`).
- Diagnostics: `rule_rows_accumulated += nrows`, `rule_bulk_commits += 1`.
- A closure error, a foreign array (`ViewError::CrossModel`), or a non-packable
  batch is a typed rejection with no model or journal mutation.

### Tests (M5-3)
- `tests/mir05_rules.rs`: closure rules are snapshot-identical to the direct L1
  `add_rows`; `rule_bulk_commits == 1`, `rule_rows_accumulated == N`, one packed
  numeric block, `general_affine == 0`; parametric rule rows derive packed
  families (`param_dep_blocks >= 1`); a failing closure and a foreign array leave
  no residue.
- `model::mir05_rule_tests`: N rows journal exactly one `Change::BulkMixedRows`;
  a non-packable batch is a typed rejection with no residue.

## Invariant (IR-26)

The meaningful gate is preserved: rules do **not** regress ROML into
one-model-mutation-per-callback. The closure executes once per index to build
expressions only; all canonical mutation, journaling, and packed-layout work
happens in the single bulk commit. `rule_bulk_commits == 1` for any row count.

## Verification

```text
cargo fmt --all -- --check                              clean
cargo check -p roml --all-targets                       clean
cargo clippy -p roml --all-targets -- -D warnings       clean
RUSTDOCFLAGS='-D warnings' cargo doc -p roml --no-deps  clean
cargo nextest run -p roml                               1548 passed, 4 skipped
scripts/check-quality-policy.sh                         pass
git diff --check                                        clean
cargo package --list -p roml                            220 files
```

## Deferred by design

- **Python decorator rules (MIR-06):** the Python rule/decorator surface wraps
  this same accumulator so Python formulations bulk-commit identically. MIR-05
  proves the Rust mechanism and the no-per-callback-mutation invariant.
- **Non-packable rule rows:** a batch that the conservative packed proof cannot
  represent is a typed rejection (the caller uses the general L1 path). Extending
  the packed representation for those forms is an optimization backlog, not a
  correctness gap.
- No second expression/row representation, `m.add` naming sugar, macros, or
  changes to the MIR-04 L1 / MIR-03 packed seams.
- `roml-mosek`/`roml-xpress` unchanged for this phase.
