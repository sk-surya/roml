# MIR-04 Execution Plan — Rust Level-1 ergonomics + label boundary

**Phase:** MIR-04. **Requirements:** IR-24, IR-25. **Depends on:** MIR-03 (merged).
**Branch:** `phase-mir-04`. **Execution base:** `main@d0de5a6969f2cefe790412bab8a7622f97ad75ce`.

**Exit gate:** native Rust BESS, transportation and min-cost-flow examples
contain no raw `VarId`/manual `LinExpr` in ordinary model code; label-typed
formulations produce equal normalized ordinal-IR fingerprints.

Read DESIGN §9 (Rust Level-1 surface) and load
`skills/roml-ir-invariants/SKILL.md` before editing. Ordinary user code must not
handle raw IDs or build `LinExpr` by hand; scalar/raw APIs stay as escape
hatches. Labels are boundary metadata and never enter expression nodes.

## Tasks (TDD, one commit each)

- **M4-1 — Array handles.** `Model::var(name, shape) -> VarArrayBuilder` with
  `.bounds(lo, hi)` / `.kind(..)` / `.build() -> VarArray`; `Model::param(name,
  shape, values) -> ParamArray`. Handles wrap the MIR-03 `VarView`/`ParamView`
  (owner + span + shape + name) and support metadata-only `slice(axis, range)`,
  `transpose`, `reshape`. Per-element names are never materialized in core.
  Tests: allocation counts (one block), slicing is metadata-only, invalid
  shape/axis is typed.

- **M4-2 — Array expression algebra.** `LinArrayExpr` over handles and scalars:
  `Add`/`Sub`/`Mul`/`Div`/`Neg`, scalar and elementwise array `*`, `sum`,
  broadcast where the conservative rule allows; otherwise the expression stays
  correct via the general path. `m.add(expr <= / >= / == bound)` and
  `m.maximize(expr)` / `m.minimize(expr)` commit through the MIR-03 automatic
  eligibility / `add_rows_from_plan` seams. Tests: differential vs the raw/scalar
  construction for a BESS objective and a balance-row block.
  *Delivered:* `Add`/`Sub`/`Neg` and scalar `Mul`/`Div` over
  `VarArray`/`LinArray`/`f64`, cell-wise rows (`le`/`ge`/`eq`/`*_each` →
  `Model::add_row`), leading-axis reduction rows (`rows_eq`/`rows_le`/`rows_ge`
  → `Model::add_rows`), array objectives (`maximize_array`/`minimize_array`),
  and contiguous `reshape`. *Deliberately deferred (amended after review
  round 1):* `m.add`/`m.maximize` naming sugar (use `add_row`/`maximize_array`);
  `sum` as a scalar expression node (the objective's sum is implicit and row
  reductions are `rows_*`, because the single-term-view `LinArray` cannot
  represent many terms per cell); the elementwise `ParamArray * LinArray`
  operator (kept as the fallible `ParamArray::try_mul`, whose conservative rule
  may decline); broadcasting beyond the conservative subset (typed fallback).

- **M4-3 — Label/component boundary (IR-25 prerequisite).** A Rust
  `Labeled<A, Axes>`-equivalent metadata wrapper around ordinal arrays; alignment
  is checked once at the boundary and mismatches are typed errors; labels do not
  enter `ValueExpr`/`LinArray`. Tests: alignment mismatch rejects; label change
  does not change the ordinal IR.

- **M4-4 — Normalized ordinal-IR fingerprint (IR-25).** A deterministic
  fingerprint over the ordinal IR (compiled/canonical ordinals, coefficients,
  topology), explicitly excluding owners, `ModelInstanceId`, absolute IDs and
  labels. Public accessor `Model::normalized_ordinal_fingerprint()`. Tests: two
  models with different names/labels but identical ordinal structure are equal;
  a structural difference changes the fingerprint; owners are not included.

- **M4-5 — Examples + grep gate (IR-24).** Rewrite native Rust BESS,
  transportation and min-cost-flow examples using the L1 surface; add a test
  that greps ordinary example code for `VarId`/`LinExpr` construction and fails
  if present.

- **M4-6 — Solution read-back.** `Solution::array_values(&VarArray)`,
  `Solution::try_array_values(&VarArray)` (strict, model-instance checked), and
  `Solution::array_value(&VarArray, ordinal)` so examples need no raw `VarId`
  for results. (`values` was already a `HashMap` accessor, so the array
  accessors carry the `array_` prefix; synthetic solutions must
  `with_source_instance` to enable strict reads.)

## Amendment (review round 1)

- Reduction and array-objective constants are **per cell**: a scalar constant
  contributes once per cell (row: `n·c`; objective: `N·c`) and a dense constant
  contributes the row/cell sum. Fixed in `builder::shift_bounds`/
  `row_constant`, `Model::add_rows_general`, and both objective paths, with
  RED→GREEN tests in `model::mir04_constant_tests`.
- Structured read-back enforces `array.owner() == solution.metadata.model_instance`;
  a foreign array is a typed `SolutionReadError::CrossModel` on strict reads and
  `None` on lenient reads.
- The planned L1 surface is delivered except the explicit deferrals recorded in
  M4-2/M4-6 above (`m.add`/`m.maximize` naming, `sum` as a scalar node, the
  elementwise `ParamArray * LinArray` operator, and non-conservative
  broadcasting).

## Non-goals
No Python work (MIR-06), no rule builders (MIR-05), no macro DSL, no Pyomo
`AbstractModel`. Scalar/raw APIs remain; MIR-04 adds the vectorized L1 over the
MIR-03 IR only.
