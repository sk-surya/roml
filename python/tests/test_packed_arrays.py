"""P1C-1 locks: packed array expressions preserve exact scalar semantics.

These pass on the Vec<Affine> implementation and must pass unchanged on
the packed representation (zero semantic differences is the gate).
"""

import numpy as np
import pytest

import roml as rm


def solve_optimum(build):
    m = build()
    with rm.Highs() as s:
        r = s.solve(m)
    assert r.is_optimal
    return r.objective


def test_small_bess_idiomatic_matches_csr():
    n, dt, eta = 4, 0.25, 0.95
    prices = np.array([50.0, 30.0, 70.0, 40.0])

    def build_idiomatic():
        m = rm.Model("bess")
        charge = m.vars("charge", n, ub=2.0)
        discharge = m.vars("discharge", n, ub=2.0)
        energy = m.vars("energy", n + 1, ub=4.0)
        m.add(energy[0] == 2.0)
        m.add(
            energy[1:]
            == energy[:-1] + dt * (eta * charge - discharge / eta)
        )
        m.add(charge + discharge <= 2.0)
        m.maximize(rm.dot(prices, discharge - charge))
        return m

    def build_csr():
        m = rm.Model("bess-csr")
        nvars = 2 * n + (n + 1)
        v = m.vars(
            "v", nvars,
            lb=np.zeros(nvars),
            ub=np.concatenate([np.full(n, 2.0), np.full(n, 2.0), np.full(n + 1, 4.0)]),
        )
        ch = v[0:n]
        di = v[n: 2 * n]
        en = v[2 * n:]
        rows_ptr, rows_idx, rows_data, lower, upper = [0], [], [], [], []
        # init row
        rows_idx += [2 * n]
        rows_data += [1.0]
        lower += [2.0]
        upper += [2.0]
        rows_ptr.append(len(rows_idx))
        for t in range(n):
            base = len(rows_idx)
            rows_idx += [2 * n + t + 1, 2 * n + t, t, n + t]
            rows_data += [1.0, -1.0, -dt * eta, dt / eta]
            lower += [0.0]
            upper += [0.0]
            rows_ptr.append(len(rows_idx))
            assert len(rows_idx) - base == 4
        for t in range(n):
            rows_idx += [t, n + t]
            rows_data += [1.0, 1.0]
            lower += [-np.inf]
            upper += [2.0]
            rows_ptr.append(len(rows_idx))
        m.add_linear_rows(
            np.array(rows_ptr, dtype=np.int64),
            np.array(rows_idx, dtype=np.int64),
            np.array(rows_data),
            variables=v,
            lower=np.array(lower),
            upper=np.array(upper),
        )
        m.maximize(rm.dot(prices, di - ch))
        return m

    assert solve_optimum(build_idiomatic) == pytest.approx(
        solve_optimum(build_csr)
    )


def test_array_arithmetic_matches_scalar():
    m1, m2 = rm.Model("a"), rm.Model("b")
    x1, x2 = m1.vars("x", 3, ub=5.0), m2.vars("x", 3, ub=5.0)
    m1.add(2.0 * x1 - x1 <= 4.0)
    e = 2.0 * x1 - x1
    m1.add(-e >= -4.0)
    m2.add(x2[0] + x2[1] + x2[2] <= 4.0)
    m2.add(-x2[0] - x2[1] - x2[2] >= -4.0)
    m1.minimize(rm.sum(x1))
    m2.minimize(rm.sum(x2))
    with rm.Highs() as s1, rm.Highs() as s2:
        assert s1.solve(m1).objective == pytest.approx(s2.solve(m2).objective)


def test_scaled_sliced_arrays():
    m = rm.Model()
    x = m.vars("x", (2, 3), ub=10.0)
    y = 3.0 * x[0, :] - x[1, :] / 2.0
    m.add(y <= 6.0)
    m.add(x >= 1.0)
    m.minimize(rm.sum(x))
    with rm.Highs() as s:
        r = s.solve(m)
    assert r.is_optimal
    # Per column: 3a - b/2 <= 6 with a, b >= 1 minimized at a = b = 1.
    assert r.objective == pytest.approx(6.0)


def test_parameterized_array_op_still_works():
    m = rm.Model()
    x = m.vars("x", 2, ub=5.0)
    p = m.params("p", [1.0, 2.0])
    m.add(p * x <= 4.0)
    m.maximize(rm.dot(p, x))
    with rm.Highs() as s:
        assert s.solve(m).objective == pytest.approx(8.0)
        m.update(p=[2.0, 1.0])
        assert s.solve(m).objective == pytest.approx(8.0)


def test_mixed_packed_and_general():
    m = rm.Model()
    x = m.vars("x", 2, ub=5.0)
    p = m.param("q", 1.0)
    # Packed array part plus a parameter-dependent scalar part.
    m.add(x + p * x <= 6.0)
    m.minimize(rm.sum(x))
    with rm.Highs() as s:
        assert s.solve(m).objective == pytest.approx(0.0)


def test_zero_diff_rows_valid():
    m = rm.Model()
    x = m.vars("x", 2, ub=5.0)
    m.add(x - x <= 0.0)
    m.add(x - x == 0.0)
    m.minimize(rm.sum(x))
    with rm.Highs() as s:
        assert s.solve(m).objective == pytest.approx(0.0)


def test_empty_comparison_array_add():
    m = rm.Model()
    x = m.vars("x", 0, ub=1.0)
    y = x + x
    arr = m.add(y <= 1.0, name="empty_rows")
    assert len(arr) == 0
