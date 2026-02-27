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

use crate::id::{CoeffId, ConId, ObjId, ParamId, VarId};
use crate::model::changelog::{ChangeLog, Change};
use crate::model::coefficient::{CoefficientData, CoefficientIndex, CoefficientTarget};
use crate::model::constraint::ConstraintStore;
use crate::model::objective::ObjectiveStore;
use crate::model::parameter::ParameterStore;
use crate::model::transaction::Transaction;
use crate::model::variable::VariableStore;
use crate::value_expr::ValueExpr;
pub use variable::{Bounds, VarType};
pub use constraint::ConstraintBounds;
pub use objective::Sense;


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
    /// Create a new empty model.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new model with a name.
    pub fn with_name(name: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
            ..Self::default()
        }
    }

    // ========== Variable Operations ==========

    /// Add a new variable with the given bounds and type.
    pub fn add_variable(&mut self, bounds: Bounds, var_type: VarType) -> VarId {
        let id = self.variables.add(bounds, var_type);
        self.changelog.push(Change::VariableAdded {
            var: id,
            bounds,
            var_type,
        });
        id
    }

    /// Add a new continuous variable with non-negative bounds.
    pub fn add_var(&mut self) -> VarId {
        self.add_variable(Bounds::NON_NEGATIVE, VarType::Continuous)
    }

    /// Add a new binary variable.
    pub fn add_binary(&mut self) -> VarId {
        self.add_variable(Bounds::BINARY, VarType::Binary)
    }

    /// Add a new integer variable with the given bounds.
    pub fn add_integer(&mut self, bounds: Bounds) -> VarId {
        self.add_variable(bounds, VarType::Integer)
    }

    /// Remove a variable and all its coefficients.
    pub fn remove_variable(&mut self, var: VarId) -> Result<(), ModelError> {
        if !self.variables.contains(var) {
            return Err(ModelError::VariableNotFound(var));
        }

        // Remove all coefficients for this variable
        let coeffs: Vec<_> = self.coefficients.for_var(var).collect();
        for coeff_id in coeffs {
            self.remove_coefficient_internal(coeff_id);
        }

        self.variables.remove(var);
        self.changelog.push(Change::VariableRemoved { var });
        Ok(())
    }

    /// Get variable bounds.
    pub fn variable_bounds(&self, var: VarId) -> Option<Bounds> {
        self.variables.get(var).map(|d| d.bounds)
    }

    /// Set variable bounds.
    pub fn set_variable_bounds(&mut self, var: VarId, bounds: Bounds) -> Result<(), ModelError> {
        let data = self.variables.get_mut(var).ok_or(ModelError::VariableNotFound(var))?;
        let old = data.bounds;
        if old != bounds {
            data.bounds = bounds;
            self.changelog.push(
                Change::VariableBoundsChanged { var, old, new: bounds }
            );
        }
        Ok(())
    }

    /// Set variable activity.
    pub fn set_variable_active(&mut self, var: VarId, active: bool) -> Result<(), ModelError> {
        let data = self.variables.get_mut(var).ok_or(ModelError::VariableNotFound(var))?;
        if data.active != active {
            data.active = active;
            self.changelog.push(
                Change::VariableActivityChanged { var, active }
            );
        }
        Ok(())
    }

    /// Get the number of variables.
    pub fn num_variables(&self) -> usize {
        self.variables.len()
    }

    // ========== Constraint Operations ==========
    
    /// Add a new constraint with the given bounds.
    pub fn add_constraint(&mut self, bounds: ConstraintBounds) -> ConId {
        let id = self.constraints.add(bounds);
        self.changelog.push(Change::ConstraintAdded { con: id, bounds });
        id
    }

    /// Remove a constraint and all its coefficients.
    pub fn remove_constraint(&mut self, con: ConId) -> Result<(), ModelError> {
        if !self.constraints.contains(con) {
            return Err(ModelError::ConstraintNotFound(con));
        }

        // Remove all coefficients for this constraint
        let coeffs: Vec<_> = self.coefficients.for_constraint(con).collect();
        for coeff_id in coeffs {
            self.remove_coefficient_internal(coeff_id);
        }

        self.constraints.remove(con);
        self.changelog.push(Change::ConstraintRemoved { con });
        Ok(())
    }

    /// Set constraint bounds.
    pub fn set_constraint_bounds(&mut self, con: ConId, bounds: ConstraintBounds) -> Result<(), ModelError> {
        let data = self.constraints.get_mut(con).ok_or(ModelError::ConstraintNotFound(con))?;
        let old = data.bounds;
        if old != bounds {
            data.bounds = bounds;
            self.changelog.push(Change::ConstraintBoundsChanged { con, old, new: bounds });
        }
        Ok(())
    }

    /// Set constraint activity.
    pub fn set_constraint_active(&mut self, con: ConId, active: bool) -> Result<(), ModelError> {
        let data = self.constraints.get_mut(con).ok_or(ModelError::ConstraintNotFound(con))?;
        let old = data.active;
        if old != active {
            data.active = active;
            self.changelog.push(
                Change::ConstraintActivityChanged { con, active }
            );
        }
        Ok(())
    }

    /// Get the number of constraints.
    pub fn num_constraints(&self) -> usize {
        self.constraints.len()
    }

    // ========== Objective Operations ==========

    /// Add a new objective with the given sense.
    pub fn add_objective(&mut self, sense: Sense) -> ObjId {
        let id = self.objectives.add(sense);
        self.changelog.push(Change::ObjectiveAdded { obj: id, sense });
        id
    }

    /// Remove an objective and all its coefficients.
    pub fn remove_objective(&mut self, obj: ObjId) -> Result<(), ModelError> {
        if !self.objectives.contains(obj) {
            return Err(ModelError::ObjectiveNotFound(obj));
        }

        // Remove all coefficients for this objective
        let coeffs: Vec<_> = self.coefficients.for_objective(obj).collect();
        for coeff_id in coeffs {
            self.remove_coefficient_internal(coeff_id);
        }

        self.objectives.remove(obj);
        self.changelog.push(Change::ObjectiveRemoved { obj });
        Ok(())
    }

    /// Set the active objective.
    pub fn set_active_objective(&mut self, obj: ObjId) -> Result<(), ModelError> {
        if !self.objectives.contains(obj) {
            return Err(ModelError::ObjectiveNotFound(obj));
        }
        let old = self.objectives.active();
        if old != Some(obj) {
            self.objectives.set_active(obj);
            self.changelog.push(Change::ActiveObjectiveChanged { old, new: Some(obj) });
        }
        Ok(())
    }

    /// Clear the active objective.
    pub fn clear_active_objective(&mut self) {
        let old = self.objectives.active();
        if old.is_some() {
            self.objectives.clear_active();
            self.changelog.push(Change::ActiveObjectiveChanged { old, new: None });
        }
    }

    /// Get the active objective.
    pub fn active_objective(&self) -> Option<ObjId> {
        self.objectives.active()
    }

    /// Get the number of objectives.
    pub fn num_objectives(&self) -> usize {
        self.objectives.len()
    }

    // ========== Parameter Operations ==========

    /// Add a new parameter with the given initial value.
    pub fn add_parameter(&mut self, value: f64) -> ParamId {
        self.parameters.add(value)
    }

    /// Get a parameter value.
    pub fn parameter_value(&self, param: ParamId) -> Option<f64> {
        self.parameters.get_value(param)
    }

    /// Queue a parameter change in the current transaction.
    ///
    /// The change is not applied until `commit()` is called.
    pub fn set_parameter(&mut self, param: ParamId, value: f64) {
        self.transaction.set_param(param, value);
    }

    /// Check if there are uncommitted parameter changes.
    pub fn has_uncommitted(&self) -> bool {
        self.transaction.has_pending()
    }

    /// Commit all pending parameter changes.
    ///
    /// This:
    /// 1. Applies all queued parameter value changes
    /// 2. Propagates changes to dependent coefficients
    /// 3. Logs all changes to the changelog
    pub fn commit(&mut self) {
        for (param, new_value) in self.transaction.take_pending() {
            self.apply_parameter_change(param, new_value);
        }
    }

    /// Apply a single parameter change and propagate to coefficients.
    fn apply_parameter_change(&mut self, param: ParamId, new_value: f64) {
        let old_value = match self.parameters.set_value(param, new_value) {
            Some(v) => v,
            None => return, // Parameter doesn't exist
        };

        if (old_value - new_value).abs() < f64::EPSILON {
            return; // No change
        }

        // Log the parameter change
        self.changelog.push(Change::ParameterValueChanged {
            param,
            old: old_value,
            new: new_value,
        });

        // Propagate to dependent coefficients
        let affected: Vec<_> = self.coefficients.for_param(param).collect();
        let lookup = self.parameters.as_lookup();

        for coeff_id in affected {
            if let Some(data) = self.coefficients.get_mut(coeff_id) {
                let old_cached = data.cached_value;
                let new_cached = data.value_expr.eval(&lookup);

                if (old_cached - new_cached).abs() >= f64::EPSILON {
                    data.cached_value = new_cached;
                    self.changelog.push(Change::CoefficientValueChanged {
                        coeff: coeff_id,
                        var: data.var,
                        target: data.target,
                        old: old_cached,
                        new: new_cached,
                    });
                }
            }
        }
    }

    /// Rollback uncommitted parameter changes.
    pub fn rollback(&mut self) {
        self.transaction.rollback();
    }

    /// Get the number of parameters.
    pub fn num_parameters(&self) -> usize {
        self.parameters.len()
    }

    // ========== Coefficient Operations ==========

    /// Add a coefficient to a constraint.
    pub fn add_constraint_coefficient(
        &mut self,
        con: ConId,
        var: VarId,
        value_expr: ValueExpr,
    ) -> Result<CoeffId, ModelError> {
        if !self.constraints.contains(con) {
            return Err(ModelError::ConstraintNotFound(con));
        }
        if !self.variables.contains(var) {
            return Err(ModelError::VariableNotFound(var));
        }

        let target = CoefficientTarget::Constraint(con);
        let initial_value = value_expr.eval(self.parameters.as_lookup());
        let id = self.coefficients.add(var, target, value_expr, initial_value);

        self.changelog.push(Change::CoefficientAdded {
            coeff: id,
            var,
            target,
            value: initial_value,
        });

        Ok(id)
    }

    /// Add a coefficient to an objective.
    pub fn add_objective_coefficient(
        &mut self,
        obj: ObjId,
        var: VarId,
        value_expr: ValueExpr,
    ) -> Result<CoeffId, ModelError> {
        if !self.objectives.contains(obj) {
            return Err(ModelError::ObjectiveNotFound(obj));
        }
        if !self.variables.contains(var) {
            return Err(ModelError::VariableNotFound(var));
        }

        let target = CoefficientTarget::Objective(obj);
        let initial_value = value_expr.eval(self.parameters.as_lookup());
        let id = self.coefficients.add(var, target, value_expr, initial_value);

        self.changelog.push(Change::CoefficientAdded {
            coeff: id,
            var,
            target,
            value: initial_value,
        });

        Ok(id)
    }

    /// Add a constant coefficient to a constraint.
    pub fn add_coeff(&mut self, con: ConId, var: VarId, value: f64) -> Result<CoeffId, ModelError> {
        self.add_constraint_coefficient(con, var, ValueExpr::constant(value))
    }

    /// Remove a coefficient.
    pub fn remove_coefficient(&mut self, coeff: CoeffId) -> Result<(), ModelError> {
        if !self.coefficients.contains(coeff) {
            return Err(ModelError::CoefficientNotFound(coeff));
        }
        self.remove_coefficient_internal(coeff);
        Ok(())
    }

    /// Internal coefficient removal (no validation).
    fn remove_coefficient_internal(&mut self, coeff: CoeffId) {
        if let Some(data) = self.coefficients.remove(coeff) {
            self.changelog.push(Change::CoefficientRemoved {
                coeff,
                var: data.var,
                target: data.target,
            });
        }
    }

    /// Get coefficient data.
    pub fn coefficient(&self, coeff: CoeffId) -> Option<&CoefficientData> {
        self.coefficients.get(coeff)
    }

    /// Get the number of coefficients.
    pub fn num_coefficients(&self) -> usize {
        self.coefficients.len()
    }

    // ========== Changelog Operations ==========
    
    /// Check if there are pending changes for the solver.
    pub fn has_pending_changes(&self) -> bool {
        !self.changelog.is_empty()
    }

    /// Drain all pending changes.
    ///
    /// If there are uncommitted parameter changes, this will:
    /// 1. Log a warning
    /// 2. Auto-commit the changes
    pub fn drain_changes(&mut self) -> Vec<Change> {
        if self.has_uncommitted() {
            log::warn!("Uncommitted parameter changes detected, auto-committing");
            self.commit();
        }
        self.changelog.drain()
    }

    /// Get the changelog sequence number.
    pub fn changelog_sequence(&self) -> u64 {
        self.changelog.sequence()
    }
}

