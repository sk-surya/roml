//! Backend session contract types and error types.
//!
//! This module collects and re-exports the backend contract types:
//! - `backend`: Backend metadata, capabilities, and typed errors
//! - `callback`: MIP callback types for cutting planes and lazy constraints
//! - `reference`: Reference projection backend for correctness verification
//! - `request`: Immutable solve requests and results
//! - `session`: Backend session traits and synchronization types
//!
//! The only type defined directly here is `SolverError`, which is preserved
//! for backward compatibility with backend crates (HiGHS, MOSEK, Xpress).

pub mod backend;
pub mod callback;
pub mod conformance;
pub mod reference;
pub mod request;
pub mod session;

/// Error type for solver operations.
#[derive(Clone, Debug)]
pub enum SolverError {
    /// Operation not supported by this solver.
    NotSupported(String),
    /// Internal solver error.
    InternalError(String),
    /// Model is invalid for this solver.
    InvalidModel(String),
}

impl std::fmt::Display for SolverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotSupported(msg) => write!(f, "Operation not supported: {}", msg),
            Self::InternalError(msg) => write!(f, "Internal solver error: {}", msg),
            Self::InvalidModel(msg) => write!(f, "Invalid model: {}", msg),
        }
    }
}

impl std::error::Error for SolverError {}

/// LpAlgorithm is defined in the `request` module as part of solve configuration.
pub use request::LpAlgorithm;
