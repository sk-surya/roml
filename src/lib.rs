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

// Re-export commonly used types for public API
pub use id::{VarId, ConId, ParamId, ObjId, CoeffId};
pub use model::{Model, Bounds, VarType, ConstraintBounds, Sense, ModelError};
pub use model::changelog::Change;
pub use expr::LinExpr;
pub use value_expr::ValueExpr;
pub use solution::{Solution, SolutionBuilder, SolutionStore};
pub use solver::{SolverStatus, SolverError, SolverAdapter};
