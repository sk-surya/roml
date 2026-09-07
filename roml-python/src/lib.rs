//! ROML Python bindings (`roml._native`, MPY).
//!
//! This crate is the only Rust/Python boundary: it wraps the solver-free
//! `roml` core and the `roml-highs` persistent session behind a typed PyO3
//! API. The core crates gain no Python/NumPy/native dependencies.

use pyo3::prelude::*;

/// ROML Python package version (tracks the `roml-python` distribution).
#[pyfunction]
fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[pymodule]
mod _native {
    #[pymodule_export]
    use super::version;
}
