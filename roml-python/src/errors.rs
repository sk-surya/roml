//! Stable Python error hierarchy (DESIGN §6).
//!
//! Every error carries a stable `code` attribute; input errors additionally
//! inherit `ValueError`/`TypeError` where practical so ordinary Python
//! checks keep working. Classification never parses human strings: core
//! `ModelError` variants map by constructor, and operational failures
//! preserve primary/cleanup attributes.

use pyo3::create_exception;
use pyo3::prelude::*;

create_exception!(roml, RomlError, pyo3::exceptions::PyException);
create_exception!(roml, InvalidModelError, RomlError);
create_exception!(roml, InvalidHandleError, RomlError);
create_exception!(roml, ModelMismatchError, RomlError);
create_exception!(roml, ShapeError, RomlError);
create_exception!(roml, UnsupportedExpressionError, RomlError);
create_exception!(roml, UnsupportedFeatureError, RomlError);
create_exception!(roml, ModelBusyError, RomlError);
create_exception!(roml, SessionBusyError, RomlError);
create_exception!(roml, ClosedSessionError, RomlError);
create_exception!(roml, SolverError, RomlError);
create_exception!(roml, NoSolutionError, RomlError);
create_exception!(roml, MissingValueError, RomlError);
create_exception!(roml, UnavailableDiagnosticError, RomlError);

/// Register all error classes on the extension module and wire the
/// `ValueError`/`TypeError` mixin bases for input errors.
///
/// PyO3 exception types support single inheritance only, so the mixins are
/// applied by extending `__bases__` once at import: pure-Python exception
/// subclasses share the `BaseException` layout, making this sound. The
/// `RomlError` base always comes first, so `except RomlError` keeps working
/// alongside `except ValueError` / `except TypeError`.
pub fn register(py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add("RomlError", py.get_type::<RomlError>())?;
    module.add("InvalidModelError", py.get_type::<InvalidModelError>())?;
    module.add("InvalidHandleError", py.get_type::<InvalidHandleError>())?;
    module.add("ModelMismatchError", py.get_type::<ModelMismatchError>())?;
    module.add("ShapeError", py.get_type::<ShapeError>())?;
    module.add(
        "UnsupportedExpressionError",
        py.get_type::<UnsupportedExpressionError>(),
    )?;
    module.add(
        "UnsupportedFeatureError",
        py.get_type::<UnsupportedFeatureError>(),
    )?;
    module.add("ModelBusyError", py.get_type::<ModelBusyError>())?;
    module.add("SessionBusyError", py.get_type::<SessionBusyError>())?;
    module.add("ClosedSessionError", py.get_type::<ClosedSessionError>())?;
    module.add("SolverError", py.get_type::<SolverError>())?;
    module.add("NoSolutionError", py.get_type::<NoSolutionError>())?;
    module.add("MissingValueError", py.get_type::<MissingValueError>())?;
    module.add(
        "UnavailableDiagnosticError",
        py.get_type::<UnavailableDiagnosticError>(),
    )?;
    let globals = pyo3::types::PyDict::new(py);
    globals.set_item("builtins", py.import("builtins")?)?;
    globals.set_item("InvalidModelError", py.get_type::<InvalidModelError>())?;
    globals.set_item("InvalidHandleError", py.get_type::<InvalidHandleError>())?;
    globals.set_item("ModelMismatchError", py.get_type::<ModelMismatchError>())?;
    globals.set_item("ShapeError", py.get_type::<ShapeError>())?;
    globals.set_item(
        "UnsupportedExpressionError",
        py.get_type::<UnsupportedExpressionError>(),
    )?;
    py.run(
        c"InvalidModelError.__bases__ = (InvalidModelError.__bases__[0], builtins.ValueError)\n\
          InvalidHandleError.__bases__ = (InvalidHandleError.__bases__[0], builtins.ValueError)\n\
          ModelMismatchError.__bases__ = (ModelMismatchError.__bases__[0], builtins.ValueError)\n\
          ShapeError.__bases__ = (ShapeError.__bases__[0], builtins.ValueError)\n\
          UnsupportedExpressionError.__bases__ = (UnsupportedExpressionError.__bases__[0], builtins.TypeError)",
        Some(&globals),
        None,
    )?;
    Ok(())
}
