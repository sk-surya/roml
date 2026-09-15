"""MIR-06 M6-2B.2: shared array objectives reach the core objective seam."""

import numpy as np
import pytest

import roml as rm

_DEBUG = hasattr(rm.Model, "_debug_mir_stats")


@pytest.mark.skipif(not _DEBUG, reason="debug-only MIR diagnostics probe")
def test_flagship_array_objective_is_packed():
    m = rm.Model("bess")
    price = m.params("price", np.full(4, 50.0))
    charge = m.vars("charge", 4, ub=2.0)
    discharge = m.vars("discharge", 4, ub=2.0)
    m.maximize(rm.sum(price * (discharge - charge)))
    stats = m._debug_mir_stats()
    assert stats["general_affine"] == 0
    assert stats["param_dep_blocks"] > 0
    assert stats["param_positions_cells"] == 0

    m._debug_reset_mir_stats()
    m.update(price=np.full(4, 60.0))
    stats = m._debug_mir_stats()
    assert stats["param_position_lookups"] == 0
    assert stats["overlay_lookups"] == 0
    assert stats["value_expr_evals"] == 0
    assert stats["coefficient_patch_batches"] == 1
    assert stats["packed_parameter_updates"] == 1


def test_general_array_objective_constructs_and_solves():
    m = rm.Model()
    x = m.vars("x", 3, ub=1.0)
    p = m.params("p", np.array([1.0, 2.0, 3.0]))
    m.add(rm.sum(x) <= 2.0)
    # (p * p) is a per-cell parameter coefficient outside the compact families,
    # so the objective is a shared general array.
    m.maximize(rm.sum((p * p) * x))
    with rm.Highs() as s:
        assert s.solve(m).is_optimal


def test_foreign_objective_array_rejects():
    a = rm.Model()
    z = a.vars("z", 3, ub=1.0)
    b = rm.Model()
    b.vars("y", 3, ub=1.0)
    with pytest.raises(rm.ModelMismatchError):
        b.maximize(rm.sum(z))


def test_general_objective_update_rejects_non_finite_atomically():
    m = rm.Model()
    x = m.vars("x", 2, ub=1.0)
    price = m.params("price", np.array([1e150, 1e150]))
    m.maximize(rm.sum((price * price) * x))
    # p = 1e150 -> p*p = 1e300 finite at construction; 1e308 -> inf on update.
    with pytest.raises(ValueError):
        m.update(price=np.array([1e308, 1e308]))
