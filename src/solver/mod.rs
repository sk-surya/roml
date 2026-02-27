//! Solver adapter traits.
//!
//! The model layer only knows about these traits. Concrete solver implementations
//! (like HiGHS, Gurobi) live in separate crates.
//!
//! # Architecture
//!
//! - Model Layer: Owns variables, constraints, parameters, coefficients, objectives
//! - Solver Layer: Translates model state into solver-specific representation
//!
//! Solver concepts must NOT leak into the model layer.

use std::collections::HashMap;

use crate::{VarId, model::changelog::Change};

/// Solver status after an optimization attempt.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum SolverStatus {
    #[default]
    NotSolved,
    Optimal,
    Infeasible,
    Unbounded,
    TimeLimit,
    IterationLimit,
    MemoryLimit,
    Error,
}

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

/// Trait that solver adapters must implement.
///
/// The model layer only knows about this trait. Concrete implementations
/// live in separate crates (e.g., roml-highs).
pub trait SolverAdapter {
    /// Apply a batch of changes from the model to solver.
    /// 
    /// Changes should be applied in order. The solver may batch or optimize
    /// the application as appropriate.
    fn apply_changes(&mut self, changes: &[Change]) -> Result<(), SolverError>;

    /// Solve the current model state
    /// 
    /// Returns the solver status after the attempt.
    fn solve(&mut self) -> Result<SolverStatus, SolverError>;

    /// Get the solver status after solving.
    fn status(&self) -> SolverStatus;

    /// Get the solution values for all variables, if available.
    fn solution_values(&self) -> Option<HashMap<VarId, f64>>;

    /// Reset the solver state for a full rebuild.
    fn reset(&mut self);

    /// Check if the solver supports incremental updates for a change type.
    fn supports_incremental(&self, change: &Change) -> bool;
}
