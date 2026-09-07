//! Rust expression wrappers with core-backed affine semantics (DESIGN §3).
//!
//! `Expr` is an affine combination of decision variables with
//! parameter-dependent coefficients (`ValueExpr`) plus a parameter-dependent
//! constant. All algebra executes in Rust with validation at construction:
//! variable-times-variable rejects as nonlinear, division admits only
//! nonzero numeric constants, and symbolic truthiness raises `TypeError`.

use pyo3::prelude::*;
use roml::{ValueExpr, VarId};

use super::errors::{InvalidModelError, UnsupportedExpressionError};
use super::handles::{Param, Var};
use super::model::{py_numeric, Model};

/// One affine term: decision variable times a parameter-dependent coefficient.
#[derive(Clone, Debug)]
pub(crate) struct ExprTerm {
    pub var: VarId,
    pub coeff: ValueExpr,
}

/// Owned affine expression with its model's identity for checks.
#[derive(Debug)]
pub(crate) struct Affine {
    pub owner: Py<Model>,
    pub terms: Vec<ExprTerm>,
    pub constant: ValueExpr,
}

impl Clone for Affine {
    fn clone(&self) -> Self {
        Python::attach(|py| Self {
            owner: self.owner.clone_ref(py),
            terms: self.terms.clone(),
            constant: self.constant.clone(),
        })
    }
}

impl Affine {
    fn add_terms(&mut self, other: &Affine, sign: f64) -> PyResult<()> {
        for term in &other.terms {
            self.push_term(term.var, term.coeff.clone() * sign)?;
        }
        Ok(())
    }

    fn push_term(&mut self, var: VarId, coeff: ValueExpr) -> PyResult<()> {
        if !is_finite_expr(&coeff) {
            return Err(InvalidModelError::new_err(
                "expression coefficient must be finite",
            ));
        }
        match self.terms.iter_mut().find(|t| t.var == var) {
            Some(existing) => {
                existing.coeff = existing.coeff.clone() + coeff;
            }
            None => self.terms.push(ExprTerm { var, coeff }),
        }
        Ok(())
    }

    fn scale(&mut self, factor: f64) -> PyResult<()> {
        if !factor.is_finite() {
            return Err(InvalidModelError::new_err(
                "expression scale factor must be finite",
            ));
        }
        for term in &mut self.terms {
            term.coeff = term.coeff.clone() * factor;
        }
        self.constant = self.constant.clone() * factor;
        Ok(())
    }
}

fn is_finite_expr(expr: &ValueExpr) -> bool {
    match expr.as_constant() {
        Some(v) => v.is_finite(),
        // Parameter-dependent coefficients validate against live values at
        // model insertion; construction only rejects constant non-finite.
        None => true,
    }
}

/// Extract a numeric operand; bools reject, non-numerics reject.
fn operand_numeric(value: &Bound<'_, PyAny>, op: &str) -> PyResult<f64> {
    py_numeric(value, &format!("{op} operand"))
}

#[pyclass(frozen, name = "Expr")]
pub struct Expr {
    pub(crate) inner: Affine,
}

#[pymethods]
impl Expr {
    fn __repr__(&self) -> String {
        format!(
            "Expr({} terms{})",
            self.inner.terms.len(),
            if self.inner.terms.is_empty() {
                String::new()
            } else {
                ", affine".to_string()
            }
        )
    }

    fn __hash__(&self) -> PyResult<isize> {
        Err(pyo3::exceptions::PyTypeError::new_err(
            "expressions are unhashable",
        ))
    }

    fn __bool__(&self) -> PyResult<bool> {
        Err(pyo3::exceptions::PyTypeError::new_err(
            "symbolic expressions have no truth value; use m.add(...) to constrain them",
        ))
    }

    fn __neg__(slf: Py<Self>) -> PyResult<Py<Self>> {
        Python::attach(|py| {
            let mut inner = slf.bind(py).borrow().inner.clone();
            inner.scale(-1.0)?;
            Ok(Bound::new(py, Self { inner })?.unbind())
        })
    }

    fn __add__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<Self>> {
        Python::attach(|py| {
            let base = slf.bind(py).borrow();
            let mut inner = base.inner.clone();
            drop(base);
            add_operand(&mut inner, &other, 1.0)?;
            Ok(Bound::new(py, Self { inner })?.unbind())
        })
    }

    fn __radd__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<Self>> {
        Self::__add__(slf, other)
    }

