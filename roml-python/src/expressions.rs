//! Rust expression wrappers with core-backed affine semantics (DESIGN §3).
//!
//! `Expr` is an affine combination of decision variables with
//! parameter-dependent coefficients (`ValueExpr`) plus a parameter-dependent
//! constant. All algebra executes in Rust with validation at construction:
//! variable-times-variable rejects as nonlinear, division admits only
//! nonzero numeric constants, and symbolic truthiness raises `TypeError`.

use pyo3::prelude::*;
use roml::{ParamId, ValueExpr, VarId};

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
        self.constant = simplify_value(self.constant.clone() + other.constant.clone() * sign);
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
                existing.coeff = simplify_value(existing.coeff.clone() + coeff);
            }
            None => self.terms.push(ExprTerm {
                var,
                coeff: simplify_value(coeff),
            }),
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
            term.coeff = simplify_value(term.coeff.clone() * factor);
        }
        self.constant = simplify_value(self.constant.clone() * factor);
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

/// Packed constant-coefficient vector form (P0 bulk path).
///
/// Produced by `rm.sum(VarArray)` and numeric `rm.dot` reductions: it retains
/// the owner plus a structural multi-term array WITHOUT expanding it into
/// per-term `Affine` structures, so a million-term objective crosses into
/// the core as flat buffers. Any arithmetic on a packed value materializes
/// it to a general `Affine` first; only `minimize`/`maximize` consume the
/// packed form directly.
#[derive(Debug)]
pub(crate) struct PackedVars {
    pub owner: Py<Model>,
    pub array: PackedLinearArray,
}

impl Clone for PackedVars {
    fn clone(&self) -> Self {
        Python::attach(|py| Self {
            owner: self.owner.clone_ref(py),
            array: self.array.clone(),
        })
    }
}

/// Coefficient storage for packed forms; always finite numerics by
/// construction (parameterized coefficients stay on the general path...
/// and on [`Scalar::PackedSymbolic`] in P1C-2).
#[derive(Clone, Debug)]
pub(crate) enum PackedCoeffs {
    /// All-ones (from `rm.sum`).
    One,
    /// Uniform scalar (from `rm.dot(scalar, ...)`).
    Scalar(f64),
    /// Dense per-variable values, C order (from `rm.dot(array, ...)`).
    Dense(Vec<f64>),
}

/// One elementwise term of a packed array: `coeffs[i] * vars[i]` per
/// element, C-order aligned with the array shape.
#[derive(Clone, Debug)]
pub(crate) struct PackedArrayTerm {
    pub vars: Vec<VarId>,
    pub coeffs: PackedCoeffs,
}

/// Structural packed linear array (P1C-1): elementwise terms plus a numeric
/// scalar constant. Coefficients are numeric-only (`One` / `Scalar` /
/// `Dense`); anything parameterized stays on the general `Vec<Affine>`
/// path. Term vectors align elementwise; construction never allocates
/// per-element expression objects.
#[derive(Clone, Debug)]
pub(crate) struct PackedLinearArray {
    pub shape: Vec<usize>,
    pub terms: Vec<PackedArrayTerm>,
    pub constant: f64,
}

impl PackedLinearArray {
    pub(crate) fn numel(&self) -> usize {
        self.shape.iter().product()
    }

    /// Single unit-coefficient term over a variable vector.
    pub(crate) fn from_vars(vars: Vec<VarId>, shape: Vec<usize>) -> Self {
        debug_assert_eq!(vars.len(), shape.iter().product::<usize>());
        Self {
            shape,
            terms: vec![PackedArrayTerm {
                vars,
                coeffs: PackedCoeffs::One,
            }],
            constant: 0.0,
        }
    }

    /// Scale every coefficient and the constant by a finite factor.
    pub(crate) fn scale(&mut self, factor: f64) {
        for term in &mut self.terms {
            term.coeffs = match std::mem::replace(&mut term.coeffs, PackedCoeffs::One) {
                PackedCoeffs::One => PackedCoeffs::Scalar(factor),
                PackedCoeffs::Scalar(c) => PackedCoeffs::Scalar(c * factor),
                PackedCoeffs::Dense(v) => {
                    PackedCoeffs::Dense(v.into_iter().map(|x| x * factor).collect())
                }
            };
        }
        self.constant *= factor;
    }

