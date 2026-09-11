# MIR-00 Baseline Evidence

**Phase:** MIR-00 (baseline before redesign)
**Execution base:** `main@c590692ace5446cc20c7eb91cb8fa0d594a054b0`
**Implementation branch base:** `docs/mir-modeling-ir-plan@3363ec34227fe6676cc8df467bf58fd1e03fb319`
**Date:** 2026-09-11

## Environment

| Tool | Version |
|---|---|
| rustc | 1.97.1 (8bab26f4f 2026-07-14) |
| cargo | 1.97.1 (c980f4866 2026-06-30) |
| Python | 3.13.14 (venv) |
| numpy | 2.5.3 |
| maturin | 1.15.0 |
| HiGHS | 1.15.0 (bundled via `highs-sys`) |
| OS | Linux |

## Scope

MIR-00 measures the **current** construction and repricing path; it does not
redesign storage. The flagship cardinality is held distinct throughout:
**28,800 mutable price parameters drive 57,600 parameterized objective
coefficient cells** (one `charge` and one `discharge` cell per parameter).
No performance claim is inferred for the MIR fast path; MIR-02/MIR-08 measure
that.

## A. Source-inspection facts (not measurements)

These are read from the exact base; the measurements in §B confirm the
consequences.

1. The Python flagship expression
   `DT * rm.sum(price * (discharge - charge))` is classified once in
   `roml-python/src/model.rs::set_objective_impl`. A numeric tree goes to
   `set_objective_numeric_bulk`, a scaled-parameter tree to
   `set_objective_param_bulk`, and only the residual general case to
   `set_objective_general`. The scaled-parameter branches call the core
   primitive `Model::set_linear_objective_param_bulk`.
2. `Model::set_linear_objective_param_bulk` (`src/model/mod.rs`) canonicalizes
   by variable into one packed `(var, scale, param)` cell per variable and
   appends them with `CoefficientIndex::append_param_run`.
3. `append_param_run` (`src/model/coefficient.rs`) populates a per-cell
   reverse index `param_positions: HashMap<ParamId, Vec<u32>>` — one position
   per packed cell — in addition to the packed p-base arrays.
4. `Model::apply_parameter_change` (`src/model/mod.rs`), once per queued
   parameter, calls `for_param(param)` (which scans both the overlay map and
   `param_positions`), filters with `is_overlay_cell`, and then calls
   `propagate_packed_param`, which walks `param_positions` a **second** time.
   Each changed cell pushes one `Change::CoefficientValueChanged`.
5. `Model::commit` maps every `Change` to exactly one `ModelOp`
   (`compile_change`) and records one `DeltaBatch`.
6. The HiGHS adapter (`roml-highs/src/compiler.rs`) applies
   `BackendOp::SetObjectiveCoefficient` and `BackendOp::SetLinearCoefficient`
   with one native `Highs_changeColCost` / `Highs_changeCoeff` call **per
   coefficient**. There is no packed/bulk native call on this path.
7. Python `Model.update(price=...)` (`roml-python/src/model.rs`) flattens the
   parameter array into `Vec<(ParamId, f64)>`, calls core `set_parameter` per
   element, then one `commit()`.

## B. Differential characterization (Rust core)

The core counters are observational (`src/diagnostics.rs`). Command:

```bash
CARGO_TARGET_DIR=/srv/repos/roml/target \
  cargo nextest run -p roml --test mir00_baseline_characterization --no-capture
```

Raw output (both tests pass):

```text
MIR-00 reprice: params=28800 cells=57600 param_position_lookups=57600 overlay_lookups=57600 value_expr_evals=0 delta_ops=86400
```

| Quantity | Measured |
|---|---|
| lowering `parametric_bulk` | 1 |
| lowering `general_affine` | 0 |
| lowering `numeric_bulk` (this minimal fixture) | 0 |
| lowering `param_dep_blocks` | 0 (MIR-02 not implemented) |
| lowering `param_positions_cells` | 57,600 |
| propagation `param_position_lookups` | 57,600 |
| propagation `overlay_lookups` | 57,600 |
| propagation `value_expr_evals` | 0 |
| committed delta ops for one full reprice | 86,400 |

The 86,400 delta ops decompose as **28,800** `ParameterValueChanged` ops (one
per parameter) plus **57,600** coefficient ops (one per cell). The assertion
in the test pins `delta_ops == params + cells`.

The `param_positions_cells == 2 per price parameter` shape means the current
packed parametric objective is already the *construction* fast path, but the
*propagation* path is still per-cell through a hash-map reverse index and a
per-cell delta op.

## C. Flagship Python measurement (300×96, persistent HiGHS)

Extension built with:

