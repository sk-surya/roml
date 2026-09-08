"""MPY-03 CSR bulk rows: validation, accumulation, round trips."""

import numpy as np
import pytest

import roml as rm


def small_model():
    m = rm.Model()
    x = m.vars("x", 3, ub=10.0)
    return m, x


def test_csr_round_trip_known_objective():
    # min x0 + 2 x1 + 3 x2 s.t. x0+x1+x2 >= 6, x <= 10 → 12 at (0,0,...)?
    # Cheapest is x0: put 6 into x0 → 6.0.
    m, x = small_model()
    m.add_linear_rows(
        [0, 3],
        [0, 1, 2],
        [1.0, 1.0, 1.0],
        variables=x,
        lower=6.0,
        upper=np.inf,
    )
    m.minimize(x[0] + 2.0 * x[1] + 3.0 * x[2])
    with rm.Highs() as solver:
        result = solver.solve(m)
        assert result.objective == pytest.approx(6.0)
        assert result.value(x[0]) == pytest.approx(6.0)


def test_csr_duplicate_entries_accumulate():
    m, x = small_model()
    # Row: 1*x0 + 2*x0 + 1*x1 >= 3 → 3*x0 + x1 >= 3; min x0+x1 → 1.0 at x1=3? no:
    # min x0 + x1 s.t. 3x0 + x1 >= 3 → x0=1 → 1.0.
    cons = m.add_linear_rows(
        [0, 3],
        [0, 0, 1],
        [1.0, 2.0, 1.0],
        variables=x,
        lower=3.0,
        upper=np.inf,
        name="rows",
    )
    assert cons.shape == (1,)
    m.minimize(x[0] + x[1])
    with rm.Highs() as solver:
        result = solver.solve(m)
        assert result.objective == pytest.approx(1.0)


def test_csr_malformed_rejects_before_any_row():
    m, x = small_model()
    cases = [
        # indptr[0] != 0
        dict(indptr=[1, 3], indices=[0, 1], data=[1.0, 1.0], lower=0.0, upper=1.0),
        # non-monotone indptr
        dict(indptr=[0, 2, 1], indices=[0, 1], data=[1.0, 1.0], lower=0.0, upper=1.0),
        # last pointer mismatch
        dict(indptr=[0, 2], indices=[0, 1, 2], data=[1.0, 1.0, 1.0], lower=0.0, upper=1.0),
        # column out of range
        dict(indptr=[0, 1], indices=[5], data=[1.0], lower=0.0, upper=1.0),
        # non-finite data
        dict(indptr=[0, 1], indices=[0], data=[float("nan")], lower=0.0, upper=1.0),
        # inverted bounds
        dict(indptr=[0, 1], indices=[0], data=[1.0], lower=2.0, upper=1.0),
        # bool indices
        dict(indptr=[0, 1], indices=[True], data=[1.0], lower=0.0, upper=1.0),
    ]
    for kwargs in cases:
        with pytest.raises((rm.ShapeError, rm.InvalidModelError)):
            m.add_linear_rows(
                kwargs["indptr"],
                kwargs["indices"],
                kwargs["data"],
                variables=x,
                lower=kwargs["lower"],
                upper=kwargs["upper"],
            )
    # No rows were added by any rejected batch: a trivial model still solves.
    m.minimize(x[0])
    with rm.Highs() as solver:
        assert solver.solve(m).is_optimal


def test_csr_explicit_infinities_and_broadcast():
    m, x = small_model()
    cons = m.add_linear_rows(
        [0, 1, 2],
        [0, 1],
        [1.0, 1.0],
        variables=x,
        lower=[0.0, float("-inf")],
        upper=[10.0, float("inf")],
    )
    assert cons.shape == (2,)
    m.minimize(x[0] + x[1])
    with rm.Highs() as solver:
        assert solver.solve(m).objective == pytest.approx(0.0)


def test_csr_numpy_inputs_and_scale():
    # 1k-coefficient bulk build with NumPy inputs, then solve.
    n = 100
    m = rm.Model()
    x = m.vars("x", n, ub=5.0)
    rng = np.random.default_rng(20260907)
    indptr = [0]
    indices = []
    data = []
    for _ in range(10):
        row_idx = rng.integers(0, n, size=10)
        indices.extend(int(i) for i in row_idx)
        data.extend(float(v) for v in rng.random(10))
        indptr.append(len(indices))
    cons = m.add_linear_rows(
        np.asarray(indptr, dtype=np.int64),
        np.asarray(indices, dtype=np.int64),
        np.asarray(data),
        variables=x,
        lower=np.zeros(10),
        upper=np.full(10, np.inf),
    )
    assert cons.shape == (10,)
    m.minimize(rm.sum(x))
    with rm.Highs() as solver:
        result = solver.solve(m)
        assert result.is_optimal
        assert result.objective == pytest.approx(0.0)
