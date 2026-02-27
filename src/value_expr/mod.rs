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

    /// Negation: -inner
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
    #[allow(clippy::should_implement_trait)]
    pub fn add(left: Self, right: Self) -> Self {
        Self::Add(Box::new(left), Box::new(right))
    }

    /// Create a subtraction expression.
    #[allow(clippy::should_implement_trait)]
    pub fn sub(left: Self, right: Self) -> Self {
        Self::Sub(Box::new(left), Box::new(right))
    }

    /// Create a multiplication expression.
    #[allow(clippy::should_implement_trait)]
    pub fn mul(left: Self, right: Self) -> Self {
        Self::Mul(Box::new(left), Box::new(right))
    }

    /// Create a division expression.
    #[allow(clippy::should_implement_trait)]
    pub fn div(left: Self, right: Self) -> Self {
        Self::Div(Box::new(left), Box::new(right))
    }

    /// Create a negation expression.
    #[allow(clippy::should_implement_trait)]
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

    /// Try to extract a constant value if this is a simple constant.
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

// Convenience: allow arith operations on ValueExpr and f64
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

impl std::ops::Mul<f64> for ValueExpr {
    type Output = Self;

    fn mul(self, rhs: f64) -> Self {
        Self::mul(self, Self::constant(rhs))
    }
}

impl std::ops::Div<f64> for ValueExpr {
    type Output = Self;

    fn div(self, rhs: f64) -> Self {
        Self::div(self, Self::constant(rhs))
    }
}

// Convenience: allow arith operations on f64 and ValueExpr
impl std::ops::Add<ValueExpr> for f64 {
    type Output = ValueExpr;

    fn add(self, rhs: ValueExpr) -> ValueExpr {
        ValueExpr::add(ValueExpr::constant(self), rhs)
    }
}

impl std::ops::Sub<ValueExpr> for f64 {
    type Output = ValueExpr;

    fn sub(self, rhs: ValueExpr) -> ValueExpr {
        ValueExpr::sub(ValueExpr::constant(self), rhs)
    }
}

impl std::ops::Mul<ValueExpr> for f64 {
    type Output = ValueExpr;

    fn mul(self, rhs: ValueExpr) -> ValueExpr {
        ValueExpr::mul(ValueExpr::constant(self), rhs)
    }
}

impl std::ops::Div<ValueExpr> for f64 {
    type Output = ValueExpr;

    fn div(self, rhs: ValueExpr) -> ValueExpr {
        ValueExpr::div(ValueExpr::constant(self), rhs)
    }
}

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

// Convenience: allow f64 and ParamId arithmetic operations directly which gives ValueExpr
impl std::ops::Add<ParamId> for f64 {
    type Output = ValueExpr;
    fn add(self, rhs: ParamId) -> ValueExpr {
        ValueExpr::add(ValueExpr::constant(self), ValueExpr::param(rhs))
    }
}

impl std::ops::Sub<ParamId> for f64 {
    type Output = ValueExpr;
    fn sub(self, rhs: ParamId) -> ValueExpr {
        ValueExpr::sub(ValueExpr::constant(self), ValueExpr::param(rhs))
    }
}

impl std::ops::Mul<ParamId> for f64 {
    type Output = ValueExpr;
    fn mul(self, rhs: ParamId) -> ValueExpr {
        ValueExpr::mul(ValueExpr::constant(self), ValueExpr::param(rhs))
    }
}

impl std::ops::Div<ParamId> for f64 {
    type Output = ValueExpr;
    fn div(self, rhs: ParamId) -> ValueExpr {
        ValueExpr::div(ValueExpr::constant(self), ValueExpr::param(rhs))
    }
}

// Convenience: allow ParamId and f64 arithmetic operations directly which gives ValueExpr
impl std::ops::Add<f64> for ParamId {
    type Output = ValueExpr;
    fn add(self, rhs: f64) -> ValueExpr {
        ValueExpr::add(ValueExpr::param(self), ValueExpr::constant(rhs))
    }
}

impl std::ops::Sub<f64> for ParamId {
    type Output = ValueExpr;
    fn sub(self, rhs: f64) -> ValueExpr {
        ValueExpr::sub(ValueExpr::param(self), ValueExpr::constant(rhs))
    }
}

impl std::ops::Mul<f64> for ParamId {
    type Output = ValueExpr;
    fn mul(self, rhs: f64) -> ValueExpr {
        ValueExpr::mul(ValueExpr::param(self), ValueExpr::constant(rhs))
    }
}

impl std::ops::Div<f64> for ParamId {
    type Output = ValueExpr;
    fn div(self, rhs: f64) -> ValueExpr {
        ValueExpr::div(ValueExpr::param(self), ValueExpr::constant(rhs))
    }
}

// Convenience: allow ParamId and ParamId arithmetic operations directly which gives ValueExpr
impl std::ops::Add<ParamId> for ParamId {
    type Output = ValueExpr;
    fn add(self, rhs: ParamId) -> ValueExpr {
        ValueExpr::add(ValueExpr::param(self), ValueExpr::param(rhs))
    }
}

impl std::ops::Sub<ParamId> for ParamId {
    type Output = ValueExpr;
    fn sub(self, rhs: ParamId) -> ValueExpr {
        ValueExpr::sub(ValueExpr::param(self), ValueExpr::param(rhs))
    }
}

impl std::ops::Mul<ParamId> for ParamId {
    type Output = ValueExpr;
    fn mul(self, rhs: ParamId) -> ValueExpr {
        ValueExpr::mul(ValueExpr::param(self), ValueExpr::param(rhs))
    }
}

