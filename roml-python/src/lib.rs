//! ROML Python bindings (`roml._native`, MPY).
//!
//! This crate is the only Rust/Python boundary: it wraps the solver-free
//! `roml` core and the `roml-highs` persistent session behind a typed PyO3
//! API. The core crates gain no Python/NumPy/native dependencies. Each
//! submodule owns registrations only through this module.

use pyo3::prelude::*;

mod arrays;
mod errors;
mod expressions;
mod handles;
mod model;
mod namespace;
mod solution;
mod solver;

/// ROML Python package version (tracks the `roml-python` distribution).
#[pyfunction]
fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[pymodule]
mod _native {
    use super::*;

    #[pymodule_export]
    use super::version;

    #[pymodule_init]
    fn init(m: &Bound<'_, PyModule>) -> PyResult<()> {
        errors::register(m.py(), m)?;
        m.add_class::<model::Model>()?;
        m.add_class::<handles::Var>()?;
        m.add_class::<handles::Param>()?;
        m.add_class::<handles::Constraint>()?;
        m.add_class::<handles::Objective>()?;
        m.add_class::<expressions::Expr>()?;
        m.add_class::<expressions::Comparison>()?;
        m.add_class::<solution::Solution>()?;
        m.add_class::<solution::SolveStatus>()?;
        m.add_class::<solver::Session>()?;
        m.add_class::<arrays::VarArray>()?;
        m.add_class::<arrays::ParamArray>()?;
        m.add_class::<arrays::ExprArray>()?;
        m.add_class::<arrays::ComparisonArray>()?;
        m.add_class::<arrays::ConstraintArray>()?;
        m.add_function(wrap_pyfunction!(arrays::sum, m)?)?;
        m.add_function(wrap_pyfunction!(arrays::dot, m)?)?;
        // The packet contract spells status members UPPER_CASE. Expose
        // aliases on the enum type (same objects, both spellings work).
        let status = m.getattr("SolveStatus")?;
        for (upper, camel) in [
            ("UNKNOWN", "Unknown"),
            ("OPTIMAL", "Optimal"),
            ("FEASIBLE", "Feasible"),
            ("INFEASIBLE", "Infeasible"),
            ("UNBOUNDED", "Unbounded"),
            ("INFEASIBLE_OR_UNBOUNDED", "InfeasibleOrUnbounded"),
            ("TIME_LIMIT", "TimeLimit"),
            ("ITERATION_LIMIT", "IterationLimit"),
            ("NODE_LIMIT", "NodeLimit"),
            ("INTERRUPTED", "Interrupted"),
            ("NUMERICAL", "Numerical"),
            ("ERROR", "Error"),
        ] {
            status.setattr(upper, status.getattr(camel)?)?;
        }
        Ok(())
    }
}
