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
//! ```
//! let mut model = Model::new();
//! let x = model.add_var();
//! let y = model.add_var();
//! 
//! // Build expression: 2*x + 3*y <= 10
//! let expr = 2.0 * x + 3.0 * y;
//! let con = model.add_constraint(expr <= 10.0);
//! 
//! // Using method syntax:
//! let expr = LinExpr::new()
//!    .term(x, 2.0)
//!    .term(y, 3.0);
//! let con = model.add_constraint(expr, ConstraintBounds::le(10.0));
//! ```

mod linear;

pub use linear::{LinExpr, Term, TermCoeff};
