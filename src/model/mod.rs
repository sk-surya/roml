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
