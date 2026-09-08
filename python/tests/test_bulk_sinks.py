"""P1E locks: lazy scalar sinks converge onto the bulk core primitives.

The classifier routes all-numeric objectives to
`set_linear_objective_param_bulk`-style parametric insertion for
`scale x Param` coefficients, all-numeric single rows to one-row
`add_linear_rows_bulk`, and everything else to the unchanged general
path. These tests pin observable behavior per class (solve equivalence
against reference spellings, updates, errors); the routing itself is
internal.
"""

import numpy as np
import pytest

import roml as rm


def solve(m):
    with rm.Highs() as s:
        r = s.solve(m)
    assert r.is_optimal
    return r.objective


def test_numeric_chain_matches_bulk_sum():
    n = 2000
    m = rm.Model()
    x = m.vars("x", n, ub=2.0)
    m.add(rm.sum(x) <= 1000.0)
    total = x[0]
    for i in range(1, n):
        total = total + x[i]
    m.maximize(total)
    assert solve(m) == pytest.approx(1000.0)


def test_numeric_duplicates_and_cancellation():
    m = rm.Model()
    x = m.vars("x", 2, ub=5.0)
    m.add(x[0] + x[1] <= 6.0)
    # 2*x0 + 4*x1 - x0 - 3*x1 -> x0 + x1.
    m.maximize(2 * x[0] + 4 * x[1] - x[0] - 3 * x[1])
    assert solve(m) == pytest.approx(6.0)
    m2 = rm.Model()
    y = m2.vars("x", 2, ub=5.0)
    m2.add(y[0] + y[1] <= 6.0)
    m2.maximize(y[0] - y[0] + 2 * y[1])
    assert solve(m2) == pytest.approx(10.0)


def test_numeric_negative_scaling_and_constants():
    m = rm.Model()
    x = m.vars("x", 2, ub=5.0)
    m.add(x[0] + x[1] <= 4.0)
    m.maximize(-2 * x[0] - 2 * x[1] + 10.0)
    assert solve(m) == pytest.approx(10.0)


def test_parametric_scalar_chain_matches_dot():
    n = 200
    q = np.linspace(1.0, 4.0, n)
    m = rm.Model()
    x = m.vars("x", n, ub=2.0)
    p = m.params("p", q)
    m.add(rm.sum(x) <= 100.0)
    total = None
    for i in range(n):
        term = p[i] * x[i]
        total = term if total is None else total + term
    m.maximize(total)
    first = solve(m)
    m2 = rm.Model()
    y = m2.vars("x", n, ub=2.0)
    q2 = m2.params("p", q)
    m2.add(rm.sum(y) <= 100.0)
    m2.maximize(rm.dot(q2, y))
    assert first == pytest.approx(solve(m2))
    # Update parity through the parametric bulk cells.
    m.update(p=q * 0.5)
    m2.update(p=q * 0.5)
    assert solve(m) == pytest.approx(solve(m2))


def test_same_param_duplicates_sum():
    m = rm.Model()
    x = m.vars("x", 1, ub=5.0)
    p = m.param("q", 3.0)
    m.add(x[0] <= 4.0)
    m.maximize(p * x[0] + 2 * p * x[0])
    assert solve(m) == pytest.approx(36.0)


def test_distinct_params_same_var_matches_reference():
    # p*x + q*x cannot pack into one parametric cell; semantics must
    # still match the scalar reference exactly.
    m = rm.Model()
    x = m.vars("x", 1, ub=5.0)
    p = m.param("p", 2.0)
    q = m.param("q", 3.0)
    m.add(x[0] <= 4.0)
    m.maximize(p * x[0] + q * x[0])
    assert solve(m) == pytest.approx(20.0)
    m.update(p=1.0, q=1.0)
    assert solve(m) == pytest.approx(8.0)


def test_general_param_expression_falls_back():
    # (p+q)*x is genuinely general; it must solve like the reference.
    m = rm.Model()
    x = m.vars("x", 1, ub=5.0)
    p = m.param("p", 2.0)
    q = m.param("q", 3.0)
    m.add(x[0] <= 4.0)
    m.maximize((p + q) * x[0])
    assert solve(m) == pytest.approx(20.0)
    m.update(p=1.0, q=1.0)
    assert solve(m) == pytest.approx(8.0)


def test_nested_scales_and_negation():
    m = rm.Model()
    x = m.vars("x", 2, ub=5.0)
    m.add(x[0] + x[1] <= 4.0)
    m.maximize(3 * (2 * (x[0] + x[1])) - x[0])
    assert solve(m) == pytest.approx(24.0)