    fn __sub__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<Self>> {
        Python::attach(|py| {
            let base = slf.bind(py).borrow();
            let mut inner = base.inner.clone();
            drop(base);
            add_operand(&mut inner, &other, -1.0)?;
            Ok(Bound::new(py, Self { inner })?.unbind())
        })
    }

    fn __rsub__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<Self>> {
        Python::attach(|py| {
            let base = slf.bind(py).borrow();
            let mut inner = Affine {
                owner: base.inner.owner.clone_ref(py),
                terms: Vec::new(),
                constant: ValueExpr::constant(0.0),
            };
            drop(base);
            add_operand(&mut inner, &other, 1.0)?;
            let neg = slf.bind(py).borrow();
            let mut result = inner;
            drop(neg);
            let orig = slf.bind(py).borrow();
            result.add_terms(&orig.inner, -1.0)?;
            Ok(Bound::new(py, Self { inner: result })?.unbind())
        })
    }

    fn __mul__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<Self>> {
        Python::attach(|py| {
            let base = slf.bind(py).borrow();
            let mut inner = base.inner.clone();
            drop(base);
            mul_operand(&mut inner, &other)?;
            Ok(Bound::new(py, Self { inner })?.unbind())
        })
    }

    fn __rmul__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<Self>> {
        Self::__mul__(slf, other)
    }

    fn __truediv__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<Self>> {
        Python::attach(|py| {
            let divisor = operand_numeric(&other, "division")?;
            if divisor == 0.0 {
                return Err(InvalidModelError::new_err("division by zero"));
            }
            let base = slf.bind(py).borrow();
            let mut inner = base.inner.clone();
            drop(base);
            inner.scale(1.0 / divisor)?;
            Ok(Bound::new(py, Self { inner })?.unbind())
        })
    }

    fn __le__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Comparison> {
        compare_operand(&slf, &other, Sense::Le)
    }

    fn __ge__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Comparison> {
        compare_operand(&slf, &other, Sense::Ge)
    }

