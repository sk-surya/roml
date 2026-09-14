# MIR-06 Report — Python arrays on the shared IR + decorator rules (in progress)

**Phase:** MIR-06. **Requirements:** IR-27, IR-28. **Branch:** `phase-mir-06`.
**Execution base:** `main@6fc6032`.

## M6-0 Baseline

- `cargo check -p roml-python` (PyO3 0.29, `extension-module`, numpy 0.29)
  compiles locally: `Finished dev profile ... in 19.54s`.
- Current duplicated array IR (migration seed, DESIGN §5):
  - `roml-python/src/arrays.rs`: `VarArray { owner, shape, vars: Vec<VarId>,
    base_name, ordinals: Option<Vec<usize>> }` and `ParamArray { .., params:
    Vec<ParamId> }` gather one identity per element (invariant #4 violation to
    retire); `ExprArrayRepr { Packed, Materialized }` is a second expression IR.
  - `roml-python/src/expressions.rs`: `Expr`, `Comparison`, packed comparison
    machinery.
- Migration target: back these handles with `roml::modeling::{VarView, View,
    LinArray, ParamView}` (MIR-03/04) and route Python rule callbacks through
  the MIR-05 `RuleBatch` accumulator.

## M6-0B — Journal fingerprint contract (complete)

Real Python baseline (built with `maturin build --release --locked`, installed
wheel outside the source tree):

```text
cargo check -p roml-python                              clean (19.5s)
python -m pytest python/tests -q                       151 passed, 4 skipped
python -m pytest python/tests/test_mpc.py -q -rs       4 passed (oracle runs)
```

Frozen contract (IR-25/IR-28):

- `Model::normalized_journal_fingerprint()` (Rust) hashes the ordered
  `ModelOp`s of the retained delta journal, normalizing absolute ids (incl.
  generations) and owners to first-occurrence ordinals and excluding derived
  caches (evaluated values, dependency layouts). The contract covers the packed
  construction ops shared by the Rust/Python BESS formulations; an op outside
  it is a typed `ModelError::JournalContract` (no silent under-approximation).
  The model must be committed first (no partial fingerprint).
- Python exposes `Model.normalized_ordinal_fingerprint()` and
  `Model.normalized_journal_fingerprint()`, flushing pending core changes first.
- Rust tests: identical semantics across owners/ids match; a structural
  difference changes the value; an uncommitted journal is a typed error.
- Python tests: determinism, owner independence, structural sensitivity
  (`python/tests/test_fingerprint.py`; suite now 154 passed, 4 skipped).

## M6-1A — Public shared-handle construction seam (complete)

`Model::var_handle(span, shape)` / `Model::param_handle(span, shape)` wrap an
already-allocated trusted `VarSpan`/`ParamSpan` as a `roml::modeling`
`VarArray`/`ParamArray` (metadata over the span; shape product must equal the
span length). This is the public seam the Python binding will use in M6-1A/B to
hold shared handles instead of `Vec<VarId>`/`Vec<ParamId>` gathers. Tests:
`model::mir06_handle_seam_tests`.

## Status

M6-0B and M6-1A (Rust seam) complete. Planned next: M6-1A/B/C Python side
(`PyVarArray`/`PyParamArray` wrap `roml::modeling` handles; `__getitem__`/
slice/transpose/reshape as `View` transforms; delete `vars`/`params`/`ordinals`
vectors and `param_array_ids`; naming/update differential + grep/LOC invariant
gate), then M6-2A/B (shared `GeneralLinArray` fallback; Python `ExprArray`
wrapper), M6-3 (decorator rules → `RuleBatch`), M6-4 (Rust vs Python BESS
fingerprint + packed-counter gate).


