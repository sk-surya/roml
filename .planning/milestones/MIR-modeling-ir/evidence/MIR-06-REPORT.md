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

## Status

Planning complete (`MIR-06-PLAN.md`). Implementation of M6-1 (Python arrays
wrap shared views) is the next step; see the plan for tasks and gates.
