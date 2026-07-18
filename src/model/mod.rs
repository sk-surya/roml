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

pub use variable::{Bounds, VarType, VariableData, VariableStore};
pub use constraint::{ConstraintBounds, ConstraintData, ConstraintStore};
pub use objective::{ObjectiveData, ObjectiveStore, Sense};
pub use parameter::{ParameterData, ParameterStore};
pub use coefficient::{CellKey, CoefficientData, CoefficientIndex, CoefficientTarget};
pub use changelog::{Change, ChangeLog};
pub use transaction::Transaction;

use crate::id::{CoeffId, ConId, ObjId, ParamId, VarId};
use crate::solver::SolveOptions;
use crate::value_expr::ValueExpr;
use crate::expr::{LinExpr, TermCoeff};
use crate::solution::Solution;

use log::warn;


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
    /// Model constants (e.g., tolerances).
    pub constants: ModelConstants,
    /// Tracks semi-continuous lower bounds per variable.
    /// A variable with an entry in this map must be 0 or ≥ the stored value.
    pub(crate) semicontinuous_lower: std::collections::HashMap<VarId, f64>,

    /// Solver options to apply before the next `solve()` call.
    /// Cleared after each solve by `SolverModelExt::solve_model`.
    pub(crate) solver_options: Option<SolveOptions>,
}

#[derive(Clone, Debug)]
pub struct ModelConstants {
    /// Tolerance for considering a constraint violated (negative slack).
    pub feasibility_tolerance: f64,
}

impl Default for ModelConstants {
    fn default() -> Self {
        // default tolerance is a small epsilon used in slack/violation checks.
        Self { feasibility_tolerance: 1e-9 }
    }
}

impl ModelConstants {
    pub fn new() -> Self {
        Self::default()
    }

    /// Helper to build a constants struct with a custom tolerance.
    pub fn set_feas_tol(feasibility_tolerance: f64) -> Self {
        Self { feasibility_tolerance }
    }

    // NOTE: we keep the inherent `default` method for backward compatibility,
    // but the `Default` trait impl above is the one used when models are
    // constructed via `Model::default()`.
    pub fn default() -> Self {
        Self::default()
    }
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

    /// Change a variable's type (Continuous, Integer, Binary).
    ///
    /// Produces a `Change::VariableTypeChanged` which the solver adapter
    /// applies on the next `sync_model` / `apply_changes` call.
    pub fn set_variable_type(&mut self, var: VarId, var_type: VarType) -> Result<(), ModelError> {
        let data = self
            .variables
            .get_mut(var)
            .ok_or(ModelError::VariableNotFound(var))?;
        let old = data.var_type;
        if old != var_type {
            data.var_type = var_type;
            self.changelog
                .push(Change::VariableTypeChanged { var, old, new: var_type });
        }
        Ok(())
    }

    /// Convenience: set variable to binary [0,1].
    pub fn set_binary(&mut self, var: VarId) -> Result<(), ModelError> {
        self.set_variable_type(var, VarType::Binary)?;
        self.set_variable_bounds(var, Bounds::new(0.0, 1.0))?;
        Ok(())
    }

    /// Mark a variable as semi-continuous with the given lower bound.
    ///
    /// A semi-continuous variable can take value 0 or any value between
    /// `lower` and its current upper bound. This tightens the LP relaxation
    /// (the variable cannot be fractionally below `lower`) while remaining
    /// feasible for all integer solutions.
    ///
    /// If `lower` exceeds the current lower bound, the lower bound is raised.
    pub fn set_semicontinuous(&mut self, var: VarId, lower: f64) -> Result<(), ModelError> {
        let bounds = self
            .variable_bounds(var)
            .ok_or(ModelError::VariableNotFound(var))?;
        if lower > bounds.upper {
            return Err(ModelError::InvalidBounds);
        }
        if lower > bounds.lower {
            self.set_variable_bounds(var, Bounds::new(lower, bounds.upper))?;
        }
        self.semicontinuous_lower.insert(var, lower);
        self.changelog
            .push(Change::SemiContinuousBoundChanged { var, lower });
        Ok(())
    }

