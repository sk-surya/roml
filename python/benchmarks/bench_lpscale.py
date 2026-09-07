"""LP-scale workload (QUALIFICATION): 96 periods x 100 batteries.

Shaped (100, 96) arrays, affine balances and bounds, 9,600 price cells
updated per gate. Measures non-solve (update + extract) vs native solve
split for the PY-27 gate. Usage: --steps N --repeats R --out PATH.
"""

from __future__ import annotations

import argparse
import json
import os
import platform
import sys
import time

import numpy as np

import roml as rm

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from benchmarks.fixtures import DT, EFF, ENERGY_CAP, POWER

PERIODS = 96
BATTERIES = 100


def build():
    m = rm.Model("lp-scale")
    shape = (BATTERIES, PERIODS)
    price = m.params("price", np.full(shape, 50.0))
    charge = m.vars("charge", shape, ub=POWER)
    discharge = m.vars("discharge", shape, ub=POWER)
    energy = m.vars("energy", (BATTERIES, PERIODS + 1), ub=ENERGY_CAP)
    m.add(energy[:, 0] == 2.0)
    m.add(energy[:, 1:] == energy[:, :-1] + DT * (EFF * charge - discharge / EFF))
    m.add(charge + discharge <= POWER)
    m.maximize(DT * rm.sum(price * (discharge - charge)))
    return m, price, charge, discharge, energy


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--steps", type=int, default=20)
    parser.add_argument("--repeats", type=int, default=10)
    parser.add_argument("--out", required=True)
    parser.add_argument("--seed", type=int, default=20260907)
    args = parser.parse_args()

    def peak_rss_mib():
        with open("/proc/self/status") as f:
            for line in f:
                if line.startswith("VmHWM:"):
                    return float(line.split()[1]) / 1024.0
        return 0.0

    rng = np.random.default_rng(args.seed)
    streams = [50.0 + 10.0 * rng.standard_normal((BATTERIES, PERIODS)) for _ in range(args.steps)]
    m, price, charge, discharge, energy = build()
    solver = rm.Highs(threads=1, time_limit=30.0)
    # Warmup: 30 gates per protocol (not recorded).
    for k in range(30):
        m.update(price=streams[k % args.steps])
        solver.solve(m)
    m.update(price=streams[0])
    highs_version = solver.solve(m).metadata["backend"]
    total, updates, solves, extracts = [], [], [], []
    for _ in range(args.repeats):
        t0 = time.monotonic()
        rep_update, rep_solve, rep_extract = 0.0, 0.0, 0.0
        for k in range(args.steps):
            u0 = time.monotonic()
            m.update(price=streams[k])
            u1 = time.monotonic()
            r = solver.solve(m)
            s1 = time.monotonic()
            assert r.has_primal
            _ = r.values(discharge)
            e1 = time.monotonic()
            rep_update += u1 - u0
            rep_solve += s1 - u1
            rep_extract += e1 - s1
        total.append((time.monotonic() - t0) / args.steps * 1000.0)
        updates.append(rep_update / args.steps * 1000.0)
        solves.append(rep_solve / args.steps * 1000.0)
        extracts.append(rep_extract / args.steps * 1000.0)
    peak = peak_rss_mib()
    solver.close()
    def pct(xs, q):
        return float(np.percentile(xs, q))

    report = {
        "workload": "lp-scale",
        "shape": [BATTERIES, PERIODS],
        "cells_per_gate": BATTERIES * PERIODS,
        "steps": args.steps,
        "repeats": args.repeats,
        "end_to_end_ms_per_gate": {"p50": pct(total, 50), "p95": pct(total, 95), "p99": pct(total, 99)},
        "nonsolve_ms_per_gate": {
            "update": {"p50": pct(updates, 50), "p95": pct(updates, 95), "p99": pct(updates, 99)},
            "solve": {"p50": pct(solves, 50), "p95": pct(solves, 95), "p99": pct(solves, 99)},
            "extract": {"p50": pct(extracts, 50), "p95": pct(extracts, 95), "p99": pct(extracts, 99)},
        },
        "peak_rss_mib": peak,
        "config": {
            "os": platform.platform(),
            "python": platform.python_version(),
            "numpy": np.__version__,
            "roml": rm.__version__,
            "highs": highs_version,
            "threads": 1,
        },
    }
    with open(args.out, "w") as f:
        json.dump(report, f, indent=1)
    print(json.dumps(report["end_to_end_ms_per_gate"], indent=1))
    print(json.dumps(report["nonsolve_ms_per_gate"], indent=1))


if __name__ == "__main__":
    main()
