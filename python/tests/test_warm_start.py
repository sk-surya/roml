"""MPY-04 warm starts: explicit requests with honest disposition."""

import pytest

import roml as rm


def mip_model():
    m = rm.Model()
    n = m.vars("n", 4, kind="binary")
    x = m.vars("x", 4, ub=5.0)
    for i in range(4):
        m.add(x[i] <= 1.0 + 4.0 * n[i])
    m.maximize(rm.sum(x))
    return m, n, x


def test_start_same_model_solves_and_reports():
    m, n, x = mip_model()
    with rm.Highs() as solver:
        first = solver.solve(m)
        assert first.is_optimal
        assert first.objective == pytest.approx(20.0)
        second = solver.solve(m, start=first)
        assert second.is_optimal
        assert second.objective == pytest.approx(20.0)
        assert second.metadata["warm_start"] in ("applied", "requested_not_applied")
        # The disposition is measured, never fabricated from object reuse:
        # a fresh solver with the same start reports its own disposition.
    with rm.Highs() as fresh:
        third = fresh.solve(m, start=first)
        assert third.is_optimal
        assert third.metadata["warm_start"] in ("applied", "requested_not_applied")


def test_start_foreign_model_rejected():
    a, _, _ = mip_model()
    b, _, _ = mip_model()
    with rm.Highs() as solver:
        result = solver.solve(a)
        with pytest.raises(rm.ModelMismatchError):
            solver.solve(b, start=result)


def test_start_without_primal_rejected():
    m, _, _ = mip_model()
    empty = rm.Model()
    z = empty.var("z", ub=1.0)
    empty.add(z >= 2.0)
    empty.minimize(z)
    with rm.Highs() as solver:
        bad = solver.solve(empty)
        assert not bad.has_primal
        with pytest.raises(rm.InvalidModelError):
            solver.solve(m, start=bad)