#[cfg(test)]
mod tests {
    use crate::expr::LinExpr;

    use super::*;

    #[test]
    fn basic_model_operations() {
        let mut model = Model::new();

        // Add variables
        let x = model.add_var();
        let y = model.add_var();

        // Add constraint
        // let c = model.add_constraint(ConstraintBounds::le(100.0));

        // // Add coefficients
        // model.add_coeff(c, x, 2.0).unwrap();
        // model.add_coeff(c, y, 3.0).unwrap();

        let c = model.add_constraint_expr(2.0 * x + 3.0 * y, ConstraintBounds::le(100.0)).unwrap();

        assert_eq!(model.num_variables(), 2);
        assert_eq!(model.num_constraints(), 1);
        assert_eq!(model.num_coefficients(), 2);
    }

    #[test]
    fn parameter_propagation() {
        let mut model = Model::new();

        let p = model.add_parameter(10.0);
        let x = model.add_var();
        let c = model.add_constraint(ConstraintBounds::le(100.0));

        // Coefficient with parameter dependency: 2 * p
        let coeff_id = model
            .add_constraint_coefficient(
                c,
                x,
                ValueExpr::mul(ValueExpr::constant(2.0), ValueExpr::param(p)),
            )
            .unwrap();

        // Initial value should be 2 * 10 = 20
        assert_eq!(model.coefficient(coeff_id).unwrap().cached_value, 20.0);

        // Change parameter
        model.set_parameter(p, 5.0);
        model.commit();

        // Value should now be 2 * 5 = 10
        assert_eq!(model.coefficient(coeff_id).unwrap().cached_value, 10.0);
    }

