"""P2A locks: variable-array namespace contract (behavior preservation).

Every case below pins CURRENT semantics measured on the pre-change build
(see /tmp/p2a_truth.py). The structural implementation must reproduce
this table exactly: same accept/reject, same error types. The ONLY
intended behavior change in P2A is the x[2:7][0] view-display fix,
covered separately in test_view_scalar_shows_root_ordinal.
"""

import pytest

import roml as rm


def rejects_model(fn):
    with pytest.raises(rm.InvalidModelError):
        fn()


def test_scalar_dupes():
    m = rm.Model()
    m.var("x")
    rejects_model(lambda: m.var("x"))
    rejects_model(lambda: m.param("x", 1.0))
    m2 = rm.Model()
    m2.param("x", 1.0)
    rejects_model(lambda: m2.var("x"))


def test_array_base_collisions():
    m = rm.Model()
    m.var("x")
    rejects_model(lambda: m.vars("x", 10, ub=1.0))
    m2 = rm.Model()
    m2.vars("x", 10, ub=1.0)
    rejects_model(lambda: m2.var("x"))
    rejects_model(lambda: m2.vars("x", 10, ub=1.0))
    rejects_model(lambda: m2.vars("x", 0))


def test_implicit_element_collisions():
    m = rm.Model()
    m.vars("x", 100, ub=1.0)
    rejects_model(lambda: m.var("x[5]"))
    rejects_model(lambda: m.param("x[5]", 1.0))
    m2 = rm.Model()
    m2.var("x[5]")
    rejects_model(lambda: m2.vars("x", 100, ub=1.0))
    m3 = rm.Model()
    m3.param("x[5]", 1.0)
    rejects_model(lambda: m3.vars("x", 100, ub=1.0))


def test_boundary_and_out_of_range_coexist():
    m = rm.Model()
    m.var("x[100]")
    m.vars("x", 100, ub=1.0)  # only [0,100) are implicit
    m2 = rm.Model()
    m2.vars("x", 100, ub=1.0)
    m2.var("x[100]")  # == len is not an element
    m2.var("x[150]")
    m3 = rm.Model()
    m3.var("x[150]")
    m3.vars("x", 100, ub=1.0)


def test_ugly_names_never_structural():
    for ugly in ["x[01]", "x[-1]", "x[+1]", "x[1.0]", "x[1x]", "x[]", "x[ 1]",
                 "x[99999999999999999999999]"]:
        m = rm.Model()
        m.var(ugly)
        m.vars("x", 10, ub=1.0)
        m2 = rm.Model()
        m2.vars("x", 10, ub=1.0)
        m2.var(ugly)


def test_bracketed_bases():
    m = rm.Model()
    m.vars("x[0]", 10, ub=1.0)
    rejects_model(lambda: m.var("x[0][5]"))
    rejects_model(lambda: m.vars("x", 100, ub=1.0))
    m2 = rm.Model()
    m2.var("x[0][5]")
    rejects_model(lambda: m2.vars("x[0]", 10, ub=1.0))
    m3 = rm.Model()
    m3.vars("x", 100, ub=1.0)
    rejects_model(lambda: m3.vars("x[0]", 10, ub=1.0))
    m4 = rm.Model()
    m4.params("q[0]", [1.0, 2.0])
    rejects_model(lambda: m4.vars("q", 100, ub=1.0))


def test_zero_length_arrays():
    m = rm.Model()
    m.vars("x", 0)
    rejects_model(lambda: m.var("x"))
    m.var("x[0]")  # no implicit elements in an empty reservation
    rejects_model(lambda: m.vars("x", 10, ub=1.0))
    m2 = rm.Model()
    m2.var("x")
    rejects_model(lambda: m2.vars("x", 0))


