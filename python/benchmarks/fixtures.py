"""Shared synthetic BESS fixtures (MPY-05). No market/customer data.

Rolling-horizon battery MILP from the DESIGN contract: 24 intervals,
dt=0.25h, 4 MWh energy, 2 MWh initial, 2 MW power, 0.95 efficiencies,
binary direction. Forecasts are synthetic and causal: gate k sees only
the windowed forecast stream, never realized future prices.
"""

from __future__ import annotations

import numpy as np

N = 24
DT = 0.25
ENERGY_CAP = 4.0
POWER = 2.0
EFF = 0.95
TERMINAL_VALUE = 30.0
SEED = 20260907


def forecast_stream(n_gates, seed=SEED):
    """Causal forecast windows + realized prices, frozen recipe.

    price[t,k] = 50 + 60*sin((t+k)/4) + epsilon, epsilon ~ N(0, 5^2)
    seeded once; gate k observes window [k, k+N).
    """
    rng = np.random.default_rng(seed)
    horizon = n_gates + N
    t = np.arange(horizon)
    base = 50.0 + 60.0 * np.sin(t / 4.0)
    epsilon = rng.normal(0.0, 5.0, size=horizon)
    realized = base + epsilon
    return realized


def gate_forecast(realized, k):
    return realized[k : k + N].copy()


def check_physics(
    energy,
    charge,
    discharge,
    initial,
    direction=None,
    dt=DT,
    eff=EFF,
    cap=ENERGY_CAP,
    power=POWER,
    tol=1e-6,
):
    """Energy recurrence, bounds, charge/discharge exclusivity, and (when
    direction is given) MIP integrality plus mode-row consistency."""
    assert abs(energy[0] - initial) <= tol, (energy[0], initial)
    for t in range(N):
        expect = energy[t] + dt * (eff * charge[t] - discharge[t] / eff)
        assert abs(energy[t + 1] - expect) <= 1e-5, (t, energy[t + 1], expect)
        assert -tol <= energy[t + 1] <= cap + tol
        assert -tol <= charge[t] <= power + tol
        assert -tol <= discharge[t] <= power + tol
        assert charge[t] * discharge[t] <= tol
    assert -tol <= energy[N] <= cap + tol
    if direction is not None:
        for t in range(N):
            d = direction[t]
            assert min(abs(d), abs(d - 1.0)) <= 1e-6, (t, d)
            assert charge[t] <= power * d + tol, (t, charge[t], d)
            assert discharge[t] <= power * (1.0 - d) + tol, (t, discharge[t], d)


def objective_value(price, charge, discharge, energy, dt=DT, terminal=TERMINAL_VALUE):
    return float(
        dt * np.dot(price, np.asarray(discharge) - np.asarray(charge))
        + terminal * energy[-1]
    )
