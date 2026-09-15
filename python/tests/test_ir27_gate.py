"""MIR-06 IR-27 gate: the Python array layer retains no gathered identity stores.

Python `VarArray`/`ParamArray` are wrappers over the shared `roml::modeling`
handles; per-element `vars`/`params`/`ordinals` vectors and the
`param_array_ids` registry are gone. (The separate Python expression IR is
migrated in M6-2, so this gate scopes to the array/registry layer.)
"""

from pathlib import Path

_SRC = Path(__file__).resolve().parents[2] / "roml-python" / "src"

_TARGET_FILES = ("arrays.rs", "model.rs", "solution.rs")
_FORBIDDEN = ("pub vars:", "pub params:", "pub ordinals", "param_array_ids")


def test_python_array_layer_has_no_gathered_identity_stores():
    for name in _TARGET_FILES:
        path = _SRC / name
        assert path.exists(), f"missing {path}"
        text = path.read_text()
        for token in _FORBIDDEN:
            assert token not in text, f"{name} still contains {token!r}"


def test_python_arrays_wrap_shared_handles():
    arrays = (_SRC / "arrays.rs").read_text()
    assert "inner: roml::modeling::VarArray" in arrays
    assert "inner: roml::modeling::ParamArray" in arrays