def test_constraint_asymmetry():
    # Constraint creation sees variable elements...
    m = rm.Model()
    m.vars("x", 10, ub=1.0)
    y = m.vars("y", 2, ub=1.0)
    rejects_model(lambda: m.add(y[0] <= 1.0, name="x[5]"))
    # ...but variable-array creation ignores constraint names.
    m2 = rm.Model()
    m2.add(m2.vars("y", 2, ub=1.0)[0] <= 1.0, name="x[5]")
    m2.vars("x", 10, ub=1.0)
    # Constraint-array bases occupy the shared base namespace both ways.
    m3 = rm.Model()
    m3.add((m3.vars("y", 2, ub=1.0) <= 1.0), name="r")
    rejects_model(lambda: m3.vars("r", 10, ub=1.0))
    m4 = rm.Model()
    m4.vars("r", 10, ub=1.0)
    rejects_model(lambda: m4.add((m4.vars("y", 2, ub=1.0) <= 1.0), name="r"))


def test_param_array_interactions():
    m = rm.Model()
    m.params("q", [1.0, 2.0])
    rejects_model(lambda: m.vars("q", 10, ub=1.0))
    m2 = rm.Model()
    m2.vars("q", 10, ub=1.0)
    rejects_model(lambda: m2.params("q", [1.0, 2.0]))


def test_atomic_rejection_no_partial_state():
    m = rm.Model()
    m.var("x[999999]")
    before = repr(m)
    rejects_model(lambda: m.vars("x", 1_000_000, ub=1.0))
    assert repr(m) == before
    # And the model still works afterwards.
    m.var("y")
    assert "2 vars" in repr(m)


def test_repr_counts():
    m = rm.Model()
    m.vars("x", (2, 3), ub=1.0)
    assert repr(m) == "Model(6 vars, 0 params, 0 constraints)"
    m.var("s")
    m.param("p", 1.0)
    assert repr(m) == "Model(7 vars, 1 params, 0 constraints)"


def test_view_scalar_shows_root_ordinal():
    # P2A intentional correction: v = x[2:7]; v[0] IS x[2], so it must
    # display as x[2], not the view-relative x[0]. Values always flowed
    # by VarId and were never wrong; only the displayed name was.
    m = rm.Model()
    x = m.vars("x", 10, ub=5.0)
    v = x[2:7]
    assert v.shape == (5,)
    z = v[0]
    assert repr(z) == 'Var("x[2]")'
    w = v[1:3]
    assert repr(w[0]) == 'Var("x[3]")'
    assert repr(w[1]) == 'Var("x[4]")'


def test_multidim_ordinal_is_flat_c_order():
    m = rm.Model()
    x = m.vars("m", (2, 3), ub=1.0)
    assert repr(x[0, 0]) == 'Var("m[0]")'
    assert repr(x[0, 2]) == 'Var("m[2]")'
    assert repr(x[1, 0]) == 'Var("m[3]")'
    assert repr(x[1, 2]) == 'Var("m[5]")'
    v = x[:, 1:]
    assert repr(v[0, 0]) == 'Var("m[1]")'
    assert repr(v[1, 1]) == 'Var("m[5]")'


def test_solve_equivalence_after_naming():
    m = rm.Model()
    x = m.vars("x", 5, ub=2.0)
    m.add(rm.sum(x) <= 6.0)
    m.maximize(rm.sum(x))
    with rm.Highs() as s:
        assert s.solve(m).objective == pytest.approx(6.0)


@pytest.mark.skipif(
    not hasattr(rm.Model, "_debug_namespace_counts"),
    reason="debug-only namespace probe (absent in release wheels)",
)
def test_namespace_cardinalities_no_implicit_strings():
    m = rm.Model()
    m.vars("x", 1_000_000, ub=1.0)
    m.var("s")
    m.var("x[1000000]")  # out-of-range explicit: indexed, not implicit
    assert m._debug_namespace_counts() == (1, 2, 1)


def test_update_unknown_name_errors_preserved():
    m = rm.Model()
    m.vars("x", 3, ub=1.0)
    m.param("p", 1.0)
    with pytest.raises(rm.InvalidModelError):
        m.update(x=1.0)
    with pytest.raises(rm.InvalidModelError):
        m.update(nope=1.0)
