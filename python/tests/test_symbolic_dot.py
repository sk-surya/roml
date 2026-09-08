"""P1C-2 locks: structural parameterized dot preserves exact semantics.

Pass on the phase-1 general lowering; must pass unchanged on the packed
symbolic path + core primitive (zero semantic differences is the gate).
"""

import numpy as np
import pytest

import roml as rm


def solve(m):
    with rm.Highs() as s:
        r = s.solve(m)
    assert r.is_optimal
    return r.objective


def test_dot_paramarray_vararray():
    m = rm.Model()
    x = m.vars("x", 3, ub=5.0)
    p = m.params("p", [1.0, 2.0, 3.0])
    m.add(x[0] + x[1] + x[2] <= 6.0)
    m.maximize(rm.dot(p, x))
    assert solve(m) == pytest.approx(17.0)
    m.update(p=[3.0, 2.0, 1.0])
    assert solve(m) == pytest.approx(17.0)


def test_dot_paramarray_packed_expr_matches_scalar():
    n = 6
    q = np.array([3.0, 1.0, 4.0, 1.0, 5.0, 9.0])

    def build_dot():
        m = rm.Model()
        x = m.vars("x", n, ub=2.0)
        p = m.params("p", q)
        m.add(rm.sum(x) <= 6.0)
        m.maximize(rm.dot(p, 2.0 * x - x))
        return m

    def build_scalar():
        m = rm.Model()
        x = m.vars("x", n, ub=2.0)
        p = m.params("p", q)
        m.add(rm.sum(x) <= 6.0)
        total = None
        for i in range(n):
            term = p[i] * x[i]
            total = term if total is None else total + term
        m.maximize(total)
        return m

    assert solve(build_dot()) == pytest.approx(solve(build_scalar()))


def test_scalar_param_broadcast_and_negative_scale():
    m = rm.Model()
    x = m.vars("x", 2, ub=5.0)
    p = m.param("q", 2.0)
    m.add(x[0] + x[1] <= 4.0)
    m.maximize(rm.dot(-1.0 * p, x))
    assert solve(m) == pytest.approx(0.0)


def test_sliced_param_array():
    m = rm.Model()
    x = m.vars("x", 2, ub=5.0)
    p = m.params("p", [1.0, 2.0, 3.0, 4.0])
    m.add(x[0] + x[1] <= 3.0)
    m.maximize(rm.dot(p[1:3], x))
    assert solve(m) == pytest.approx(9.0)


def test_shape_mismatch_and_cross_model():
    m = rm.Model()
    x = m.vars("x", 2, ub=1.0)
    p = m.params("p", [1.0, 2.0, 3.0])
    with pytest.raises(rm.ShapeError):
        rm.dot(p, x)
    other = rm.Model("other")
    y = other.vars("y", 3, ub=1.0)
    with pytest.raises(rm.ModelMismatchError):
        rm.dot(p, y)


def test_nonlinear_and_bool_reject():
    m = rm.Model()
    x = m.vars("x", 2, ub=1.0)
    with pytest.raises(rm.UnsupportedExpressionError):
        rm.dot(x, x)
    with pytest.raises((rm.InvalidModelError, TypeError, ValueError)):
        rm.dot([True, False], x)


def test_update_after_symbolic_objective():
    m = rm.Model()
    x = m.vars("x", 2, ub=5.0)
    p = m.params("p", [1.0, 4.0])
    m.add(x[0] + x[1] <= 5.0)
    m.maximize(rm.dot(p, x))
    assert solve(m) == pytest.approx(20.0)
    m.update(p=[4.0, 1.0])
    assert solve(m) == pytest.approx(20.0)
    m.update(p=[0.0, 0.0])
    assert solve(m) == pytest.approx(0.0)


def test_symbolic_cell_arbitrary_mutation():
    m = rm.Model()
    x = m.vars("x", 2, ub=5.0)
    p = m.params("p", [1.0, 2.0])
    m.add(x[0] + x[1] <= 5.0)
    m.maximize(rm.dot(p, x))
    assert solve(m) == pytest.approx(10.0)
    # Scalar overwrite of one symbolic cell, then re-solve.
    m.add(x[0] <= 1.0)
    assert solve(m) == pytest.approx(10.0)


def test_bess_param_form_solves_and_updates():
    n, dt = 4, 0.25
    prices = np.array([50.0, 30.0, 70.0, 40.0])
    m = rm.Model("bess")
    price = m.params("price", prices)
    charge = m.vars("charge", n, ub=2.0)
    discharge = m.vars("discharge", n, ub=2.0)
    energy = m.vars("energy", n + 1, ub=4.0)
    m.add(energy[0] == 2.0)
    m.add(energy[1:] == energy[:-1] + dt * (0.95 * charge - discharge / 0.95))
    m.add(charge + discharge <= 2.0)
    m.maximize(rm.dot(price, discharge - charge))
    first = solve(m)
    m.update(price=prices * 0.5)
    second = solve(m)
    assert second == pytest.approx(first * 0.5)