    /// Elementwise dense scaling. Requires a zero scalar constant (a dense
    /// factor would otherwise densify it); callers fall back to the general
    /// path when the constant is nonzero.
    pub(crate) fn scale_dense(&mut self, s: &[f64]) {
        debug_assert_eq!(s.len(), self.numel());
        debug_assert_eq!(self.constant, 0.0);
        for term in &mut self.terms {
            term.coeffs = match std::mem::replace(&mut term.coeffs, PackedCoeffs::One) {
                PackedCoeffs::One => PackedCoeffs::Dense(s.to_vec()),
                PackedCoeffs::Scalar(c) => PackedCoeffs::Dense(s.iter().map(|x| x * c).collect()),
                PackedCoeffs::Dense(v) => {
                    PackedCoeffs::Dense(v.into_iter().zip(s.iter()).map(|(a, b)| a * b).collect())
                }
            };
        }
    }

    /// Add a scalar to the constant.
    pub(crate) fn add_scalar(&mut self, v: f64) {
        self.constant += v;
    }

    /// Add (`sign` +1) or subtract (−1) another same-shape packed array.
    pub(crate) fn combine(&mut self, other: &PackedLinearArray, sign: f64) {
        debug_assert_eq!(self.shape, other.shape);
        for term in &other.terms {
            let coeffs = match &term.coeffs {
                PackedCoeffs::One => PackedCoeffs::Scalar(sign),
                PackedCoeffs::Scalar(c) => PackedCoeffs::Scalar(c * sign),
                PackedCoeffs::Dense(v) => PackedCoeffs::Dense(v.iter().map(|x| x * sign).collect()),
            };
            self.terms.push(PackedArrayTerm {
                vars: term.vars.clone(),
                coeffs,
            });
        }
        self.constant += sign * other.constant;
    }

    /// Sub-select C-order flat positions into a new shape.
    pub(crate) fn select(&self, flat: &[usize], shape: Vec<usize>) -> Self {
        debug_assert_eq!(flat.len(), shape.iter().product::<usize>());
        let terms = self
            .terms
            .iter()
            .map(|t| {
                let vars = flat.iter().map(|&f| t.vars[f]).collect();
                let coeffs = match &t.coeffs {
                    PackedCoeffs::One => PackedCoeffs::One,
                    PackedCoeffs::Scalar(c) => PackedCoeffs::Scalar(*c),
                    PackedCoeffs::Dense(v) => {
                        PackedCoeffs::Dense(flat.iter().map(|&f| v[f]).collect())
                    }
                };
                PackedArrayTerm { vars, coeffs }
            })
            .collect();
        Self {
            shape,
            terms,
            constant: self.constant,
        }
    }

    /// Expand to per-element affines (fallback/materialization boundary).
    pub(crate) fn materialize(&self, py: Python<'_>, owner: &pyo3::Py<Model>) -> Vec<Affine> {
        let n = self.numel();
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            let mut terms = Vec::with_capacity(self.terms.len());
            for t in &self.terms {
                let c = match &t.coeffs {
                    PackedCoeffs::One => 1.0,
                    PackedCoeffs::Scalar(c) => *c,
                    PackedCoeffs::Dense(v) => v[i],
                };
                terms.push(ExprTerm {
                    var: t.vars[i],
                    coeff: ValueExpr::constant(c),
                });
            }
            out.push(Affine {
                owner: owner.clone_ref(py),
                terms,
                constant: ValueExpr::constant(self.constant),
            });
        }
        out
    }
}

/// A scalar objective/expression form: a general `Affine`, a packed
/// constant-coefficient array, or a packed scaled-parameter vector
/// (P1C-2: one `(variable, scale, parameter)` cell per element, no
/// per-term `Affine` expansion).
#[derive(Debug)]
pub(crate) enum Scalar {
    Affine(Affine),
    Packed(PackedVars),
    PackedSymbolic(PackedSymbolic),
}

