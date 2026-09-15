"""MIR-06 M6-2B.1: Python ExprArray wraps shared LinArray/GeneralLinArray."""

import numpy as np
import pytest

import roml as rm


def _bess():
    m = rm.Model("bess")
    price = m.params("price", np.full(4, 50.0))
    charge = m.vars("charge", 4, ub=2.0)
    discharge = m.vars("discharge", 4, ub=2.0)
    return m, price, charge, discharge


def test_var_arithmetic_stays_compact():
    _, _, charge, discharge = _bess()
    assert (discharge - charge)._debug_is_compact()
    assert (charge + discharge)._debug_is_compact()
    assert (-charge)._debug_is_compact()
    assert (2.0 * charge)._debug_is_compact()
    assert (charge * 2.0)._debug_is_compact()
    assert (charge / 2.0)._debug_is_compact()


def test_param_times_var_stays_compact():
    _, price, charge, discharge = _bess()
    assert (price * (discharge - charge))._debug_is_compact()


def test_compact_plus_general_is_general():
    _, price, charge, discharge = _bess()
    # A per-cell parameter constant is outside the compact coefficient families.
    general = charge + price
    assert not general._debug_is_compact()
    assert (discharge - charge + general)._debug_is_compact() is False


def test_cross_model_composition_rejects():
    a = rm.Model()
    x = a.vars("x", 3, ub=1.0)
    b = rm.Model()
    y = b.vars("y", 3, ub=1.0)
    with pytest.raises(rm.ModelMismatchError):
        _ = x - y
    with pytest.raises(rm.ModelMismatchError):
        _ = (x + 0.0) - (y + 0.0)


def test_shape_mismatch_rejects():
    m = rm.Model()
    x = m.vars("x", (2, 3), ub=1.0)
    y = m.vars("y", (3, 2), ub=1.0)
    with pytest.raises(rm.ShapeError):
        _ = x - y
