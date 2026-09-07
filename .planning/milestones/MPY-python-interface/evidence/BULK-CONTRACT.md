# Bulk Performance Contract (amended, evidence-backed)

Supersedes the universal "bulk end-to-end >= 3x" threshold, which gated
physics rather than code (see reconciliation below). Owner-accepted
direction 2026-09-07 with the conditions recorded here.

## Corrected cost breakdown (release builds, quiet host, 100k fixture)

Raw medians, 5 repeats (`bench_bulk.py` + `examples/sync_cost_probe.rs`):

| Component | Time (s) |
|---|---|
| Core inserts, 100k vars (`add_variable`) | 0.0061 |
| Core inserts, 10k x 10-coef rows (`add_constraint`) | 0.0345 |
| **Identical core work C (both arms)** | **0.0406** |
| Scalar arm end-to-end T_s | 0.076 |
| Bulk arm end-to-end T_b | 0.053 |

Decomposition:

- Identical fraction f = C / T_s = 0.0406 / 0.076 = **0.53**.
- Amdahl bound: no implementation can exceed 1/f = **1.87x**
  end-to-end on this fixture shape. Measured 1.43x is consistent
  (0.53 identical + eliminable remainder).
- The earlier "85% identical" estimate was a debug-build artifact
  (`maturin develop` runs validation-heavy paths ~10x slower,
  inflating binding overhead and distorting every ratio). It is
  withdrawn and replaced by the release-measured 53%.
- Eliminable binding work: B_s = T_s - C = 0.0354,
  B_b = T_b - C = 0.0124. **Bulk benefit on eliminable work:
  B_s / B_b = 2.9x.**
- Sub-rates (release, reported not gated): vars 1.19x (both arms
  dominated by name registration + hashing, Rust slightly faster),
  rows 1.83x (bulk binding adds ~0.03 us/coef over the 0.35 us/coef
  core cost; scalar adds ~0.35 us/coef binding).

## Amended gate (pass/fail)

1. **Equivalent models** (asserted in-harness): same entity counts
   and identical optima under one solver.
2. **Rust-side bulk execution**: construction via `vars` +
   `add_linear_rows` (array/CSR APIs); inputs as NumPy arrays with
   **no Python loop per coefficient** in the bulk arm.
3. **Eliminable-work benefit B_s / B_b >= 2x** on the 100k fixture
   (measured 2.9x). This gates what bulk construction controls.
4. **Recorded (not gated)**: end-to-end ratio (~1.4x, Amdahl-capped
   at ~1.9x), vars sub-rate, rows sub-rate, raw timings above.
5. **Unchanged**: the pre-existing wrapper-overhead gates (PY-27 on
   LP-scale non-solve, PY-28 on LP-small end-to-end).

No core batch-insertion API was added to chase any threshold.
