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

/// Builder for constructing solutions.
#[derive(Clone, Debug, Default)]
pub struct SolutionBuilder {
    values: HashMap<VarId, f64>,
    objective_value: Option<f64>,
    objective_id: Option<ObjId>,
    status: SolverStatus,
    duals: Option<HashMap<ConId, f64>>,
    reduced_costs: Option<HashMap<VarId, f64>>,
}

impl SolutionBuilder {
    /// Create a new builder with NotSolved status.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the solver status.
    pub fn status(mut self, status: SolverStatus) -> Self {
        self.status = status;
        self
    }

    /// Set a variable value.
    pub fn value(mut self, var: VarId, value: f64) -> Self {
        self.values.insert(var, value);
        self
    }

    /// Set all variable values.
    pub fn values(mut self, values: HashMap<VarId, f64>) -> Self {
        self.values = values;
        self
    }

    /// Set the objective value.
    pub fn objective_value(mut self, value: f64) -> Self {
        self.objective_value = Some(value);
        self
    }

    /// Set which objective this solution is for.
    pub fn objective_id(mut self, obj: ObjId) -> Self {
        self.objective_id = Some(obj);
        self
    }

    /// Set a dual value for a constraint.
    pub fn dual(mut self, con: ConId, value: f64) -> Self {
        self.duals.get_or_insert_with(HashMap::new).insert(con, value);
        self
    }

    /// Set all dual values.
    pub fn duals(mut self, duals: HashMap<ConId, f64>) -> Self {
        self.duals = Some(duals);
        self
    }

    /// Set a reduced cost for a variable.
    pub fn reduced_cost(mut self, var: VarId, value: f64) -> Self {
        self.reduced_costs.get_or_insert_with(HashMap::new).insert(var, value);
        self
    }

    /// Set all reduced costs.
    pub fn reduced_costs(mut self, costs: HashMap<VarId, f64>) -> Self {
        self.reduced_costs = Some(costs);
        self
    }

    /// Build the solution.
    pub fn build(self) -> Solution {
        Solution {
            values: self.values,
            objective_value: self.objective_value,
            objective_id: self.objective_id,
            status: self.status,
            duals: self.duals,
            reduced_costs: self.reduced_costs,
        }
    }
}

/// Storage for multiple solutions (latest, named, etc.).
#[derive(Clone, Debug, Default)]
pub struct SolutionStore {
    /// The most recent solution.
    latest: Option<Solution>,
    /// Named solution snapshots.
    named: HashMap<String, Solution>,
}

impl SolutionStore {
    /// Create an empty solution store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Store a solution as the latest.
    pub fn set_latest(&mut self, solution: Solution) {
        self.latest = Some(solution);
    }

    /// Get the latest solution.
    pub fn latest(&self) -> Option<&Solution> {
        self.latest.as_ref()
    }

    /// Take the latest solution (removing it from the store).
    pub fn take_latest(&mut self) -> Option<Solution> {
        self.latest.take()
    }

    /// Save the latest solution with a name.
    pub fn save_as(&mut self, name: impl Into<String>) -> bool {
        if let Some(solution) = &self.latest {
            self.named.insert(name.into(), solution.clone());
            true
        } else {
            false
        }
    }

    /// Store a named solution.
    pub fn set_named(&mut self, name: impl Into<String>, solution: Solution) {
        self.named.insert(name.into(), solution);
    }

    /// Get a named solution.
    pub fn get_named(&self, name: &str) -> Option<&Solution> {
        self.named.get(name)
    }

    /// Remove a named solution.
    pub fn remove_named(&mut self, name: &str) -> Option<Solution> {
        self.named.remove(name)
    }

    /// List all named solution names.
    pub fn named_solutions(&self) -> impl Iterator<Item = &str> {
        self.named.keys().map(|s| s.as_str())
    }

    /// Clear all solutions.
    pub fn clear(&mut self) {
        self.latest = None;
        self.named.clear();
    }
}