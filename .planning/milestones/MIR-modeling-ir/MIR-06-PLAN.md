# MIR-06 Execution Plan — Python arrays on the shared IR + decorator rules

**Phase:** MIR-06. **Requirements:** IR-27, IR-28. **Depends on:** MIR-03,
MIR-04, MIR-05 (merged). **Branch:** `phase-mir-06`. **Execution base:**
`main@6fc6032`.

**Exit gate:**
- **IR-27:** Python `VarArray`/`ParamArray`/expression arrays are backed by the
  shared `roml::modeling` IR (`View<VarSpan>`/`View<ParamSpan>`, `LinArray`);
  no duplicated permanent per-element array IR remains. Current Python L1/flagship
  suites still pass.
- **IR-28:** equivalent Rust and Python BESS formulations produce equal
  **normalized ordinal-IR fingerprints** (`Model::normalized_ordinal_fingerprint`,
  MIR-04) and equal semantic-journal fingerprints.

Read DESIGN §8–§10 and load `skills/roml-ir-invariants/SKILL.md` before editing.
The invariants that bind here are #4 (views are metadata, never gathered
`Vec<VarId>`/`Vec<ParamId>`), #3 (symbolic arrays are model-owned), #18/#19
(covered families keep packed/scale representation, no per-cell `ValueExpr`
materialization), and #20 (rule/decorator APIs accumulate CSR and bulk-commit
once).

## Current state (baseline to measure in M6-0)
`roml-python/src/arrays.rs` currently stores a gathered identity vector per
array (`VarArray { vars: Vec<VarId>, .. }`, `ParamArray { params: Vec<ParamId> }`)
plus a parallel `ordinals: Option<Vec<usize>>` for naming — a duplicated
per-element array IR. `roml-python/src/expressions.rs` maintains a separate
`ExprArrayRepr`/packed representation. These are the migration seeds named in
DESIGN §5, not a permanent IR.

## Tasks (TDD, one commit each)

- **M6-0 — Baseline + harness.** Build/test the Python crate locally
  (`cargo check -p roml-python`, `maturin develop`/pytest as available);
  characterize the current array IR, the Python BESS lowering path, and the
  existing Python suites that must stay green. Record in
  `evidence/MIR-06-REPORT.md` (initial).
- **M6-1 — Python variable/parameter arrays wrap shared views.**
  `VarArray` holds `(owner: Py<Model>, view: roml::modeling::VarView,
  base_name)`; `ParamArray` holds the `ParamView` equivalent. `__getitem__`,
  reshape, transpose, broadcast and reductions become metadata operations over
  `View` (O(1) in cells); scalar handles resolve through `VarView::member`.
  No `Vec<VarId>`/`Vec<ParamId>` gather remains on the array path (invariant
  #4). All existing Python array tests stay green.
- **M6-2 — Python expression arrays wrap `LinArray`.** `ExprArray` holds a
  `roml::modeling::LinArray` (packed `CoeffView` families) plus the same
  conservative fast/fallback rule as Rust (`ParamView * LinArray`), so Python
  and Rust share one coefficient IR. General path only when the fast IR is not
  covered.
- **M6-3 — Python decorator rules over the MIR-05 accumulator.** A decorator /
  rule-collection API runs the Python callback once per index to *construct*
  rows into `RuleBatch`, then bulk-commits once. Gate:
  `rule_bulk_commits == 1` and one `BulkMixedRows` for N rows (no per-row core
  hash/journal work).
- **M6-4 — Cross-language fingerprint fixture (IR-28).** A Rust BESS
  formulation and the equivalent Python formulation produce the same
  `normalized_ordinal_fingerprint()` and the same semantic-journal fingerprint.
  Fixture emitted to `evidence/`.
- **M6-5 — Report, docs, policy.** `evidence/MIR-06-REPORT.md`; `CHANGELOG.md`;
  `STATE.md`.

## Non-goals (MIR-07)
`ConcreteModel`, `Set`/`RangeSet`, NumPy ingestion, pandas adapters, and
`Template::bind` ergonomics. MIR-06 migrates the existing Python array/expression
surface onto the shared IR and adds decorator rules; it does not add new OO
ergonomics.

## Risks
- The Python array surface is ~5k LOC; migration must keep every existing
  Python test green and preserve error/typing behavior.
- Fingerprint equality (IR-28) requires the Python formulation to lower through
  the same packed seams, not a parallel path — the differential fixture is the
  gate.
