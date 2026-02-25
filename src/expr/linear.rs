//! Linear expression type and operations.

use std::collections::HashMap;

use crate::id::{VarId, ConId, ObjId, ParamId};
use crate::value_expr::ValueExpr;

/// Coefficient type in a linear expression term.
/// 
/// Can be a constant, a parameter, or a more complex expression.
#[derive(Clone, Debug)]
pub enum TermCoeff {
    /// A constant coefficient.
    Constant(f64),
    /// A coefficient that depends on parameters.
    Expr(ValueExpr),
}


/// A term in a linear expression: coefficient * variable.
#[derive(Clone, Debug)]
pub struct Term {
    /// The coefficient (constant or parameter-based).
    pub coeff: TermCoeff,
    /// The variable this term multiplies.
    pub var: VarId,
}

/// A linear expression: sum of terms + constant.
pub struct LinearExpr {
    /// Terms in the expression.
    pub terms: Vec<Term>,
    /// Constant offset.
    pub constant: f64,
}