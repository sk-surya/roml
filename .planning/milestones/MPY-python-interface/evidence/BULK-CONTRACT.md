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

Definitions (B_s and B_b are derived by subtraction):

- T_s, T_b: measured end-to-end wall times of the scalar and bulk
  construction arms on the same fixture (medians of 5 repeats,
  release builds, quiet host).
- C: independently measured core entity-insertion cost for the same
  entity counts (`examples/sync_cost_probe.rs`: 100k `add_variable`
  + 10k 10-term `add_constraint`, median of 5, release). C is the
  work both arms must perform identically.
- B_s = T_s - C (scalar binding overhead: per-element calls,
  expression building, namespace formatting).
- B_b = T_b - C (bulk binding overhead: input parsing, two extension
  calls, namespace reservation).
- Because B_s and B_b are differences of medians, their variability
  compounds: with per-arm repeat std ~3-5%, the B_s/B_b ratio carries
  roughly ±10% run-to-run on this host. The gate threshold (2x) sits
  well below the measured 2.9x; re-measure on gate disputes.

Decomposition (release medians):

- Identical fraction f = C / T_s = 0.0406 / 0.076 = **0.53**.
- Amdahl bound: no implementation can exceed 1/f = **1.87x**
  end-to-end on this fixture shape. Measured 1.43x is consistent
  (0.53 identical + eliminable remainder).
- Superseded evidence (retained for the record, DO NOT USE): an
  earlier "85% identical" estimate measured on `maturin develop`
  (debug) builds, which run validation-heavy paths ~10x slower and
  inflate binding overhead, distorting every ratio (vars 3.1x, rows
  2.3x on debug). Those debug numbers are explicitly superseded by
  the release measurements in this document.
- Eliminable binding work: B_s = 0.0354, B_b = 0.0124.
  **Bulk benefit on eliminable work: B_s / B_b = 2.9x.**
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
