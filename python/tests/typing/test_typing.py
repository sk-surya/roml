"""Typing spot-checks (MPY-06): keystrokes an IDE must resolve.

Run: python -m mypy --strict python/tests/typing/
Covers: construction, scalar and shaped expressions, bulk reductions,
updates, session lifecycle, solution reads, sessions, and errors.
"""

from typing import Any

import numpy as np
from typing_extensions import assert_type

import roml as rm


def build() -> rm.Model:
    m = rm.Model("typed")
    x = m.var("x", lb=0.0)
    assert_type(x, rm.Var)
    p = m.param("price", 1.0)
    assert_type(p, rm.Param)
    c = m.add(x <= 2.0 * p, name="cap")
    assert_type(c, rm.Constraint)
    o = m.maximize(p * x)
    assert_type(o, rm.Objective)
    return m


def arrays(m: rm.Model) -> None:
    charge = m.vars("charge", (4, 6), ub=2.0)
    assert_type(charge, rm.VarArray)
    assert_type(charge.shape, "tuple[int, ...]")
    price = m.params("price", np.full((4, 6), 50.0))
    assert_type(price, rm.ParamArray)
    net: rm.ExprArray = charge - price
    assert_type(net, rm.ExprArray)
    total = rm.sum(net)
    assert_type(total, Any)
    revenue = rm.dot(price, charge)
    assert_type(revenue, Any)
    rows = m.add(charge <= 2.0)
    assert_type(rows, rm.ConstraintArray)
    m.update(price=np.full((4, 6), 51.0))


def lifecycle(m: rm.Model) -> rm.Solution:
    solver = rm.Highs(threads=1, output=False)
    try:
        result = solver.solve(m, time_limit=2.0)
    finally:
        solver.close()
    assert_type(result, rm.Solution)
    assert_type(result.status, rm.SolveStatus)
    assert_type(result.objective, "float | None")
    assert_type(result.metadata, "dict[str, Any]")
    x = m.var("probe", ub=1.0)
    assert_type(result.value(x), float)
    return result


def errors() -> None:
    assert_type(rm.RomlError("x"), rm.RomlError)
    assert issubclass(rm.InvalidModelError, ValueError)
    assert issubclass(rm.UnsupportedExpressionError, TypeError)
