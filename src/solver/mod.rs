//! Backend session contract and related types.
//!
//! This module collects the backend contract types:
//! - `backend`: Backend metadata, capabilities, and typed errors
//! - `callback`: MIP callback types for cutting planes and lazy constraints
//! - `reference`: Reference projection backend for correctness verification
//! - `request`: Immutable solve requests and results
//! - `session`: Backend session traits and synchronization types
//! - `conformance`: Shared parameterized test runner

pub mod backend;
pub mod callback;
pub mod conformance;
pub mod reference;
pub mod request;
pub mod session;

/// Error type for solver operations.
#[derive(Clone, Debug, PartialEq)]
pub struct SolverError(pub String);

impl std::fmt::Display for SolverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SolverError({})", self.0)
    }
}

impl std::error::Error for SolverError {}

/// Legacy solver status — preserved for backward compatibility with Solution.
/// New code should use `TerminationStatus` instead.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SolverStatus {
    #[default]
    Unknown,
    Optimal,
    Feasible,
    Infeasible,
    Unbounded,
    InfeasibleOrUnbounded,
    TimeLimit,
    IterationLimit,
    Error,
}

/// Legacy solver algorithm selection — preserved for backward compatibility.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LpAlgorithm {
    Primal,
    Dual,
    Barrier,
    DualSimplex,
    Automatic,
}
