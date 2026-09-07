# M3 Performance (P34 Task 34-06)

**Fixture:** `P34_PRIMITIVE_PARAMETER_UPDATE_V1` (`tools/p34-perf/`).
512 continuous variables `[0, 100]`; 512 `>=` rows; 8 deterministic nonzeros
per row (4096 nnz, xorshift seed `0x524F4D4C`); one scalar parameter combined
into 64 coefficient cells; deterministic positive linear minimization;
bundled HiGHS, output off, threads 1; 20 warmups + 200 measured attempts in
`--release` mode. Same harness source builds unmodified on both arms with
bit-identical mathematics (no adapter patch required).

**Machine:** 32 CPUs, 59 GiB RAM, Linux 7.0.0-30-generic x86_64.
**Toolchain:** rustc/cargo 1.97.1. **HiGHS:** bundled 1.15.0 (both arms).
**Arms run sequentially** on the same machine (no contention).

## Results

| Arm | Head | Median | p25 | p75 | Initial objective | Sync |
|---|---|---|---|---|---|---|
| Baseline | `4d111cc` (pre-M3 semantic implementation) | 9.409397 ms | 7.259204 ms | 11.641744 ms | 1284.736622 | Delta 219, NoChange 1 |
| Candidate | P34 branch (post-P31) | 9.596605 ms | 7.410067 ms | 11.851386 ms | 1284.736622 | Delta 219, NoChange 1 |

Raw JSON retained with the P34 evidence set (`p34-perf-baseline.json`,
`p34-perf-candidate.json`).

## Gate

```text
candidate median - baseline median <= max(5% of baseline median, 50 microseconds)
0.187208 ms <= max(0.470470 ms, 0.050 ms) = 0.470470 ms  →  PASS (+1.99%)
```

Identical initial objectives and identical synchronization classification on
both arms confirm the same mathematical workload. No profiler exception was
needed.

**SM-15.5: PASS.**
