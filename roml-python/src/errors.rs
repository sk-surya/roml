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
/// Stable machine-readable codes, one per error class. Set as class
/// attributes at import so every instance (including ones raised from
/// any call site without extra plumbing) carries `code`.
macro_rules! register_error {
    ($py:expr, $module:expr, $name:expr, $ty:ty, $code:expr) => {{
        let class = $py.get_type::<$ty>();
        class.setattr("code", $code)?;
        $module.add($name, class)?;
    }};
}

pub fn register(py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_error!(py, module, "RomlError", RomlError, "roml-error");
    register_error!(
        py,
        module,
        "InvalidModelError",
        InvalidModelError,
        "invalid-model"
    );
    register_error!(
        py,
        module,
        "InvalidHandleError",
        InvalidHandleError,
        "invalid-handle"
    );
    register_error!(
        py,
        module,
        "ModelMismatchError",
        ModelMismatchError,
        "model-mismatch"
    );
    register_error!(py, module, "ShapeError", ShapeError, "shape");
    register_error!(
        py,
        module,
        "UnsupportedExpressionError",
        UnsupportedExpressionError,
        "unsupported-expression"
    );
    register_error!(
        py,
        module,
        "UnsupportedFeatureError",
        UnsupportedFeatureError,
        "unsupported-feature"
    );
    register_error!(py, module, "ModelBusyError", ModelBusyError, "model-busy");
    register_error!(
        py,
        module,
        "SessionBusyError",
        SessionBusyError,
        "session-busy"
    );
    register_error!(
        py,
        module,
        "ClosedSessionError",
        ClosedSessionError,
        "closed-session"
    );
    register_error!(py, module, "SolverError", SolverError, "solver");
    register_error!(
        py,
        module,
        "NoSolutionError",
        NoSolutionError,
        "no-solution"
    );
    register_error!(
        py,
        module,
        "MissingValueError",
        MissingValueError,
        "missing-value"
    );
    register_error!(
        py,
        module,
        "UnavailableDiagnosticError",
        UnavailableDiagnosticError,
        "unavailable-diagnostic"
    );
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
