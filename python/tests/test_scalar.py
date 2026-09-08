"""MPY-02 scalar API: golden LP, identity, errors (packet test seeds)."""

import pytest

import roml as rm


def test_production_reprice():
    m = rm.Model("production")
    x, y = m.var("x"), m.var("y")
    price = m.param("price", 1.0)
    m.add(x + y <= 4.0, name="capacity")
    m.add(x <= 3.0)
    m.maximize(price * x + y)
    with rm.Highs() as solver:
        first = solver.solve(m)
        assert first.objective == pytest.approx(4.0)
        m.update(price=3.0)
        second = solver.solve(m)
        assert second.value(x) == pytest.approx(3.0)
        assert second.objective == pytest.approx(10.0)
        assert first.objective == pytest.approx(4.0)


def test_symbolic_misuse_is_loud():
    m = rm.Model()
    x = m.var("x")
    with pytest.raises(TypeError):
        bool(x <= 2.0)
    with pytest.raises(rm.UnsupportedExpressionError):
        x * x


def test_foreign_handle_rejected():
    a, b = rm.Model("a"), rm.Model("b")
    x = a.var("x")
    with pytest.raises(rm.ModelMismatchError):
        b.add(x <= 1.0)
