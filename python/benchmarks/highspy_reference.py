"""Independent highspy implementation of the BESS MILP (MPY-05 oracle arm).

Same mathematics as the ROML model, built directly as CSR data with no
shared code: 24 periods, binary direction, energy balance, mode rows,
revenue objective with terminal value. Used for cross-arm equivalence
(matched inputs) and closed-loop validation (each arm at its own state).
"""

from __future__ import annotations

import highspy
import numpy as np

from benchmarks.fixtures import DT, EFF, ENERGY_CAP, N, POWER, TERMINAL_VALUE


def build_highs_model(price, initial_energy):
    """Return (h, cols) with column order charge[n], discharge[n],
    energy[n+1], direction[n]."""
    n = N
    n_charge, n_discharge, n_energy, n_dir = n, n, n + 1, n
    ncols = n_charge + n_discharge + n_energy + n_dir
    off_d = n_charge
    off_e = off_d + n_discharge
    off_b = off_e + n_energy
    assert off_b + n_dir == ncols

    h = highspy.Highs()
    h.setOptionValue("output_flag", False)
    h.setOptionValue("threads", 1)
    empty_i = np.zeros(0, dtype=np.int32)
    empty_v = np.zeros(0)
    # Columns: charge, discharge (continuous [0, power]).
    for _ in range(n_charge + n_discharge):
        assert h.addCol(0.0, 0.0, POWER, 0, empty_i, empty_v) == highspy.HighsStatus.kOk
    # energy (continuous [0, cap]).
    for _ in range(n_energy):
        assert h.addCol(0.0, 0.0, ENERGY_CAP, 0, empty_i, empty_v) == highspy.HighsStatus.kOk
    # direction (binary via integrality change).
    for j in range(n_dir):
        assert (
            h.addCol(0.0, 0.0, 1.0, 0, empty_i, empty_v) == highspy.HighsStatus.kOk
        )
        assert (
            h.changeColIntegrality(off_b + j, highspy.HighsVarType.kInteger) == highspy.HighsStatus.kOk
        )

    # Objective: dt*price*(discharge - charge) + terminal*energy[-1].
    # Maximize -> minimize the negation.
    costs = np.zeros(ncols)
    costs[0:n_charge] = DT * price
    costs[n_charge : n_charge + n_discharge] = -DT * price
    costs[off_e + n] = -TERMINAL_VALUE
    for j in range(ncols):
        assert h.changeColCost(j, float(costs[j])) == highspy.HighsStatus.kOk

    # Rows.
    # initial_soc: energy[0] == initial.
    assert (
        h.addRow(initial_energy, initial_energy, 1, [off_e], [1.0])
        == highspy.HighsStatus.kOk
    )
    # balance: energy[t+1] - energy[t] - dt*eff*charge[t] + dt/eff*discharge[t] == 0.
    for t in range(n):
        assert (
            h.addRow(
                0.0,
                0.0,
                4,
                [off_e + t + 1, off_e + t, t, n_charge + t],
                [1.0, -1.0, -DT * EFF, DT / EFF],
            )
            == highspy.HighsStatus.kOk
        )
    # charge_mode: charge[t] - power*direction[t] <= 0.
    for t in range(n):
        assert (
            h.addRow(-highspy.kHighsInf, 0.0, 2, [t, off_b + t], [1.0, -POWER])
            == highspy.HighsStatus.kOk
        )
    # discharge_mode: discharge[t] + power*direction[t] <= power.
    for t in range(n):
        assert (
            h.addRow(
                -highspy.kHighsInf, POWER, 2, [n_charge + t, off_b + t], [1.0, POWER]
            )
            == highspy.HighsStatus.kOk
        )
    cols = {
        "charge": np.arange(0, n_charge),
        "discharge": np.arange(n_charge, n_charge + n_discharge),
        "energy": np.arange(off_e, off_e + n_energy),
    }
    return h, cols


def solve_gate(price, initial_energy, time_limit=2.0):
    h, cols = build_highs_model(np.asarray(price), float(initial_energy))
    h.setOptionValue("time_limit", float(time_limit))
    h.run()
    status = h.getModelStatus()
    info = h.getInfo()
    sol = h.getSolution()
    return {
        "status": str(status),
        "objective": -info.objective_function_value,
        "charge": np.array(sol.col_value)[cols["charge"]].copy(),
        "discharge": np.array(sol.col_value)[cols["discharge"]].copy(),
        "energy": np.array(sol.col_value)[cols["energy"]].copy(),
    }


class PersistentHighs:
    """Persistent highspy model with in-place coefficient/bound edits."""

    def __init__(self, price, initial_energy):
        self.h, self.cols = build_highs_model(np.asarray(price), float(initial_energy))
        self.n = N

    def update(self, price, initial_energy):
        price = np.asarray(price)
        # Objective costs: dt*price on charge, -dt*price on discharge.
        for t in range(self.n):
            assert (
                self.h.changeColCost(int(self.cols["charge"][t]), float(DT * price[t]))
                == highspy.HighsStatus.kOk
            )
            assert (
                self.h.changeColCost(
                    int(self.cols["discharge"][t]), float(-DT * price[t])
                )
                == highspy.HighsStatus.kOk
            )
        # initial_soc row 0 bounds.
        assert (
            self.h.changeRowBounds(0, float(initial_energy), float(initial_energy))
            == highspy.HighsStatus.kOk
        )

    def solve(self, time_limit=2.0):
        self.h.setOptionValue("time_limit", float(time_limit))
        self.h.run()
        status = str(self.h.getModelStatus())
        info = self.h.getInfo()
        sol = self.h.getSolution()
        return {
            "status": status,
            "objective": -info.objective_function_value,
            "charge": np.array(sol.col_value)[self.cols["charge"]].copy(),
            "discharge": np.array(sol.col_value)[self.cols["discharge"]].copy(),
            "energy": np.array(sol.col_value)[self.cols["energy"]].copy(),
        }
