"""P1D locks: persistent lazy scalar expressions preserve exact semantics.

Construction is O(1) per operator (no term copying, no canonicalization);
a single iterative flattening at each model sink lowers. These tests pin
meaning, not performance: persistence, accumulation, cancellation,
scaling, packed algebra, parameters, nonlinearity, foreign models, deep
chains, and solve equivalence against the packed/bulk paths.
"""

import numpy as np
import pytest

import roml as rm


def solve(m):
    with rm.Highs() as s:
        r = s.solve(m)
    assert r.is_optimal
    return r.objective


def test_persistence_add_does_not_mutate():
    m = rm.Model()
    x = m.vars("x", 4, ub=5.0)
    m.add(x[0] + x[1] + x[2] + x[3] <= 10.0)
    a = x[0] + x[1]
    b = a + x[2]
    _c = a - x[3]
    # If `a` had been mutated by the derivations above (i.e. were really
    # x0+x1+x2), pinning it to 3 would cap `b` at 3. Intact, `b` reaches 8.
    m.add(a == 3.0)
    m.maximize(b)
    assert solve(m) == pytest.approx(8.0)


def test_duplicate_accumulation_and_cancellation():
    m = rm.Model()
    x = m.vars("x", 2, ub=5.0)
    m.add(x[0] + x[1] <= 6.0)
    m.maximize(x[0] + x[1] + x[0] - x[1])
    assert solve(m) == pytest.approx(10.0)
    m2 = rm.Model()
    y = m2.vars("x", 2, ub=5.0)
    m2.add(y[0] - y[0] + y[1] <= 4.0)
    m2.maximize(y[0] + y[1])
    assert solve(m2) == pytest.approx(9.0)


def test_constants_fold():
    m = rm.Model()
    x = m.vars("x", 1, ub=5.0)
    m.add(x[0] + 2 - 2 <= 3.0)
    m.maximize(x[0])
    assert solve(m) == pytest.approx(3.0)


def test_scaling_forms():
    m = rm.Model()
    x = m.vars("x", 2, ub=5.0)
    m.add(x[0] + x[1] <= 4.0)
    m.maximize(2 * (x[0] + x[1]))
    assert solve(m) == pytest.approx(8.0)
    m2 = rm.Model()
    y = m2.vars("x", 2, ub=5.0)
    m2.add(y[0] + y[1] <= 4.0)
    m2.maximize(-(y[0] + y[1]))
    assert solve(m2) == pytest.approx(0.0)
    m3 = rm.Model()
    z = m3.vars("x", 2, ub=5.0)
    m3.add(z[0] + z[1] <= 4.0)
    m3.maximize((z[0] + z[1]) / 2)
    assert solve(m3) == pytest.approx(2.0)


def test_packed_surrounding_algebra_matches_scalar():
    n = 200
    m = rm.Model()
    x = m.vars("x", n, ub=2.0)
    m.add(rm.sum(x) <= 100.0)
    m.maximize(2 * rm.sum(x) + 5 - x[0])
    got = solve(m)
    m2 = rm.Model()
    y = m2.vars("x", n, ub=2.0)
    m2.add(rm.sum(y) <= 100.0)
    total = None
    for i in range(n):
        term = 2.0 * y[i]
        total = term if total is None else total + term
    m2.maximize(total + 5 - y[0])
    assert got == pytest.approx(solve(m2))


def test_parameter_semantics_unchanged():
    m = rm.Model()
    x = m.vars("x", 2, ub=5.0)
    p = m.params("p", [2.0, 3.0])
    m.add(x[0] + x[1] <= 5.0)
    m.maximize(p[0] * x[0] + p[1] * x[1] + x[0])
    assert solve(m) == pytest.approx(15.0)
    m.update(p=[3.0, 2.0])
    assert solve(m) == pytest.approx(20.0)


def test_nonlinear_rejection_unchanged():
    m = rm.Model()
    x = m.vars("x", 2, ub=5.0)
    with pytest.raises(rm.UnsupportedExpressionError):
        x[0] * x[1]
    with pytest.raises(rm.UnsupportedExpressionError):
        (x[0] + 1.0) * (x[1] + 1.0)
    with pytest.raises(rm.UnsupportedExpressionError):
        (x[0] * 2.0) * x[1]
    # Parameter-only times decision is fine, even when nested.
    p = m.param("q", 2.0)
    m.add(x[0] + x[1] <= 4.0)
    m.maximize(p * (p * x[0] + x[1]))
    assert solve(m) == pytest.approx(16.0)


def test_foreign_model_rejection_unchanged():
    a, b = rm.Model("a"), rm.Model("b")
    x = a.var("x")
    y = b.var("y")
    with pytest.raises(rm.ModelMismatchError):
        x + y
    with pytest.raises(rm.ModelMismatchError):
        (x + 1.0) + y
    with pytest.raises(rm.ModelMismatchError):
        2.0 * (x + y)


def test_deep_chain_lowes_without_recursion():
    # A 200k-deep left-associated tree flattens and tears down without
    # touching the Rust call stack (iterative lowering + iterative drop).
    # No solve here: general-path backend sync at this size is a known
    # pre-existing cost (packed 200k solves in ~5 s; general-path sync is
    # P2/P3 territory, not P1D). Value correctness at depth is covered by
    # test_objective_equivalence_lazy_vs_bulk with a solve.
    n = 200_000
    m = rm.Model()
    x = m.vars("x", n, ub=1.0)
    total = x[0]
    for i in range(1, n):
        total = total + x[i]
    m.add(total <= n)
    m.minimize(total)
    # The temporaries must also tear down without stack overflow.
    del total
    m2 = rm.Model()
    y = m2.vars("x", 10, ub=1.0)
    t = y[0]
    for i in range(1, 10):
        t = t + y[i]
    m2.maximize(t)
    assert solve(m2) == pytest.approx(10.0)


def test_objective_equivalence_lazy_vs_bulk():
    n = 5000
    q = np.linspace(1.0, 5.0, n)
    m = rm.Model()
    x = m.vars("x", n, ub=2.0)
    m.add(rm.sum(x) <= 3000.0)
    total = None
    for i in range(n):
        term = q[i] * x[i]
        total = term if total is None else total + term
    m.maximize(total)
    lazy_obj = solve(m)
    m2 = rm.Model()
    y = m2.vars("x", n, ub=2.0)
    m2.add(rm.sum(y) <= 3000.0)
    m2.maximize(rm.dot(q, y))
    assert lazy_obj == pytest.approx(solve(m2))


def test_constraint_equivalence_scalar_vs_rows():
    m = rm.Model()
    x = m.vars("x", 3, ub=5.0)
    m.add(2 * x[0] + 3 * x[1] - x[2] + 4 <= 20.0)
    m.maximize(x[0] + x[1] + x[2])
    assert solve(m) == pytest.approx(41.0 / 3.0)


def test_rsub_and_neg_forms():
    m = rm.Model()
    x = m.vars("x", 2, ub=5.0)
    m.add(x[0] + x[1] <= 4.0)
    m.maximize(10.0 - (x[0] + x[1]))
    assert solve(m) == pytest.approx(10.0)
    m2 = rm.Model()
    y = m2.vars("x", 2, ub=5.0)
    m2.add(y[0] + y[1] <= 4.0)
    m2.maximize(3.0 - y[0] - y[1])
    assert solve(m2) == pytest.approx(3.0)
