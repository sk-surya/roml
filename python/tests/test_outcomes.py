"""MPY-04 outcomes: infeasible/unbounded/limits/unknown are results."""

import pytest

import roml as rm


def test_infeasible_is_a_result_not_a_fake_point():
    m = rm.Model()
    x = m.var("x", ub=1.0)
    m.add(x >= 2.0)
    m.minimize(x)
    with rm.Highs() as solver:
        result = solver.solve(m)
    assert result.status == rm.SolveStatus.INFEASIBLE
    assert not result.has_primal
    assert result.objective is None
    with pytest.raises(rm.NoSolutionError):
        result.value(x)


def test_unbounded_is_a_result():
    m = rm.Model()
    x = m.var("x", lb=float("-inf"), ub=float("inf"))
    m.minimize(x)
    with rm.Highs() as solver:
        result = solver.solve(m)
    assert result.status == rm.SolveStatus.UNBOUNDED
    assert not result.has_primal
    assert result.objective is None


def test_limit_with_or_without_incumbent_is_consistent():
    # A MIP no solver finishes in a nanosecond: the outcome is a time
    # limit, with or without an incumbent. Both are consistent results.
    m = rm.Model()
    n = 40
    xs = m.vars("x", n, kind="binary")
    for i in range(n):
        m.add(xs[i] + xs[(i + 1) % n] <= 1.0)
    m.maximize(rm.sum(xs))
    with rm.Highs() as solver:
        result = solver.solve(m, time_limit=1e-3)
    assert result.status == rm.SolveStatus.TIME_LIMIT
    if result.has_primal:
        assert result.objective is not None
        v = result.value(xs[0])
        assert v in (0.0, 1.0)
    else:
        assert result.objective is None
        with pytest.raises(rm.NoSolutionError):
            result.value(xs[0])
    assert result.metadata["effective_time_limit"] == pytest.approx(1e-3)


def test_limit_preserves_infeasibility():
    m = rm.Model()
    x = m.var("x", ub=1.0)
    m.add(x >= 2.0)
    m.minimize(x)
    with rm.Highs() as solver:
        result = solver.solve(m, time_limit=60.0)
    assert result.status == rm.SolveStatus.INFEASIBLE
    assert not result.has_primal


def test_close_is_idempotent_outcomes():
    solver = rm.Highs()
    solver.close()
    solver.close()
    with pytest.raises(rm.ClosedSessionError):
        solver.solve(rm.Model())


def test_timeout_without_incumbent_is_not_a_primal():
    # P1-1: a time limit with no solver-reported incumbent must not
    # expose buffer defaults as a usable primal. Either outcome is
    # contract-correct; an infeasible "solution" with infinite
    # objective is not.
    import numpy as np

    m = rm.Model()
    x = m.vars("x", 2000, kind="binary")
    m.add(rm.sum(x) >= 1000)
    m.maximize(rm.dot(np.random.default_rng(123).uniform(size=2000), x))
    with rm.Highs() as solver:
        result = solver.solve(m, time_limit=0.000001)
    assert result.status == rm.SolveStatus.TIME_LIMIT
    if result.has_primal:
        assert result.objective is not None
        assert result.objective != float("inf")
        values = np.asarray(result.values(x))
        assert set(np.unique(values)) <= {0.0, 1.0}
        assert float(values.sum()) >= 1000.0
    else:
        assert result.objective is None
        with pytest.raises(rm.NoSolutionError):
            result.values(x)
