# MIR-05 Execution Plan — rule builders (Rust closure rules)

**Phase:** MIR-05. **Requirement:** IR-26. **Depends on:** MIR-03/MIR-04
(merged). **Branch:** `phase-mir-05`. **Execution base:** `main@052091f`.

**Exit gate:** rules accumulate row descriptors and **bulk-commit once**:
a closure that adds `N` rows produces one `Change::BulkMixedRows` /
`ModelOp::AddMixedRows`, `N` constraints, `rule_rows_accumulated == N`, and
`rule_bulk_commits == 1` — never `N` per-row model mutations. Rule syntax is
sugar over the frozen MIR-04 L1, not a second modeling representation.

Read DESIGN §8 and load `skills/roml-ir-invariants/SKILL.md` before editing.
The invariants that bind here are #4 (views, not gathered id vectors), #18
(covered families do not materialize per-cell `ValueExpr`), and #20 (rule APIs
accumulate CSR and bulk-commit once).

## Tasks (TDD, one commit each)

- **M5-1 — `RuleBatch` accumulator over `RowBatch`.** A model-independent
  accumulator owned by one `ModelInstanceId`:
  - `add_row(coeffs, lower, upper)` / `add_eq` / `add_le` / `add_ge` assign a
    local row ordinal and push into the existing `RowBatch`;
  - `into_plan() -> Option<RowBlockPlan>` yields the single self-contained
    plan, or `None` when the batch is not packable;
  - `VarArray::row(i)` / `ParamArray::row(i)` give a per-index coefficient
    array (leading-axis slice + contiguous reshape) so reductions iterate
    naturally; infallible `From<VarArray> for LinArray` conversions keep the
    closure call sites clean.
- **M5-2 — `Model::add_rules(closure)`.** Runs the closure once per index to
  *construct* row expressions, accumulates without touching the model, then
  commits the batch with **one** `add_rows_from_plan` call. Diagnostics:
  `rule_rows_accumulated += nrows`, `rule_bulk_commits += 1`. A closure error,
  a foreign array, or a non-packable batch is a typed error with no model or
  journal mutation.
- **M5-3 — Equivalence and no-regression tests.**
  - `rule_bulk_commits == 1` and one `BulkMixedRows` change for `N` rows;
    `rule_rows_accumulated == N`.
  - Differential vs the direct L1 paths (`add_rows`/`add_row`): snapshot and
    delta-replay equality.
  - A closure that builds `N` rows performs zero model mutations until commit;
    a closure that errors halfway (or a foreign array) leaves state unchanged.
  - A non-packable batch is a typed rejection with no residue.
  - A numeric rule batch stays on the packed path (`general_affine == 0`).
- **M5-4 — Report + policy.** `evidence/MIR-05-REPORT.md`; `STATE.md`.

## Non-goals and deferrals
- Python decorator rules: MIR-06 wraps the same accumulator through the Python
  surface (explicitly deferred; MIR-05 proves the Rust bulk-commit invariant).
- No second expression/row representation, no `m.add` naming sugar, no macros,
  no change to the MIR-04 L1 or MIR-03 packed seams.
- Broadcasting/reductions beyond the conservative subset keep the typed
  fallback; rule rows are backed by the same packed `RowBlockPlan` seam.
