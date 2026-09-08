"""P1C-2 locks: structural parameterized dot preserves exact semantics.

Pass on the phase-1 general lowering; must pass unchanged on the packed
symbolic path + core primitive (zero semantic differences is the gate).
The second block pins the packed-symbolic form itself: materialization
through arithmetic/constraints, fallback parity, and update validation.
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


# --- Packed-symbolic form pins (P1C-2 phase 2) ---


def build_pair(n=4):
    """Two identical models' handles; caller builds each side's objective."""
    q = np.array([3.0, 1.0, 4.0, 2.0])

    def make():
        m = rm.Model()
        x = m.vars("x", n, ub=2.0)
        p = m.params("p", q)
        m.add(rm.sum(x) <= 5.0)
        return m, x, p

    return make, q


def test_symbolic_arithmetic_materializes_identically():
    make, q = build_pair()
    m1, x1, p1 = make()
    m1.maximize(rm.dot(p1, x1) + 1.0)
    m2, x2, p2 = make()
    total = None
    for i in range(4):
        term = p2[i] * x2[i]
        total = term if total is None else total + term
    m2.maximize(total + 1.0)
    assert solve(m1) == pytest.approx(solve(m2))
    # Scaling and negation of a packed-symbolic value also materialize.
    m3, x3, p3 = make()
    m3.maximize(2.0 * rm.dot(p3, x3) - rm.dot(p3, x3))
    assert solve(m3) == pytest.approx(solve(m2) - 1.0)


def test_symbolic_in_constraint():
    m = rm.Model()
    x = m.vars("x", 3, ub=5.0)
    p = m.params("p", [1.0, 2.0, 3.0])
    m.add(rm.dot(p, x) <= 10.0)
    m.maximize(rm.sum(x))
    # Cheapest objective-per-cost first: x0=5 (ub), then x1=2.5.
    assert solve(m) == pytest.approx(7.5)


def test_symbolic_nonzero_right_constant_rejects_like_scalar():
    m = rm.Model()
    x = m.vars("x", 2, ub=5.0)
    p = m.params("p", [1.0, 2.0])
    with pytest.raises(rm.UnsupportedExpressionError):
        m.maximize(rm.dot(p, x + 1.0))


def test_mixed_numeric_param_falls_back_with_same_answer():
    n = 4
    q = [3.0, 1.0, 4.0, 2.0]
    m = rm.Model()
    x = m.vars("x", n, ub=2.0)
    p = m.params("p", q)
    m.add(rm.sum(x) <= 5.0)
    # Masked parameters: zeroed entries simplify to numerics, so the
    # coefficient mix cannot pack and keeps the general lowering.
    m.maximize(rm.dot(p * np.array([1.0, 0.0, 1.0, 0.0]), x))
    got = solve(m)
    m2 = rm.Model()
    y = m2.vars("x", n, ub=2.0)
    q2 = m2.params("p", q)
    m2.add(rm.sum(y) <= 5.0)
    m2.maximize(q2[0] * y[0] + q2[2] * y[2])
    assert got == pytest.approx(solve(m2))


def test_scaled_update_validation_matches_scalar():
    # A scaled coefficient that overflows under update rejects the batch
    # identically on both paths (template parity).
    big = 1e308

    def make_packed():
        m = rm.Model()
        x = m.vars("x", 2, ub=5.0)
        p = m.params("p", [1.0, 1.0])
        m.add(x[0] + x[1] <= 5.0)
        m.maximize(rm.dot(2.0 * p, x))
        return m

    def make_scalar():
        m = rm.Model()
        x = m.vars("x", 2, ub=5.0)
        p = m.params("p", [1.0, 1.0])
        m.add(x[0] + x[1] <= 5.0)
        m.maximize(2.0 * p[0] * x[0] + 2.0 * p[1] * x[1])
        return m

    assert solve(make_packed()) == pytest.approx(solve(make_scalar()))
    for make in (make_packed, make_scalar):
        m = make()
        with pytest.raises((rm.InvalidModelError, ValueError)):
            m.update(p=[big, big])
