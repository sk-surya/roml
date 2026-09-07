"""MPY-05 MPC integration: oracle equivalence, causal replay, physics."""

import numpy as np
import pytest

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
    gate_forecast,
    objective_value,
)
from benchmarks.highspy_reference import solve_gate


def build_roml():
    m = rm.Model("rolling-battery")
    price = m.params("price", np.full(N, 50.0))
    initial_energy = m.param("initial_energy", 2.0)
    charge = m.vars("charge", N, ub=POWER)
    discharge = m.vars("discharge", N, ub=POWER)
    energy = m.vars("energy", N + 1, ub=ENERGY_CAP)
    direction = m.vars("direction", N, kind="binary")
    m.add(energy[0] == initial_energy, name="initial_soc")
    m.add(
        energy[1:] == energy[:-1] + DT * (EFF * charge - discharge / EFF),
        name="balance",
    )
    m.add(charge <= POWER * direction, name="charge_mode")
    m.add(discharge <= POWER * (1.0 - direction), name="discharge_mode")
    m.maximize(DT * rm.dot(price, discharge - charge) + TERMINAL_VALUE * energy[-1])
    return m, price, initial_energy, charge, discharge, energy


def read_solution(result, charge, discharge, energy):
    ch = result.values(charge)
    dh = result.values(discharge)
    en = result.values(energy)
    return np.asarray(ch), np.asarray(dh), np.asarray(en)


def tol(a, b):
    return abs(a - b) <= 1e-7 + 1e-7 * max(abs(a), abs(b))


def test_matched_input_equivalence_with_highspy():
    # Identical exogenous inputs to both arms: objectives and feasibility
    # must agree (primal schedules may differ among alternate optima).
    realized = forecast_stream(4)
    m, price, initial_energy, charge, discharge, energy = build_roml()
    with rm.Highs(threads=1, time_limit=2.0) as solver:
        for k in range(4):
            forecasts = gate_forecast(realized, k)
            level = 2.0
            m.update(price=forecasts, initial_energy=level)
            result = solver.solve(m)
            assert result.has_primal
            ch, dh, en = read_solution(result, charge, discharge, energy)
            check_physics(en, ch, dh, level)
            ref = solve_gate(forecasts, level)
            assert ref["status"] == "HighsModelStatus.kOptimal"
            assert tol(result.objective, ref["objective"]), (k, result.objective, ref["objective"])
            # Independent recomputation from each arm's own schedule.
            assert tol(
                objective_value(forecasts, ch, dh, en), ref["objective"]
            )
            check_physics(ref["energy"], ref["charge"], ref["discharge"], level)


def test_fresh_rebuild_matches_persistent():
    realized = forecast_stream(2)
    forecasts = gate_forecast(realized, 0)
    with rm.Highs(threads=1, time_limit=2.0) as solver:
        m, price, initial_energy, charge, discharge, energy = build_roml()
        m.update(price=forecasts, initial_energy=1.0)
        warm = solver.solve(m)
        assert warm.metadata["sync_mode"] in ("Delta", "Rebuild", "NoChange")
        # A second identical solve synchronizes nothing.
        warm2 = solver.solve(m)
        assert warm2.metadata["sync_mode"] == "NoChange"
        # Fresh model + fresh session, same mathematics.
        m2, price2, initial2, _, _, _ = build_roml()
        m2.update(price=forecasts, initial_energy=1.0)
        with rm.Highs(threads=1, time_limit=2.0) as fresh:
            cold = fresh.solve(m2)
    assert tol(warm.objective, cold.objective)


def test_causal_rolling_replay_with_own_energy():
    # Closed loop: each gate advances from its own applied first action.
    # Includes negative-price gates (forecast stream dips below zero).
    realized = forecast_stream(10)
    assert bool((realized < 0).any()), "fixture must contain negative prices"
    m, price, initial_energy, charge, discharge, energy = build_roml()
    level = 2.0
    with rm.Highs(threads=1, time_limit=2.0) as solver:
        for k in range(10):
            forecasts = gate_forecast(realized, k)
            m.update(price=forecasts, initial_energy=level)
            result = solver.solve(m)
            assert result.has_primal, f"gate {k} must stay feasible"
            ch, dh, en = read_solution(result, charge, discharge, energy)
            check_physics(en, ch, dh, level)
            level = float(np.clip(level + DT * (EFF * ch[0] - dh[0] / EFF), 0.0, ENERGY_CAP))
    assert 0.0 <= level <= ENERGY_CAP


def test_result_values_are_owned_copies():
    m, price, initial_energy, charge, discharge, energy = build_roml()
    with rm.Highs(threads=1) as solver:
        result = solver.solve(m)
        first = result.values(charge)
        first[0] = -12345.0
        second = result.values(charge)
        assert second[0] != pytest.approx(-12345.0)