/// Packed scaled-parameter vector form (P1C-2).
///
/// Produced by `rm.dot` with structurally cheap parameter-only
/// coefficients (`ParamArray`, broadcast scalar `Param`, trivially
/// representable parameter scalar expressions) over a packed numeric
/// decision array: it retains parallel `(variable, scale, parameter)`
/// vectors WITHOUT expanding them into per-term `Affine` structures, so a
/// 57.6k-term parameterized objective crosses into the core as three flat
/// buffers via `set_linear_objective_param_bulk`. Any arithmetic on a
/// packed-symbolic value materializes it to a general `Affine` first (with
/// coefficient forms bit-identical to the scalar fold); only
/// `minimize`/`maximize` consume the packed form directly.
#[derive(Debug)]
pub(crate) struct PackedSymbolic {
    pub owner: Py<Model>,
    pub vars: Vec<VarId>,
    pub params: Vec<ParamId>,
    pub scales: Vec<f64>,
    pub constant: ValueExpr,
}

impl Clone for PackedSymbolic {
    fn clone(&self) -> Self {
        Python::attach(|py| Self {
            owner: self.owner.clone_ref(py),
            vars: self.vars.clone(),
            params: self.params.clone(),
            scales: self.scales.clone(),
            constant: self.constant.clone(),
        })
    }
}

impl Clone for Scalar {
    fn clone(&self) -> Self {
        match self {
            Self::Affine(a) => Self::Affine(a.clone()),
            Self::Packed(p) => Python::attach(|py| {
                Self::Packed(PackedVars {
                    owner: p.owner.clone_ref(py),
                    array: p.array.clone(),
                })
            }),
            Self::PackedSymbolic(s) => Self::PackedSymbolic(s.clone()),
        }
    }
}

impl Scalar {
    /// Owner reference without materializing.
    pub(crate) fn owner_ref(&self, py: Python<'_>) -> Py<Model> {
        match self {
            Self::Affine(a) => a.owner.clone_ref(py),
            Self::Packed(p) => p.owner.clone_ref(py),
            Self::PackedSymbolic(s) => s.owner.clone_ref(py),
        }
    }

    /// General `Affine` view, expanding packed forms term-by-term.
    /// Only non-bulk consumers pay this; `minimize`/`maximize` match on
    /// [`Scalar::Packed`] and [`Scalar::PackedSymbolic`] directly.
    /// Packed-symbolic coefficients expand to the canonical scaled-parameter
    /// form, bit-identical to what the scalar fold stores.
    pub(crate) fn materialize(&self, py: Python<'_>) -> Affine {
        match self {
            Self::Affine(a) => a.clone(),
            Self::PackedSymbolic(s) => Affine {
                owner: s.owner.clone_ref(py),
                terms: s
                    .vars
                    .iter()
                    .zip(s.params.iter())
                    .zip(s.scales.iter())
                    .map(|((var, param), scale)| ExprTerm {
                        var: *var,
                        coeff: ValueExpr::scaled_param(*scale, *param),
                    })
                    .collect(),
                constant: s.constant.clone(),
            },
            Self::Packed(p) => {
                let array = &p.array;
                let mut terms = Vec::with_capacity(array.terms.iter().map(|t| t.vars.len()).sum());
                for t in &array.terms {
                    match &t.coeffs {
                        PackedCoeffs::One => {
                            for v in &t.vars {
                                terms.push(ExprTerm {
                                    var: *v,
                                    coeff: ValueExpr::constant(1.0),
                                });
                            }
                        }
                        PackedCoeffs::Scalar(c) => {
                            for v in &t.vars {
                                terms.push(ExprTerm {
                                    var: *v,
                                    coeff: ValueExpr::constant(*c),
                                });
                            }
                        }
                        PackedCoeffs::Dense(values) => {
                            for (v, c) in t.vars.iter().zip(values.iter()) {
                                terms.push(ExprTerm {
                                    var: *v,
                                    coeff: ValueExpr::constant(*c),
                                });
                            }
                        }
                    }
                }
                Affine {
                    owner: p.owner.clone_ref(py),
                    terms,
                    constant: ValueExpr::constant(array.constant),
                }
            }
        }
    }
}

#[pyclass(frozen, name = "Expr")]
pub struct Expr {
    pub(crate) inner: Scalar,
}