    fn __eq__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Comparison> {
        compare_operand(&slf, &other, Sense::Eq)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Sense {
    Le,
    Ge,
    Eq,
}

/// Add (sign +1) or subtract (sign -1) an operand into an affine expression.
fn add_operand(inner: &mut Affine, other: &Bound<'_, PyAny>, sign: f64) -> PyResult<()> {
    if let Ok(var) = other.cast::<Var>() {
        let var = var.borrow();
        same_owner(&var.owner, &inner.owner)?;
        inner.push_term(var.id, ValueExpr::constant(sign))?;
        return Ok(());
    }
    if let Ok(param) = other.cast::<Param>() {
        let param = param.borrow();
        same_owner(&param.owner, &inner.owner)?;
        inner.constant =
            inner.constant.clone() + ValueExpr::param(param.id) * ValueExpr::constant(sign);
        return Ok(());
    }
    if let Ok(expr) = other.cast::<Expr>() {
        let expr = expr.borrow();
        same_owner(&expr.inner.owner, &inner.owner)?;
        inner.add_terms(&expr.inner, sign)?;
        return Ok(());
    }
    let v = operand_numeric(other, "addition")?;
    inner.constant = inner.constant.clone() + ValueExpr::constant(sign * v);
    Ok(())
}

/// Multiply an affine expression by a numeric or parameter-only operand.
/// Variable-times-variable rejects as nonlinear: at least one side must be
/// free of decision variables.
fn mul_operand(inner: &mut Affine, other: &Bound<'_, PyAny>) -> PyResult<()> {
    if let Ok(param) = other.cast::<Param>() {
        let param = param.borrow();
        same_owner(&param.owner, &inner.owner)?;
        let factor = ValueExpr::param(param.id);
        for term in &mut inner.terms {
            term.coeff = term.coeff.clone() * factor.clone();
        }
        inner.constant = inner.constant.clone() * factor;
        return Ok(());
    }
    if let Ok(var) = other.cast::<Var>() {
        let var = var.borrow();
        same_owner(&var.owner, &inner.owner)?;
        if !inner.terms.is_empty() {
            return Err(nonlinear());
        }
        // Parameter-only self times a variable: the constant becomes the
        // coefficient of the new term.
        let coeff = std::mem::replace(&mut inner.constant, ValueExpr::constant(0.0));
        inner.terms.push(ExprTerm { var: var.id, coeff });
        return Ok(());
    }
    if let Ok(expr) = other.cast::<Expr>() {
        let expr = expr.borrow();
        same_owner(&expr.inner.owner, &inner.owner)?;
        if !inner.terms.is_empty() && !expr.inner.terms.is_empty() {
            return Err(nonlinear());
        }
        if inner.terms.is_empty() {
            // Parameter-only self times affine other.
            let mine = std::mem::replace(&mut inner.constant, ValueExpr::constant(0.0));
            for term in &expr.inner.terms {
                inner.push_term(term.var, term.coeff.clone() * mine.clone())?;
            }
            inner.constant = expr.inner.constant.clone() * mine;
            return Ok(());
        }
        // Affine self times parameter-only other.
        let factor = expr.inner.constant.clone();
        for term in &mut inner.terms {
            term.coeff = term.coeff.clone() * factor.clone();
        }
        inner.constant = inner.constant.clone() * factor;
        return Ok(());
    }
    let v = operand_numeric(other, "multiplication")?;
    inner.scale(v)?;
    Ok(())
}

fn chained() -> PyErr {
    pyo3::exceptions::PyTypeError::new_err(
        "chained comparisons such as 0 <= x <= 1 are not supported; write m.add(...) for each bound",
    )
}

fn nonlinear() -> PyErr {
    UnsupportedExpressionError::new_err(
        "variable-times-variable products are not supported: this interface is LP/MILP only",
    )
}

fn compare_operand(slf: &Py<Expr>, other: &Bound<'_, PyAny>, sense: Sense) -> PyResult<Comparison> {
    Python::attach(|py| {
        // Move the operand to the left: lhs = self - other, bound 0.
        let base = slf.bind(py).borrow();
        let mut lhs = base.inner.clone();
        drop(base);
        add_operand(&mut lhs, other, -1.0)?;
        let rhs = match sense {
            Sense::Le => BoundSide::Upper(ValueExpr::constant(0.0)),
            Sense::Ge => BoundSide::Lower(ValueExpr::constant(0.0)),
            Sense::Eq => BoundSide::Eq(ValueExpr::constant(0.0)),
        };
        Ok(Comparison {
            owner: lhs.owner.clone_ref(py),
            expr: lhs,
            rhs,
        })
    })
}

/// Identity check between two owner references.
fn same_owner(a: &Py<Model>, b: &Py<Model>) -> PyResult<()> {
    if a.as_ptr() == b.as_ptr() {
        Ok(())
    } else {
        Err(super::errors::ModelMismatchError::new_err(
            "variable, parameter, or expression from a different model",
        ))
    }
}

#[derive(Clone, Debug)]
pub(crate) enum BoundSide {
    Upper(ValueExpr),
    Lower(ValueExpr),
    Eq(ValueExpr),
}

/// A symbolic comparison (`<=`, `>=`, `==`) awaiting `Model.add`.
/// Truthiness raises `TypeError`; chained comparisons fail loudly because
/// each comparison step returns this descriptor rather than a bool.
#[pyclass(frozen)]
pub struct Comparison {
    pub(crate) owner: Py<Model>,
    pub(crate) expr: Affine,
    pub(crate) rhs: BoundSide,
}

impl Comparison {
    pub(crate) fn owner_check(&self, model: &Bound<'_, Model>) -> PyResult<()> {
        super::handles::check_owner(&self.owner, model)
    }

    /// Bound expressions as (lower, upper) symbolic pair.
    pub(crate) fn bound_exprs(&self) -> (Option<ValueExpr>, Option<ValueExpr>) {
        match &self.rhs {
            BoundSide::Upper(u) => (None, Some(u.clone())),
            BoundSide::Lower(l) => (Some(l.clone()), None),
            BoundSide::Eq(e) => (Some(e.clone()), Some(e.clone())),
        }
    }
}

#[pymethods]
impl Comparison {
    fn __repr__(&self) -> &'static str {
        "Comparison(<symbolic>)"
    }

    fn __hash__(&self) -> PyResult<isize> {
        Err(pyo3::exceptions::PyTypeError::new_err(
            "comparisons are unhashable",
        ))
    }

    fn __bool__(&self) -> PyResult<bool> {
        Err(pyo3::exceptions::PyTypeError::new_err(
            "symbolic comparisons have no truth value; pass them to m.add(...). Chained comparisons such as 0 <= x <= 1 are not supported.",
        ))
    }

    fn __le__(&self, _other: Bound<'_, PyAny>) -> PyResult<Comparison> {
        Err(chained())
    }

    fn __ge__(&self, _other: Bound<'_, PyAny>) -> PyResult<Comparison> {
        Err(chained())
    }

    fn __eq__(&self, _other: Bound<'_, PyAny>) -> PyResult<Comparison> {
        Err(chained())
    }
}

/// Convert a scalar operand into an owned affine expression homed to
/// `model`: `Var` lifts to a unit term, `Param` to a parameter-only
/// expression, numerics to a constant, and `Expr` clones through.
/// Foreign-model handles fail here, so `Model.minimize` never sees them.
pub(crate) fn to_affine(model: &Bound<'_, Model>, other: &Bound<'_, PyAny>) -> PyResult<Affine> {
    Python::attach(|py| {
        if let Ok(var) = other.cast::<Var>() {
            let var = var.borrow();
            same_owner(&var.owner, &model.clone().unbind())?;
            return Ok(var_affine(py, &var));
        }
        if let Ok(param) = other.cast::<Param>() {
            let param = param.borrow();
            same_owner(&param.owner, &model.clone().unbind())?;
            return Ok(param_affine(py, &param));
        }
        if let Ok(expr) = other.cast::<Expr>() {
            let expr = expr.borrow();
            same_owner(&expr.inner.owner, &model.clone().unbind())?;
            return Ok(expr.inner.clone());
        }
        let v = operand_numeric(other, "objective")?;
        Ok(Affine {
            owner: model.clone().unbind(),
            terms: Vec::new(),
            constant: ValueExpr::constant(v),
        })
    })
}

/// Var arithmetic: each operator lifts the variable to an `Expr` first.
fn var_affine(py: Python<'_>, var: &Var) -> Affine {
    Affine {
        owner: var.owner.clone_ref(py),
        terms: vec![ExprTerm {
            var: var.id,
            coeff: ValueExpr::constant(1.0),
        }],
        constant: ValueExpr::constant(0.0),
    }
}

#[pymethods]
impl Var {
    fn __repr__(&self) -> String {
        format!("Var({:?})", self.name)
    }