impl std::ops::Div<ParamId> for ParamId {
    type Output = ValueExpr;
    fn div(self, rhs: ParamId) -> ValueExpr {
        ValueExpr::div(ValueExpr::param(self), ValueExpr::param(rhs))
    }
}

// Convenience: allow ValueExpr and ParamId arithmetic operations directly which gives ValueExpr
impl std::ops::Add<ParamId> for ValueExpr {
    type Output = ValueExpr;
    fn add(self, rhs: ParamId) -> ValueExpr {
        ValueExpr::add(self, ValueExpr::param(rhs))
    }
}

impl std::ops::Sub<ParamId> for ValueExpr {
    type Output = ValueExpr;
    fn sub(self, rhs: ParamId) -> ValueExpr {
        ValueExpr::sub(self, ValueExpr::param(rhs))
    }
}

impl std::ops::Mul<ParamId> for ValueExpr {
    type Output = ValueExpr;
    fn mul(self, rhs: ParamId) -> ValueExpr {
        ValueExpr::mul(self, ValueExpr::param(rhs))
    }
}

impl std::ops::Div<ParamId> for ValueExpr {
    type Output = ValueExpr;
    fn div(self, rhs: ParamId) -> ValueExpr {
        ValueExpr::div(self, ValueExpr::param(rhs))
    }
}

// Convenience: allow ParamId and ValueExpr arithmetic operations directly which gives ValueExpr
impl std::ops::Add<ValueExpr> for ParamId {
    type Output = ValueExpr;
    fn add(self, rhs: ValueExpr) -> ValueExpr {
        ValueExpr::add(ValueExpr::param(self), rhs)
    }
}

impl std::ops::Sub<ValueExpr> for ParamId {
    type Output = ValueExpr;
    fn sub(self, rhs: ValueExpr) -> ValueExpr {
        ValueExpr::sub(ValueExpr::param(self), rhs)
    }
}

impl std::ops::Mul<ValueExpr> for ParamId {
    type Output = ValueExpr;
    fn mul(self, rhs: ValueExpr) -> ValueExpr {
        ValueExpr::mul(ValueExpr::param(self), rhs)
    }
}

impl std::ops::Div<ValueExpr> for ParamId {
    type Output = ValueExpr;
    fn div(self, rhs: ValueExpr) -> ValueExpr {
        ValueExpr::div(ValueExpr::param(self), rhs)
    }
}

// Convenience: Negation of parameter results in ValueExpr
impl std::ops::Neg for ParamId {
    type Output = ValueExpr;
    fn neg(self) -> ValueExpr {
        ValueExpr::neg(ValueExpr::param(self))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::Generation;

    fn make_param(index: u32) -> ParamId {
        ParamId::new(index, Generation::new())
    }

    #[test]
    fn constant_eval() {
        let expr = ValueExpr::constant(42.0);
        assert_eq!(expr.eval(|_| 0.0), 42.0);
    }

    #[test]
    fn param_eval() {
        let p = make_param(0);
        let expr = ValueExpr::param(p);
        assert_eq!(expr.eval(|_| 10.0), 10.0);
    }

    #[test]
    fn arithmetic_eval() {
        let p1 = make_param(0);
        let p2 = make_param(1);

        // 2 * p1 + p2
        let expr1 = 2.0 * p1 + p2;
        let expr2 = ValueExpr::constant(2.0) * ValueExpr::param(p1) + ValueExpr::param(p2);
        let expr3 = ValueExpr::add(
            ValueExpr::mul(ValueExpr::constant(2.0), ValueExpr::param(p1)),
            ValueExpr::param(p2),
        );

        let get_param = |id| {
            if id == p1 {
                3.0
            } else if id == p2 {
                5.0
            } else {
                0.0
            }
        };

        let result1 = expr1.eval(&get_param);
        let result2 = expr2.eval(&get_param);
        let result3 = expr3.eval(&get_param);

        assert_eq!(result1, 2.0 * 3.0 + 5.0); // 11.0
        assert_eq!(result1, result2);
        assert_eq!(result1, result3);
    }

    #[test]
    fn dependencies_extraction() {
        let p1 = make_param(0);
        let p2 = make_param(1);
        let p3 = make_param(2);

        // p1 * p2 + 5.0 (doesn't use p3)
        let expr = p1 * p2 + 5.0;
        // let expr = ValueExpr::add(
        //     ValueExpr::mul(ValueExpr::param(p1), ValueExpr::param(p2)),
        //     ValueExpr::constant(5.0),
        // );

        let deps = expr.dependencies();
        assert!(deps.contains(&p1));
        assert!(deps.contains(&p2));
        assert!(!deps.contains(&p3));
        assert_eq!(deps.len(), 2);
    }

    #[test]
    fn constant_has_no_dependencies() {
        let expr = ValueExpr::constant(42.0);
        assert!(expr.is_constant());
        assert!(!expr.has_dependencies());
        assert!(expr.dependencies().is_empty());
    }

    #[test]
    fn operator_overloads() {
        let p = make_param(0);
        let expr = ValueExpr::param(p) * 2.0 + 1.0;

        let result = expr.eval(|_| 5.0);
        assert_eq!(result, 5.0 * 2.0 + 1.0); // 11.0
    }

    #[test]
    fn negation() {
        let p = make_param(0);
        // let expr = -ValueExpr::param(p);
        let expr = -p;

        let result = expr.eval(|_| 5.0);
        assert_eq!(result, -5.0);
    }

    #[test]
    fn division() {
        let p1 = make_param(0);
        let p2 = make_param(1);

        let expr = ValueExpr::param(p1) / ValueExpr::param(p2);

        let result = expr.eval(|id| if id == p1 { 10.0 } else { 2.0 });
        assert_eq!(result, 5.0);
    }
}
