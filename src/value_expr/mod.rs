//! Coefficient value expressions.
//! 
//! This module provides an AST for representing coefficient values that can depend
//! on parameters. When parameters change, all coefficients using these expressions
//! can be re-evaluated without rebuilding the model.
//! 
//! # Example
//! 
//! // A coefficient that is 2 * param_a * param_b
//! let expr = ValueExpr::mul(
//!    ValueExpr::constant(2.0),
//!    ValueExpr::mul(
//!       ValueExpr::param(param_a),
//!       ValueExpr::param(param_b),
//!    ),
//! );

use crate::id::ParamId;

/// A value expression for coefficient computation.
/// 
/// Expression forms an AST that can be evaluated against a parameter store.
/// They track which parameters they depend on for efficient propagation.
#[derive(Clone, Debug, PartialEq)]
pub enum ValueExpr {
    /// A constant value.
    Constant(f64),

    /// A parameter reference.
    Param(ParamId),

    /// Addition: left + right
    Add(Box<ValueExpr>, Box<ValueExpr>),

    /// Subtraction: left - right
    Sub(Box<ValueExpr>, Box<ValueExpr>),

    /// Multiplication: left * right
    Mul(Box<ValueExpr>, Box<ValueExpr>),

    /// Division: left / right
    Div(Box<ValueExpr>, Box<ValueExpr>),

    /// Negation: -expr
    Neg(Box<ValueExpr>),
}

impl ValueExpr {
    // ========== Constructors ==========

    /// Create a constant expression.
    #[inline]
    pub fn constant(value: f64) -> Self {
        Self::Constant(value)
    }

    /// Create a parameter reference expression.
    #[inline]
    pub fn param(id: ParamId) -> Self {
        Self::Param(id)
    }

    /// Create an addition expression.
    pub fn add(left: Self, right: Self) -> Self {
        Self::Add(Box::new(left), Box::new(right))
    }

    /// Create a subtraction expression.
    pub fn sub(left: Self, right: Self) -> Self {
        Self::Sub(Box::new(left), Box::new(right))
    }

    /// Create a multiplication expression.
    pub fn mul(left: Self, right: Self) -> Self {
        Self::Mul(Box::new(left), Box::new(right))
    }

    /// Create a division expression.
    pub fn div(left: Self, right: Self) -> Self {
        Self::Div(Box::new(left), Box::new(right))
    }

    /// Create a negation expression.
    pub fn neg(inner: Self) -> Self {
        Self::Neg(Box::new(inner))
    }
}