    fn __str__(&self) -> String {
        format!("Var({:?})", self.name)
    }

    fn __hash__(&self) -> PyResult<isize> {
        Err(pyo3::exceptions::PyTypeError::new_err(
            "variables are unhashable: symbolic == does not define key equality; use solution.value(x)",
        ))
    }

    fn __neg__(slf: Py<Self>) -> PyResult<Py<Expr>> {
        Python::attach(|py| {
            let mut inner = var_affine(py, &slf.bind(py).borrow());
            inner.scale(-1.0)?;
            Ok(Bound::new(py, Expr { inner })?.unbind())
        })
    }

    fn __add__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<Expr>> {
        Python::attach(|py| {
            let mut inner = var_affine(py, &slf.bind(py).borrow());
            add_operand(&mut inner, &other, 1.0)?;
            Ok(Bound::new(py, Expr { inner })?.unbind())
        })
    }

    fn __radd__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<Expr>> {
        Self::__add__(slf, other)
    }

    fn __sub__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<Expr>> {
        Python::attach(|py| {
            let mut inner = var_affine(py, &slf.bind(py).borrow());
            add_operand(&mut inner, &other, -1.0)?;
            Ok(Bound::new(py, Expr { inner })?.unbind())
        })
    }

    fn __rsub__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<Expr>> {
        Python::attach(|py| {
            let base = slf.bind(py).borrow();
            let mut inner = Affine {
                owner: base.owner.clone_ref(py),
                terms: Vec::new(),
                constant: ValueExpr::constant(0.0),
            };
            drop(base);
            add_operand(&mut inner, &other, 1.0)?;
            let orig = slf.bind(py).borrow();
            let single = var_affine(py, &orig);
            drop(orig);
            inner.add_terms(&single, -1.0)?;
            Ok(Bound::new(py, Expr { inner })?.unbind())
        })
    }

    fn __mul__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<Expr>> {
        Python::attach(|py| {
            let mut inner = var_affine(py, &slf.bind(py).borrow());
            mul_operand(&mut inner, &other)?;
            Ok(Bound::new(py, Expr { inner })?.unbind())
        })
    }

    fn __rmul__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<Expr>> {
        Self::__mul__(slf, other)
    }

    fn __truediv__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<Expr>> {
        Python::attach(|py| {
            let divisor = operand_numeric(&other, "division")?;
            if divisor == 0.0 {
                return Err(InvalidModelError::new_err("division by zero"));
            }
            let mut inner = var_affine(py, &slf.bind(py).borrow());
            inner.scale(1.0 / divisor)?;
            Ok(Bound::new(py, Expr { inner })?.unbind())
        })
    }

