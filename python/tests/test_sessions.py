"""MPY-04 sessions: binding, options, lifecycle."""

import pytest

import roml as rm


def test_solver_binds_first_model():
    a, b = rm.Model("a"), rm.Model("b")
    x = a.var("x", ub=1.0)
    a.maximize(x)
    y = b.var("y", ub=1.0)
    b.maximize(y)
    with rm.Highs() as solver:
        solver.solve(a)
        with pytest.raises(rm.ModelMismatchError):
            solver.solve(b)
        # The bound model still solves.
        assert solver.solve(a).is_optimal


def test_per_call_options_do_not_leak():
    m = rm.Model()
    x = m.var("x", ub=10.0)
    m.maximize(x)
    with rm.Highs(time_limit=60.0) as solver:
        first = solver.solve(m, time_limit=60.0)
        assert first.is_optimal
        assert first.metadata["effective_time_limit"] == pytest.approx(60.0)
        second = solver.solve(m)
        assert second.is_optimal
        # Omitted values inherit constructor defaults, not the override.
        assert second.metadata["effective_time_limit"] == pytest.approx(60.0)


def test_invalid_options_rejected():
    with pytest.raises(rm.InvalidModelError):
        rm.Highs(threads=0)
    with pytest.raises(rm.InvalidModelError):
        rm.Highs(time_limit=-1.0)
    with pytest.raises(rm.InvalidModelError):
        rm.Highs(relative_gap=2.0)
    m = rm.Model()
    x = m.var("x", ub=1.0)
    m.maximize(x)
    with rm.Highs() as solver:
        with pytest.raises(rm.InvalidModelError):
            solver.solve(m, time_limit=-5.0)


def test_context_manager_closes():
    m = rm.Model()
    x = m.var("x", ub=1.0)
    m.maximize(x)
    with rm.Highs() as solver:
        assert solver.solve(m).is_optimal
    with pytest.raises(rm.ClosedSessionError):
        solver.solve(m)


def test_empty_model_solve_is_optimal_without_primal():
    with rm.Highs() as solver:
        result = solver.solve(rm.Model())
    assert result.status == rm.SolveStatus.OPTIMAL
    assert result.objective is None
    assert not result.has_primal


def test_close_during_solve_reports_busy():
    import threading

    m = rm.Model()
    x = m.vars("x", 60, ub=5.0)
    for i in range(60):
        m.add(x[i] + x[(i + 1) % 60] <= 6.0)
    m.maximize(rm.sum(x))
    seen_busy = []
    for _ in range(10):
        solver = rm.Highs()
        stop = threading.Event()
        errors = []

        def hammer():
            while not stop.is_set():
                try:
                    solver.solve(m)
                except (rm.SessionBusyError, rm.ClosedSessionError):
                    pass
                except Exception as e:  # noqa: BLE001 - any other type fails the test
                    errors.append(e)

        threads = [threading.Thread(target=hammer) for _ in range(3)]
        for t in threads:
            t.start()
        try:
            for _ in range(2000):
                try:
                    solver.close()
                    break
                except rm.SessionBusyError:
                    seen_busy.append(True)
                    break
        finally:
            stop.set()
            for t in threads:
                t.join(timeout=60)
        assert not errors
        if seen_busy:
            break
    assert seen_busy, "expected to observe close-during-solve contention"
