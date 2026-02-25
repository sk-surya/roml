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

impl TermCoeff {
    /// Convert to a ValueExpr for storage.
    pub fn into_value_expr(self) -> ValueExpr {
        match self {
            Self::Constant(v) => ValueExpr::constant(v),
            Self::Expr(e) => e,
        }
    }

    /// Get the value if this is a constant.
    pub fn as_constant(&self) -> Option<f64> {
        match self {
            Self::Constant(v) => Some(*v),
            Self::Expr(e) => e.as_constant(),
        }
    }
    
}

impl From<f64> for TermCoeff {
    fn from(value: f64) -> Self {
        Self::Constant(value)
    }
}

impl From<ValueExpr> for TermCoeff {
    fn from(expr: ValueExpr) -> Self {
        Self::Expr(expr)
    }
}

impl From<ParamId> for TermCoeff {
    fn from(param: ParamId) -> Self {
        Self::Expr(ValueExpr::param(param))
    }
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
/// 
/// LinExpr is a temporary builder. It collects terms and can be compiled
pub struct LinExpr {
    /// Terms in the expression.
    pub terms: Vec<Term>,
    /// Constant offset.
    pub constant: f64,
}