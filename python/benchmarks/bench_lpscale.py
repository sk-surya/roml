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

    rng = np.random.default_rng(args.seed)
    streams = [50.0 + 10.0 * rng.standard_normal((BATTERIES, PERIODS)) for _ in range(args.steps)]
    m, price, charge, discharge, energy = build()
    solver = rm.Highs(threads=1, time_limit=30.0)
    # Warmup.
    for k in range(3):
        m.update(price=streams[k % args.steps])
        solver.solve(m)
    nonsolve, total = [], []
    for _ in range(args.repeats):
        t_nonsolve = 0.0
        t0 = time.monotonic()
        for k in range(args.steps):
            u0 = time.monotonic()
            m.update(price=streams[k])
            r = solver.solve(m)
            assert r.has_primal
            _ = r.values(discharge)
            t_nonsolve += time.monotonic() - u0 - 0.0
            # Non-solve approximated below with dedicated split runs.
        total.append((time.monotonic() - t0) / args.steps * 1000.0)
    # Dedicated split: update+extract vs solve on one gate.
    m.update(price=streams[0])
    u0 = time.monotonic()
    m.update(price=streams[1])
    u1 = time.monotonic()
    r = solver.solve(m)
    s1 = time.monotonic()
    _ = r.values(discharge)
    e1 = time.monotonic()
    solver.close()
    report = {
        "workload": "lp-scale",
        "shape": [BATTERIES, PERIODS],
        "cells_per_gate": BATTERIES * PERIODS,
        "steps": args.steps,
        "repeats": args.repeats,
        "end_to_end_ms_per_gate": {
            "p50": float(np.percentile(total, 50)),
            "p95": float(np.percentile(total, 95)),
        },
        "nonsolve_ms_per_gate": {
            "update": (u1 - u0) * 1000.0,
            "extract": (e1 - s1) * 1000.0,
            "solve": (s1 - u1) * 1000.0,
        },
        "config": {
            "os": platform.platform(),
            "python": platform.python_version(),
            "numpy": np.__version__,
            "roml": rm.__version__,
        },
    }
    with open(args.out, "w") as f:
        json.dump(report, f, indent=1)
    print(json.dumps(report["end_to_end_ms_per_gate"], indent=1))
    print(json.dumps(report["nonsolve_ms_per_gate"], indent=1))


if __name__ == "__main__":
    main()
