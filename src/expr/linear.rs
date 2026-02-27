//! Linear expression type and operations.

use std::collections::HashMap;

use crate::id::{VarId, ConId, ObjId, ParamId};
use crate::value_expr::ValueExpr;
use crate::model::{Model, ModelError};

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

    // ========== Build into the Model ==========

    /// Compile this expression into coefficients for a constraint.
    ///
    /// Creates coefficient entries in the model and returns the constant term
    /// (which should be subtracted from the constraint bounds).
    pub fn compile_for_constraint(
        self,
        model: &mut Model,
        con: ConId,
    ) -> Result<f64, ModelError> {
        let simplified = self.simplify();

        for term in simplified.terms {
            let value_expr = term.coeff.into_value_expr();
            model.add_constraint_coefficient(con, term.var, value_expr)?;
        }

        Ok(simplified.constant)
    }

    /// Compile this expression into coefficients for an objective.
    ///
    /// Creates coefficient entries in the model and returns the constant offset.
    pub fn compile_for_objective(
        self,
        model: &mut Model,
        obj: ObjId,
    ) -> Result<f64, ModelError> {
        let simplified = self.simplify();

        for term in simplified.terms {
            let value_expr = term.coeff.into_value_expr();
            model.add_objective_coefficient(obj, term.var, value_expr)?;
        }

        Ok(simplified.constant)
    }

    /// Evaluate this expression given variable values.
    ///
    /// Used for solution evaluation and feasibility checking.
    pub fn evaluate<F, G>(&self, get_var: F, get_param: G) -> f64
    where 
        F: Fn(VarId) -> f64,
        G: Fn(ParamId) -> f64,
    {
        let mut result = self.constant;

        for term in &self.terms {
            let coeff_value = match &term.coeff {
                TermCoeff::Constant(v) => *v,
                TermCoeff::Expr(e) => e.eval(&get_param),
            };
            result += coeff_value * get_var(term.var);
        }

        result
    }
}

// ========== Operator Overloads ==========

// Allow converting a single variable into a one-term expression.
impl From<VarId> for LinExpr {
    fn from(var: VarId) -> LinExpr {
        LinExpr::new().term(1.0, var)
    }
}

// Add two full expressions.
impl std::ops::Add for LinExpr {
    type Output = Self;

    fn add(mut self, other: Self) -> Self {
        self.terms.extend(other.terms);
        self.constant += other.constant;
        self
    }
}

impl std::ops::Add<f64> for LinExpr {
    type Output = Self;

    fn add(self, value: f64) -> Self {
        self.constant(value)
    }
}

// Add a bare variable to an expression (coefficient 1).
impl std::ops::Add<VarId> for LinExpr {
    type Output = Self;

    fn add(self, var: VarId) -> Self {
        self.term(1.0, var)
    }
}

// Add an expression to a bare variable.
impl std::ops::Add<LinExpr> for VarId {
    type Output = LinExpr;

    fn add(self, other: LinExpr) -> LinExpr {
        LinExpr::from(self) + other
    }
}

// Add two variables directly, yielding an expression with both terms.
impl std::ops::Add<VarId> for VarId {
    type Output = LinExpr;

    fn add(self, rhs: VarId) -> LinExpr {
        LinExpr::from(self) + rhs
    }
}

// Combine a constant and a variable, producing an expression.
impl std::ops::Add<VarId> for f64 {
    type Output = LinExpr;

    fn add(self, rhs: VarId) -> LinExpr {
        LinExpr::from_constant(self).term(1.0, rhs)
    }
}

impl std::ops::Add<f64> for VarId {
    type Output = LinExpr;

    fn add(self, rhs: f64) -> LinExpr {
        LinExpr::from(self).constant(rhs)
    }
}

impl std::ops::Sub for LinExpr {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        self + (other * -1.0)
    }
}

// Subtraction variants involving bare variables/constants.
impl std::ops::Sub<VarId> for LinExpr {
    type Output = Self;

    fn sub(self, var: VarId) -> Self {
        self + (LinExpr::from(var) * -1.0)
    }
}

impl std::ops::Sub<LinExpr> for VarId {
    type Output = LinExpr;

    fn sub(self, other: LinExpr) -> LinExpr {
        LinExpr::from(self) - other
    }
}

impl std::ops::Sub<f64> for VarId {
    type Output = LinExpr;

