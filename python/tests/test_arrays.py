"""MPY-03 arrays, atomic updates, CSR (packet test seeds)."""

import numpy as np
import pytest

import roml as rm


def test_rejected_batch_preserves_prior_pending_update():
    m = rm.Model()
    x = m.vars("x", 2, ub=1.0)
    p = m.params("price", [1.0, 2.0])
    m.param("unused", 0.0)
    m.maximize(rm.dot(p, x))
    with rm.Highs() as solver:
        solver.solve(m)
        m.update(price=[3.0, 4.0])
        with pytest.raises(ValueError):
            m.update(price=[8.0, 9.0], unused=float("nan"))
        assert solver.solve(m).objective == pytest.approx(7.0)


def test_array_shape_and_empty_reduction():
    m = rm.Model()
    x = m.vars("x", (2, 3), ub=1.0)
    assert x.shape == (2, 3)
    assert x[0, :].shape == (3,)
    with pytest.raises(rm.ShapeError):
        rm.dot(np.ones((3, 2)), x)
    assert rm.sum(m.vars("empty", 0)) == 0.0


def test_indexing_negative_slices_ellipsis():
    m = rm.Model()
    x = m.vars("x", (2, 3), ub=10.0)
    assert x[0, :].shape == (3,)
    assert x[:, 1].shape == (2,)
    assert x[...].shape == (2, 3)
    assert x[0:2, 0:2].shape == (2, 2)
    assert x[1].shape == (3,)
    with pytest.raises(rm.ShapeError):
        x[2, 0]
    with pytest.raises(rm.ShapeError):
        x[0, 3]
    with pytest.raises(rm.ShapeError):
        x[0, 0, 0]
    with pytest.raises(rm.ShapeError):
        x[0.5]
    # Slices solve through to the right values.
    m.add(x[0, :] <= 1.0)
    m.add(x[1, 0] + x[1, 1] + x[1, 2] <= 6.0)
    m.maximize(rm.sum(x))
    with rm.Highs() as solver:
        result = solver.solve(m)
        assert result.objective == pytest.approx(9.0)
        got = result.values(x)
        assert got.shape == (2, 3)
        assert float(np.sum(got)) == pytest.approx(9.0)
        assert result.value(x[-1, -1]) == pytest.approx(float(got[1, 2]))


def test_zero_dimensional_and_scalar_broadcast():
    m = rm.Model()
    p = m.params("scalar_like", 2.0)
    assert p.shape == ()
    x = m.vars("x", 2, ub=10.0)
    m.add(x <= p)
    m.maximize(rm.sum(x))
    with rm.Highs() as solver:
        result = solver.solve(m)
        assert result.objective == pytest.approx(4.0)
    with pytest.raises(rm.ShapeError):
        m.vars("bad", (2, 3), lb=np.ones((3, 2)))


def test_numpy_bool_and_bytes_rejected():
    import numpy as np

    m = rm.Model()
    with pytest.raises((rm.ShapeError, rm.InvalidModelError)):
        m.vars("x", np.True_)
    m2 = rm.Model()
    with pytest.raises(rm.InvalidModelError):
        m2.params("p", b"\x01\x02")
    m3 = rm.Model()
    y = m3.vars("y", 3, ub=5.0)
    with pytest.raises(rm.ShapeError):
        y[np.True_]


def test_csr_multidimensional_indices_rejected():
    import numpy as np

    m = rm.Model()
    x = m.vars("x", 3, ub=10.0)
    with pytest.raises(rm.ShapeError):
        m.add_linear_rows(
            np.array([[0, 1]]),
            np.array([0]),
            np.array([1.0]),
            variables=x,
            lower=0.0,
            upper=1.0,
        )
