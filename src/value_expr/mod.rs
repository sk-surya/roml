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

use std::collections::HashSet;

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

    // ========== Evaluation ==========

    /// Evaluate this expression against a parameter lookup function.
    /// 
    /// The `get_param` function should return the current value of a parameter by its ID.
    /// If a parameter is missing, expected behavior is panic!
    pub fn eval<F>(&self, get_param: F) -> f64
    where 
        F: Fn(ParamId) -> f64,
    {
        self.eval_recursive(&get_param)
    }

    fn eval_recursive<F>(&self, get_param: &F) -> f64
    where
        F: Fn(ParamId) -> f64,
    {
        match self {
            Self::Constant(v) => *v,
            Self::Param(id) => get_param(*id),
            Self::Add(l, r) => l.eval_recursive(get_param) + r.eval_recursive(get_param),
            Self::Sub(l, r) => l.eval_recursive(get_param) - r.eval_recursive(get_param),
            Self::Mul(l, r) => l.eval_recursive(get_param) * r.eval_recursive(get_param),
            Self::Div(l, r) => l.eval_recursive(get_param) / r.eval_recursive(get_param),
            Self::Neg(inner) => -inner.eval_recursive(get_param),
        }
    }

    // ========== Dependency Extraction ==========

    /// Extract all parameter IDs this expression depends on.
    /// 
    /// Used to build the reverse dependency index for parameter propagation.
    pub fn dependencies(&self) -> HashSet<ParamId> {
        let mut deps = HashSet::new();
        self.collect_dependencies(&mut deps);
        deps
    }

    fn collect_dependencies(&self, deps: &mut HashSet<ParamId>) {
        match self {
            Self::Constant(_) => {}
            Self::Param(id) => {
                deps.insert(*id);
            }
            Self::Add(l, r) | Self::Sub(l, r) | Self::Mul(l, r) | Self::Div(l, r) => {
                l.collect_dependencies(deps); 
                r.collect_dependencies(deps);
            }
            Self::Neg(inner) => {
                inner.collect_dependencies(deps);
            }
        }
    }

    // ========== Inspection ==========

    /// Check if this expression is a simple constant (no parameter dependencies).
    pub fn is_constant(&self) -> bool {
        matches!(self, Self::Constant(_))
    }

    /// Check if this expression depends on any parameters.
    pub fn has_dependencies(&self) -> bool {
        match self {
            Self::Constant(_) => false,
            Self::Param(_) => true,
            Self::Add(l,r) | Self::Sub(l,r) | Self::Mul(l,r) | Self::Div(l,r) => {
                l.has_dependencies() || r.has_dependencies()
            },
            Self::Neg(inner) => inner.has_dependencies(),
        }
    }

    pub fn as_constant(&self) -> Option<f64> {
        match self {
            Self::Constant(v) => Some(*v),
            _ => None,
        }
    }
}

// ========== Operator Overloads ==========

impl std::ops::Add for ValueExpr {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self::add(self, rhs)
    }
}

impl std::ops::Sub for ValueExpr {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Self::sub(self, rhs)
    }
}

impl std::ops::Mul for ValueExpr {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self {
        Self::mul(self, rhs)
    }
}

impl std::ops::Div for ValueExpr {
    type Output = Self;

    fn div(self, rhs: Self) -> Self {
        Self::div(self, rhs)
    }
}

impl std::ops::Neg for ValueExpr {
    type Output = Self;

    fn neg(self) -> Self {
        Self::neg(self)
    }
}

// Convenience: allow f64 * ValueExpr and ValueExpr * f64
impl std::ops::Mul<f64> for ValueExpr {
    type Output = Self;

    fn mul(self, rhs: f64) -> Self {
        Self::mul(self, Self::constant(rhs))
    }
}

impl std::ops::Mul<ValueExpr> for f64 {
    type Output = ValueExpr;

    fn mul(self, rhs: ValueExpr) -> ValueExpr {
        ValueExpr::mul(ValueExpr::constant(self), rhs)
    }
}

impl std::ops::Add<f64> for ValueExpr {
    type Output = Self;

    fn add(self, rhs: f64) -> Self {
        Self::add(self, Self::constant(rhs))
    }
}

impl std::ops::Sub<f64> for ValueExpr {
    type Output = Self;

    fn sub(self, rhs: f64) -> Self {
        Self::sub(self, Self::constant(rhs))
    }
}

// Convenience: construct ValueExpr from f64 and ParamId
impl From<f64> for ValueExpr {
    fn from(value: f64) -> Self {
        Self::constant(value)
    }
}

impl From<ParamId> for ValueExpr {
    fn from(id: ParamId) -> Self {
        Self::param(id)
    }
}