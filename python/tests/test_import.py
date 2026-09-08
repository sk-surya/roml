def test_native_import():
    import roml
    from roml import _native

    assert isinstance(roml.__version__, str)
    assert _native.__name__ == "roml._native"
    assert _native.version() == roml.__version__
