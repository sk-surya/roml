# MPY Performance Notes (living; finalized in MPY-05)

## MPY-03 bulk construction (2026-09-07, Linux x86_64, 100k-coefficient fixture)

Decomposition (seconds, 100k vars + 10k rows x 10 coefs):

| Arm | vars | rows | total |
|---|---|---|---|
| Bulk (`vars` + CSR, NumPy inputs) | 0.052 | 0.371 | ~0.43 |
| Scalar loop (`m.var` + expr `m.add`) | 0.162 | ~0.38 | ~0.55 |

Findings:

- Bulk `vars()` is 3.1x faster than the per-element `m.var` loop.
- Bulk CSR matches or beats scalar adds per row (5us vs 6us at 1 coef/row).
- Per-coefficient rate: ~3.7us bulk vs ~8.5us scalar (2.3x).
- End-to-end fixture ratio is ~1.3x because ~85% of wall time is identical
  core entity-insertion work (changelog, validation, coefficient index) in
  both arms — not interpreter overhead.
- No Python element loop exists on the bulk path (2 extension calls build
  100k variables; 1 call inserts 100k CSR coefficients).

## MPY-05 gate risk

QUALIFICATION requires >=3x end-to-end on this fixed fixture shape. With
core per-entity costs identical in both arms, 3x is unachievable by wrapper
work alone; the gap is a fixture/threshold property, not a wrapper defect.
Carried as an at-risk item into MPY-05 with this mechanism documented. No
speculative core batch API is added to chase it.