impl Expr {
    fn inner_term_count(&self) -> usize {
        match &self.inner {
            Scalar::Affine(a) => a.terms.len(),
            Scalar::Packed(p) => p.array.numel(),
            Scalar::PackedSymbolic(s) => s.vars.len(),
        }
    }
}

#[pymethods]
impl Expr {
    fn __repr__(&self) -> String {
        format!(
            "Expr({} terms{})",
            self.inner_term_count(),
            if self.inner_term_count() == 0 {
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
            let mut inner = slf.bind(py).borrow().inner.materialize(py);
            inner.scale(-1.0)?;
            Ok(Bound::new(
                py,
                Self {
                    inner: Scalar::Affine(inner),
                },
            )?
            .unbind())
        })
    }

    fn __add__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Python::attach(|py| {
            if super::arrays::is_array_operand(&other) {
                return Ok(py.NotImplemented());
            }
            let base = slf.bind(py).borrow();
            let mut inner = base.inner.materialize(py);
            drop(base);
            add_operand(&mut inner, &other, 1.0)?;
            Ok(Bound::new(
                py,
                Self {
                    inner: Scalar::Affine(inner),
                },
            )?
            .into_any()
            .unbind())
        })
    }

    fn __radd__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Self::__add__(slf, other)
    }

    fn __sub__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Python::attach(|py| {
            if super::arrays::is_array_operand(&other) {
                return Ok(py.NotImplemented());
            }
            let base = slf.bind(py).borrow();
            let mut inner = base.inner.materialize(py);
            drop(base);
            add_operand(&mut inner, &other, -1.0)?;
            Ok(Bound::new(
                py,
                Self {
                    inner: Scalar::Affine(inner),
                },
            )?
            .into_any()
            .unbind())
        })
    }

    fn __rsub__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Python::attach(|py| {
            if super::arrays::is_array_operand(&other) {
                return Ok(py.NotImplemented());
            }
            let base = slf.bind(py).borrow();
            let mut inner = Affine {
                owner: base.inner.owner_ref(py),
                terms: Vec::new(),
                constant: ValueExpr::constant(0.0),
            };
            drop(base);
            add_operand(&mut inner, &other, 1.0)?;
            let neg = slf.bind(py).borrow();
            let mut result = inner;
            drop(neg);
            let orig = slf.bind(py).borrow();
            let orig_affine = orig.inner.materialize(py);
            drop(orig);
            result.add_terms(&orig_affine, -1.0)?;
            Ok(Bound::new(
                py,
                Self {
                    inner: Scalar::Affine(result),
                },
            )?
            .into_any()
            .unbind())
        })
    }

    fn __mul__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Python::attach(|py| {
            if super::arrays::is_array_operand(&other) {
                return Ok(py.NotImplemented());
            }
            let base = slf.bind(py).borrow();
            let mut inner = base.inner.materialize(py);
            drop(base);
            mul_operand(&mut inner, &other)?;
            Ok(Bound::new(
                py,
                Self {
                    inner: Scalar::Affine(inner),
                },
            )?
            .into_any()
            .unbind())
        })
    }

    fn __rmul__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Self::__mul__(slf, other)
    }

    fn __truediv__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Python::attach(|py| {
            if super::arrays::is_array_operand(&other) {
                return Ok(py.NotImplemented());
            }
            let divisor = operand_numeric(&other, "division")?;
            if divisor == 0.0 {
                return Err(InvalidModelError::new_err("division by zero"));
            }
            let base = slf.bind(py).borrow();
            let mut inner = base.inner.materialize(py);
            drop(base);
            inner.scale(1.0 / divisor)?;
            Ok(Bound::new(
                py,
                Self {
                    inner: Scalar::Affine(inner),
                },
            )?
            .into_any()
            .unbind())
        })
    }

    fn __le__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Python::attach(|py| {
            if super::arrays::is_array_operand(&other) {
                return Ok(py.NotImplemented());
            }
            let c = compare_operand(&slf, &other, Sense::Le)?;
            Ok(Bound::new(py, c)?.into_any().unbind())
        })
    }

    fn __ge__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Python::attach(|py| {
            if super::arrays::is_array_operand(&other) {
                return Ok(py.NotImplemented());
            }
            let c = compare_operand(&slf, &other, Sense::Ge)?;
            Ok(Bound::new(py, c)?.into_any().unbind())
        })
    }

    fn __eq__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Python::attach(|py| {
            if super::arrays::is_array_operand(&other) {
                return Ok(py.NotImplemented());
            }
            let c = compare_operand(&slf, &other, Sense::Eq)?;
            Ok(Bound::new(py, c)?.into_any().unbind())
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Sense {
    Le,
    Ge,
    Eq,
}

/// Add (sign +1) or subtract (sign -1) an operand into an affine expression.
/// Array operands cannot fold into a scalar: callers route them through
/// the array engine before reaching here.
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
        let py = other.py();
        same_owner(&expr.inner.owner_ref(py), &inner.owner)?;
        let other_affine = expr.inner.materialize(py);
        drop(expr);
        inner.add_terms(&other_affine, sign)?;
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
        let py = other.py();
        same_owner(&expr.inner.owner_ref(py), &inner.owner)?;
        let other_affine = expr.inner.materialize(py);
        drop(expr);
        if !inner.terms.is_empty() && !other_affine.terms.is_empty() {
            return Err(nonlinear());
        }
        if inner.terms.is_empty() {
            // Parameter-only self times affine other.
            let mine = std::mem::replace(&mut inner.constant, ValueExpr::constant(0.0));
            for term in &other_affine.terms {
                inner.push_term(term.var, term.coeff.clone() * mine.clone())?;
            }
            inner.constant = other_affine.constant.clone() * mine;
            return Ok(());
        }
        // Affine self times parameter-only other.
        let factor = other_affine.constant.clone();
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
        // Array operands never reach here: every comparison dunder
        // returns NotImplemented for them first.
        let base = slf.bind(py).borrow();
        let mut lhs = base.inner.materialize(py);
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

    fn __le__(&self, _other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Err(chained())
    }

    fn __ge__(&self, _other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Err(chained())
    }

    fn __eq__(&self, _other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Err(chained())
    }
}

/// Convert a scalar operand into an owned scalar form homed to `model`:
/// `Var` lifts to a unit term, `Param` to a parameter-only expression,
/// numerics to a constant, and `Expr` clones through (preserving the packed
/// form so `minimize`/`maximize` can take the bulk path).
/// Foreign-model handles fail here, so `Model.minimize` never sees them.
pub(crate) fn to_scalar(model: &Bound<'_, Model>, other: &Bound<'_, PyAny>) -> PyResult<Scalar> {
    Python::attach(|py| {
        if let Ok(var) = other.cast::<Var>() {
            let var = var.borrow();
            same_owner(&var.owner, &model.clone().unbind())?;
            return Ok(Scalar::Affine(var_affine(py, &var)));
        }
        if let Ok(param) = other.cast::<Param>() {
            let param = param.borrow();
            same_owner(&param.owner, &model.clone().unbind())?;
            return Ok(Scalar::Affine(param_affine(py, &param)));
        }
        if let Ok(expr) = other.cast::<Expr>() {
            let expr = expr.borrow();
            same_owner(&expr.inner.owner_ref(py), &model.clone().unbind())?;
            return Ok(expr.inner.clone());
        }
        let v = operand_numeric(other, "objective")?;
        Ok(Scalar::Affine(Affine {
            owner: model.clone().unbind(),
            terms: Vec::new(),
            constant: ValueExpr::constant(v),
        }))
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
            Ok(Bound::new(
                py,
                Expr {
                    inner: Scalar::Affine(inner),
                },
            )?
            .unbind())
        })
    }

    fn __add__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Python::attach(|py| {
            if super::arrays::is_array_operand(&other) {
                return Ok(py.NotImplemented());
            }
            let mut inner = var_affine(py, &slf.bind(py).borrow());
            add_operand(&mut inner, &other, 1.0)?;
            Ok(Bound::new(
                py,
                Expr {
                    inner: Scalar::Affine(inner),
                },
            )?
            .into_any()
            .unbind())
        })
    }

    fn __radd__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Self::__add__(slf, other)
    }

    fn __sub__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Python::attach(|py| {
            if super::arrays::is_array_operand(&other) {
                return Ok(py.NotImplemented());
            }
            let mut inner = var_affine(py, &slf.bind(py).borrow());
            add_operand(&mut inner, &other, -1.0)?;
            Ok(Bound::new(
                py,
                Expr {
                    inner: Scalar::Affine(inner),
                },
            )?
            .into_any()
            .unbind())
        })
    }

    fn __rsub__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Python::attach(|py| {
            if super::arrays::is_array_operand(&other) {
                return Ok(py.NotImplemented());
            }
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
            Ok(Bound::new(
                py,
                Expr {
                    inner: Scalar::Affine(inner),
                },
            )?
            .into_any()
            .unbind())
        })
    }

    fn __mul__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Python::attach(|py| {
            if super::arrays::is_array_operand(&other) {
                return Ok(py.NotImplemented());
            }
            let mut inner = var_affine(py, &slf.bind(py).borrow());
            mul_operand(&mut inner, &other)?;
            Ok(Bound::new(
                py,
                Expr {
                    inner: Scalar::Affine(inner),
                },
            )?
            .into_any()
            .unbind())
        })
    }

    fn __rmul__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Self::__mul__(slf, other)
    }

    fn __truediv__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Python::attach(|py| {
            if super::arrays::is_array_operand(&other) {
                return Ok(py.NotImplemented());
            }
            let divisor = operand_numeric(&other, "division")?;
            if divisor == 0.0 {
                return Err(InvalidModelError::new_err("division by zero"));
            }
            let mut inner = var_affine(py, &slf.bind(py).borrow());
            inner.scale(1.0 / divisor)?;
            Ok(Bound::new(
                py,
                Expr {
                    inner: Scalar::Affine(inner),
                },
            )?
            .into_any()
            .unbind())
        })
    }

    fn __le__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Python::attach(|py| {
            if super::arrays::is_array_operand(&other) {
                return Ok(py.NotImplemented());
            }
            let inner = var_affine(py, &slf.bind(py).borrow());
            let expr = Bound::new(
                py,
                Expr {
                    inner: Scalar::Affine(inner),
                },
            )?;
            let c = compare_operand(&expr.unbind(), &other, Sense::Le)?;
            Ok(Bound::new(py, c)?.into_any().unbind())
        })
    }

    fn __ge__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Python::attach(|py| {
            if super::arrays::is_array_operand(&other) {
                return Ok(py.NotImplemented());
            }
            let inner = var_affine(py, &slf.bind(py).borrow());
            let expr = Bound::new(
                py,
                Expr {
                    inner: Scalar::Affine(inner),
                },
            )?;
            let c = compare_operand(&expr.unbind(), &other, Sense::Ge)?;
            Ok(Bound::new(py, c)?.into_any().unbind())
        })
    }

    fn __eq__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Python::attach(|py| {
            if super::arrays::is_array_operand(&other) {
                return Ok(py.NotImplemented());
            }
            let inner = var_affine(py, &slf.bind(py).borrow());
            let expr = Bound::new(
                py,
                Expr {
                    inner: Scalar::Affine(inner),
                },
            )?;
            let c = compare_operand(&expr.unbind(), &other, Sense::Eq)?;
            Ok(Bound::new(py, c)?.into_any().unbind())
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
            Ok(Bound::new(
                py,
                Expr {
                    inner: Scalar::Affine(inner),
                },
            )?
            .unbind())
        })
    }

    fn __add__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Python::attach(|py| {
            if super::arrays::is_array_operand(&other) {
                return Ok(py.NotImplemented());
            }
            let mut inner = param_affine(py, &slf.bind(py).borrow());
            add_operand(&mut inner, &other, 1.0)?;
            Ok(Bound::new(
                py,
                Expr {
                    inner: Scalar::Affine(inner),
                },
            )?
            .into_any()
            .unbind())
        })
    }

    fn __radd__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Self::__add__(slf, other)
    }

    fn __sub__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Python::attach(|py| {
            if super::arrays::is_array_operand(&other) {
                return Ok(py.NotImplemented());
            }
            let mut inner = param_affine(py, &slf.bind(py).borrow());
            add_operand(&mut inner, &other, -1.0)?;
            Ok(Bound::new(
                py,
                Expr {
                    inner: Scalar::Affine(inner),
                },
            )?
            .into_any()
            .unbind())
        })
    }

    fn __rsub__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Python::attach(|py| {
            if super::arrays::is_array_operand(&other) {
                return Ok(py.NotImplemented());
            }
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
            Ok(Bound::new(
                py,
                Expr {
                    inner: Scalar::Affine(inner),
                },
            )?
            .into_any()
            .unbind())
        })
    }

    fn __mul__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Python::attach(|py| {
            if super::arrays::is_array_operand(&other) {
                return Ok(py.NotImplemented());
            }
            let mut inner = param_affine(py, &slf.bind(py).borrow());
            mul_operand(&mut inner, &other)?;
            Ok(Bound::new(
                py,
                Expr {
                    inner: Scalar::Affine(inner),
                },
            )?
            .into_any()
            .unbind())
        })
    }

    fn __rmul__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Self::__mul__(slf, other)
    }

    fn __truediv__(slf: Py<Self>, other: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Python::attach(|py| {
            if super::arrays::is_array_operand(&other) {
                return Ok(py.NotImplemented());
            }
            let divisor = operand_numeric(&other, "division")?;
            if divisor == 0.0 {
                return Err(InvalidModelError::new_err("division by zero"));
            }
            let mut inner = param_affine(py, &slf.bind(py).borrow());
            inner.scale(1.0 / divisor)?;
            Ok(Bound::new(
                py,
                Expr {
                    inner: Scalar::Affine(inner),
                },
            )?
            .into_any()
            .unbind())
        })
    }

    fn __bool__(&self) -> PyResult<bool> {
        Err(pyo3::exceptions::PyTypeError::new_err(
            "parameters have no truth value",
        ))
    }
}

