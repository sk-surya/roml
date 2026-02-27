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

// Re-export commonly used types for public API
pub use id::{VarId, ConId, ParamId, ObjId, CoeffId};
pub use model::{Model, Bounds, VarType, ConstraintBounds, Sense, ModelError};
pub use expr::LinExpr;
pub use value_expr::ValueExpr;

// what else do we need?
// Solution
// Solver interface