    #[test]
    fn transaction_batching() {
        let mut model = Model::new();

        let p1 = model.add_parameter(1.0);
        let p2 = model.add_parameter(2.0);
        let x = model.add_var();
        let c = model.add_constraint(ConstraintBounds::le(100.0));

        // Coefficient: p1 * p2
        let coeff_id = model
            .add_constraint_coefficient(
                c,
                x,
                ValueExpr::mul(ValueExpr::param(p1), ValueExpr::param(p2)),
            )
            .unwrap();

        assert_eq!(model.coefficient(coeff_id).unwrap().cached_value, 2.0); // 1 * 2

        // Batch changes
        model.set_parameter(p1, 3.0);
        model.set_parameter(p2, 4.0);

        // Not committed yet - value unchanged
        assert_eq!(model.coefficient(coeff_id).unwrap().cached_value, 2.0);

        model.commit();

        // Now it's 3 * 4 = 12
        assert_eq!(model.coefficient(coeff_id).unwrap().cached_value, 12.0);
    }

    #[test]
    fn changelog_tracking() {
        let mut model = Model::new();

        let x = model.add_var();
        let c = model.add_constraint(ConstraintBounds::le(100.0));
        model.add_coeff(c, x, 2.0).unwrap();

        let changes = model.drain_changes();
        assert_eq!(changes.len(), 3); // variable, constraint, coefficient
    }

