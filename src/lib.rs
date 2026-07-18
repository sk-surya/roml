//! ROML - Rust Optimization Modeling Library
//!
//! A production-grade, incremental MILP modeling library that:
//! - Supports efficient model mutation
//! - Cleanly separates model and solver concerns
//! - Supports multiple solvers (first target: HiGHS)
//! - Stores and reasons about solutions
//! - Allows algebraic introspection (slack, infeasibility, evaluation)

pub mod id;
pub mod model;
pub mod value_expr;
pub mod expr;
pub mod solution;
pub mod solver;

pub mod delta;
pub mod revision;
pub mod sync;
pub mod journal;
pub mod snapshot;

mod logging;

// Re-export commonly used types for public API
pub use id::{VarId, ConId, ParamId, ObjId, CoeffId};
pub use model::{Model, Bounds, VarType, ConstraintBounds, Sense, ModelError};
pub use expr::{ConstraintExprExt, ConstraintSpec, LinExpr, ObjectiveExprExt, ObjectiveSpec};
pub use value_expr::ValueExpr;
pub use solution::{Solution, SolutionBuilder, SolutionStore};
pub use solver::{LpAlgorithm, SolveOptions, SolverAdapter, SolverError, SolverModelExt, SolverStatus};

/// Build a [`ConstraintSpec`] from math-like tokens.
///
/// Supports infix `<=`, `>=`, and `==` forms, plus an explicit ranged form:
///
/// ```ignore
/// use roml::{constraint, Model};
///
/// let mut model = Model::new();
/// let x = model.add_var();
/// let y = model.add_var();
///
/// let cap = constraint!(2.0 * x + y <= 10.0);
/// let floor = constraint!(x >= 1.0);
/// let band = constraint!(between: 0.0, x + y, 5.0);
/// ```
#[macro_export]
macro_rules! constraint {
	(between: $lower:expr, $expr:expr, $upper:expr) => {
		$crate::ConstraintSpec::new($expr, $crate::ConstraintBounds::range($lower, $upper))
	};
	(@scan [$($lhs:tt)+] <= $rhs:expr) => {
		$crate::ConstraintSpec::new(
			$crate::constraint!(@expr $($lhs)+),
			$crate::ConstraintBounds::le($rhs),
		)
	};
	(@scan [$($lhs:tt)+] >= $rhs:expr) => {
		$crate::ConstraintSpec::new(
			$crate::constraint!(@expr $($lhs)+),
			$crate::ConstraintBounds::ge($rhs),
		)
	};
	(@scan [$($lhs:tt)+] == $rhs:expr) => {
		$crate::ConstraintSpec::new(
			$crate::constraint!(@expr $($lhs)+),
			$crate::ConstraintBounds::eq($rhs),
		)
	};
	(@scan [$($lhs:tt)*] $next:tt $($rest:tt)*) => {
		$crate::constraint!(@scan [$($lhs)* $next] $($rest)*)
	};
	(@scan [$($lhs:tt)*]) => {
		compile_error!(
			"constraint! expects `expr <= rhs`, `expr >= rhs`, `expr == rhs`, or `between: lower, expr, upper`",
		)
	};
	(@expr $expr:expr) => {
		$expr
	};
	($($tokens:tt)+) => {
		$crate::constraint!(@scan [] $($tokens)+)
	};
}

/// Build an [`ObjectiveSpec`] from a sense and expression.
///
/// ```ignore
/// use roml::{objective, Model};
///
/// let mut model = Model::new();
/// let x = model.add_var();
/// let y = model.add_var();
///
/// let max_profit = objective!(maximize: x + 2.0 * y);
/// let min_cost = objective!(minimize: 3.0 * x + y);
/// ```
#[macro_export]
macro_rules! objective {
	(minimize: $expr:expr) => {
		$crate::ObjectiveSpec::new($crate::Sense::Minimize, $expr)
	};
	(maximize: $expr:expr) => {
		$crate::ObjectiveSpec::new($crate::Sense::Maximize, $expr)
	};
	($($tokens:tt)*) => {
		compile_error!("objective! expects `minimize: expr` or `maximize: expr`")
	};
}

/// Add a constraint to a model from math-like tokens.
///
/// ```ignore
/// use roml::{constrain, Model};
///
/// let mut model = Model::new();
/// let x = model.add_var();
/// let y = model.add_var();
///
/// constrain!(model, x + y <= 4.0)?;
/// constrain!(model, between: 0.0, x, 3.0)?;
/// ```
#[macro_export]
macro_rules! constrain {
	($model:expr, between: $lower:expr, $expr:expr, $upper:expr) => {
		$model.constrain($crate::constraint!(between: $lower, $expr, $upper))
	};
	($model:expr, $($tokens:tt)+) => {
		$model.constrain($crate::constraint!($($tokens)+))
	};
}

/// Add and activate an objective on a model from a sense and expression.
///
/// ```ignore
/// use roml::{set_objective, Model};
///
/// let mut model = Model::new();
/// let x = model.add_var();
/// let y = model.add_var();
///
/// let obj = set_objective!(model, maximize: x + 2.0 * y + 3.0)?;
/// assert_eq!(model.objective_constant(obj), Some(3.0));
/// ```
#[macro_export]
macro_rules! set_objective {
	($model:expr, minimize: $expr:expr) => {
		$model.set_objective($crate::objective!(minimize: $expr))
	};
	($model:expr, maximize: $expr:expr) => {
		$model.set_objective($crate::objective!(maximize: $expr))
	};
	($model:expr, $spec:expr) => {
		$model.set_objective($spec)
	};
}

/// Common imports for the fluent modeling API.
pub mod prelude {
	pub use crate::{
		Bounds,
		CoeffId,
		ConId,
		ConstraintBounds,
		ConstraintExprExt,
		ConstraintSpec,
		constrain,
		LinExpr,
		Model,
		ModelError,
		ObjId,
		ObjectiveExprExt,
		ObjectiveSpec,
		ParamId,
		Sense,
		Solution,
		SolverAdapter,
		SolverError,
		SolverModelExt,
		SolverStatus,
		set_objective,
		ValueExpr,
		VarId,
		VarType,
	};
}

// Logging initialization helper re-exported at crate root for consumers that
// want to configure logging via a `log4rs.yaml` file.  This function will
// attempt to load the configuration from the path given by the
// `LOG4RS_CONFIG` environment variable, falling back to `log4rs.yaml` in the
// current working directory.  It returns a `log4rs::Handle` on success so the
// caller can optionally hold it or ignore it.

pub use logging::init_logging;

