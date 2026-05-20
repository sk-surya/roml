//! Linear expression builders for constraints and objectives.
//! NOTE - "builders" - they go away after building is complete.
//! 
//! Expressions are temporary algebraic builders used to construct constraints
//! and objectives. They are compiled into coefficients and then discarded.
//! 
//! # Design
//! 
//! Expressions are NOT:
//! - Stored in the model permanently
//! - Mutated incrementally
//! - Solver-visible
//! 
//! They are used for:
//! - Constraint construction
//! - Objective construction
//! - Expression evaluation with solutions
//! 
//! # Example
//! 
//! ```ignore
//! use roml::{ConstraintExprExt, Model, ObjectiveExprExt};
//!
//! let mut model = Model::new();
//! let x = model.add_var();
//! let y = model.add_var();
//! 
//! // Build and add a constraint: 2*x + 3*y <= 10
//! let con = model.constrain((2.0 * x + 3.0 * y).le(10.0))?;
//! 
//! // Build and activate an objective in one step.
//! let obj = model.maximize(x + 4.0 * y + 5.0)?;
//! assert_eq!(model.objective_constant(obj), Some(5.0));
//! 
//! // Using method syntax:
//! let expr = LinExpr::new()
//!    .term(2.0, x)
//!    .term(3.0, y);
//! let con = model.add_constraint_expr(expr, ConstraintBounds::le(10.0))?;
//! ```

mod linear;

pub use linear::{
	ConstraintExprExt,
	ConstraintSpec,
	LinExpr,
	ObjectiveExprExt,
	ObjectiveSpec,
	Term,
	TermCoeff,
};
