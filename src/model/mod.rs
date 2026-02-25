//! Core model layer (solver-agnostic).
//!
//! The Model owns all modeling entities and is completely solver-agnostic.
//! It supports:
//! - Adding/removing/modifying variables, constraints, objectives, parameters
//! - Coefficient management with automatic parameter propagation
//! - Change tracking for incremental solver updates
//! - Transaction-based parameter batching

pub mod variable;
pub mod constraint;
pub mod objective;
pub mod parameter;
pub mod coefficient;
pub mod changelog;
pub mod transaction;

use crate::{id::{CoeffId, ConId, ObjId, ParamId, VarId}, model::{changelog::ChangeLog, coefficient::CoefficientIndex, constraint::ConstraintStore, objective::ObjectiveStore, parameter::ParameterStore, transaction::Transaction, variable::VariableStore}};

/// Error type for model operations.
#[derive(Clone, Debug, PartialEq)]
pub enum ModelError {
    /// The specified variable was not found.
    VariableNotFound(VarId),
    /// The specified constraint was not found.
    ConstraintNotFound(ConId),
    /// The specified objective was not found.
    ObjectiveNotFound(ObjId),
    /// The specified parameter was not found.
    ParameterNotFound(ParamId),
    /// The specified coefficient was not found.
    CoefficientNotFound(CoeffId),
    /// Invalid bounds (lower > upper).
    InvalidBounds,
}

impl std::fmt::Display for ModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::VariableNotFound(id) => write!(f, "Variable not found: {:?}", id),
            Self::ConstraintNotFound(id) => write!(f, "Constraint not found: {:?}", id),
            Self::ObjectiveNotFound(id) => write!(f, "Objective not found: {:?}", id),
            Self::ParameterNotFound(id) => write!(f, "Parameter not found: {:?}", id),
            Self::CoefficientNotFound(id) => write!(f, "Coefficient not found: {:?}", id),
            Self::InvalidBounds => write!(f, "Invalid bounds: lower > upper"),
        }
    }
}

impl std::error::Error for ModelError {}

/// The core MILP model - solver-agnostic representation.
///
/// # Architecture
///
/// The model maintains:
/// - Variables with bounds, type, and activity
/// - Constraints with bounds and activity
/// - Objectives with sense and activity (only one active)
/// - Parameters as coefficient value sources
/// - Coefficients linking variables to constraints/objectives
/// - A changelog for incremental solver updates
/// - A transaction for batched parameter changes
///
/// # Invariants
///
/// - IDs are never reused (stable identity)
/// - Only one objective can be active at a time
/// - Parameter changes propagate to all dependent coefficients
/// - All mutations are logged for solver consumption
#[derive(Clone, Debug, Default)]
pub struct Model {
    /// Variable storage.
    pub(crate) variables: VariableStore,
    /// Constraint storage.
    pub(crate) constraints: ConstraintStore,
    /// Objective storage.
    pub(crate) objectives: ObjectiveStore,
    /// Parameter storage.
    pub(crate) parameters: ParameterStore,
    /// Coefficient storage with multi-indexing.
    pub(crate) coefficients: CoefficientIndex,
    /// Change tracking for solver sync.
    pub(crate) changelog: ChangeLog,
    /// Transaction for batched parameter updates.
    pub(crate) transaction: Transaction,
    /// Optional model name.
    pub name: Option<String>,
}


impl Model {
    pub fn add_constraint_coefficient(
        &mut self,
        con: ConId,
        var: VarId,
        value_expr: ValueExpr,
    ) -> Result<CoeffId, ModelError> {
        unimplemented!();
    }

    pub fn add_objective_coefficient(
        &mut self,
        obj: ObjId,
        var: VarId,
        value_expr: ValueExpr,
    ) -> Result<CoeffId, ModelError> {
        unimplemented!();
    }
}