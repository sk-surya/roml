

use std::collections::HashMap;
use crate::id::ConId;

#[derive(Debug, Clone)]
pub struct Constraint {
    /// Unique identifier for the constraint.
    pub id: ConId,
    /// Lower bound of the constraint.
    pub lower_bound: f64,
    /// Upper bound of the constraint.
    pub upper_bound: f64,
}


#[derive(Debug, Clone, Default)]
pub struct ConstraintStore {
    constraints: HashMap<ConId, Constraint>,
}