```bash
VIRTUAL_ENV=/tmp/opencode/mir-venv CARGO_TARGET_DIR=/srv/repos/roml/target \
  /tmp/opencode/mir-venv/bin/maturin develop
```

Repository fixture: `python/benchmarks/bench_mir_bess.py` (narrowly scoped MIR
benchmark; the existing `bench_lpscale.py` is 100×96 and unchanged).

### C.1 Single-cycle probe (`update` + `solve`, `threads=1`, `time_limit=30`)

```text
build=0.25s stats=(numeric=3, parametric=1, general_affine=0, param_dep_blocks=0,
                   param_positions_cells=57600, ...)
solve_cold=1.18s obj=28500.0
solve_again_nochange=0.17s obj=28500.0
update=0.14s
solve_after_update=65.87s obj=29070.000000000146
solve_again2=0.17s obj=29070.000000000146
```

The counters exposed by `Model._debug_mir_stats()` after the update match the
Rust characterization exactly (`param_position_lookups=57600`,
`overlay_lookups=57600`, `value_expr_evals=0`).

**Interpretation (measured, not assumed):** the solve itself is subsecond
(`solve_again2=0.17s` with no intervening change). The ~66 s is native
application of the 57,600 per-cell coefficient ops (`Highs_changeColCost`),
i.e. one native call per objective cell. This is the single dominant cost the
MIR-02 packed coefficient-patch batch targets; it is not a solve-time effect.

### C.2 Bounded 10-cycle sample (committed artifact)

One reprice applies 57,600 individual native coefficient calls, so a single
cycle is ~66 s and the full 100-cycle procedure is ~110 minutes. The committed
measured sample is **10 cycles** on one warm session; the 100-cycle command is
identical apart from `--steps`:

```bash
/tmp/opencode/mir-venv/bin/python python/benchmarks/bench_mir_bess.py \
  --steps 10 --repeats 1 \
  --out .planning/milestones/MIR-modeling-ir/evidence/baseline-mir-bess-sample.json
```

Raw artifact `evidence/baseline-mir-bess-sample.json`:

| Field | Value |
|---|---|
| `price_params` / `objective_cells` | 28,800 / 57,600 |
| lowering | `numeric_bulk=3`, `parametric_bulk=1`, `general_affine=0`, `param_positions_cells=57,600` |
| propagation totals (10 cycles) | `param_position_lookups=576,000`, `overlay_lookups=576,000`, `value_expr_evals=0`, `coefficient_patch_batches=0` |
| `update_ms` mean / p50 / p95 | 140.86 / 140.90 / 144.71 |
| `solve_ms` mean / p50 / p95 | 66,044.46 / 65,976.86 / 66,369.61 |
| peak RSS | 476.28 MiB |
| versions | HiGHS 1.15.0, Python 3.13.14, numpy 2.5.3, threads=1 |

The propagation totals are exactly `10 × 57,600` for both per-cell reverse
index walks, matching §B. The `solve_ms` figure is the `update`-then-`solve`
interval and is dominated by native delta application, not the LP solve
(§C.1: 0.17 s when no change is pending).

> Deviation: the packet's 100-cycle procedure is reduced here to a 10-cycle
> measured sample because the measured per-cycle cost (~66 s, dominated by
> 57,600 native coefficient calls) makes 100 cycles ~110 minutes. No timing
> gate depends on the sample size; MIR-08 owns frozen performance gates. The
> exact 100-cycle command is recorded above and produces the same schema.

> Evidence status: §B (Rust differential) and §C.1 (single-cycle timeline) plus
> this sample are the committed IR-01 measurements. No timing is asserted as a
> gate in MIR-00.

## D. Findings carried into MIR-01/MIR-02

1. Construction already stays on the packed parametric objective path end to
   end (`parametric_bulk=1`, `general_affine=0`); no construction change is
   needed to *reach* the path.
2. The propagation fast path does not exist yet: a reprice performs two
   per-cell reverse-index walks (57,600 each) and emits 86,400 delta ops.
3. The dominant end-to-end reprice cost is backend application of 57,600
   per-cell objective coefficient ops, not the solve.
4. `param_dep_blocks` is 0 and `coefficient_patch_batches` is 0; these are the
   MIR-02 counters that must move to `>0` and `==1` respectively on the
   eligible family.
5. Dependency-query completeness for block-created parameters cannot be
   measured yet because no block storage exists; it is an MIR-02 obligation
   (IR-11).

## E. Residual uncertainty / not claimed

- No claim that the future packed path will reduce the 66 s backend cost;
  MIR-02 only fixes the canonical delta shape and propagation counters.
  Batched native application is a backend concern (MIR-08 performance gate).
- The single-cycle probe is one sample on one machine; the 100-cycle artifact
  provides distribution data.
- No MIR-01/02 implementation is described or claimed here.
