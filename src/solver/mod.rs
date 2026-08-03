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
pub mod error;
pub mod facade;
pub mod options;
pub mod overlay;
pub mod reference;
pub mod request;
pub mod session;

pub use error::SolveError;
pub use facade::SolverSession;
pub use options::SolveOptions;

/// Error type for solver operations.
///
/// Legacy error kept for the pre-1.0 compatibility window; the golden path
/// uses [`SolveError`].
#[derive(Clone, Debug, PartialEq)]
pub struct SolverError(
    /// Human-readable error message.
    pub String,
);

impl std::fmt::Display for SolverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SolverError({})", self.0)
    }
}

impl std::error::Error for SolverError {}

/// Unified golden-path solve status (API-03.2, D4).
///
/// One status type preserved across optimal, feasible-limit, infeasible,
/// unbounded, limit, interrupted, and numerical outcomes. Operational
/// failures (license, backend errors, uninterpretable terminations) surface
/// as [`SolveError`] instead (API-03.3).
///
/// `SolverStatus` is a compatibility alias of this type.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SolveStatus {
    /// The status has not been determined (default / pre-solve).
    #[default]
    Unknown,
    /// Proven optimal solution.
    Optimal,
    /// Feasible solution found (not proven optimal — e.g. MIP limit).
    Feasible,
    /// Proven infeasible.
    Infeasible,
    /// Proven unbounded.
    Unbounded,
    /// Proven infeasible or unbounded (solver-preserved ambiguity).
    InfeasibleOrUnbounded,
    /// Time limit reached.
    TimeLimit,
    /// Iteration limit reached.
    IterationLimit,
    /// Node limit reached (MIP).
    NodeLimit,
    /// Solver interrupted (e.g. callback).
    Interrupted,
    /// Numerical difficulties prevented a reliable solve.
    Numerical,
    /// The solve terminated in an uninterpretable solver error state.
    Error,
}

impl SolveStatus {
    /// Map a backend [`crate::solver::backend::TerminationStatus`] into a
    /// [`SolveStatus`] or a [`SolveError`].
    ///
    /// Mathematical outcomes map to `Ok(SolveStatus)`. A termination that
    /// means the solve could not be performed or interpreted (`Error`,
    /// `Unknown`) maps to `Err(SolveError::Status(..))` (API-03.3). The match
    /// is exhaustive — no wildcard arm — so a new backend status cannot be
    /// silently dropped.
    ///
    /// # Errors
    ///
    /// Returns [`SolveError::Status`] for the uninterpretable terminations
    /// [`crate::solver::backend::TerminationStatus::Error`] and
    /// [`crate::solver::backend::TerminationStatus::Unknown`].
    pub fn from_termination(
        termination: crate::solver::backend::TerminationStatus,
    ) -> Result<SolveStatus, crate::solver::error::SolveError> {
        use crate::solver::backend::TerminationStatus::{
            Error, Feasible, Infeasible, InfeasibleOrUnbounded, Interrupted, IterationLimit,
            NodeLimit, NumericalIssue, Optimal, TimeLimit, Unbounded, Unknown,
        };
        use crate::solver::error::SolveError;
        match termination {
            Optimal => Ok(SolveStatus::Optimal),
            Feasible => Ok(SolveStatus::Feasible),
            Infeasible => Ok(SolveStatus::Infeasible),
            Unbounded => Ok(SolveStatus::Unbounded),
            InfeasibleOrUnbounded => Ok(SolveStatus::InfeasibleOrUnbounded),
            TimeLimit => Ok(SolveStatus::TimeLimit),
            IterationLimit => Ok(SolveStatus::IterationLimit),
            NodeLimit => Ok(SolveStatus::NodeLimit),
            Interrupted => Ok(SolveStatus::Interrupted),
            NumericalIssue => Ok(SolveStatus::Numerical),
            Error => Err(SolveError::Status(Error)),
            Unknown => Err(SolveError::Status(Unknown)),
        }
    }

    /// True when the solve proved optimality.
    pub fn is_optimal(self) -> bool {
        self == SolveStatus::Optimal
    }
}

/// Compatibility alias for the unified [`SolveStatus`] (M2 open item 2:
/// `SolveStatus` replaces `SolverStatus`, shipped as an alias first).
pub type SolverStatus = SolveStatus;

/// Legacy solver algorithm selection — preserved for backward compatibility.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LpAlgorithm {
    /// Primal simplex method.
    Primal,
    /// Dual simplex method.
    Dual,
    /// Interior-point (barrier) method.
    Barrier,
    /// Dual simplex with crossover.
    DualSimplex,
    /// Let the solver choose automatically.
    Automatic,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solver_error_display_and_status_default() {
        let err = SolverError("boom".to_string());
        assert_eq!(err.to_string(), "SolverError(boom)");
        assert_eq!(SolverStatus::default(), SolverStatus::Unknown);
    }
}
