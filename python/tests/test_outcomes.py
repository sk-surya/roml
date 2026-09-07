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
        result = solver.solve(m, time_limit=1e-9)
    assert result.status == rm.SolveStatus.TIME_LIMIT
    if result.has_primal:
        assert result.objective is not None
        v = result.value(xs[0])
        assert v in (0.0, 1.0)
    else:
        assert result.objective is None
        with pytest.raises(rm.NoSolutionError):
            result.value(xs[0])
    assert result.metadata["effective_time_limit"] == pytest.approx(1e-9)


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
