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

impl Term {
    /// Create a new term with the given coefficient and variable.
    pub fn new(coeff: impl Into<TermCoeff>, var: VarId) -> Self {
        Self {
            coeff: coeff.into(),
            var,
        }
    }
}

/// A linear expression: sum of terms + constant.
/// 
/// Represents: Σ(coeff_i * var_i) + constant
/// 
/// # Design
/// 
/// LinExpr is a temporary builder. It collects terms and can be compiled
/// into model coefficients. After compilation, the expression is typically
/// discarded (not stored in the model), but the user can store them outside 
/// if they want to.
/// 
/// Terms with the same variable are automatically combined when compiled.
#[derive(Clone, Debug, Default)]
pub struct LinExpr {
    /// Terms in the expression.
    pub terms: Vec<Term>,
    /// Constant offset.
    pub constant: f64,
}

impl LinExpr {
    /// Create an empty expression.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create an expression with just a constant.
    pub fn from_constant(value: f64) -> Self {
        Self {
            terms: Vec::new(),
            constant: value,
        }
    }

    /// Add a term with a constant coefficient.
    pub fn term(mut self, coeff: impl Into<TermCoeff>, var: VarId) -> Self {
        self.terms.push(Term::new(coeff, var));
        self
    }

    /// Add a constant offset.
    pub fn constant(mut self, value: f64) -> Self {
        self.constant += value;
        self
    }

    /// Add a term directly.
    pub fn add_term(mut self, term: Term) -> Self {
        self.terms.push(term);
        self
    }

    /// Get the constant offset.
    pub fn get_constant(&self) -> f64 {
        self.constant
    }

    /// Get the terms.
    pub fn terms(&self) -> &[Term] {
        &self.terms
    }

    /// Check if this expression is empty (no terms and zero constant).
    pub fn is_empty(&self) -> bool {
        self.terms.is_empty() && self.constant == 0.0
    }

    /// Check if this is just a constant (no variable terms).
    pub fn is_constant(&self) -> bool {
        self.terms.is_empty()
    }

    /// Get the number of terms.
    pub fn num_terms(&self) -> usize {
        self.terms.len()
    }

    /// Combine terms with the same variable.
    ///
    /// This consolidates the expression so each variable appears at most once.
    /// Only simplifies the constant coefficients; parameter-based terms are kept as-is.
    pub fn simplify(self) -> Self {
        let mut constant_terms: HashMap<VarId, f64> = HashMap::new();
        let mut expr_terms: Vec<Term> = Vec::new();

        for term in self.terms {
            match term.coeff {
                TermCoeff::Constant(v) => {
                    *constant_terms.entry(term.var).or_insert(0.0) += v;
                }
                TermCoeff::Expr(_) => {
                    // Can't combine expression-based terms
                    expr_terms.push(term);
                }
            }
        }

        let mut terms: Vec<Term> = constant_terms
            .into_iter()
            .filter(|(_, v)| v.abs() >= f64::EPSILON) // Filter out zeros
            .map(|(var, coeff)| Term::new(coeff, var))
            .collect();

        terms.extend(expr_terms);

        Self {
            terms,
            constant: self.constant,
        }
    }
}