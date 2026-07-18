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

pub mod backend;
pub mod callback;
pub mod request;
pub mod session;

use std::collections::HashMap;

use crate::{
    ConId,
    Model,
    Solution,
    SolutionBuilder,
    VarId,
    model::changelog::Change,
};

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

/// Algorithm selection for LP optimization.
///
/// Each solver adapter maps these to its own controls.
/// Unsupported options are silently ignored (best-effort).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LpAlgorithm {
    /// Let the solver choose automatically (default).
    #[default]
    Automatic,
    /// Primal simplex.
    PrimalSimplex,
    /// Dual simplex.
    DualSimplex,
    /// Barrier / interior point method.
    Barrier,
}

/// Generic solver options that can be passed to any solver adapter.
///
/// Options are best-effort: if a solver does not support a particular
/// option, it is silently ignored.
#[derive(Debug, Clone, Default)]
pub struct SolveOptions {
    /// LP algorithm to use for the next solve.
    /// `None` = solver default / automatic selection.
    pub lp_algorithm: Option<LpAlgorithm>,
}

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

    /// Objective value from the last solve (if optimal).
    ///
    /// Default implementation returns `None`. Solvers should override this.
    fn objective_value_raw(&self) -> Option<f64> {
        None
    }

    /// Dual values for constraints from the last solve.
    ///
    /// Only meaningful for LP (not MIP). Default returns `None`.
    fn dual_values(&self) -> Option<HashMap<ConId, f64>> {
        None
    }

    /// Reduced costs for variables from the last solve.
    ///
    /// Only meaningful for LP (not MIP). Default returns `None`.
    fn reduced_costs_raw(&self) -> Option<HashMap<VarId, f64>> {
        None
    }

    /// Reset the solver state for a full rebuild.
    fn reset(&mut self);

    /// Check if the solver supports incremental updates for a change type.
    fn supports_incremental(&self, change: &Change) -> bool;

    /// Check if the solver supports semi-continuous variables.
    fn supports_semi_continuous(&self) -> bool {
        false
    }

    /// Apply solver-specific options before the next `solve()`.
    ///
    /// Options are best-effort: unsupported options are silently ignored.
    /// Default implementation is a no-op.
    fn apply_options(&mut self, _options: &SolveOptions) -> Result<(), SolverError> {
        Ok(())
    }

    /// Enable or disable solver console output (e.g. iteration log).
    ///
    /// When enabled, the solver prints progress information to the console
    /// during `solve()`. Default: disabled (no output).
    fn set_console_output(&mut self, _enabled: bool) -> Result<(), SolverError> {
        Ok(())
    }

    /// Register a callback handler for MIP callbacks (lazy constraints, cuts).
    ///
    /// The handler is invoked during `solve()` when the solver finds
    /// candidate integer solutions. This enables lazy constraints,
    /// cutting planes, and solution inspection during branch-and-cut.
    ///
    /// Default: returns `NotSupported`.
    fn set_callback_handler(
        &mut self,
        _handler: Box<dyn callback::CallbackHandler>,
    ) -> Result<(), SolverError> {
        Err(SolverError::NotSupported(
            "callbacks not supported by this solver".into(),
        ))
    }
}

/// Convenience helpers for syncing and solving a model with a solver adapter.
pub trait SolverModelExt: SolverAdapter {
    /// Drain pending model changes and apply them to the adapter.
    fn sync_model(&mut self, _model: &mut Model) -> Result<(), SolverError> {
        // NOTE: This is a legacy method. Model no longer exposes drain_changes.
        // Use model.commit() + DeltaBatch synchronization instead.
        Ok(())
    }

    /// Synchronize a model, solve it, and assemble a [`Solution`].
    fn solve_model(&mut self, model: &mut Model) -> Result<Solution, SolverError> {
        self.sync_model(model)?;

        // SolveOptions is no longer stored on Model; default options are used.
        let status = self.solve()?;
        let mut builder = SolutionBuilder::new().status(status);

        if let Some(values) = self.solution_values() {
            builder = builder.values(values);
        }

        if let Some(objective_id) = model.active_objective() {
            builder = builder.objective_id(objective_id);
            if let Some(objective_value) = self.objective_value_raw() {
                let objective_value = objective_value
                    + model.objective_constant(objective_id).unwrap_or(0.0);
                builder = builder.objective_value(objective_value);
            }
        } else if let Some(objective_value) = self.objective_value_raw() {
            builder = builder.objective_value(objective_value);
        }

        if let Some(duals) = self.dual_values() {
            builder = builder.duals(duals);
        }

        if let Some(reduced_costs) = self.reduced_costs_raw() {
            builder = builder.reduced_costs(reduced_costs);
        }

        Ok(builder.build())
    }
}

impl<T> SolverModelExt for T where T: SolverAdapter + ?Sized {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Change;
    use crate::ConstraintExprExt;

    #[derive(Default)]
    struct MockAdapter {
        applied_change_count: usize,
        solve_calls: usize,
        status: SolverStatus,
        values: Option<HashMap<VarId, f64>>,
        objective_value: Option<f64>,
        duals: Option<HashMap<ConId, f64>>,
        reduced_costs: Option<HashMap<VarId, f64>>,
    }

    impl SolverAdapter for MockAdapter {
        fn apply_changes(&mut self, changes: &[Change]) -> Result<(), SolverError> {
            self.applied_change_count += changes.len();
            Ok(())
        }

        fn solve(&mut self) -> Result<SolverStatus, SolverError> {
            self.solve_calls += 1;
            Ok(self.status)
        }

        fn status(&self) -> SolverStatus {
            self.status
        }

        fn solution_values(&self) -> Option<HashMap<VarId, f64>> {
            self.values.clone()
        }

        fn objective_value_raw(&self) -> Option<f64> {
            self.objective_value
        }

        fn dual_values(&self) -> Option<HashMap<ConId, f64>> {
            self.duals.clone()
        }

        fn reduced_costs_raw(&self) -> Option<HashMap<VarId, f64>> {
            self.reduced_costs.clone()
        }

        fn reset(&mut self) {}

        fn supports_incremental(&self, _change: &Change) -> bool {
            true
        }
    }

    #[test]
    fn solve_model_syncs_changes_and_builds_solution() {
        let mut model = Model::new();
        let x = model.add_var();
        let obj = model.minimize(x + 2.0).unwrap();

        let mut duals = HashMap::new();
        let con = model.constraint(x.ge(0.0)).unwrap();
        duals.insert(con, 0.0);

        let mut values = HashMap::new();
        values.insert(x, 3.5);

        let mut reduced_costs = HashMap::new();
        reduced_costs.insert(x, 0.0);

        let mut adapter = MockAdapter {
            status: SolverStatus::Optimal,
            values: Some(values),
            objective_value: Some(3.5),
            duals: Some(duals),
            reduced_costs: Some(reduced_costs),
            ..Default::default()
        };

        let solution = adapter.solve_model(&mut model).unwrap();

        assert_eq!(adapter.solve_calls, 1);
        assert_eq!(solution.status(), SolverStatus::Optimal);
        assert_eq!(solution.objective_id(), Some(obj));
        assert_eq!(solution.objective_value(), Some(5.5));
        assert_eq!(solution.value(x), Some(3.5));
        assert_eq!(solution.dual(con), Some(0.0));
        assert_eq!(solution.reduced_cost(x), Some(0.0));
    }
}
