"""MPY-04 concurrency: detached solves, busy errors, heartbeats."""

import threading
import time

import pytest

import roml as rm


def make_model(n=20):
    m = rm.Model()
    x = m.vars("x", n, ub=5.0)
    for i in range(n):
        m.add(x[i] + x[(i + 1) % n] <= 6.0)
    m.maximize(rm.sum(x))
    return m, x


def test_heartbeat_progresses_during_native_solve():
    m, x = make_model(200)
    beats = []
    stop = threading.Event()

    def heartbeat():
        while not stop.is_set():
            beats.append(time.monotonic())
            time.sleep(0.001)

    pulse = threading.Thread(target=heartbeat)
    pulse.start()
    try:
        with rm.Highs() as solver:
            result = solver.solve(m)
            assert result.is_optimal
    finally:
        stop.set()
        pulse.join()
    # The GIL is released during native work: the heartbeat progressed.
    assert len(beats) >= 2


def test_overlapping_solves_are_safe():
    # Barrier-aligned solves on one shared session: every call either
    # succeeds or reports a deterministic busy error. No hangs, no
    # crashes, no fabricated results.
    m, x = make_model(30)
    solver = rm.Highs()
    barrier = threading.Barrier(8)
    outcomes = []

    def worker():
        barrier.wait()
        try:
            result = solver.solve(m)
            outcomes.append(("ok", result.is_optimal))
        except (rm.SessionBusyError, rm.ModelBusyError) as e:
            outcomes.append(("busy", type(e).__name__))

    threads = [threading.Thread(target=worker) for _ in range(8)]
    for t in threads:
        t.start()
    for t in threads:
        t.join(timeout=60)
        assert not t.is_alive()
    solver.close()
    assert len(outcomes) == 8
    assert all(kind in ("ok", "busy") for kind, _ in outcomes)
    assert all(flag for kind, flag in outcomes if kind == "ok")


def test_independent_sessions_run_concurrently():
    results = []

    def worker(seed):
        m, x = make_model(15)
        with rm.Highs() as solver:
            results.append(solver.solve(m).objective)

    threads = [threading.Thread(target=worker, args=(i,)) for i in range(4)]
    for t in threads:
        t.start()
    for t in threads:
        t.join(timeout=60)
    assert len(results) == 4
    assert all(v == pytest.approx(45.0) for v in results)


def test_worker_thread_create_use_drop():
    def worker():
        m, x = make_model(10)
        with rm.Highs() as solver:
            assert solver.solve(m).is_optimal

    threads = [threading.Thread(target=worker) for _ in range(8)]
    for t in threads:
        t.start()
    for t in threads:
        t.join(timeout=60)
        assert not t.is_alive()
