"""MPY-05 memory soak: 10k update/solve cycles on LP-small.

Discards all but one result per cycle. Reports RSS after warmup and at
the end (with explicit collection at boundaries). Pass bar: retained
growth <= max(32 MiB, 10% of post-warmup RSS).
"""

import gc
import os
import resource
import time

import numpy as np

import roml as rm


def rss_mib():
    # Current resident set (ru_maxrss is a monotonic high-water mark and
    # cannot observe releases).
    with open("/proc/self/status") as f:
        for line in f:
            if line.startswith("VmRSS:"):
                return float(line.split()[1]) / 1024.0
    raise RuntimeError("cannot read VmRSS")


def main(cycles=10_000, warmup=1_000):
    m = rm.Model("soak")
    price = m.params("price", np.full(24, 50.0))
    charge = m.vars("charge", 24, ub=2.0)
    discharge = m.vars("discharge", 24, ub=2.0)
    energy = m.vars("energy", 25, ub=4.0)
    m.add(energy[0] == 2.0)
    m.add(energy[1:] == energy[:-1] + 0.25 * (0.95 * charge - discharge / 0.95))
    m.add(charge + discharge <= 2.0)
    m.maximize(0.25 * rm.dot(price, discharge - charge) + 30.0 * energy[-1])
    rng = np.random.default_rng(20260907)
    with rm.Highs(threads=1) as solver:
        for k in range(warmup):
            m.update(price=50.0 + 10.0 * rng.standard_normal(24))
            solver.solve(m)
        gc.collect()
        base = rss_mib()
        t0 = time.monotonic()
        for k in range(cycles):
            m.update(price=50.0 + 10.0 * rng.standard_normal(24))
            result = solver.solve(m)
            assert result.has_primal
            del result
        gc.collect()
        final = rss_mib()
    elapsed = time.monotonic() - t0
    growth = final - base
    allowance = max(32.0, 0.10 * base)
    print(f"post-warmup RSS: {base:.1f} MiB")
    print(f"final RSS:       {final:.1f} MiB")
    print(f"growth: {growth:.1f} MiB (allowance {allowance:.1f} MiB)")
    print(f"{cycles} cycles in {elapsed:.1f}s ({elapsed / cycles * 1000:.2f} ms/cycle)")
    print("LEAK CHECK:", "PASS" if growth <= allowance else "FAIL")


if __name__ == "__main__":
    main()
