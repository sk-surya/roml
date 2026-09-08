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


def test_failed_array_insertion_preserves_model():
    # P1-2: overflow discovered in a later row must not install earlier
    # rows. The model still solves to the original optimum afterwards.
    m = rm.Model()
    x = m.vars("x", 2, ub=10.0)
    p = m.params("p", [1.0, 1e308])
    m.maximize(x[0])
    with rm.Highs() as solver:
        assert solver.solve(m).value(x[0]) == pytest.approx(10.0)
        with pytest.raises(rm.InvalidModelError):
            m.add((p * 2) * x <= [1.0, 1.0], name="rows")
        assert solver.solve(m).value(x[0]) == pytest.approx(10.0)
        assert solver.solve(m).is_current(m)


def test_scalar_complex_coefficient_repricing():
    # P1-3: a valid scalar update against a product coefficient must
    # apply; an overflowing one must reject without partial mutation.
    m = rm.Model()
    x = m.var("x", ub=10.0)
    p = m.param("p", 1.0)
    m.add((2 * p) * x <= 10)
    m.maximize(x)
    with rm.Highs() as solver:
        assert solver.solve(m).objective == pytest.approx(5.0)
        m.update(p=2.0)
        assert solver.solve(m).objective == pytest.approx(2.5)
        with pytest.raises(rm.InvalidModelError):
            m.update(p=1e308)
        assert solver.solve(m).objective == pytest.approx(2.5)


def test_zero_dimensional_update_round_trip():
    # P2-7: 0-d parameter arrays accept scalar and 0-d updates; (1,)
    # inputs stay rejected under exact-shape discipline.
    import numpy as np

    m = rm.Model()
    p = m.params("p", 1.0)
    assert p.shape == ()
    x = m.var("x", ub=10.0)
    m.add(x <= p)
    m.maximize(x)
    with rm.Highs() as solver:
        m.update(p=2.0)
        assert solver.solve(m).objective == pytest.approx(2.0)
        m.update(p=np.array(3.0))
        assert solver.solve(m).objective == pytest.approx(3.0)
        with pytest.raises(rm.ShapeError):
            m.update(p=np.array([4.0]))
        assert solver.solve(m).objective == pytest.approx(3.0)


def test_consecutive_updates_before_solve():
    # P1 (owner repro): the second update must validate against the
    # first update's accepted values, not stale committed ones.
    m = rm.Model()
    x = m.var("x", ub=100.0)
    a = m.param("a", 1.0)
    b = m.param("b", 1.0)
    m.add(x <= a + b)
    m.maximize(x)
    with rm.Highs() as solver:
        assert solver.solve(m).objective == pytest.approx(2.0)
        m.update(a=2.0)
        m.update(b=3.0)
        assert solver.solve(m).objective == pytest.approx(5.0)


def test_sequential_equals_combined_batch():
    import numpy as np

    # Scalar path.
    m1 = rm.Model()
    m2 = rm.Model()
    for m in (m1, m2):
        x = m.var("x", ub=100.0)
        a = m.param("a", 1.0)
        b = m.param("b", 1.0)
        m.add(x <= a + b)
        m.maximize(x)
    m1.update(a=2.0, b=3.0)
    m2.update(a=2.0)
    m2.update(b=3.0)
    with rm.Highs() as s1, rm.Highs() as s2:
        assert s1.solve(m1).objective == pytest.approx(s2.solve(m2).objective)
        assert s1.solve(m1).objective == pytest.approx(5.0)

    # Array path: objective coefficients updated in one vs two batches.
    n1 = rm.Model()
    n2 = rm.Model()
    for m in (n1, n2):
        x = m.vars("x", 2, ub=100.0)
        p = m.params("p", [1.0, 1.0])
        m.add(x[0] + x[1] <= 100.0)
        m.maximize(rm.dot(p, x))
    n1.update(p=np.array([2.0, 3.0]))
    n2.update(p=np.array([2.0, 1.0]))
    n2.update(p=np.array([2.0, 3.0]))
    with rm.Highs() as s1, rm.Highs() as s2:
        assert s1.solve(n1).objective == pytest.approx(s2.solve(n2).objective)


def test_second_update_overflow_rejects_then_solves():
    # Overflow on the second update rejects without corrupting the
    # first update's accepted state; the model still solves.
    # (Magnitudes stay inside the backend's tractable finite range so
    # the surviving state solves cleanly.)
    m = rm.Model()
    x = m.var("x", ub=1e308)
    a = m.param("a", 1.0)
    b = m.param("b", 1.0)
    m.add((a * b) * x <= 10)
    m.maximize(x)
    with rm.Highs() as solver:
        m.update(a=1e10)
        with pytest.raises(rm.InvalidModelError):
            m.update(b=1e300)
        result = solver.solve(m)
        assert result.is_optimal
        assert result.objective == pytest.approx(10.0 / 1e10)


def test_lowering_after_pending_update_uses_fresh_values():
    # Expression lowering after an accepted-but-uncommitted update must
    # see the pending values, not stale committed ones.
    m = rm.Model()
    x = m.var("x", ub=100.0)
    a = m.param("a", 1.0)
    m.update(a=4.0)
    m.add(x <= a)
    m.maximize(x)
    with rm.Highs() as solver:
        assert solver.solve(m).objective == pytest.approx(4.0)
