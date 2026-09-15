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
  `ModelOp`s of the retained delta journal, mapping absolute ids (incl.
  generations) and owners through the final normalized snapshot ordinal maps
  and excluding derived caches (evaluated values, dependency layouts). The
  contract covers the packed construction ops shared by the Rust/Python BESS
  formulations; an op outside it is a typed `ModelError::JournalContract` (no
  silent under-approximation).
  The model must be committed first (no partial fingerprint).
- Python exposes `Model.normalized_ordinal_fingerprint()` and
  `Model.normalized_journal_fingerprint()`, flushing pending core changes first.
- Rust tests: identical semantics across owners/ids match; a structural
  difference changes the value; an uncommitted journal is a typed error.
- Python tests: determinism, owner independence, structural sensitivity
  (`python/tests/test_fingerprint.py`; suite now 154 passed, 4 skipped).

## M6-1A — Atomic shared-handle construction seam (complete; ownership fixed)

`Model::add_variable_array_block(shape, ty, bounds)` /
`Model::add_parameter_array_block(shape, values)` allocate a structured block and
return its shared `roml::modeling` handle in one atomic operation; the allocating
model stamps its own owner, so no foreign span can be relabeled. Parameter
updates go through the owner-checked `Model::set_parameter_array(&ParamArray,
values)` (packed bulk path for a full-block view, per-member for a sliced view),
which accesses the trusted span internally.

Ownership remediation (review round 1): the first cut exposed
`var_handle(span, shape)` / `param_handle(span, shape)`, which accepted a naked
span and stamped the caller's owner — a span allocated by model A could be
relabeled as model B's. A RED test
(`foreign_span_cannot_be_relabelled_as_this_model`) demonstrated that laundering
succeeded before the fix; the naked-span seam is now removed, so the capability
cannot be expressed in the public API. Tests: `model::mir06_handle_seam_tests`
(atomic ownership; owner-checked update; foreign array rejected; length
mismatch rejected).

## M6-1B/C — Python arrays on the shared handles (complete)

Python `VarArray`/`ParamArray` now wrap shared handles:
```text
PyVarArray   { owner: Py<Model>, inner: roml::modeling::VarArray,   base_name }
PyParamArray { owner: Py<Model>, inner: roml::modeling::ParamArray, base_name }
```
- `Model.vars` allocates via `Model::add_variable_array_block` and returns the
  shared handle; `Model.params` allocates via `add_parameter_array_block` and
  retains the handle in `ModelState.param_arrays` (`name -> ParamArray`) for
  `m.update(name=...)` (which resolves members from the handle; no id vector).
- `__getitem__` lowers `int`/positive-step `slice`/`ellipsis` to per-axis
  `subsample`/`squeeze` metadata transforms (`normalize_index` returns
  `AxisSelection`s, not a flat position vector). Scalar results resolve through
  the squeezed view; the root ordinal for naming comes from
  `inner.view().view().get(0)`.
- Deleted: `VarArray.{vars, ordinals}`, `ParamArray.{params, ordinals}`,
  `ModelState.param_array_ids`. No cached `shape` field — `dims()` derives it
  from `inner.shape()`.
- Materialized expression/comparison/constraint arrays still need flat
  positions to gather their own `Vec<Affine>`/`Vec<ConId>`; that is M6-2 work.

Evidence:
- Full release wheel (`maturin build --release --locked`, installed outside the
  source tree) + `pytest python/tests -q` — **157 passed, 4 skipped** (baseline
  was 151 passed, 4 skipped; +3 gate/fingerprint tests, same skips).
- `python/tests/test_ir27_gate.py`: no `pub vars:`/`pub params:`/`pub
  ordinals`/`param_array_ids` in `arrays.rs`/`model.rs`/`solution.rs`; Python
  arrays declare `inner: roml::modeling::{VarArray,ParamArray}`.
- `python/tests/test_fingerprint.py`: array-built and explicit scalar-built
  models produce equal `normalized_ordinal_fingerprint()` (construction
  differential); fingerprints stay deterministic and structure-sensitive.
- `tests/mir06_view_subsample.rs`: rank-0 integer path, chained subsampling
  root mapping, empty selection.

## Status

Complete: M6-0B, M6-1A, M6-1B/C. The gathered-ID layer is deleted; Python arrays
are views over the shared IR.

**Review checkpoint before M6-2** (per owner): deleting the gathered-ID layer
was the biggest IR-27 transition; M6-2A/B (shared `GeneralLinArray` fallback +
Python `ExprArray` wrapper) starts only after that checkpoint.