    fn sub(self, rhs: f64) -> LinExpr {
        LinExpr::from(self).constant(-rhs)
    }
}

impl std::ops::Sub<f64> for LinExpr {
    type Output = Self;

    fn sub(self, value: f64) -> Self {
        self.constant(-value)
    }
}

impl std::ops::Mul<f64> for LinExpr {
    type Output = Self;

    fn mul(mut self, scalar: f64) -> Self {
        for term in &mut self.terms {
            term.coeff = match std::mem::replace(&mut term.coeff, TermCoeff::Constant(0.0)) {
                TermCoeff::Constant(v) => TermCoeff::Constant(v * scalar),
                TermCoeff::Expr(e) => TermCoeff::Expr(e * scalar),
            };
        }
        self.constant *= scalar;
        self
    }
}

// Multiply a variable by a scalar to get an expression.
impl std::ops::Mul<f64> for VarId {
    type Output = LinExpr;

    fn mul(self, rhs: f64) -> LinExpr {
        LinExpr::from(self).mul(rhs)
    }
}

impl std::ops::Mul<VarId> for f64 {
    type Output = LinExpr;

    fn mul(self, rhs: VarId) -> LinExpr {
        LinExpr::from(rhs).mul(self)
    }
}

// Parameter-coefficient multiplication: p * x or x * p
impl std::ops::Mul<ParamId> for VarId {
    type Output = LinExpr;

    fn mul(self, p: ParamId) -> LinExpr {
        LinExpr::new().term(p, self)
    }
}

impl std::ops::Mul<VarId> for ParamId {
    type Output = LinExpr;

    fn mul(self, rhs: VarId) -> LinExpr {
        LinExpr::new().term(self, rhs)
    }
}

// ========= Division overloads ========

impl std::ops::Div<f64> for LinExpr {
    type Output = Self;

    fn div(mut self, scalar: f64) -> Self {
        for term in &mut self.terms {
            term.coeff = match std::mem::replace(&mut term.coeff, TermCoeff::Constant(0.0)) {
                TermCoeff::Constant(v) => TermCoeff::Constant(v / scalar),
                TermCoeff::Expr(e) => TermCoeff::Expr(e / scalar),
            };
        }
        self.constant /= scalar;
        self
    }
}

impl std::ops::Div<f64> for VarId {
    type Output = LinExpr;

    fn div(self, rhs: f64) -> LinExpr {
        LinExpr::from(self).div(rhs)
    }
}

impl std::ops::Div<ParamId> for LinExpr {
    type Output = LinExpr;

    fn div(self, p: ParamId) -> LinExpr {
        // divide each coefficient and constant by parameter
        let mut result = LinExpr::new().constant(self.constant);
        for term in self.terms {
            let coeff_expr = term.coeff.into_value_expr() / ValueExpr::param(p);
            result = result.add_term(Term::new(coeff_expr, term.var));
        }
        result
    }
}

impl std::ops::Div<ParamId> for VarId {
    type Output = LinExpr;

    fn div(self, p: ParamId) -> LinExpr {
        LinExpr::new().term(ValueExpr::constant(1.0) / ValueExpr::param(p), self)
    }
}

impl std::ops::Neg for LinExpr {
    type Output = Self;

    fn neg(self) -> Self {
        self * -1.0
    }
}

// ========== Model Integration ==========

impl Model {
    /// Add a constraint from a linear expression.
    ///
    /// The expression's constant term is automatically incorporated into the bounds.
    pub fn add_constraint_expr(
        &mut self,
        expr: LinExpr,
        bounds: crate::model::ConstraintBounds,
    ) -> Result<ConId, ModelError> {
        let con = self.add_constraint(bounds);
        let constant = expr.compile_for_constraint(self, con)?;

        // Adjust bounds for constant term: expr <= b becomes (expr - c) <= (b - c)
        if constant.abs() >= f64::EPSILON {
            let adjusted_bounds = crate::model::ConstraintBounds {
                lower: bounds.lower - constant,
                upper: bounds.upper - constant,
            };
            self.set_constraint_bounds(con, adjusted_bounds)?;
        }

        Ok(con)
    }

