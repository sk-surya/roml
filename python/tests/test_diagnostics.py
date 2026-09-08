"""MPY-04 diagnostics: LP duals, reduced costs, metadata honesty."""

import numpy as np
import pytest

import roml as rm


def lp_model():
    m = rm.Model()
    x = m.var("x", lb=0.0)
    y = m.var("y", lb=0.0)
    cap = m.add(x + y <= 4.0, name="capacity")
    m.add(x <= 3.0)
    m.maximize(x + 2.0 * y)
    return m, x, y, cap


def test_lp_duals_and_reduced_costs():
    m, x, y, cap = lp_model()
    with rm.Highs() as solver:
        result = solver.solve(m)
    assert result.is_optimal
    assert result.objective == pytest.approx(8.0)
    # max x + 2y s.t. x + y <= 4 (tight), x <= 3 (slack): marginal value 2.
    assert result.dual(cap) == pytest.approx(2.0)
    # x is nonbasic at 0 (raising x trades 2-for-1 against y).
    assert result.reduced_cost(x) == pytest.approx(-1.0)
    assert result.reduced_cost(y) == pytest.approx(0.0)
    meta = result.metadata
    assert meta["has_primal"] is True
    assert meta["warm_start"] == "none"
    assert meta["wall_seconds"] >= 0.0
    assert meta["backend"]


def test_mip_duals_unavailable():
    m = rm.Model()
    n = m.var("n", kind="binary")
    x = m.var("x", ub=5.0)
    cap = m.add(x <= 2.0 + 3.0 * n)
    m.maximize(x)
    with rm.Highs() as solver:
        result = solver.solve(m)
    assert result.is_optimal
    assert result.objective == pytest.approx(5.0)
    with pytest.raises(rm.UnavailableDiagnosticError):
        result.dual(cap)
    assert result.metadata["has_primal"] is True


def test_duals_array_and_missing():
    m = rm.Model()
    v = m.vars("v", 2, lb=0.0)
    rows = m.add(v <= [4.0, 10.0])
    m.maximize(v[0] + 2.0 * v[1])
    with rm.Highs() as solver:
        result = solver.solve(m)
    assert result.is_optimal
    assert result.objective == pytest.approx(24.0)
    duals = result.duals(rows)
    assert duals.shape == (2,)
    assert float(duals[0]) == pytest.approx(1.0)
    assert float(duals[1]) == pytest.approx(2.0)
    # A constraint created after the solve has no reported dual.
    late = m.add(v[0] <= 50.0)
    with pytest.raises(rm.MissingValueError):
        result.dual(late)


def test_no_primal_duals_raise():
    m = rm.Model()
    x = m.var("x", ub=1.0)
    cap = m.add(x >= 2.0)
    m.minimize(x)
    with rm.Highs() as solver:
        result = solver.solve(m)
    assert not result.has_primal
    with pytest.raises(rm.NoSolutionError):
        result.dual(cap)
    with pytest.raises(rm.NoSolutionError):
        result.reduced_cost(x)
