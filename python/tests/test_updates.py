"""MPY-03 atomic named updates: rollback, overflow, dtype handling."""

import numpy as np
import pytest

import roml as rm


def test_mixed_batch_rejects_entirely():
    m = rm.Model()
    x = m.vars("x", 2, ub=10.0)
    p = m.params("price", [1.0, 2.0])
    m.maximize(rm.dot(p, x))
    with rm.Highs() as solver:
        assert solver.solve(m).objective == pytest.approx(30.0)
        # Second element invalid (NaN): nothing is installed.
        with pytest.raises(rm.InvalidModelError):
            m.update(price=[5.0, float("nan")])
        assert solver.solve(m).objective == pytest.approx(30.0)


def test_unknown_and_wrong_shape_reject():
    m = rm.Model()
    m.params("price", [1.0, 2.0])
    with pytest.raises(rm.InvalidModelError):
        m.update(nope=1.0)
    with pytest.raises(rm.ShapeError):
        m.update(price=[1.0, 2.0, 3.0])
    with pytest.raises(rm.ShapeError):
        m.update(price=[[1.0, 2.0]])
    # Scalar broadcast into an array parameter is rejected (exact-shape only).
    with pytest.raises(rm.ShapeError):
        m.update(price=1.0)


def test_derived_overflow_rejects_batch():
    m = rm.Model()
    x = m.var("x", ub=1e308)
    cap = m.param("cap", 10.0)
    m.add(x <= 2.0 * cap)
    m.maximize(x)
    with rm.Highs() as solver:
        assert solver.solve(m).objective == pytest.approx(20.0)
        # Finite input producing a non-finite derived bound rejects.
        with pytest.raises(rm.InvalidModelError):
            m.update(cap=1e308)
        # Prior state untouched.
        assert solver.solve(m).objective == pytest.approx(20.0)


def test_update_rejects_variable_names():
    m = rm.Model()
    m.var("x")
    m.param("p", 1.0)
    with pytest.raises(rm.InvalidModelError):
        m.update(x=2.0)


def test_bool_and_ragged_inputs_rejected():
    m = rm.Model()
    m.params("p", [1.0, 2.0])
    with pytest.raises(rm.InvalidModelError):
        m.update(p=[True, False])
    with pytest.raises(rm.ShapeError):
        m.update(p=[[1.0], [2.0, 3.0]])
    with pytest.raises(rm.InvalidModelError):
        m.update(p=["a", "b"])


def test_readonly_and_noncontiguous_inputs_accepted():
    m = rm.Model()
    p = m.params("p", [1.0, 2.0, 3.0, 4.0])
    x = m.vars("x", 4, ub=1.0)
    m.maximize(rm.dot(p, x))
    base = np.array([10.0, 20.0, 30.0, 40.0])
    view = base[::2]
    assert not view.flags["C_CONTIGUOUS"]
    ro = np.array([10.0, 20.0, 30.0, 40.0])
    ro.setflags(write=False)
    with rm.Highs() as solver:
        m.update(p=ro)
        assert solver.solve(m).objective == pytest.approx(100.0)
        # Mutating the caller's array afterwards cannot affect stored state.
        ro2 = np.array([10.0, 20.0, 30.0, 40.0])
        m.update(p=ro2)
        ro2[0] = -999.0
        assert solver.solve(m).objective == pytest.approx(100.0)


def test_dot_with_param_constants_inside_affine():
    # rm.dot(price, discharge - charge): parameter-only left, affine right
    # with parameter-dependent coefficients inside.
    m = rm.Model()
    n = 3
    price = m.params("price", [5.0, 1.0, 4.0])
    scale = m.param("scale", 2.0)
    charge = m.vars("charge", n, ub=2.0)
    discharge = m.vars("discharge", n, ub=2.0)
    m.add(charge <= 1.0)
    m.add(discharge <= 1.0)
    m.maximize(rm.dot(price, scale * discharge - charge))
    with rm.Highs() as solver:
        result = solver.solve(m)
        # Per unit: 2*price[i] discharge revenue vs price[i] charge cost;
        # discharge everything, charge nothing: 2*(5+1+4) = 20.
        assert result.objective == pytest.approx(20.0)


def test_decision_times_decision_rejected():
    m = rm.Model()
    x = m.vars("x", 2, ub=1.0)
    y = m.vars("y", 2, ub=1.0)
    with pytest.raises(rm.UnsupportedExpressionError):
        x * y
    with pytest.raises(rm.UnsupportedExpressionError):
        rm.dot(x, y)


def test_vars_rejects_atomically_on_later_element():
    m = rm.Model()
    with pytest.raises(rm.InvalidModelError):
        m.vars("x", 3, lb=[0.0, 0.0, 5.0], ub=[1.0, 1.0, 1.0])
    # Nothing reserved: the name is reusable and no orphans persist.
    x = m.vars("x", 3, lb=[0.0, 0.0, 0.0], ub=[1.0, 1.0, 1.0])
    assert x.shape == (3,)
    m.maximize(rm.sum(x))
    with rm.Highs() as solver:
        assert solver.solve(m).objective == pytest.approx(3.0)


def test_coefficient_overflow_rejects_batch():
    m = rm.Model()
    x = m.vars("x", 2, ub=10.0)
    a = m.param("a", 2.0)
    b = m.param("b", 3.0)
    m.add(x[0] + x[1] <= 10.0)
    # Objective coefficient a*b is parameter-dependent.
    m.maximize(a * b * (x[0] + x[1]))
    with rm.Highs() as solver:
        assert solver.solve(m).objective == pytest.approx(60.0)
        # Finite inputs driving the coefficient to +inf reject atomically.
        with pytest.raises(rm.InvalidModelError):
            m.update(a=1e308, b=1e308)
        assert solver.solve(m).objective == pytest.approx(60.0)


def test_cross_model_values_rejected():
    m1 = rm.Model()
    x1 = m1.vars("x", 2, ub=5.0)
    m1.maximize(rm.sum(x1))
    m2 = rm.Model()
    x2 = m2.vars("x", 2, ub=5.0)
    m2.maximize(rm.sum(x2))
    with rm.Highs() as solver1, rm.Highs() as solver2:
        r1 = solver1.solve(m1)
        solver2.solve(m2)
        with pytest.raises(rm.ModelMismatchError):
            r1.values(x2)
        with pytest.raises(rm.ModelMismatchError):
            r1.value(x2[0])


def test_array_path_coefficient_overflow_rejects_batch():
    m = rm.Model()
    x = m.vars("x", 2, ub=10.0)
    a = m.param("a", 2.0)
    b = m.param("b", 3.0)
    # Array-lowered constraint with a product coefficient a*b.
    m.add((a * b) * x <= 10.0)
    m.maximize(rm.sum(x))
    with rm.Highs() as solver:
        assert solver.solve(m).objective == pytest.approx(20.0 / 6.0)
        # Finite inputs driving the recorded coefficient to +inf reject.
        with pytest.raises(rm.InvalidModelError):
            m.update(a=1e308, b=1e308)
        assert solver.solve(m).objective == pytest.approx(20.0 / 6.0)