    fn __le__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Comparison> {
        Python::attach(|py| {
            let inner = var_affine(py, &slf.bind(py).borrow());
            let expr = Bound::new(py, Expr { inner })?;
            compare_operand(&expr.unbind(), &other, Sense::Le)
        })
    }

    fn __ge__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Comparison> {
        Python::attach(|py| {
            let inner = var_affine(py, &slf.bind(py).borrow());
            let expr = Bound::new(py, Expr { inner })?;
            compare_operand(&expr.unbind(), &other, Sense::Ge)
        })
    }

    fn __eq__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Comparison> {
        Python::attach(|py| {
            let inner = var_affine(py, &slf.bind(py).borrow());
            let expr = Bound::new(py, Expr { inner })?;
            compare_operand(&expr.unbind(), &other, Sense::Eq)
        })
    }

    fn __bool__(&self) -> PyResult<bool> {
        Err(pyo3::exceptions::PyTypeError::new_err(
            "variables have no truth value; use m.add(...) to constrain them",
        ))
    }
}

/// Param arithmetic produces parameter-only `Expr` values (no var terms).
fn param_affine(py: Python<'_>, param: &Param) -> Affine {
    Affine {
        owner: param.owner.clone_ref(py),
        terms: Vec::new(),
        constant: ValueExpr::param(param.id),
    }
}

#[pymethods]
impl Param {
    fn __repr__(&self) -> String {
        format!("Param({:?})", self.name)
    }

    fn __str__(&self) -> String {
        format!("Param({:?})", self.name)
    }

    fn __hash__(&self) -> PyResult<isize> {
        Err(pyo3::exceptions::PyTypeError::new_err(
            "parameters are unhashable; use model.update(name=value)",
        ))
    }

    fn __neg__(slf: Py<Self>) -> PyResult<Py<Expr>> {
        Python::attach(|py| {
            let mut inner = param_affine(py, &slf.bind(py).borrow());
            inner.scale(-1.0)?;
            Ok(Bound::new(py, Expr { inner })?.unbind())
        })
    }

    fn __add__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<Expr>> {
        Python::attach(|py| {
            let mut inner = param_affine(py, &slf.bind(py).borrow());
            add_operand(&mut inner, &other, 1.0)?;
            Ok(Bound::new(py, Expr { inner })?.unbind())
        })
    }

    fn __radd__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<Expr>> {
        Self::__add__(slf, other)
    }

    fn __sub__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<Expr>> {
        Python::attach(|py| {
            let mut inner = param_affine(py, &slf.bind(py).borrow());
            add_operand(&mut inner, &other, -1.0)?;
            Ok(Bound::new(py, Expr { inner })?.unbind())
        })
    }

    fn __rsub__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<Expr>> {
        Python::attach(|py| {
            let base = slf.bind(py).borrow();
            let mut inner = Affine {
                owner: base.owner.clone_ref(py),
                terms: Vec::new(),
                constant: ValueExpr::constant(0.0),
            };
            drop(base);
            add_operand(&mut inner, &other, 1.0)?;
            let orig = slf.bind(py).borrow();
            let single = param_affine(py, &orig);
            drop(orig);
            inner.add_terms(&single, -1.0)?;
            Ok(Bound::new(py, Expr { inner })?.unbind())
        })
    }

    fn __mul__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<Expr>> {
        Python::attach(|py| {
            let mut inner = param_affine(py, &slf.bind(py).borrow());
            mul_operand(&mut inner, &other)?;
            Ok(Bound::new(py, Expr { inner })?.unbind())
        })
    }

    fn __rmul__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<Expr>> {
        Self::__mul__(slf, other)
    }

    fn __truediv__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<Expr>> {
        Python::attach(|py| {
            let divisor = operand_numeric(&other, "division")?;
            if divisor == 0.0 {
                return Err(InvalidModelError::new_err("division by zero"));
            }
            let mut inner = param_affine(py, &slf.bind(py).borrow());
            inner.scale(1.0 / divisor)?;
            Ok(Bound::new(py, Expr { inner })?.unbind())
        })
    }

    fn __bool__(&self) -> PyResult<bool> {
        Err(pyo3::exceptions::PyTypeError::new_err(
            "parameters have no truth value",
        ))
    }
}
