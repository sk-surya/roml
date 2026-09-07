"""MPY-02 identity, names, bounds, and numeric validation."""

import pytest

import roml as rm


def test_duplicate_names_rejected():
    m = rm.Model()
    m.var("x")
    with pytest.raises(rm.InvalidModelError):
        m.var("x")
    with pytest.raises(rm.InvalidModelError):
        m.param("x", 1.0)
    m.add(m.var("y") <= 1.0, name="c")
    with pytest.raises(rm.InvalidModelError):
        m.add(m.var("z") <= 1.0, name="c")


def test_empty_names_rejected():
    m = rm.Model()
    with pytest.raises(rm.InvalidModelError):
        m.var("")
    with pytest.raises(rm.InvalidModelError):
        m.param("", 1.0)


def test_invalid_bounds_rejected():
    m = rm.Model()
    with pytest.raises(rm.InvalidModelError):
        m.var("a", lb=5.0, ub=1.0)
    with pytest.raises(rm.InvalidModelError):
        m.var("b", lb=float("nan"))
    # Omitted binary bounds produce the standard [0, 1] domain.
    binary_var = m.var("bin", kind="binary")
    assert binary_var is not None
    with pytest.raises(rm.InvalidModelError):
        m.var("c", lb=2.0, ub=3.0, kind="binary")
    with pytest.raises(rm.InvalidModelError):
        m.var("d", kind="quadratic")
    with pytest.raises(rm.InvalidModelError):
        m.var("e", lb=True)


def test_bool_and_nonfinite_coefficients_rejected():
    m = rm.Model()
    x = m.var("x")
    with pytest.raises(rm.InvalidModelError):
        m.param("p", True)
    with pytest.raises(rm.InvalidModelError):
        m.param("q", float("nan"))
    with pytest.raises(rm.InvalidModelError):
        m.param("r", float("inf"))
    with pytest.raises(rm.UnsupportedExpressionError):
        x * x
    with pytest.raises(rm.InvalidModelError):
        x / 0.0


def test_foreign_param_in_expression_rejected():
    a, b = rm.Model("a"), rm.Model("b")
    x = a.var("x")
    q = b.param("q", 2.0)
    with pytest.raises(rm.ModelMismatchError):
        a.maximize(q * x)


def test_stale_and_cross_model_solution_access():
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


def test_close_is_idempotent():
    solver = rm.Highs()
    solver.close()
    solver.close()
    with pytest.raises(rm.ClosedSessionError):
        solver.solve(rm.Model())


def test_old_result_survives_update():
    m = rm.Model()
    x = m.var("x", ub=10.0)
    p = m.param("p", 1.0)
    m.maximize(p * x)
    with rm.Highs() as solver:
        first = solver.solve(m)
        assert first.is_current(m)
        m.update(p=2.0)
        assert not first.is_current(m)
        # The old result still answers for its original revision.
        assert first.value(x) == pytest.approx(10.0)
        second = solver.solve(m)
        assert second.objective == pytest.approx(20.0)
        assert second.is_current(m)


def test_repr_is_bounded_and_descriptive():
    m = rm.Model("production")
    x = m.var("x")
    assert "vars" in repr(m)
    assert "x" in repr(x)
    # No implicit solve in repr: repeated reprs agree and carry no values.
    assert repr(x + 1.0) == repr(x + 1.0)
    assert "Expr" in repr(x + 1.0)


def test_chained_comparison_fails_loudly():
    m = rm.Model()
    x = m.var("x")
    with pytest.raises(TypeError):
        0 <= x <= 1  # noqa: B015
    with pytest.raises(TypeError):
        bool(x + 1.0)


def test_division_only_by_nonzero_constants():
    m = rm.Model()
    x = m.var("x")
    p = m.param("p", 2.0)
    y = (x + 2.0 * p) / 2.0
    m.add(y <= 5.0)
    m.minimize(x)
    with rm.Highs() as solver:
        result = solver.solve(m)
        assert result.is_optimal
    with pytest.raises(rm.InvalidModelError):
        x / p


def test_affine_times_param_stays_affine():
    m = rm.Model()
    x = m.var("x", ub=10.0)
    y = m.var("y", ub=10.0)
    p = m.param("p", 2.0)
    m.add(x + y <= 4.0)
    m.maximize(p * (x + y))
    with rm.Highs() as solver:
        result = solver.solve(m)
        assert result.objective == pytest.approx(8.0)
    with pytest.raises(rm.UnsupportedExpressionError):
        (x + 1.0) * (y + 1.0)


def test_error_codes_are_stable():
    m = rm.Model()
    m.var("x")
    with pytest.raises(rm.InvalidModelError) as exc:
        m.var("x")
    assert exc.value.code == "invalid-model"
    assert rm.ModelMismatchError("x").code == "model-mismatch"
    assert issubclass(rm.ShapeError, rm.RomlError)
