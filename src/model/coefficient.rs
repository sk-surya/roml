//! Coefficient storage with multi-indexing.
//! 
//! Coefficients are first-class objects linking variables to targets (constraints or objectives).
//! They support efficient lookup by:
//! - Variable (for deletion, solver projection)
//! - Constraint (for deletion, iteration)
//! - Objective (for deletion, iteration)
//! - Parameter (for value propagation)
//! Key idea is the use of expr from which value can be evaluated.

use crate::id::{ConId, ObjId, VarId};

/// Target of a coefficient (constraint or objective).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CoefficientTarget {
    /// Coefficient belongs to a constraint.
    Constraint(ConId),
    /// Coefficient belongs to an objective.
    Objective(ObjId),
}

/// Internal data for a coefficient.
pub struct CoefficientData {
    /// The variable this coefficient is multiplied with.
    pub var: VarId,
    /// The target (constraint or objective) this coefficient belongs to.
    pub target: CoefficientTarget,
    /// The value expression (constant or can depend on parameters).
    // pub value_expr: ValueExpr,
    /// Cached evaluated value (updated on parameter changes)
    pub cached_value: f64,
}


#[derive(Clone, Debug, Default)]
pub struct CoefficientIndex {
}
