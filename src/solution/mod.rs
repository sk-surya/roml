//! Solution storage and introspection.
//!
//! Solutions are immutable once stored and contain:
//! - Variable values
//! - Objective value(s)
//! - Solver status
//! - Optional duals and reduced costs
//! //!
//! # Design
//!
//! Solutions are stored separately from the model. Multiple solutions can
//! be kept (latest, named snapshots, etc.). Expression evaluation against
//! solutions does not require solver access.

use std::collections::HashMap;

use crate::id::{ConId, ObjId, VarId};
use crate::solver::SolverStatus;

/// A solution to the optimization problem.
///
/// Contains variable values, objective value, and solver status.
/// Solutions are immutable once created.
#[derive(Clone, Debug)]
pub struct Solution {
    /// Variable values.
    values: HashMap<VarId, f64>,
    /// Objective value (if solved successfully).
    objective_value: Option<f64>,
    /// Which objective this solution is solution for.
    objective_id: Option<ObjId>,
    /// Solver status
    status: SolverStatus,
    /// Dual values for constraints (if available).
    duals: Option<HashMap<ConId, f64>>,
    /// Reduced costs for variables (if available).
    reduced_costs: Option<HashMap<VarId, f64>>,
}

impl Solution {
     /// Create a new solution with the given status.
    pub fn new(status: SolverStatus) -> Self {
        Self {
            values: HashMap::new(),
            objective_value: None,
            objective_id: None,
            status,
            duals: None,
            reduced_costs: None,
        }
    }

    /// Create a solution from variable values.
    pub fn from_values(values: HashMap<VarId, f64>, status: SolverStatus) -> Self {
        Self {
            values,
            objective_value: None,
            objective_id: None,
            status,
            duals: None,
            reduced_costs: None,
        }
    }

    /// Get the solver status.
    pub fn status(&self) -> SolverStatus {
        self.status
    }

    /// Check if the solution is optimal.
    pub fn is_optimal(&self) -> bool {
        self.status == SolverStatus::Optimal
    }

    /// Check if the solution has variable values.
    pub fn has_values(&self) -> bool {
        !self.values.is_empty()
    }

    /// Get a variable's value.
    pub fn value(&self, var: VarId) -> Option<f64> {
        self.values.get(&var).copied()
    }

    /// Get a variable's value, defaulting to 0.0 if not found.
    pub fn value_or_zero(&self, var: VarId) -> f64 {
        self.values.get(&var).copied().unwrap_or(0.0)
    }

    /// Get all variable values.
    pub fn values(&self) -> &HashMap<VarId, f64> {
        &self.values
    }

    /// Get the objective value.
    pub fn objective_value(&self) -> Option<f64> {
        self.objective_value
    }

    /// Get which objective this solution is for.
    pub fn objective_id(&self) -> Option<ObjId> {
        self.objective_id
    }

    /// Get the dual value for a constraint (if available).
    pub fn dual(&self, con: ConId) -> Option<f64> {
        self.duals.as_ref()?.get(&con).copied()
    }

    /// Check if dual values are available.
    pub fn has_duals(&self) -> bool {
        self.duals.is_some() && !self.duals().unwrap().is_empty()
    }

    /// Get all dual values.
    pub fn duals(&self) -> Option<&HashMap<ConId, f64>> {
        self.duals.as_ref()
    }

    /// Get the reduced cost for a variable (if available).
    pub fn reduced_cost(&self, var: VarId) -> Option<f64> {
        self.reduced_costs.as_ref()?.get(&var).copied()
    }

    /// Check if reduced costs are available.
    pub fn has_reduced_costs(&self) -> bool {
        self.reduced_costs.is_some() && !self.reduced_costs().unwrap().is_empty()
    }

    /// Get all reduced costs.
    pub fn reduced_costs(&self) -> Option<&HashMap<VarId, f64>> {
        self.reduced_costs.as_ref()
    }

    /// Create a lookup function for variable values.
    ///
    /// Useful for expression evaluation.
    pub fn as_var_lookup(&self) -> impl Fn(VarId) -> f64 + '_ {
        move |var| self.value_or_zero(var)
    }
}