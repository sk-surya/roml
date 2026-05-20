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

mod logging;

// Re-export commonly used types for public API
pub use id::{VarId, ConId, ParamId, ObjId, CoeffId};
pub use model::{Model, Bounds, VarType, ConstraintBounds, Sense, ModelError};
pub use model::changelog::Change;
pub use expr::{ConstraintExprExt, ConstraintSpec, LinExpr, ObjectiveExprExt, ObjectiveSpec};
pub use value_expr::ValueExpr;
pub use solution::{Solution, SolutionBuilder, SolutionStore};
pub use solver::{SolverAdapter, SolverError, SolverModelExt, SolverStatus};

/// Common imports for the fluent modeling API.
pub mod prelude {
	pub use crate::{
		Bounds,
		Change,
		CoeffId,
		ConId,
		ConstraintBounds,
		ConstraintExprExt,
		ConstraintSpec,
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

