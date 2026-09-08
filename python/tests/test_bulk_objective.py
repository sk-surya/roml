"""P0 packed-objective contract: sum/dot fast paths match scalar semantics."""

import numpy as np
import pytest

import roml as rm


def test_packed_sum_matches_scalar_optimum():
    m1, m2 = rm.Model("packed"), rm.Model("scalar")
    x1, x2 = m1.vars("x", 4, ub=5.0), m2.vars("x", 4, ub=5.0)
    for m, x in ((m1, x1), (m2, x2)):
        m.add(x[0] + x[1] <= 3.0)
        m.add(x[2] + x[3] <= 4.0)
    m1.minimize(rm.sum(x1))
    total = x2[0] + x2[1] + x2[2] + x2[3]
    m2.minimize(total)
    with rm.Highs() as s1, rm.Highs() as s2:
        r1, r2 = s1.solve(m1), s2.solve(m2)
    assert r1.is_optimal and r2.is_optimal
    assert r1.objective == pytest.approx(r2.objective)


def test_packed_dot_numeric_matches_scalar():
    m1, m2 = rm.Model("packed"), rm.Model("scalar")
    x1, x2 = m1.vars("x", 3, ub=10.0), m2.vars("x", 3, ub=10.0)
    c = np.array([1.0, 2.0, 3.0])
    for m, x in ((m1, x1), (m2, x2)):
        m.add(x[0] + x[1] + x[2] <= 6.0)
    m1.maximize(rm.dot(c, x1))
    m2.maximize(c[0] * x2[0] + c[1] * x2[1] + c[2] * x2[2])
    with rm.Highs() as s1, rm.Highs() as s2:
        r1, r2 = s1.solve(m1), s2.solve(m2)
    assert r1.objective == pytest.approx(r2.objective)


def test_packed_dot_scalar_broadcast():
    m = rm.Model()
    x = m.vars("x", 3, ub=2.0)
    m.add(x[0] + x[1] + x[2] <= 3.0)
    m.maximize(rm.dot(2.0, x))
    with rm.Highs() as s:
        r = s.solve(m)
    assert r.is_optimal
    assert r.objective == pytest.approx(6.0)


def test_parameterized_dot_still_general_path():
    m = rm.Model()
    x = m.vars("x", 2, ub=5.0)
    p = m.params("p", [1.0, 2.0])
    m.add(x[0] + x[1] <= 4.0)
    m.maximize(rm.dot(p, x))
    with rm.Highs() as s:
        assert s.solve(m).objective == pytest.approx(8.0)
        m.update(p=[2.0, 1.0])
        assert s.solve(m).objective == pytest.approx(8.0)


def test_packed_expr_materializes_in_arithmetic():
    m = rm.Model()
    x = m.vars("x", 2, ub=5.0)
    m.add(x[0] + x[1] <= 3.0)
    m.minimize(rm.sum(x) + 1.0)
    with rm.Highs() as s:
        assert s.solve(m).objective == pytest.approx(1.0)
    m2 = rm.Model()
    y = m2.vars("y", 2, ub=5.0)
    m2.add(y[0] + y[1] <= 3.0)
    m2.minimize(-rm.sum(y))
    with rm.Highs() as s:
        assert s.solve(m2).objective == pytest.approx(-3.0)


def test_sum_empty_stays_float_zero():
    m = rm.Model()
    assert rm.sum(m.vars("empty", 0)) == 0.0


def test_bess_spelling_unaffected():
    # The fused parameterized BESS objective keeps working idiomatically
    # (it stays on the general path in P0; this locks the spelling).
    n, dt = 4, 0.25
    m = rm.Model("bess")
    price = m.params("price", np.full(n, 50.0))
    charge = m.vars("charge", n, ub=2.0)
    discharge = m.vars("discharge", n, ub=2.0)
    energy = m.vars("energy", n + 1, ub=4.0)
    m.add(energy[0] == 2.0)
    m.add(
        energy[1:]
        == energy[:-1] + dt * (0.95 * charge - discharge / 0.95)
    )
    m.maximize(dt * rm.dot(price, discharge - charge) + 30.0 * energy[-1])
    with rm.Highs() as s:
        assert s.solve(m).has_primal
