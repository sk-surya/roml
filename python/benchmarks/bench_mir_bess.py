"""MIR-00 baseline: BESS 300x96 repricing through the *current* Python path.

Flagship cardinality (DESIGN §QUALIFICATION): 300 batteries x 96 periods =
28,800 `price` parameters driving 57,600 parameterized objective cells (one
charge + one discharge cell per parameter). This fixture measures the current
route and per-cycle reprice cost before any MIR-02 storage redesign.

`Model._debug_mir_stats()` is a debug-only read-only counter probe (absent in
release wheels); the fixture skips counter capture when it is unavailable and
still reports wall-time.

Usage: --steps N --repeats R --out PATH.
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

BATTERIES = 300
PERIODS = 96
PARAMS = BATTERIES * PERIODS
CELLS = 2 * PARAMS

COUNTER_NAMES = (
    "numeric_bulk",
    "parametric_bulk",
    "general_affine",
    "param_dep_blocks",
    "param_positions_cells",
    "param_position_lookups",
    "overlay_lookups",
    "value_expr_evals",
    "coefficient_patch_batches",
)


def has_counters(model: rm.Model) -> bool:
    return hasattr(model, "_debug_mir_stats")


def counters(model: rm.Model) -> dict[str, int]:
    return dict(model._debug_mir_stats())


def build():
    m = rm.Model("mir-bess")
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
    parser.add_argument("--steps", type=int, default=100)
    parser.add_argument("--repeats", type=int, default=1)
    parser.add_argument("--out", required=True)
    parser.add_argument("--seed", type=int, default=20260911)
    args = parser.parse_args()

    def peak_rss_mib():
        with open("/proc/self/status") as f:
            for line in f:
                if line.startswith("VmHWM:"):
                    return float(line.split()[1]) / 1024.0
        return 0.0

    rng = np.random.default_rng(args.seed)
    streams = [50.0 + 10.0 * rng.standard_normal((BATTERIES, PERIODS)) for _ in range(16)]
    m, price, charge, discharge, energy = build()
    capture = has_counters(m)
    lowering = counters(m) if capture else None
    solver = rm.Highs(threads=1, time_limit=30.0)
    highs_version = solver.solve(m).metadata["backend"]

    update_ms, solve_ms = [], []
    propagation_totals = {name: 0 for name in COUNTER_NAMES}
    for _ in range(args.repeats):
        for k in range(args.steps):
            if capture:
                m._debug_reset_mir_stats()
            t0 = time.monotonic()
            m.update(price=streams[k % len(streams)])
            t1 = time.monotonic()
            if capture:
                for name, value in counters(m).items():
                    propagation_totals[name] += value
            r = solver.solve(m)
            t2 = time.monotonic()
            assert r.has_primal
            _ = r.values(discharge)
            update_ms.append((t1 - t0) * 1000.0)
            solve_ms.append((t2 - t1) * 1000.0)

    peak = peak_rss_mib()
    solver.close()

    def summary(xs):
        return {
            "mean": float(np.mean(xs)),
            "p50": float(np.percentile(xs, 50)),
            "p95": float(np.percentile(xs, 95)),
        }

    report = {
        "workload": "mir-bess-30x96-baseline",
        "shape": [BATTERIES, PERIODS],
        "price_params": PARAMS,
        "objective_cells": CELLS,
        "steps": args.steps,
        "repeats": args.repeats,
        "counters_available": capture,
        "lowering": lowering,
        # Sums over all measured cycles; divide by steps*repeats for per-cycle.
        "propagation_totals": propagation_totals if capture else None,
        "update_ms": summary(update_ms),
        "solve_ms": summary(solve_ms),
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
    print(json.dumps({k: report[k] for k in ("lowering", "update_ms", "solve_ms")}, indent=1))


if __name__ == "__main__":
    main()