    #[test]
    fn remove_cascades() {
        let mut model = Model::new();

        let x = model.add_var();
        let c = model.add_constraint(ConstraintBounds::le(100.0));
        model.add_coeff(c, x, 2.0).unwrap();

        assert_eq!(model.num_coefficients(), 1);

        // Removing the variable should remove its coefficient
        model.remove_variable(x).unwrap();

        assert_eq!(model.num_coefficients(), 0);
    }

    #[test]
    fn complex_model_flow() {
        // build a model with variables, parameters, constraints, objective
        let mut model = Model::new();
        let x = model.add_variable(Bounds::NON_NEGATIVE, VarType::Continuous);
        let y = model.add_variable(Bounds::NON_NEGATIVE, VarType::Continuous);
        let z = model.add_variable(Bounds::NON_NEGATIVE, VarType::Continuous);

        let p = model.add_parameter(2.0);
        let q = model.add_parameter(3.0);

        // constraint: 2*x + p*y - q*z <= 100
        let cons_expr: LinExpr = 2.0 * x + p * y - q * z;
        let cons_bounds = ConstraintBounds::le(100.0);
        let con = model.add_constraint_expr(cons_expr, cons_bounds).unwrap();

        // objective: minimize p*x + 3*y + 5
        let obj_expr: LinExpr = p * x + 3.0 * y + 5.0;
        let (obj, offset) = model.add_objective_expr(obj_expr, Sense::Minimize).unwrap();
        assert_eq!(offset, 5.0);

        // record coefficient ids for later
        let mut con_coeffs: Vec<_> = model.coefficients.for_constraint(con).collect();
        let mut obj_coeffs: Vec<_> = model.coefficients.for_objective(obj).collect();

        // check initial cached values
        let mut map = std::collections::HashMap::new();
        for cid in &con_coeffs {
            let dat = model.coefficient(*cid).unwrap();
            map.insert(dat.var, dat.cached_value);
        }
        assert_eq!(map.get(&x), Some(&2.0));
        assert_eq!(map.get(&y), Some(&2.0)); // p=2 initial
        assert_eq!(map.get(&z), Some(&-3.0));

        let mut objmap = std::collections::HashMap::new();
        for oid in &obj_coeffs {
            let dat = model.coefficient(*oid).unwrap();
            objmap.insert(dat.var, dat.cached_value);
        }
        assert_eq!(objmap.get(&x), Some(&2.0));
        assert_eq!(objmap.get(&y), Some(&3.0));

        // update parameters and commit
        model.set_parameter(p, 4.0);
        model.set_parameter(q, 6.0);
        model.commit();

        // after update, cached values should change
        let mut map2 = std::collections::HashMap::new();
        for cid in &con_coeffs {
            let dat = model.coefficient(*cid).unwrap();
            map2.insert(dat.var, dat.cached_value);
        }
        assert_eq!(map2.get(&y), Some(&4.0));
        assert_eq!(map2.get(&z), Some(&-6.0));

        let mut objmap2 = std::collections::HashMap::new();
        for oid in &obj_coeffs {
            let dat = model.coefficient(*oid).unwrap();
            objmap2.insert(dat.var, dat.cached_value);
        }
        assert_eq!(objmap2.get(&x), Some(&4.0));

        // also reconstruct expressions to ensure they still look right
        let recon = model.constraint_expression(con).unwrap();
        assert_eq!(recon.num_terms(), 3);
        let recon_obj = model.objective_expression(obj).unwrap();
        assert_eq!(recon_obj.num_terms(), 2);
    }
}