    /// Set solver options to apply before the next solve.
    ///
    /// Options are cleared automatically after each solve by the
    /// `SolverModelExt::solve_model` implementation.
    pub fn set_solver_options(&mut self, opts: SolveOptions) {
        self.solver_options = Some(opts);
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

    /// Get the constant offset for an objective.
    pub fn objective_constant(&self, obj: ObjId) -> Option<f64> {
        self.objectives.get(obj).map(|data| data.constant)
    }

    /// Get the constant offset for the active objective.
    pub fn active_objective_constant(&self) -> Option<f64> {
        self.active_objective().and_then(|obj| self.objective_constant(obj))
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
    pub fn add_constraint_coefficient<E>(
        &mut self,
        con: ConId,
        var: VarId,
        value_expr: E,
    ) -> Result<CoeffId, ModelError>
    where
        E: Into<ValueExpr>,
    {
        if !self.constraints.contains(con) {
            return Err(ModelError::ConstraintNotFound(con));
        }
        if !self.variables.contains(var) {
            return Err(ModelError::VariableNotFound(var));
        }

        let value_expr = value_expr.into();
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
    pub fn add_objective_coefficient<E>(
        &mut self,
        obj: ObjId,
        var: VarId,
        value_expr: E,
    ) -> Result<CoeffId, ModelError>
    where
        E: Into<ValueExpr>,
    {
        if !self.objectives.contains(obj) {
            return Err(ModelError::ObjectiveNotFound(obj));
        }
        if !self.variables.contains(var) {
            return Err(ModelError::VariableNotFound(var));
        }

        let value_expr = value_expr.into();
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

    /// Add a constant coefficient to an objective.
    pub fn add_objective_coeff(&mut self, obj: ObjId, var: VarId, value: f64) -> Result<CoeffId, ModelError> {
        self.add_objective_coefficient(obj, var, value)
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
            warn!("Uncommitted parameter changes detected, auto-committing");
            self.commit();
        }
        self.changelog.drain()
    }

    /// Get the changelog sequence number.
    pub fn changelog_sequence(&self) -> u64 {
        self.changelog.sequence()
    }
}

// ── Introspection helpers ────────────────────────────────────────────────────

fn format_bound(v: f64) -> String {
    if v == f64::NEG_INFINITY {
        "-inf".to_string()
    } else if v == f64::INFINITY {
        "+inf".to_string()
    } else {
        format!("{v}")
    }
}

fn format_lin_expr(expr: &LinExpr) -> String {
    let terms = expr.terms();
    let constant = expr.get_constant();

    if terms.is_empty() && constant == 0.0 {
        return "0".to_string();
    }

    let mut out = String::new();
    for (i, term) in terms.iter().enumerate() {
        let coeff = match &term.coeff {
            TermCoeff::Constant(v) => *v,
            TermCoeff::Expr(e) => e.as_constant().unwrap_or(f64::NAN),
        };
        let abs_coeff = coeff.abs();
        let negative = coeff < 0.0;

        if i == 0 {
            if (coeff - 1.0).abs() < f64::EPSILON {
                out.push_str(&format!("x[{}]", term.var.index()));
            } else if (coeff + 1.0).abs() < f64::EPSILON {
                out.push_str(&format!("-x[{}]", term.var.index()));
            } else {
                out.push_str(&format!("{coeff}*x[{}]", term.var.index()));
            }
        } else if negative {
            out.push_str(" - ");
            if (abs_coeff - 1.0).abs() < f64::EPSILON {
                out.push_str(&format!("x[{}]", term.var.index()));
            } else {
                out.push_str(&format!("{abs_coeff}*x[{}]", term.var.index()));
            }
        } else {
            out.push_str(" + ");
            if (abs_coeff - 1.0).abs() < f64::EPSILON {
                out.push_str(&format!("x[{}]", term.var.index()));
            } else {
                out.push_str(&format!("{abs_coeff}*x[{}]", term.var.index()));
            }
        }
    }

    if constant.abs() > f64::EPSILON {
        if out.is_empty() {
            out.push_str(&format!("{constant}"));
        } else if constant < 0.0 {
            out.push_str(&format!(" - {}", constant.abs()));
        } else {
            out.push_str(&format!(" + {constant}"));
        }
    }

    if out.is_empty() { "0".to_string() } else { out }
}

// ── Model introspection methods ─────────────────────────────────────────────

impl Model {
    /// Return a human-readable string representation of the model.
    ///
    /// Output format is deterministic (sorted by internal index) and suitable
    /// for debugging and diffing. Similar to Pyomo's `.pprint()`.
    pub fn pprint(&self) -> String {
        use std::fmt::Write;
        let mut out = String::new();

        let name = self.name.as_deref().unwrap_or("unnamed");
        writeln!(out, "Model: {name}").unwrap();

        // Variables
        writeln!(out, "  Variables ({}):", self.variables.len()).unwrap();
        let mut vars: Vec<_> = self.variables.iter().collect();
        vars.sort_by_key(|(id, _)| id.index());
        for (id, data) in &vars {
            let lb = format_bound(data.bounds.lower);
            let ub = format_bound(data.bounds.upper);
            let type_s = match data.var_type {
                VarType::Continuous => "Continuous",
                VarType::Integer => "Integer",
                VarType::Binary => "Binary",
            };
            let inactive = if !data.active { " [inactive]" } else { "" };
            writeln!(out, "    x[{}]: [{lb}, {ub}] {type_s}{inactive}", id.index()).unwrap();
        }

        // Parameters
        writeln!(out, "  Parameters ({}):", self.parameters.len()).unwrap();
        let mut params: Vec<_> = self.parameters.iter().collect();
        params.sort_by_key(|(id, _)| id.index());
        for (id, data) in &params {
            writeln!(out, "    p[{}]: {}", id.index(), data.value).unwrap();
        }

        // Constraints
        writeln!(out, "  Constraints ({}):", self.constraints.len()).unwrap();
        let mut cons: Vec<_> = self.constraints.iter().collect();
        cons.sort_by_key(|(id, _)| id.index());
        for (id, data) in &cons {
            let lb = format_bound(data.bounds.lower);
            let ub = format_bound(data.bounds.upper);
            let inactive = if !data.active { " [inactive]" } else { "" };
            let expr_s = self
                .constraint_expression(*id)
                .map(|e| format_lin_expr(&e))
                .unwrap_or_else(|_| "?".to_string());
            writeln!(out, "    c[{}]: {lb} <= {expr_s} <= {ub}{inactive}", id.index()).unwrap();
        }

        // Objectives
        writeln!(out, "  Objectives ({}):", self.objectives.len()).unwrap();
        let mut objs: Vec<_> = self.objectives.iter().collect();
        objs.sort_by_key(|(id, _)| id.index());
        for (id, data) in &objs {
            let sense = match data.sense {
                Sense::Minimize => "Minimize",
                Sense::Maximize => "Maximize",
            };
            let active = if data.active { " [active]" } else { "" };
            let expr_s = self
                .objective_expression(*id)
                .map(|e| format_lin_expr(&e))
                .unwrap_or_else(|_| "?".to_string());
            writeln!(out, "    obj[{}]: {sense} {expr_s}{active}", id.index()).unwrap();
        }

        out
    }

    /// Compute slack values for a constraint given a solution.
    ///
    /// Returns `(lower_slack, upper_slack)` where:
    /// - `lower_slack = lhs - lower_bound` (positive → lower bound is satisfied)
    /// - `upper_slack = upper_bound - lhs` (positive → upper bound is satisfied)
    pub fn constraint_slack(
        &self,
        con: ConId,
        solution: &Solution,
    ) -> Result<(f64, f64), ModelError> {
        let bounds = self
            .constraints
            .get(con)
            .ok_or(ModelError::ConstraintNotFound(con))?
            .bounds;
        let expr = self.constraint_expression(con)?;
        let lhs = expr.evaluate(solution.as_var_lookup(), self.parameters.as_lookup());
        Ok((lhs - bounds.lower, bounds.upper - lhs))
    }

    /// Iterate over active constraints that are violated by the given solution.
    ///
    /// Yields `(con, lower_slack, upper_slack)` where either slack is negative
    /// (more than a small tolerance).
    pub fn violated_constraints<'a>(
        &'a self,
        solution: &'a Solution,
    ) -> impl Iterator<Item = (ConId, f64, f64)> + 'a {
        self.constraints.iter_active().filter_map(move |(con, _)| {
            let (lower_slack, upper_slack) = self.constraint_slack(con, solution).ok()?;
            if lower_slack < -self.constants.feasibility_tolerance || upper_slack < -self.constants.feasibility_tolerance {
                Some((con, lower_slack, upper_slack))
            } else {
                None
            }
        })
    }

    /// Iterate over active variables whose solution values violate their bounds.
    ///
    /// Yields `(var, violation)` where `violation` is the distance outside the
    /// feasible region (always positive).
    pub fn bound_violations<'a>(
        &'a self,
        solution: &'a Solution,
    ) -> impl Iterator<Item = (VarId, f64)> + 'a {
        self.variables.iter_active().filter_map(move |(var, data)| {
            let val = solution.value_or_zero(var);
            let lower_viol = data.bounds.lower - val; // positive if val < lb
            let upper_viol = val - data.bounds.upper; // positive if val > ub
            let violation = lower_viol.max(upper_viol);
            if violation > self.constants.feasibility_tolerance {
                Some((var, violation))
            } else {
                None
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::expr::LinExpr;

    use super::*;

    // helper to initialize logging once per test run. we ignore the result in
    // case the user hasn't provided a config file; most unit tests don't care
    // about logs but they should compile/link when the function exists.
    fn init_test_logging() {
        static INIT: std::sync::Once = std::sync::Once::new();
        INIT.call_once(|| {
            let _ = crate::init_logging();
        });
    }

    #[test]
    fn basic_model_operations() {
        init_test_logging();
        let mut model = Model::new();

        // Add variables
        let x = model.add_var();
        let y = model.add_var();

        // Add constraint
        // let c = model.add_constraint(ConstraintBounds::le(100.0));

        // // Add coefficients
        // model.add_coeff(c, x, 2.0).unwrap();
        // model.add_coeff(c, y, 3.0).unwrap();

        let _c = model.add_constraint_expr(2.0 * x + 3.0 * y, ConstraintBounds::le(100.0)).unwrap();

        assert_eq!(model.num_variables(), 2);
        assert_eq!(model.num_constraints(), 1);
        assert_eq!(model.num_coefficients(), 2);
    }

    #[test]
    fn parameter_propagation() {
        init_test_logging();
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
    fn coefficient_api_accepts_constants_and_parameters_symmetrically() {
        init_test_logging();
        let mut model = Model::new();

        let p = model.add_parameter(2.5);
        let x = model.add_var();
        let y = model.add_var();
        let con = model.add_constraint(ConstraintBounds::le(100.0));
        let obj = model.add_objective(Sense::Minimize);

        let constraint_coeff = model.add_constraint_coefficient(con, x, p).unwrap();
        let objective_coeff = model.add_objective_coefficient(obj, x, 1.5).unwrap();
        let objective_shorthand = model.add_objective_coeff(obj, y, 3.0).unwrap();

        assert_eq!(model.coefficient(constraint_coeff).unwrap().cached_value, 2.5);
        assert_eq!(model.coefficient(objective_coeff).unwrap().cached_value, 1.5);
        assert_eq!(model.coefficient(objective_shorthand).unwrap().cached_value, 3.0);
    }

    #[test]
    fn transaction_batching() {
        init_test_logging();
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
        init_test_logging();
        let mut model = Model::new();

        let x = model.add_var();
        let c = model.add_constraint(ConstraintBounds::le(100.0));
        model.add_coeff(c, x, 2.0).unwrap();

        let changes = model.drain_changes();
        assert_eq!(changes.len(), 3); // variable, constraint, coefficient
    }

    #[test]
    fn remove_cascades() {
        init_test_logging();
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
        init_test_logging();
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
        assert_eq!(model.objective_constant(obj), Some(5.0));

        // record coefficient ids for later
        let con_coeffs: Vec<_> = model.coefficients.for_constraint(con).collect();
        let obj_coeffs: Vec<_> = model.coefficients.for_objective(obj).collect();

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
        assert_eq!(recon_obj.get_constant(), 5.0);
    }

    // ── pprint ────────────────────────────────────────────────────────────

    /// A production-style LP:
    ///
    ///   3 variables: x (continuous), y (continuous), z (binary)
    ///   2 parameters: a=2.0, b=5.0
    ///   2 constraints:
    ///     c1: a*x + y <= 10     (resource)
    ///     c2: x + b*z >= 1      (activation)
    ///   1 objective:  minimize 3*x + 2*y + 4*z
    ///
    /// pprint is checked for structural presence of key tokens. Visual review
    /// of the printed output is the primary check.
    #[test]
    fn pprint_medium_model() {
        init_test_logging();
        let mut model = Model::with_name("production_lp");

        let x = model.add_variable(Bounds::NON_NEGATIVE, VarType::Continuous);
        let y = model.add_variable(Bounds::NON_NEGATIVE, VarType::Continuous);
        let z = model.add_variable(Bounds::BINARY, VarType::Binary);

        let a = model.add_parameter(2.0);
        let b = model.add_parameter(5.0);

        // c1: a*x + y <= 10
        let c1 = model.add_constraint_expr(
            LinExpr::new().term(a, x).add_term_with(1.0, y),
            ConstraintBounds::le(10.0),
        ).unwrap();

        // c2: x + b*z >= 1
        let _c2 = model.add_constraint_expr(
            LinExpr::new().add_term_with(1.0, x).term(b, z),
            ConstraintBounds::ge(1.0),
        ).unwrap();

        // objective: minimize 3x + 2y + 4z
        let (obj, _) = model.add_objective_expr(
            3.0 * x + 2.0 * y + 4.0 * z,
            Sense::Minimize,
        ).unwrap();
        model.set_active_objective(obj).unwrap();

        // Deactivate c1 to exercise [inactive] marker
        model.set_constraint_active(c1, false).unwrap();

        let output = model.pprint();
        println!("{output}");

        // Structural checks
        assert!(output.contains("Model: production_lp"), "missing model header");
        assert!(output.contains("Variables (3):"), "wrong variable count");
        assert!(output.contains("Parameters (2):"), "wrong parameter count");
        assert!(output.contains("Constraints (2):"), "wrong constraint count");
        assert!(output.contains("Objectives (1):"), "wrong objective count");
        assert!(output.contains("[inactive]"), "missing inactive marker on c1");
        assert!(output.contains("[active]"), "missing active marker on objective");
        assert!(output.contains("Binary"), "missing Binary type for z");
        assert!(output.contains("Minimize"), "missing Minimize sense");
        assert!(output.contains("p["), "missing parameter display");
        assert!(output.contains("c["), "missing constraint display");
        assert!(output.contains("obj["), "missing objective display");
    }

    // ── constraint_slack ─────────────────────────────────────────────────

    #[test]
    fn constraint_slack_feasible() {
        init_test_logging();
        let mut model = Model::new();
        let x = model.add_var();
        let y = model.add_var();

        // 2x + 3y <= 12  →  with x=1, y=2: lhs = 2+6 = 8
        let c = model.add_constraint_expr(
            2.0 * x + 3.0 * y,
            ConstraintBounds::le(12.0),
        ).unwrap();

        use crate::solution::SolutionBuilder;
        use crate::solver::SolverStatus;
        let sol = SolutionBuilder::new()
            .status(SolverStatus::Optimal)
            .value(x, 1.0)
            .value(y, 2.0)
            .build();

        let (lower_slack, upper_slack) = model.constraint_slack(c, &sol).unwrap();
        // lower bound is -inf → lower_slack = lhs - (-inf) = +inf
        assert!(lower_slack.is_infinite() && lower_slack > 0.0,
            "expected +inf lower slack, got {lower_slack}");
        // upper_slack = 12 - 8 = 4
        assert!((upper_slack - 4.0).abs() < model.constants.feasibility_tolerance,
            "expected upper slack = 4, got {upper_slack}");
    }

    #[test]
    fn default_tolerance_is_small_nonzero() {
        // ensure the default value is not zero; it should match the constant
        let m = Model::new();
        assert!(m.constants.feasibility_tolerance > 0.0,
            "default tolerance should be positive");
        assert_eq!(m.constants.feasibility_tolerance, 1e-9);
    }

    #[test]
    fn constraint_slack_violated() {
        init_test_logging();
        let mut model = Model::new();
        let x = model.add_var();

        // x >= 5  → with x=2: lhs=2, lower_slack = 2-5 = -3 (violated)
        let c = model.add_constraint_expr(
            LinExpr::from(x),
            ConstraintBounds::ge(5.0),
        ).unwrap();

        use crate::solution::SolutionBuilder;
        use crate::solver::SolverStatus;
        let sol = SolutionBuilder::new()
            .status(SolverStatus::Optimal)
            .value(x, 2.0)
            .build();

        let (lower_slack, upper_slack) = model.constraint_slack(c, &sol).unwrap();
        // lower_slack = 2 - 5 = -3
        assert!((lower_slack - (-3.0)).abs() < model.constants.feasibility_tolerance,
            "expected lower slack = -3, got {lower_slack}");
        // upper bound is +inf → upper_slack = inf - 2 = +inf
        assert!(upper_slack.is_infinite() && upper_slack > 0.0,
            "expected +inf upper slack, got {upper_slack}");
    }

    // ── violated_constraints ─────────────────────────────────────────────

    #[test]
    fn violated_constraints_finds_violations() {
        init_test_logging();
        let mut model = Model::new();
        let x = model.add_var();
        let y = model.add_var();

        // c1: x <= 3  → satisfied with x=2
        let c1 = model.add_constraint_expr(
            LinExpr::from(x),
            ConstraintBounds::le(3.0),
        ).unwrap();

        // c2: y >= 5  → violated with y=1
        let c2 = model.add_constraint_expr(
            LinExpr::from(y),
            ConstraintBounds::ge(5.0),
        ).unwrap();

        use crate::solution::SolutionBuilder;
        use crate::solver::SolverStatus;
        let sol = SolutionBuilder::new()
            .status(SolverStatus::Optimal)
            .value(x, 2.0)
            .value(y, 1.0)
            .build();

        let violations: Vec<_> = model.violated_constraints(&sol).collect();
        assert_eq!(violations.len(), 1, "expected exactly 1 violated constraint");
        let (con, lower_slack, _upper_slack) = violations[0];
        assert_eq!(con, c2, "violated constraint should be c2");
        assert!((lower_slack - (-4.0)).abs() < 1e-9,
            "expected lower_slack = -4, got {lower_slack}");

        // c1 should not appear
        assert!(!violations.iter().any(|(c, _, _)| *c == c1));
    }

    // ── bound_violations ─────────────────────────────────────────────────

    #[test]
    fn bound_violations_detects_out_of_bounds() {
        init_test_logging();
        let mut model = Model::new();

        // x in [0, 10]  → solution x=12 violates upper bound
        let x = model.add_variable(Bounds::new(0.0, 10.0), VarType::Continuous);

        // y in [2, 8]   → solution y=1 violates lower bound
        let y = model.add_variable(Bounds::new(2.0, 8.0), VarType::Continuous);

        // z in [0, 5]   → solution z=3 is fine
        let z = model.add_variable(Bounds::new(0.0, 5.0), VarType::Continuous);

        use crate::solution::SolutionBuilder;
        use crate::solver::SolverStatus;
        let sol = SolutionBuilder::new()
            .status(SolverStatus::Optimal)
            .value(x, 12.0) // above ub
            .value(y, 1.0)  // below lb
            .value(z, 3.0)  // feasible
            .build();

        let violations: Vec<_> = model.bound_violations(&sol).collect();
        assert_eq!(violations.len(), 2, "expected 2 bound violations");

        let viol_x = violations.iter().find(|(v, _)| *v == x).map(|(_, d)| *d);
        let viol_y = violations.iter().find(|(v, _)| *v == y).map(|(_, d)| *d);
        assert!(viol_x.is_some(), "x should have a bound violation");
        assert!((viol_x.unwrap() - 2.0).abs() < 1e-9,
            "x violation = 12-10 = 2, got {:?}", viol_x);
        assert!(viol_y.is_some(), "y should have a bound violation");
        assert!((viol_y.unwrap() - 1.0).abs() < 1e-9,
            "y violation = 2-1 = 1, got {:?}", viol_y);

        let z_violation = violations.iter().find(|(v, _)| *v == z);
        assert!(z_violation.is_none(), "z should have no bound violation");
    }
}
