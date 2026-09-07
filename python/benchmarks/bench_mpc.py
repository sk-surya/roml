"""MPY-05 MPC benchmark driver (QUALIFICATION workloads).

Implements the exact CLI: --seed, --repeats, --steps, --output. Runs the
causal rolling BESS replay (MILP-MPC workload) for the requested steps and
repetitions, writing configuration, versions, and per-sample data as JSON.

Also supports --workload {milp-mpc,lp-small,lp-scale} and --arm
{python-persistent,python-fresh} for the comparison arms.
"""

from __future__ import annotations

import argparse
import os
import json
import platform
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

import numpy as np

import roml as rm
from benchmarks.fixtures import (
    DT,
    EFF,
    ENERGY_CAP,
    N,
    POWER,
    TERMINAL_VALUE,
    check_physics,
    forecast_stream,
)


def build_model(lp=False):
    m = rm.Model("rolling-battery-lp" if lp else "rolling-battery")
    price = m.params("price", np.full(N, 50.0))
    initial_energy = m.param("initial_energy", 2.0)
    charge = m.vars("charge", N, ub=POWER)
    discharge = m.vars("discharge", N, ub=POWER)
    energy = m.vars("energy", N + 1, ub=ENERGY_CAP)
    # LP-small: explicitly labeled LP relaxation (continuous direction).
    direction = m.vars(
        "direction", N, kind="continuous" if lp else "binary", ub=1.0
    )
    m.add(energy[0] == initial_energy, name="initial_soc")
    m.add(energy[1:] == energy[:-1] + DT * (EFF * charge - discharge / EFF))
    m.add(charge <= POWER * direction)
    m.add(discharge <= POWER * (1.0 - direction))
    m.maximize(DT * rm.dot(price, discharge - charge) + TERMINAL_VALUE * energy[-1])
    return m, price, initial_energy, charge, discharge, energy


def replay(steps, seed, fresh_each_gate, mode="closed", realized=None, lp=False):
    if realized is None:
        realized = forecast_stream(steps, seed=seed)
    level = 2.0
    samples = []
    solver = None
    m = None
    for k in range(steps):
        if fresh_each_gate or m is None:
            if solver is not None:
                solver.close()
            m, price, initial_energy, charge, discharge, energy = build_model(lp=lp)
            solver = rm.Highs(threads=1, time_limit=2.0)
        forecasts = realized[k : k + N].copy()
        gate_level = 2.0 if mode == "matched" else level
        t0 = time.monotonic()
        m.update(price=forecasts, initial_energy=gate_level)
        result = solver.solve(m)
        t1 = time.monotonic()
        assert result.has_primal, f"gate {k} infeasible"
        ch = np.asarray(result.values(charge))
        dh = np.asarray(result.values(discharge))
        en = np.asarray(result.values(energy))
        check_physics(en, ch, dh, gate_level)
        level = float(np.clip(level + DT * (EFF * ch[0] - dh[0] / EFF), 0.0, ENERGY_CAP))
        samples.append(
            {
                "gate": k,
                "wall_ms": (t1 - t0) * 1000.0,
                "objective": result.objective,
                "level": level,
            }
        )
    if solver is not None:
        solver.close()
    return samples


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--seed", type=int, required=True)
    parser.add_argument("--repeats", type=int, required=True)
    parser.add_argument("--steps", type=int, required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--workload", default="milp-mpc")
    parser.add_argument("--arm", default="python-persistent",
                        choices=["python-persistent", "python-fresh"])
    parser.add_argument("--mode", default="closed", choices=["closed", "matched"])
    parser.add_argument("--lp", action="store_true",
                        help="LP-small: continuous direction relaxation")
    parser.add_argument("--export-forecasts", default=None)
    parser.add_argument("--import-forecasts", default=None)
    args = parser.parse_args()

    fresh = args.arm == "python-fresh"
    lp = args.lp
    if lp and args.workload == "milp-mpc":
        args.workload = "lp-small"
    realized = None
    if args.import_forecasts is not None:
        realized = np.loadtxt(args.import_forecasts, delimiter=",")
    if args.export_forecasts is not None:
        np.savetxt(args.export_forecasts,
                   forecast_stream(args.steps, seed=args.seed), delimiter=",")
    # Warmup (not recorded).
    replay(5, args.seed, fresh, mode=args.mode, realized=None, lp=lp)
    repetitions = []
    for r in range(args.repeats):
        samples = replay(args.steps, args.seed + r, fresh,
                         mode=args.mode, realized=realized, lp=lp)
        walls = [s["wall_ms"] for s in samples]
        repetitions.append(
            {
                "repeat": r,
                "p50_ms": float(np.percentile(walls, 50)),
                "p95_ms": float(np.percentile(walls, 95)),
                "p99_ms": float(np.percentile(walls, 99)),
                "mean_objective": float(np.mean([s["objective"] for s in samples])),
                "final_level": samples[-1]["level"],
            }
        )
    report = {
        "fixture": "LP-small" if lp else "MILP-MPC",
        "workload": args.workload,
        "arm": args.arm,
        "seed": args.seed,
        "mode": args.mode,
        "repeats": args.repeats,
        "steps": args.steps,
        "config": {
            "os": platform.platform(),
            "cpu": platform.processor() or platform.machine(),
            "python": platform.python_version(),
            "numpy": np.__version__,
            "roml": rm.__version__,
            "threads": 1,
            "time_limit": 2.0,
        },
        "repetitions": repetitions,
    }
    with open(args.output, "w") as f:
        json.dump(report, f, indent=1)
    medians = [r["p50_ms"] for r in repetitions]
    print(f"arm={args.arm} steps={args.steps} repeats={args.repeats}")
    print(f"p50-of-p50: {float(np.median(medians)):.2f} ms/gate")


if __name__ == "__main__":
    main()
