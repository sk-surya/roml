"""MIR debug diagnostics hook (MIR-00/MIR-02).

The `_debug_mir_stats` / `_debug_reset_mir_stats` hooks are debug-build-only
read-only counter probes; release wheels expose no such surface.
"""

from __future__ import annotations

import numpy as np
import pytest

import roml as rm

pytestmark = pytest.mark.skipif(
    not hasattr(rm.Model, "_debug_mir_stats"),
    reason="debug-only MIR diagnostics probe (absent in release wheels)",
)

EXPECTED_KEYS = {
    "numeric_bulk",
    "parametric_bulk",
    "general_affine",
    "param_dep_blocks",
    "param_positions_cells",
    "param_position_lookups",
    "overlay_lookups",
    "value_expr_evals",
    "coefficient_patch_batches",
}


def test_debug_mir_stats_reports_the_parametric_route():
    m = rm.Model()
    price = m.params("price", np.array([1.0, 2.0, 3.0]))
    x = m.vars("x", 3, ub=1.0)
    m.maximize(rm.sum(price * x))

    stats = m._debug_mir_stats()
    assert set(stats) == EXPECTED_KEYS
    assert stats["parametric_bulk"] == 1
    assert stats["general_affine"] == 0
    # The non-layout path keeps per-cell reverse positions.
    assert stats["param_positions_cells"] == 3
    assert stats["param_dep_blocks"] == 0


def test_debug_reset_mir_stats_zeroes_every_counter():
    m = rm.Model()
    price = m.params("price", np.array([1.0, 2.0]))
    x = m.vars("x", 2, ub=1.0)
    m.maximize(rm.sum(price * x))
    assert any(v != 0 for v in m._debug_mir_stats().values())

    m._debug_reset_mir_stats()
    assert all(v == 0 for v in m._debug_mir_stats().values())


def test_update_reports_propagation_work():
    m = rm.Model()
    price = m.params("price", np.array([1.0, 2.0, 3.0]))
    x = m.vars("x", 3, ub=1.0)
    m.maximize(rm.sum(price * x))

    m._debug_reset_mir_stats()
    m.update(price=np.array([4.0, 5.0, 6.0]))
    stats = m._debug_mir_stats()
    # One packed reverse-index position per cell, no ValueExpr evaluations.
    assert stats["param_position_lookups"] == 3
    assert stats["value_expr_evals"] == 0
