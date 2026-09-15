"""MIR-06 (IR-25/IR-28): normalized ordinal-IR and semantic-journal fingerprints.

Freezes the cross-language fingerprint contract's Python surface: both
fingerprints are deterministic, owner/absolute-id independent, and change with
model structure.
"""

import numpy as np

import roml as rm


def _bess(n=4, dt=0.25, extra=False):
    m = rm.Model("bess")
    price = m.params("price", np.full(n, 50.0))
    charge = m.vars("charge", n, ub=2.0)
    discharge = m.vars("discharge", n, ub=2.0)
    energy = m.vars("energy", n + 1, ub=4.0)
    m.add(energy[0] == 2.0)
    m.add(energy[1:] == energy[:-1] + dt * (0.95 * charge - discharge / 0.95))
    if extra:
        m.add(charge <= 1.5)
    m.maximize(dt * rm.dot(price, discharge - charge) + 30.0 * energy[-1])
    return m


def test_fingerprints_are_deterministic_and_owner_independent():
    a = _bess()
    b = _bess()
    assert isinstance(a.normalized_ordinal_fingerprint(), int)
    assert isinstance(a.normalized_journal_fingerprint(), int)
    assert a.normalized_ordinal_fingerprint() == b.normalized_ordinal_fingerprint()
    assert a.normalized_journal_fingerprint() == b.normalized_journal_fingerprint()


def test_fingerprints_change_with_structure():
    base = _bess()
    extra = _bess(extra=True)
    assert (
        base.normalized_ordinal_fingerprint()
        != extra.normalized_ordinal_fingerprint()
    )
    assert (
        base.normalized_journal_fingerprint()
        != extra.normalized_journal_fingerprint()
    )


def test_fingerprints_are_stable_across_repeated_calls():
    m = _bess()
    first_ordinal = m.normalized_ordinal_fingerprint()
    first_journal = m.normalized_journal_fingerprint()
    assert m.normalized_ordinal_fingerprint() == first_ordinal
    assert m.normalized_journal_fingerprint() == first_journal


def test_array_and_scalar_construction_have_equal_ordinal_fingerprint():
    """The shared-handle array path and the explicit scalar path lower to the
    same normalized ordinal IR."""
    array_model = rm.Model()
    xa = array_model.vars("x", 3, ub=1.0)
    pa = array_model.params("p", [1.0, 2.0, 3.0])
    array_model.add(rm.sum(xa) <= 2.0)
    array_model.maximize(rm.dot(pa, xa))

    scalar_model = rm.Model()
    xs = [scalar_model.var(f"x{i}", ub=1.0) for i in range(3)]
    ps = [scalar_model.param(f"p{i}", float(i + 1)) for i in range(3)]
    scalar_model.add(xs[0] + xs[1] + xs[2] <= 2.0)
    scalar_model.maximize(ps[0] * xs[0] + ps[1] * xs[1] + ps[2] * xs[2])

    assert (
        array_model.normalized_ordinal_fingerprint()
        == scalar_model.normalized_ordinal_fingerprint()
    )