    /// Add an objective from a linear expression.
    ///
    /// Returns the objective ID and the constant offset (which should be added
    /// to the objective value when reporting).
    pub fn add_objective_expr(
        &mut self,
        expr: LinExpr,
        sense: crate::model::Sense,
    ) -> Result<(ObjId, f64), ModelError> {
        let obj = self.add_objective(sense);
        let constant = expr.compile_for_objective(self, obj)?;
        Ok((obj, constant))
    }

    /// Reconstruct a linear expression from a constraint's coefficients.
    ///
    /// Uses cached coefficient values (not the ValueExpr).
    pub fn constraint_expression(&self, con: ConId) -> Result<LinExpr, ModelError> {
        if !self.constraints.contains(con) {
            return Err(ModelError::ConstraintNotFound(con));
        }

        let mut expr = LinExpr::new();
        for coeff_id in self.coefficients.for_constraint(con) {
            if let Some(data) = self.coefficients.get(coeff_id) {
                expr = expr.term(data.cached_value, data.var);
            }
        }

        Ok(expr)
    }

    /// Reconstruct a linear expression from an objective's coefficients.
    ///
    /// Uses cached coefficient values (not the ValueExpr).
    pub fn objective_expression(&self, obj: ObjId) -> Result<LinExpr, ModelError> {
        if !self.objectives.contains(obj) {
            return Err(ModelError::ObjectiveNotFound(obj));
        }

        let mut expr = LinExpr::new();
        for coeff_id in self.coefficients.for_objective(obj) {
            if let Some(data) = self.coefficients.get(coeff_id) {
                expr = expr.term(data.cached_value, data.var);
            }
        }

        Ok(expr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::Generation;
    use crate::model::{Bounds, ConstraintBounds, VarType};

    fn make_var(index: u32) -> VarId {
        VarId::new(index, Generation::new())
    }

    fn make_param(index: u32) -> ParamId {
        ParamId::new(index, Generation::new())
    }

    #[test]
    fn basic_expression() {
        let x = make_var(0);
        let y = make_var(1);

        // let expr = LinExpr::new()
        //     .term(2.0, x)
        //     .term(3.0, y)
        //     .constant(5.0);

        let expr = LinExpr::from(LinExpr::from_constant(5.0) + x + y);

        assert_eq!(expr.num_terms(), 2);
        assert_eq!(expr.get_constant(), 5.0);
    }

    #[test]
    fn simplify_combines_terms() {
        let x = make_var(0);

        let expr = LinExpr::new()
            .term(2.0, x)
            .term(3.0, x)
            .constant(1.0);

        let simplified = expr.simplify();
        assert_eq!(simplified.num_terms(), 1);
        assert_eq!(simplified.terms()[0].coeff.as_constant(), Some(5.0));
    }

    #[test]
    fn evaluate_expression() {
        let x = make_var(0);
        let y = make_var(1);

        let expr = LinExpr::new()
            .term(2.0, x)
            .term(3.0, y)
            .constant(1.0);

        // 2*10 + 3*5 + 1 = 36
        let result = expr.evaluate(
            |id| if id == x { 10.0 } else { 5.0 },
            |_| 0.0,
        );

        assert_eq!(result, 36.0);
    }

    #[test]
    fn operator_add() {
        let x = make_var(0);
        let y = make_var(1);

        let e1 = LinExpr::new().term(2.0, x);
        let e2 = LinExpr::new().term(3.0, y).constant(1.0);

        let combined = e1 + e2;
        assert_eq!(combined.num_terms(), 2);
        assert_eq!(combined.get_constant(), 1.0);
    }

    #[test]
    fn operator_mul() {
        let x = make_var(0);

        let expr = LinExpr::new().term(2.0, x).constant(3.0);
        let scaled = expr * 2.0;

        assert_eq!(scaled.terms()[0].coeff.as_constant(), Some(4.0));
        assert_eq!(scaled.get_constant(), 6.0);
    }

    #[test]
    fn convenience_var_scalar() {
        let x = make_var(0);
        let expr1: LinExpr = 2.0 * x; // f64 * VarId
        assert_eq!(expr1.num_terms(), 1);
        assert_eq!(expr1.terms()[0].coeff.as_constant(), Some(2.0));

        let expr2: LinExpr = x * 3.0; // VarId * f64
        assert_eq!(expr2.terms()[0].coeff.as_constant(), Some(3.0));

        let p = make_param(5);
        let expr3: LinExpr = p * x; // ParamId * VarId
        assert!(matches!(expr3.terms()[0].coeff, TermCoeff::Expr(_)));

        let expr4: LinExpr = x * p; // VarId * ParamId
        assert!(matches!(expr4.terms()[0].coeff, TermCoeff::Expr(_)));
    }

    #[test]
    fn convenience_addition() {
        let x = make_var(0);
        let y = make_var(1);

        let expr: LinExpr = x + y; // VarId + VarId
        assert_eq!(expr.num_terms(), 2);

        let expr2: LinExpr = 1.0 + x; // f64 + VarId
        assert_eq!(expr2.get_constant(), 1.0);

        let expr3: LinExpr = x + 2.0; // VarId + f64
        assert_eq!(expr3.get_constant(), 2.0);

        let expr4: LinExpr = x + LinExpr::new().term(4.0, y); // VarId + LinExpr
        assert_eq!(expr4.num_terms(), 2);

        let expr5: LinExpr = LinExpr::from_constant(3.0) + x; // LinExpr + VarId
        assert_eq!(expr5.get_constant(), 3.0);
    }

    #[test]
    fn convenience_division() {
        let x = make_var(0);
        let p = make_param(7);

        let expr1: LinExpr = (LinExpr::new().term(4.0, x).constant(2.0)) / 2.0;
        assert_eq!(expr1.terms()[0].coeff.as_constant(), Some(2.0));
        assert_eq!(expr1.get_constant(), 1.0);

        let expr2: LinExpr = x / 4.0;
        assert_eq!(expr2.terms()[0].coeff.as_constant(), Some(0.25));

        let expr3: LinExpr = x / p;
        // coefficient should be an expression representing 1/p
        assert!(matches!(expr3.terms()[0].coeff, TermCoeff::Expr(_)));

        let expr4: LinExpr = (LinExpr::new().term(p, x)) / p;
        // dividing a p*x expression by p should numerically equal 1 when the parameter
        // has a concrete value.  Simplification of p/p isn't performed by the helper.
        match &expr4.terms()[0].coeff {
            TermCoeff::Constant(v) => assert_eq!(*v, 1.0),
            TermCoeff::Expr(e) => {
                // evaluate with arbitrary parameter value
                let val = e.eval(&|id| if id == p { 5.0 } else { 0.0 });
                assert_eq!(val, 1.0);
            }
        }
    }

    #[test]
    fn compile_for_constraint() {
        let mut model = Model::new();
        let x = model.add_variable(Bounds::NON_NEGATIVE, VarType::Continuous);
        let y = model.add_variable(Bounds::NON_NEGATIVE, VarType::Continuous);

        let expr = LinExpr::new()
            .term(2.0, x)
            .term(3.0, y);

        let con = model.add_constraint_expr(expr, ConstraintBounds::le(10.0)).unwrap();

        // Should have 2 coefficients
        let coeff_count = model.coefficients.for_constraint(con).count();
        assert_eq!(coeff_count, 2);
    }

    #[test]
    fn reconstruct_expression() {
        let mut model = Model::new();
        let x = model.add_variable(Bounds::NON_NEGATIVE, VarType::Continuous);
        let y = model.add_variable(Bounds::NON_NEGATIVE, VarType::Continuous);

        let expr = LinExpr::new()
            .term(2.0, x)
            .term(3.0, y);

        let con = model.add_constraint_expr(expr, ConstraintBounds::le(10.0)).unwrap();

        let reconstructed = model.constraint_expression(con).unwrap();
        assert_eq!(reconstructed.num_terms(), 2);
    }

    #[test]
    fn param_term_expression() {
        let mut model = Model::new();
        let p = model.add_parameter(5.0);
        let x = model.add_variable(Bounds::NON_NEGATIVE, VarType::Continuous);

        // Expression: p * x
        let expr = LinExpr::new().term(p, x);

        let con = model.add_constraint_expr(expr, ConstraintBounds::le(100.0)).unwrap();

        // Coefficient should have value 5.0 (current param value)
        let coeff_id = model.coefficients.for_constraint(con).next().unwrap();
        assert_eq!(model.coefficient(coeff_id).unwrap().cached_value, 5.0);

        // Change parameter and commit
        model.set_parameter(p, 10.0);
        model.commit();

        // Coefficient should now be 10.0
        assert_eq!(model.coefficient(coeff_id).unwrap().cached_value, 10.0);
    }
}
