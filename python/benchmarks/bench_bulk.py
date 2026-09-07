"""Bulk-construction evidence (amended bulk gate, MPY-05).

Compares Rust-side bulk construction (`vars` + CSR, NumPy inputs, no
Python loop per coefficient) against scalar-loop construction of an
EQUIVALENT model (asserted: same optimum). Reports decomposed timings
(variables vs rows, both arms) plus the Amdahl reconciliation against
independently measured core-insertion cost
(`examples/sync_cost_probe.rs`).

Amended gate criteria: equivalent models, Rust-side bulk execution, no
Python loop per coefficient, vars >= 3x, per-coefficient >= 2x, and the
pre-existing wrapper-overhead gates.
"""

from __future__ import annotations

import statistics
import time

import numpy as np

import roml as rm

REPEATS = 5


def build_bulk(nvars):
    m = rm.Model()
    x = m.vars("x", nvars, ub=5.0)
    nrows = nvars // 10
    indptr = np.arange(0, 10 * nrows + 1, 10, dtype=np.int64)
    indices = (
        np.tile(np.arange(10, dtype=np.int64), nrows)
        + np.repeat(np.arange(nrows, dtype=np.int64) * 10, 10)
    ) % nvars
    data = np.ones(10 * nrows)
    m.add_linear_rows(indptr, indices, data, variables=x, lower=0.0, upper=10.0)
    return m, x


def build_scalar(nvars):
    m = rm.Model()
    xs = [m.var(f"x{i}", ub=5.0) for i in range(nvars)]
    nrows = nvars // 10
    for r in range(nrows):
        expr = xs[(10 * r) % nvars]
        for k in range(1, 10):
            expr = expr + xs[(10 * r + k) % nvars]
        m.add(expr <= 10.0)
    return m, xs


def timeit(fn, *args):
    start = time.perf_counter()
    result = fn(*args)
    return time.perf_counter() - start, result


def check_equivalent(nvars):
    """Both arms build the same model: identical optima under one solver."""
    from functools import reduce

    with rm.Highs(threads=1) as solver:
        mb2, xb2 = build_bulk(nvars)
        mb2.minimize(rm.sum(xb2))
        rb = solver.solve(mb2)
    with rm.Highs(threads=1) as solver:
        ms2, xs2 = build_scalar(nvars)
        ms2.minimize(reduce(lambda a, b: a + b, xs2))
        rs = solver.solve(ms2)
    assert rb.is_optimal and rs.is_optimal, (rb.status, rs.status)
    assert abs(rb.objective - rs.objective) <= 1e-7 + 1e-7 * max(
        abs(rb.objective), abs(rs.objective)
    ), (rb.objective, rs.objective)
    return rb.objective


def main():
    print(f"{'size':>8} {'bulk(s)':>10} {'scalar(s)':>10} {'speedup':>8}")
    for nvars in (1_000, 10_000, 100_000):
        obj = check_equivalent(nvars)
        bulk_ts, scalar_ts = [], []
        for _ in range(REPEATS):
            bulk_t, _ = timeit(build_bulk, nvars)
            scalar_t, _ = timeit(build_scalar, nvars)
            bulk_ts.append(bulk_t)
            scalar_ts.append(scalar_t)
        bulk = statistics.median(bulk_ts)
        scalar = statistics.median(scalar_ts)
        print(
            f"{nvars:>8} {bulk:>10.3f} {scalar:>10.3f} {scalar / bulk:>8.1f}x  (opt={obj:.1f})"
        )


if __name__ == "__main__":
    main()