def test_flat_and_packed_leaves_route():
    # Masked-param dot falls back to a Flat leaf; combining it further
    # must still solve identically to the reference.
    n = 100
    q = np.linspace(1.0, 3.0, n)
    m = rm.Model()
    x = m.vars("x", n, ub=2.0)
    p = m.params("p", q)
    m.add(rm.sum(x) <= 50.0)
    base = rm.dot(p * np.array([1.0 if i % 2 == 0 else 0.0 for i in range(n)]), x)
    m.maximize(base + rm.sum(x))
    got = solve(m)
    m2 = rm.Model()
    y = m2.vars("x", n, ub=2.0)
    q2 = m2.params("p", q)
    m2.add(rm.sum(y) <= 50.0)
    total = None
    for i in range(n):
        c = (q[i] if i % 2 == 0 else 0.0) + 1.0
        term = c * y[i]
        total = term if total is None else total + term
    m2.maximize(total)
    assert got == pytest.approx(solve(m2))


def test_packed_arithmetic_objective_matches():
    # 2*sum(x)+x[0] classifies numeric and must match the bulk spelling.
    n = 500
    m = rm.Model()
    x = m.vars("x", n, ub=2.0)
    m.add(rm.sum(x) <= 200.0)
    m.maximize(2 * rm.sum(x) + x[0])
    assert solve(m) == pytest.approx(402.0)


def test_scalar_numeric_constraint_equivalence():
    m = rm.Model()
    x = m.vars("x", 3, ub=5.0)
    m.add(2 * x[0] + 3 * x[1] - x[2] <= 16.0)
    m.maximize(x[0] + x[1] + x[2])
    assert solve(m) == pytest.approx(41.0 / 3.0)
    # Duplicate accumulation and cancellation inside one row.
    m2 = rm.Model()
    y = m2.vars("x", 2, ub=5.0)
    m2.add(y[0] + y[0] + y[1] - y[0] <= 6.0)
    m2.maximize(y[0] + y[1])
    assert solve(m2) == pytest.approx(6.0)


def test_scalar_equality_and_vacuous_rows():
    m = rm.Model()
    x = m.vars("x", 2, ub=5.0)
    m.add(x[0] + x[1] == 4.0)
    m.maximize(x[0])
    assert solve(m) == pytest.approx(4.0)
    # A trivially satisfied row still installs and solves.
    m2 = rm.Model()
    y = m2.vars("x", 1, ub=5.0)
    m2.add(y[0] - y[0] + 3.0 <= 5.0)
    m2.maximize(y[0])
    assert solve(m2) == pytest.approx(5.0)


def test_named_scalar_row_and_persistent_solve():
    m = rm.Model()
    x = m.vars("x", 2, ub=5.0)
    c = m.add(2 * x[0] + x[1] <= 6.0, name="cap")
    assert repr(c) == 'Constraint("cap")'
    m.maximize(x[0] + x[1])
    with rm.Highs() as s:
        assert s.solve(m).objective == pytest.approx(5.5)
        assert s.solve(m).objective == pytest.approx(5.5)


def test_parametric_scalar_update_persistent():
    n = 50
    q = np.linspace(2.0, 6.0, n)
    m = rm.Model()
    x = m.vars("x", n, ub=3.0)
    p = m.params("p", q)
    m.add(rm.sum(x) <= 60.0)
    total = None
    for i in range(n):
        term = p[i] * x[i]
        total = term if total is None else total + term
    m.maximize(total)
    with rm.Highs() as s:
        first = s.solve(m)
        assert first.is_optimal
        m.update(p=q * 2.0)
        second = s.solve(m)
        assert second.is_optimal
        assert second.objective == pytest.approx(first.objective * 2.0)


def test_deep_nonlinear_still_rejects_at_construction():
    m = rm.Model()
    x = m.vars("x", 100, ub=1.0)
    total = x[0]
    for i in range(1, 100):
        total = total + x[i]
    with pytest.raises(rm.UnsupportedExpressionError):
        total * x[0]
    with pytest.raises(rm.UnsupportedExpressionError):
        x[0] * total


def test_foreign_model_through_classifier():
    a, b = rm.Model("a"), rm.Model("b")
    x = a.vars("x", 3, ub=1.0)
    y = b.vars("x", 3, ub=1.0)
    total = x[0] + x[1] + x[2]
    with pytest.raises(rm.ModelMismatchError):
        total + y[0]
    with pytest.raises(rm.ModelMismatchError):
        b.maximize(total)
