"""MPY-03 bulk-construction probe: bulk array/CSR build vs scalar loops.

Detects accidental Python element loops on the hot path. Prints timings;
the MPY-05 qualification asserts the 3x bulk gate on the 100k fixture.
"""

import time

import numpy as np

import roml as rm


def build_bulk(nvars):
    m = rm.Model()
    x = m.vars("x", nvars, ub=5.0)
    # One sparse row per 10 variables, NumPy bulk inputs.
    nrows = nvars // 10
    indptr = np.arange(0, 10 * nrows + 1, 10, dtype=np.int64)
    indices = (
        np.tile(np.arange(10, dtype=np.int64), nrows)
        + np.repeat(np.arange(nrows, dtype=np.int64) * 10, 10)
    ) % nvars
    data = np.ones(10 * nrows)
    m.add_linear_rows(indptr, indices, data, variables=x, lower=0.0, upper=10.0)
    return m


def build_scalar(nvars):
    m = rm.Model()
    xs = [m.var(f"x{i}", ub=5.0) for i in range(nvars)]
    nrows = nvars // 10
    for r in range(nrows):
        expr = xs[(10 * r) % nvars]
        for k in range(1, 10):
            expr = expr + xs[(10 * r + k) % nvars]
        m.add(expr <= 10.0)
    return m


def timeit(fn, *args):
    start = time.perf_counter()
    result = fn(*args)
    return time.perf_counter() - start, result


def main():
    print(f"{'size':>8} {'bulk(s)':>10} {'scalar(s)':>10} {'speedup':>8}")
    for nvars in (1_000, 10_000, 100_000):
        bulk_t, _ = timeit(build_bulk, nvars)
        scalar_t, _ = timeit(build_scalar, nvars)
        print(f"{nvars:>8} {bulk_t:>10.3f} {scalar_t:>10.3f} {scalar_t / bulk_t:>8.1f}x")


if __name__ == "__main__":
    main()
