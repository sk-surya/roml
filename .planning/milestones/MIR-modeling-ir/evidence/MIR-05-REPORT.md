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

### Indexed rule API and `Model::add_rules` (M5-2)
- `Model::add_indexed_rules(indices, |rules, i| ...)` drives the iteration inside
  ROML: `indices` may be any `IntoIterator` (`0..N`, a slice, etc.), and the
  closure constructs each index's row expressions into the shared `RuleBatch`.
  `Model::add_rules(closure)` remains for callers that prefer to drive the loop
  themselves; both share one `commit_rule_batch` helper.
- The batch commits with **one** `add_rows_from_plan` call (one
  `Change::BulkMixedRows` / `ModelOp::AddMixedRows`).
- Diagnostics: `rule_rows_accumulated += nrows`, `rule_bulk_commits += 1`.
- A closure error, a foreign array, or a non-packable batch is a typed rejection
  with no model or journal mutation.
- `ModelError::View(ViewError)` preserves the exact structured-array failure
  (cross-model owner, slice out of range, shape mismatch) with a `source()`
  chain instead of a generic reason string.

### Tests (M5-3)
- `tests/mir05_rules.rs`: closure and indexed rules are snapshot-identical to
  the direct L1 `add_rows`; `rule_bulk_commits == 1`, `rule_rows_accumulated ==
  N`, one packed numeric block, `general_affine == 0`; parametric rule rows
  derive packed families (`param_dep_blocks >= 1`); a failing closure and a
  foreign array leave no residue; detailed `ViewError`s are preserved
  (`ModelError::View(CrossModel/SliceOutOfRange)`).
- `model::mir05_rule_tests`: N rows journal exactly one `Change::BulkMixedRows`;
  a non-packable batch is a typed rejection with no residue; the emitted
  `BulkMixedRows` **replays through the retained deltas** to the same normalized
  backend view as a clean rebuild, before and after a bulk parameter reprice
  (0 live-model lookups/evals, 1 packed patch batch).

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
cargo nextest run -p roml                               1551 passed, 4 skipped
scripts/check-quality-policy.sh                         pass
git diff --check                                        clean
cargo package --list -p roml                            220 files
```

## Review round 1 remediation

The review asked for three changes before merge; all are implemented and
covered.

1. **Indexed rule API (ROML drives the iteration).**
   `Model::add_indexed_rules(indices, |rules, i| ...)` iterates `indices` (any
   `IntoIterator`) and constructs each index's rows into the shared `RuleBatch`;
   `add_rules` remains for callers that loop themselves. Both commit once
   (`rule_bulk_commits == 1`). Tests:
   `tests/mir05_rules.rs::indexed_rules_drive_iteration_and_match_direct_rows`.
2. **Delta replay + reprice qualification.** The rule-batch `BulkMixedRows`
   replays from retained deltas to the same normalized reference backend view as
   a clean rebuild, before and after a bulk reprice; the reprice uses the stored
   dependency blocks (0 lookups/evals, 1 patch batch). Test:
   `model::mir05_rule_tests::rule_batch_delta_replay_matches_rebuild_across_reprice`.
3. **Detailed `ViewError` diagnostics.** `ModelError::View(ViewError)` (with a
   `source()` chain and `From<ViewError>`) preserves the exact cross-model /
   slice / shape failure. Tests:
   `tests/mir05_rules.rs::rule_batch_preserves_detailed_view_errors`.

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