/// Canonicalize a coefficient/constant expression: constant folding plus
/// zero/one identities. `ValueExpr` trees never simplify themselves, so
/// binding-built arithmetic (`0 * p`, accumulated folds) would otherwise
/// carry phantom parameter dependencies. Only exact identities apply;
/// nothing is reordered or approximated.
pub(crate) fn simplify_value(expr: ValueExpr) -> ValueExpr {
    use roml::ValueExpr as V;
    match expr {
        V::Constant(_) | V::Param(_) => expr,
        V::Neg(inner) => match simplify_value(*inner) {
            V::Constant(c) => V::constant(-c),
            s => V::neg(s),
        },
        V::Add(l, r) => {
            let (l, r) = (simplify_value(*l), simplify_value(*r));
            match (&l, &r) {
                (V::Constant(a), V::Constant(b)) => V::constant(a + b),
                (V::Constant(a), _) if *a == 0.0 => r,
                (_, V::Constant(b)) if *b == 0.0 => l,
                _ => V::add(l, r),
            }
        }
        V::Sub(l, r) => {
            let (l, r) = (simplify_value(*l), simplify_value(*r));
            match (&l, &r) {
                (V::Constant(a), V::Constant(b)) => V::constant(a - b),
                (_, V::Constant(b)) if *b == 0.0 => l,
                _ => V::sub(l, r),
            }
        }
        V::Mul(l, r) => {
            let (l, r) = (simplify_value(*l), simplify_value(*r));
            match (&l, &r) {
                (V::Constant(a), V::Constant(b)) => V::constant(a * b),
                (V::Constant(a), _) if *a == 0.0 => V::constant(0.0),
                (_, V::Constant(b)) if *b == 0.0 => V::constant(0.0),
                (V::Constant(a), _) if *a == 1.0 => r,
                (_, V::Constant(b)) if *b == 1.0 => l,
                _ => V::mul(l, r),
            }
        }
        V::Div(l, r) => {
            let (l, r) = (simplify_value(*l), simplify_value(*r));
            match (&l, &r) {
                (V::Constant(a), V::Constant(b)) => V::constant(a / b),
                (V::Constant(a), _) if *a == 0.0 => V::constant(0.0),
                (_, V::Constant(b)) if *b == 1.0 => l,
                _ => V::div(l, r),
            }
        }
    }
}
