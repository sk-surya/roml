//! Shaped bulk modeling: arrays, slices, bulk math, CSR rows (DESIGN §3).
//!
//! Arrays are Rust-owned structures (shape metadata plus typed handle or
//! expression vectors), never NumPy object arrays. Elementwise operations
//! and reductions execute in Rust; Python arithmetic invokes one extension
//! call per vector operation. Numeric inputs are copied once into owned
//! Rust buffers with strict dtype/shape validation.

use pyo3::prelude::*;
use pyo3::types::{PyAny, PyBool, PyDict, PySequence, PyTuple};
use roml::{ParamId, ValueExpr, VarId};

use super::errors::{InvalidModelError, ModelMismatchError, ShapeError};
pub(crate) use super::expressions::BoundSide;
use super::expressions::{
    as_scaled_param, simplify_value, Affine, PackedArrayTerm, PackedCoeffs, PackedLinearArray,
    PackedVars, Scalar,
};
use super::expressions::{Comparison, ExprTerm};
use super::handles::{Param, Var};
use super::model::Model;

/// Parse a shape argument: a non-negative int or a tuple of non-negative
/// ints. Bools and floats reject; negative dimensions reject.
pub(crate) fn parse_shape(obj: &Bound<'_, PyAny>) -> PyResult<Vec<usize>> {
    if obj.is_instance_of::<PyBool>() {
        return Err(ShapeError::new_err("shape dimensions must be integers"));
    }
    if is_numpy_bool_scalar(obj) {
        return Err(ShapeError::new_err(
            "shape dimensions must be integers, not bools",
        ));
    }
    if let Ok(n) = obj.extract::<isize>() {
        if n < 0 {
            return Err(ShapeError::new_err("shape dimensions must be >= 0"));
        }
        return Ok(vec![n as usize]);
    }
    if let Ok(tuple) = obj.cast::<PyTuple>() {
        let mut shape = Vec::with_capacity(tuple.len());
        for item in tuple.iter() {
            if item.is_instance_of::<PyBool>() || is_numpy_bool_scalar(&item) {
                return Err(ShapeError::new_err("shape dimensions must be integers"));
            }
            let n: isize = item
                .extract()
                .map_err(|_| ShapeError::new_err("shape dimensions must be integers"))?;
            if n < 0 {
                return Err(ShapeError::new_err("shape dimensions must be >= 0"));
            }
            shape.push(n as usize);
        }
        return Ok(shape);
    }
    Err(ShapeError::new_err(
        "shape must be an int or a tuple of ints",
    ))
}

pub(crate) fn numel(shape: &[usize]) -> usize {
    shape.iter().product()
}

/// Numeric input mode: stored values must be finite; bounds additionally
/// admit -inf (lower) / +inf (upper) but never NaN.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum NumericMode {
    Finite,
    Bounds,
}

/// Parsed dense numeric input: C-order flat values plus shape.
pub(crate) struct NumericInput {
    pub shape: Vec<usize>,
    pub values: Vec<f64>,
}

/// Parse dense numeric input from a NumPy array or a (nested) sequence.
/// Scalars reject here (callers handle scalar broadcast explicitly).
pub(crate) fn parse_numeric(
    py: Python<'_>,
    obj: &Bound<'_, PyAny>,
    mode: NumericMode,
    what: &str,
) -> PyResult<NumericInput> {
    if is_numpy_array(obj) {
        return parse_numpy(py, obj, mode, what);
    }
    parse_sequence(obj, mode, what)
}

pub(crate) fn is_numpy_array(obj: &Bound<'_, PyAny>) -> bool {
    obj.hasattr("dtype").unwrap_or(false) && obj.hasattr("shape").unwrap_or(false)
}

fn check_value(v: f64, mode: NumericMode, what: &str) -> PyResult<f64> {
    match mode {
        NumericMode::Finite => {
            if !v.is_finite() {
                return Err(InvalidModelError::new_err(format!("{what} must be finite")));
            }
        }
        NumericMode::Bounds => {
            if v.is_nan() {
                return Err(InvalidModelError::new_err(format!(
                    "{what} must not be NaN"
                )));
            }
        }
    }
    Ok(v)
}

fn parse_numpy(
    py: Python<'_>,
    obj: &Bound<'_, PyAny>,
    mode: NumericMode,
    what: &str,
) -> PyResult<NumericInput> {
    let dtype = obj.getattr("dtype")?;
    let kind: String = dtype.getattr("kind")?.extract()?;
    match kind.as_str() {
        "f" | "i" | "u" => {}
        "b" => {
            return Err(InvalidModelError::new_err(format!(
                "{what}: bool dtype is not accepted as numeric input"
            )))
        }
        "c" => {
            return Err(InvalidModelError::new_err(format!(
                "{what}: complex dtype is not accepted as numeric input"
            )))
        }
        _ => {
            return Err(InvalidModelError::new_err(format!(
                "{what}: dtype kind {kind:?} is not accepted as numeric input"
            )))
        }
    }
    let numpy = py.import("numpy")?;
    // Read the shape from the ORIGINAL object: `ascontiguousarray`
    // promotes 0-d inputs to shape (1,), which would corrupt 0-d
    // shape tracking below.
    let shape_obj = obj.getattr("shape")?;
    let shape_tuple = shape_obj
        .cast::<PyTuple>()
        .map_err(|_| InvalidModelError::new_err(format!("{what}: cannot read array shape")))?;
    let mut shape = Vec::with_capacity(shape_tuple.len());
    for item in shape_tuple.iter() {
        let n: usize = item
            .extract()
            .map_err(|_| InvalidModelError::new_err(format!("{what}: invalid array shape")))?;
        shape.push(n);
    }
    // One deliberate contiguous float64 copy: also normalizes int inputs
    // and noncontiguous strides.
    let flat = numpy.call_method(
        "ascontiguousarray",
        (obj,),
        Some(&{
            let kwargs = PyDict::new(py);
            kwargs.set_item("dtype", numpy.getattr("float64")?)?;
            kwargs
        }),
    )?;
    use numpy::{IxDyn, PyArray, PyArrayMethods};
    let ravel: Vec<f64> = flat
        .call_method0("ravel")?
        .cast::<PyArray<f64, IxDyn>>()
        .map_err(|_| InvalidModelError::new_err(format!("{what}: cannot read array data")))?
        .to_vec()
        .map_err(|_| InvalidModelError::new_err(format!("{what}: cannot read array data")))?;
    if ravel.len() != numel(&shape) {
        return Err(ShapeError::new_err(format!(
            "{what}: data length does not match shape"
        )));
    }
    let mut values = Vec::with_capacity(ravel.len());
    for v in ravel {
        values.push(check_value(v, mode, what)?);
    }
    Ok(NumericInput { shape, values })
}

/// Recursive sequence parser returning (shape, C-order values). Ragged
/// nesting, bools, strings, complex, and None reject with typed errors.
fn parse_sequence(obj: &Bound<'_, PyAny>, mode: NumericMode, what: &str) -> PyResult<NumericInput> {
    if obj.is_instance_of::<PyBool>() {
        return Err(InvalidModelError::new_err(format!(
            "{what}: bools are not accepted as numeric input"
        )));
    }
    if let Ok(v) = obj.extract::<f64>() {
        // A bare scalar is not dense array input; callers treat scalars as
        // broadcast explicitly. Reject here to keep shapes honest.
        let _ = check_value(v, mode, what)?;
        return Err(ShapeError::new_err(format!(
            "{what}: expected an array, got a scalar (scalars broadcast only where documented)"
        )));
    }
    // Strings/bytes are sequences but never numeric input.
    if obj.hasattr("encode").unwrap_or(false) || obj.hasattr("decode").unwrap_or(false) {
        return Err(InvalidModelError::new_err(format!(
            "{what}: strings and bytes are not accepted as numeric input"
        )));
    }
    let seq = obj.cast::<PySequence>().map_err(|_| {
        InvalidModelError::new_err(format!("{what}: expected an array or nested sequence"))
    })?;
    parse_sequence_level(seq, mode, what)
}

fn parse_sequence_level(
    seq: &Bound<'_, PySequence>,
    mode: NumericMode,
    what: &str,
) -> PyResult<NumericInput> {
    let n = seq.len()?;
    if n == 0 {
        return Ok(NumericInput {
            shape: vec![0],
            values: Vec::new(),
        });
    }
    let first = seq.get_item(0)?;
    // Nested level?
    let nested = first.cast::<PySequence>().ok().filter(|s| {
        !s.is_instance_of::<pyo3::types::PyString>()
            && !s.is_instance_of::<pyo3::types::PyBytes>()
            && !s.is_instance_of::<PyBool>()
    });
    if nested.is_none()
        && (first.hasattr("encode").unwrap_or(false) || first.hasattr("decode").unwrap_or(false))
    {
        return Err(InvalidModelError::new_err(format!(
            "{what}: strings and bytes are not accepted as numeric input"
        )));
    }
    if let Some(nested_seq) = nested {
        let head = parse_sequence_level(nested_seq, mode, what)?;
        let mut values = head.values;
        for i in 1..n {
            let item = seq.get_item(i)?;
            let item_seq = item
                .cast::<PySequence>()
                .map_err(|_| ShapeError::new_err(format!("{what}: ragged nested sequences")))?;
            let part = parse_sequence_level(item_seq, mode, what)?;
            if part.shape != head.shape {
                return Err(ShapeError::new_err(format!(
                    "{what}: ragged nested sequences"
                )));
            }
            values.extend(part.values);
        }
        let mut shape = vec![n];
        shape.extend(head.shape);
        return Ok(NumericInput { shape, values });
    }
    // Flat level: every item must be int/float (not bool).
    let mut values = Vec::with_capacity(n);
    for i in 0..n {
        let item = seq.get_item(i)?;
        if item.is_instance_of::<PyBool>() {
            return Err(InvalidModelError::new_err(format!(
                "{what}: bools are not accepted as numeric input"
            )));
        }
        if item.hasattr("encode").unwrap_or(false) || item.hasattr("decode").unwrap_or(false) {
            return Err(InvalidModelError::new_err(format!(
                "{what}: strings and bytes are not accepted as numeric input"
            )));
        }
        if item.cast::<PySequence>().is_ok() {
            return Err(ShapeError::new_err(format!(
                "{what}: ragged nested sequences"
            )));
        }
        let v: f64 = item.extract().map_err(|_| {
            InvalidModelError::new_err(format!("{what}: array elements must be real numbers"))
        })?;
        values.push(check_value(v, mode, what)?);
    }
    Ok(NumericInput {
        shape: vec![n],
        values,
    })
}

/// Normalize an index tuple against a shape: returns (flat positions,
/// result shape, is_scalar). Integers (negative-normalized), slices, and
/// one ellipsis are supported; anything else is a typed error.
pub(crate) fn normalize_index(
    shape: &[usize],
    index: &Bound<'_, PyAny>,
) -> PyResult<(Vec<usize>, Vec<usize>, bool)> {
    let py = index.py();
    // Collect index items: a bare index counts as a 1-tuple.
    let items: Vec<Bound<'_, PyAny>> = if index.is_instance_of::<PyTuple>() {
        index.cast::<PyTuple>().unwrap().iter().collect()
    } else {
        vec![index.clone()]
    };
    // Split on ellipsis.
    let mut ellipsis_at: Option<usize> = None;
    for (i, item) in items.iter().enumerate() {
        if item.is(py.Ellipsis()) {
            if ellipsis_at.is_some() {
                return Err(ShapeError::new_err(
                    "an index can contain only one ellipsis",
                ));
            }
            ellipsis_at = Some(i);
        }
    }
    let rank = shape.len();
    if ellipsis_at.is_none() && items.len() > rank {
        return Err(ShapeError::new_err(format!(
            "too many indices for shape {shape:?}"
        )));
    }
    let expanded: Vec<Bound<'_, PyAny>> = match ellipsis_at {
        Some(at) => {
            let before = at;
            let after = items.len() - at - 1;
            if before + after > rank {
                return Err(ShapeError::new_err(format!(
                    "too many indices for shape {shape:?}"
                )));
            }
            let mut full = Vec::with_capacity(rank);
            full.extend(items[..before].iter().cloned());
            let full_slice = py
                .eval(pyo3::ffi::c_str!("slice(None)"), None, None)
                .unwrap();
            for _ in 0..(rank - before - after) {
                full.push(full_slice.clone());
            }
            full.extend(items[at + 1..].iter().cloned());
            full
        }
        None => {
            if items.len() > rank {
                return Err(ShapeError::new_err(format!(
                    "too many indices for shape {shape:?}"
                )));
            }
            let mut full: Vec<Bound<'_, PyAny>> = items.clone();
            let full_slice = py
                .eval(pyo3::ffi::c_str!("slice(None)"), None, None)
                .unwrap();
            while full.len() < rank {
                full.push(full_slice.clone());
            }
            full
        }
    };
    // Per-dimension selection.
    let mut selections: Vec<Vec<usize>> = Vec::with_capacity(rank);
    let mut result_shape: Vec<usize> = Vec::new();
    let mut scalar = true;
    for (dim, sel) in expanded.iter().enumerate().take(rank) {
        let len = shape[dim];
        if let Ok(slice) = sel.cast::<pyo3::types::PySlice>() {
            let indices = slice
                .indices(len as isize)
                .map_err(|_| ShapeError::new_err("slice indices out of range"))?;
            if indices.step <= 0 {
                return Err(ShapeError::new_err(
                    "negative slice steps are not supported",
                ));
            }
            let mut picked = Vec::new();
            let mut i = indices.start.max(0);
            let mut remaining = indices.slicelength;
            while remaining > 0 {
                picked.push(i as usize);
                i += indices.step;
                remaining -= 1;
            }
            result_shape.push(picked.len());
            selections.push(picked);
            scalar = false;
        } else if sel.is_instance_of::<PyBool>() || is_numpy_bool_scalar(sel) {
            return Err(ShapeError::new_err("boolean indices are not supported"));
        } else if let Ok(mut idx) = sel.extract::<isize>() {
            if idx < 0 {
                idx += len as isize;
            }
            if idx < 0 || idx >= len as isize {
                return Err(ShapeError::new_err(format!(
                    "index {idx} out of range for dimension of size {len}"
                )));
            }
            selections.push(vec![idx as usize]);
        } else {
            return Err(ShapeError::new_err(
                "indices must be integers, slices, or an ellipsis",
            ));
        }
    }
    // C-order flat positions via cartesian walk (first dim outermost).
    let mut strides = vec![1usize; rank];
    for i in (0..rank.saturating_sub(1)).rev() {
        strides[i] = strides[i + 1] * shape[i + 1];
    }
    let mut flat = Vec::new();
    fn walk(
        dim: usize,
        rank: usize,
        selections: &[Vec<usize>],
        strides: &[usize],
        offset: usize,
        out: &mut Vec<usize>,
    ) {
        if dim == rank {
            out.push(offset);
            return;
        }
        for &s in &selections[dim] {
            walk(
                dim + 1,
                rank,
                selections,
                strides,
                offset + s * strides[dim],
                out,
            );
        }
    }
    walk(0, rank, &selections, &strides, 0, &mut flat);
    Ok((flat, result_shape, scalar && (rank > 0 || items.is_empty())))
}

fn model_mismatch() -> PyErr {
    ModelMismatchError::new_err("array belongs to a different model")
}

/// Shaped variable array: C-order handle vector with shape metadata.
#[pyclass(frozen, name = "VarArray")]
pub struct VarArray {
    pub owner: pyo3::Py<Model>,
    pub shape: Vec<usize>,
    pub vars: Vec<VarId>,
    pub base_name: String,
    /// Root flat ordinals parallel to [`VarArray::vars`] (P2A).
    ///
    /// `None` means identity (`vars[i]` is element `i`): root arrays pay
    /// nothing. Slices/views carry the gathered root ordinals so scalar
    /// handles display the canonical root element name, not the
    /// view-relative offset (previously `x[2:7][0]` mislabeled the
    /// underlying `x[2]` as `x[0]`; values always flowed by `VarId`).
    pub ordinals: Option<Vec<usize>>,
}

/// Shaped parameter array: shape inferred once and immutable.
#[pyclass(frozen, name = "ParamArray")]
pub struct ParamArray {
    pub owner: pyo3::Py<Model>,
    pub shape: Vec<usize>,
    pub params: Vec<roml::ParamId>,
    pub base_name: String,
}

/// Shaped affine expression array.
#[pyclass(frozen, name = "ExprArray")]
pub struct ExprArray {
    pub owner: pyo3::Py<Model>,
    pub shape: Vec<usize>,
    pub(crate) repr: ExprArrayRepr,
}

/// Elementwise comparison array awaiting `Model.add`.
#[pyclass(frozen, name = "ComparisonArray")]
pub struct ComparisonArray {
    pub owner: pyo3::Py<Model>,
    pub shape: Vec<usize>,
    pub(crate) repr: ComparisonArrayRepr,
}

/// Shaped constraint array.
#[pyclass(frozen, name = "ConstraintArray")]
pub struct ConstraintArray {
    pub owner: pyo3::Py<Model>,
    pub shape: Vec<usize>,
    pub cons: Vec<roml::ConId>,
}

impl ExprArray {
    /// Per-element affines, expanding packed form (general-path boundary).
    pub(crate) fn materialize(&self, py: Python<'_>) -> Vec<Affine> {
        match &self.repr {
            ExprArrayRepr::Packed(packed) => packed.materialize(py, &self.owner),
            ExprArrayRepr::Materialized(exprs) => exprs.clone(),
        }
    }

    /// Element count.
    pub(crate) fn numel(&self) -> usize {
        numel(&self.shape)
    }
}

impl ComparisonArray {}

/// Element name for diagnostics: flat C-order index.
pub(crate) fn element_name(base: &str, flat: usize) -> String {
    format!("{base}[{flat}]")
}

fn shape_tuple<'py>(py: Python<'py>, shape: &[usize]) -> PyResult<Bound<'py, PyTuple>> {
    PyTuple::new(py, shape.iter().map(|n| *n as u64))
}

fn owners_match(a: &pyo3::Py<Model>, b: &pyo3::Py<Model>) -> PyResult<()> {
    if a.as_ptr() == b.as_ptr() {
        Ok(())
    } else {
        Err(model_mismatch())
    }
}

/// Fold a slice of affines with signs into one Rust-owned affine.
///
/// Terms combine through a hash map (linear time, deterministic `VarId`
/// order at the end). Constants combine through a BALANCED pairwise tree
/// (logarithmic depth): a linear fold would nest `ValueExpr` trees to
/// depth O(n), overflowing the stack in recursive evaluation on large
/// bulk reductions.
fn fold_affines(py: Python<'_>, owner: &pyo3::Py<Model>, parts: &[(&Affine, f64)]) -> Affine {
    let mut terms: std::collections::HashMap<VarId, ValueExpr> = std::collections::HashMap::new();
    let mut consts: Vec<ValueExpr> = Vec::with_capacity(parts.len());
    for (affine, sign) in parts {
        for term in &affine.terms {
            terms
                .entry(term.var)
                .and_modify(|e| *e = simplify_value(e.clone() + term.coeff.clone() * *sign))
                .or_insert_with(|| simplify_value(term.coeff.clone() * *sign));
        }
        consts.push(affine.constant.clone() * *sign);
    }
    let mut term_vec: Vec<ExprTerm> = terms
        .into_iter()
        .map(|(var, coeff)| ExprTerm { var, coeff })
        .collect();
    term_vec.sort_by_key(|t| t.var);
    Affine {
        owner: owner.clone_ref(py),
        terms: term_vec,
        constant: balanced_sum(consts),
    }
}

/// Pairwise-balanced sum of value expressions (logarithmic tree depth).
/// Each pair simplifies eagerly so pure constants collapse flat.
fn balanced_sum(mut exprs: Vec<ValueExpr>) -> ValueExpr {
    use super::expressions::simplify_value;
    if exprs.is_empty() {
        return ValueExpr::constant(0.0);
    }
    while exprs.len() > 1 {
        let mut next = Vec::with_capacity(exprs.len().div_ceil(2));
        let mut iter = exprs.into_iter();
        while let Some(a) = iter.next() {
            match iter.next() {
                Some(b) => next.push(simplify_value(a + b)),
                None => next.push(a),
            }
        }
        exprs = next;
    }
    exprs.pop().unwrap()
}

fn var_affine_of(py: Python<'_>, owner: &pyo3::Py<Model>, var: VarId) -> Affine {
    Affine {
        owner: owner.clone_ref(py),
        terms: vec![ExprTerm {
            var,
            coeff: ValueExpr::constant(1.0),
        }],
        constant: ValueExpr::constant(0.0),
    }
}

/// A normalized array operand: either a broadcast scalar affine or a
/// shape-matched affine vector.
enum Operand {
    Scalar(Affine),
    Vector(Vec<usize>, Vec<Affine>),
}

/// Internal representation of an expression array: packed structural form
/// or general per-element affines.
#[derive(Clone, Debug)]
pub(crate) enum ExprArrayRepr {
    Packed(PackedLinearArray),
    Materialized(Vec<Affine>),
}

/// Comparison sense for a packed comparison (numeric bound).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PackedSense {
    Le,
    Ge,
    Eq,
}

/// Structural packed comparison: packed array with a numeric bound.
#[derive(Clone, Debug)]
pub(crate) struct PackedComparison {
    pub shape: Vec<usize>,
    pub array: PackedLinearArray,
    pub sense: PackedSense,
    pub bound: f64,
}

/// Internal representation of a comparison array.
#[derive(Clone, Debug)]
pub(crate) enum ComparisonArrayRepr {
    Packed(PackedComparison),
    Materialized(Vec<(Affine, BoundSide)>),
}

fn affine_of_var(py: Python<'_>, owner: &pyo3::Py<Model>, var: VarId) -> Affine {
    var_affine_of(py, owner, var)
}

/// Normalize any supported operand against an expected shape: numbers,
/// `Var`, `Param`, and `Expr` broadcast as scalars; the three array types
/// must match the shape exactly.
fn normalize_operand(
    py: Python<'_>,
    owner: &pyo3::Py<Model>,
    obj: &Bound<'_, PyAny>,
    shape: &[usize],
    op: &str,
) -> PyResult<Operand> {
    if let Ok(arr) = obj.cast::<VarArray>() {
        let arr = arr.borrow();
        owners_match(&arr.owner, owner)?;
        if arr.shape.is_empty() {
            // 0-d arrays broadcast as scalars.
            return Ok(Operand::Scalar(affine_of_var(py, owner, arr.vars[0])));
        }
        if arr.shape != shape {
            return Err(ShapeError::new_err(format!(
                "{op}: array shape {:?} does not match {:?}",
                arr.shape, shape
            )));
        }
        return Ok(Operand::Vector(
            arr.shape.clone(),
            arr.vars
                .iter()
                .map(|v| affine_of_var(py, owner, *v))
                .collect(),
        ));
    }
    if let Ok(arr) = obj.cast::<ParamArray>() {
        let arr = arr.borrow();
        owners_match(&arr.owner, owner)?;
        if arr.shape.is_empty() {
            return Ok(Operand::Scalar(Affine {
                owner: owner.clone_ref(py),
                terms: Vec::new(),
                constant: ValueExpr::param(arr.params[0]),
            }));
        }
        if arr.shape != shape {
            return Err(ShapeError::new_err(format!(
                "{op}: array shape {:?} does not match {:?}",
                arr.shape, shape
            )));
        }
        return Ok(Operand::Vector(
            arr.shape.clone(),
            arr.params
                .iter()
                .map(|p| Affine {
                    owner: owner.clone_ref(py),
                    terms: Vec::new(),
                    constant: ValueExpr::param(*p),
                })
                .collect(),
        ));
    }
    if let Ok(arr) = obj.cast::<ExprArray>() {
        let arr = arr.borrow();
        owners_match(&arr.owner, owner)?;
        if arr.shape.is_empty() {
            return Ok(Operand::Scalar(arr.materialize(py)[0].clone()));
        }
        if arr.shape != shape {
            return Err(ShapeError::new_err(format!(
                "{op}: array shape {:?} does not match {:?}",
                arr.shape, shape
            )));
        }
        return Ok(Operand::Vector(arr.shape.clone(), arr.materialize(py)));
    }
    // Scalar broadcast: Var, Param, Expr, or a number.
    if let Ok(var) = obj.cast::<Var>() {
        let var = var.borrow();
        owners_match(&var.owner, owner)?;
        return Ok(Operand::Scalar(affine_of_var(py, owner, var.id)));
    }
    if let Ok(param) = obj.cast::<Param>() {
        let param = param.borrow();
        owners_match(&param.owner, owner)?;
        return Ok(Operand::Scalar(Affine {
            owner: owner.clone_ref(py),
            terms: Vec::new(),
            constant: ValueExpr::param(param.id),
        }));
    }
    if let Ok(expr) = obj.cast::<super::expressions::Expr>() {
        let expr = expr.borrow();
        owners_match(&expr.inner.owner_ref(py), owner)?;
        return Ok(Operand::Scalar(expr.inner.materialize(py)));
    }
    if obj.is_instance_of::<PyBool>() {
        return Err(InvalidModelError::new_err(format!(
            "{op}: bools are not accepted as numeric values"
        )));
    }
    if let Ok(v) = obj.extract::<f64>() {
        if !v.is_finite() {
            return Err(InvalidModelError::new_err(format!(
                "{op}: operand must be finite"
            )));
        }
        return Ok(Operand::Scalar(Affine {
            owner: owner.clone_ref(py),
            terms: Vec::new(),
            constant: ValueExpr::constant(v),
        }));
    }
    // Dense numeric bounds (NumPy or nested sequences) with matching shape.
    let parsed = parse_numeric(py, obj, NumericMode::Finite, op)?;
    if parsed.shape != shape {
        return Err(ShapeError::new_err(format!(
            "{op}: bound shape {:?} does not match array shape {:?}",
            parsed.shape, shape
        )));
    }
    Ok(Operand::Vector(
        parsed.shape,
        parsed
            .values
            .into_iter()
            .map(|v| Affine {
                owner: owner.clone_ref(py),
                terms: Vec::new(),
                constant: ValueExpr::constant(v),
            })
            .collect(),
    ))
}

/// Expand a scalar-or-vector operand pair into per-element affine pairs.
fn expand_pair(
    own_affines: Vec<Affine>,
    other: Operand,
    op: &str,
) -> PyResult<Vec<(Affine, Affine)>> {
    match other {
        Operand::Scalar(s) => Ok(own_affines.into_iter().map(|a| (a, s.clone())).collect()),
        Operand::Vector(shape, vec) => {
            if vec.len() != own_affines.len() {
                return Err(ShapeError::new_err(format!("{op}: array shapes differ")));
            }
            let _ = shape;
            Ok(own_affines.into_iter().zip(vec).collect())
        }
    }
}

/// Elementwise binary operator on two affines.
fn apply_binary(
    py: Python<'_>,
    owner: &pyo3::Py<Model>,
    a: &Affine,
    b: &Affine,
    op: char,
) -> PyResult<Affine> {
    match op {
        '+' | '-' => {
            let sign = if op == '+' { 1.0 } else { -1.0 };
            Ok(fold_affines(py, owner, &[(a, 1.0), (b, sign)]))
        }
        '*' => {
            if !a.terms.is_empty() && !b.terms.is_empty() {
                return Err(super::errors::UnsupportedExpressionError::new_err(
                    "variable-times-variable products are not supported: this interface is LP/MILP only",
                ));
            }
            if a.terms.is_empty() {
                // Parameter-only (or constant) a scales b. Simplify so
                // degenerate `1.0 * p` folds collapse to lone parameters.
                let mut terms = Vec::with_capacity(b.terms.len());
                for term in &b.terms {
                    terms.push(ExprTerm {
                        var: term.var,
                        coeff: simplify_value(term.coeff.clone() * a.constant.clone()),
                    });
                }
                return Ok(Affine {
                    owner: owner.clone_ref(py),
                    terms,
                    constant: simplify_value(b.constant.clone() * a.constant.clone()),
                });
            }
            let mut terms = Vec::with_capacity(a.terms.len());
            for term in &a.terms {
                terms.push(ExprTerm {
                    var: term.var,
                    coeff: simplify_value(term.coeff.clone() * b.constant.clone()),
                });
            }
            Ok(Affine {
                owner: owner.clone_ref(py),
                terms,
                constant: simplify_value(a.constant.clone() * b.constant.clone()),
            })
        }
        _ => Err(ShapeError::new_err("unknown array operator")),
    }
}

/// Elementwise engine: both sides as affine vectors (other broadcast or
/// matched by `expand_pair`), combined per element in Rust.
fn elementwise(
    py: Python<'_>,
    owner: &pyo3::Py<Model>,
    shape: Vec<usize>,
    own: Vec<Affine>,
    other: Bound<'_, PyAny>,
    op: char,
    opname: &str,
) -> PyResult<ExprArray> {
    let other_norm = normalize_operand(py, owner, &other, &shape, opname)?;
    let pairs = expand_pair(own, other_norm, opname)?;
    let mut exprs = Vec::with_capacity(pairs.len());
    for (a, b) in &pairs {
        exprs.push(apply_binary(py, owner, a, b, op)?);
    }
    Ok(ExprArray {
        owner: owner.clone_ref(py),
        shape,
        repr: ExprArrayRepr::Materialized(exprs),
    })
}

fn compare_elementwise(
    py: Python<'_>,
    owner: &pyo3::Py<Model>,
    shape: Vec<usize>,
    own: Vec<Affine>,
    other: Bound<'_, PyAny>,
    sense: BoundSide,
    opname: &str,
) -> PyResult<ComparisonArray> {
    let other_norm = normalize_operand(py, owner, &other, &shape, opname)?;
    let pairs = expand_pair(own, other_norm, opname)?;
    let mut items = Vec::with_capacity(pairs.len());
    for (a, b) in &pairs {
        // Move rhs to the left so every item is (affine, zero-bound side).
        let diff = apply_binary(py, owner, a, b, '-')?;
        let side = match &sense {
            BoundSide::Upper(_) => BoundSide::Upper(ValueExpr::constant(0.0)),
            BoundSide::Lower(_) => BoundSide::Lower(ValueExpr::constant(0.0)),
            BoundSide::Eq(_) => BoundSide::Eq(ValueExpr::constant(0.0)),
        };
        items.push((diff, side));
    }
    Ok(ComparisonArray {
        owner: owner.clone_ref(py),
        shape,
        repr: ComparisonArrayRepr::Materialized(items),
    })
}

fn numeric_scalar(other: &Bound<'_, PyAny>, op: &str) -> PyResult<f64> {
    if other.is_instance_of::<PyBool>() {
        return Err(InvalidModelError::new_err(format!(
            "{op}: bools are not accepted as numeric values"
        )));
    }
    let v: f64 = other
        .extract()
        .map_err(|_| InvalidModelError::new_err(format!("{op}: expected a real number")))?;
    if !v.is_finite() {
        return Err(InvalidModelError::new_err(format!(
            "{op}: operand must be finite"
        )));
    }
    Ok(v)
}

/// Plain finite number (not bool, not array): broadcastable scalar operand
/// for packed detection. Anything else yields `None` so the general path
/// owns all error behavior (bools, non-finite, junk, arrays).
fn packed_scalar_number(obj: &Bound<'_, PyAny>) -> Option<f64> {
    use pyo3::types::PyBool;
    if obj.is_instance_of::<PyBool>() || is_numpy_array(obj) {
        return None;
    }
    match obj.extract::<f64>() {
        Ok(v) if v.is_finite() => Some(v),
        _ => None,
    }
}

/// Dense finite-numeric operand with exactly `shape`, for packed scaling.
/// Anything else (including shape mismatches, which the general path
/// rejects with `ShapeError`) yields `None`.
fn packed_dense_number(
    py: Python<'_>,
    obj: &Bound<'_, PyAny>,
    shape: &[usize],
    op: &str,
) -> PyResult<Option<Vec<f64>>> {
    if !is_numpy_array(obj) && obj.cast::<PySequence>().is_err() {
        return Ok(None);
    }
    let parsed = parse_numeric(py, obj, NumericMode::Finite, op)?;
    if parsed.shape == shape {
        Ok(Some(parsed.values))
    } else {
        Ok(None)
    }
}

fn var_view(
    py: Python<'_>,
    owner: &pyo3::Py<Model>,
    base: &str,
    shape: Vec<usize>,
    vars: Vec<VarId>,
    ordinals: Option<Vec<usize>>,
) -> VarArray {
    VarArray {
        owner: owner.clone_ref(py),
        shape,
        vars,
        base_name: base.to_string(),
        ordinals,
    }
}

/// Packed fast path for `VarArray` arithmetic (P1C-1): same-shape
/// `VarArray` or packed-`ExprArray` operands for `+`/`-`, finite numeric
/// scalars, and dense numerics for scaling. Returns `None` when the general
/// (which owns all error behavior: shape mismatches, nonlinear products,
/// bools, parameters, mixed expressions).
fn packed_vararray_op(
    py: Python<'_>,
    slf: &Bound<'_, VarArray>,
    other: &Bound<'_, PyAny>,
    op: char,
    opname: &str,
) -> PyResult<Option<ExprArray>> {
    let borrowed = slf.borrow();
    let owner = borrowed.owner.clone_ref(py);
    let shape = borrowed.shape.clone();
    let make = |array: PackedLinearArray| {
        Ok(Some(ExprArray {
            owner: owner.clone_ref(py),
            shape: shape.clone(),
            repr: ExprArrayRepr::Packed(array),
        }))
    };
    // Same-shape variable array: the only array/array case that stays packed
    // for `+`/`-` (anything else, including products, runs generally).
    if let Ok(rhs) = other.cast::<VarArray>() {
        let rhs = rhs.borrow();
        owners_match(&rhs.owner, &owner)?;
        if rhs.shape == shape && (op == '+' || op == '-') {
            let mut array = PackedLinearArray::from_vars(borrowed.vars.clone(), shape.clone());
            let sign = if op == '+' { 1.0 } else { -1.0 };
            array.combine(
                &PackedLinearArray::from_vars(rhs.vars.clone(), rhs.shape.clone()),
                sign,
            );
            drop(rhs);
            drop(borrowed);
            return make(array);
        }
        return Ok(None);
    }
    // Packed expression vector of identical shape for `+`/`-` (the
    // symmetric case of ExprArray ± VarArray handled in packed_expr_op).
    if let Ok(arr) = other.cast::<ExprArray>() {
        let arr = arr.borrow();
        owners_match(&arr.owner, &owner)?;
        if let ExprArrayRepr::Packed(rhs) = &arr.repr {
            if rhs.shape == shape && (op == '+' || op == '-') {
                let mut array = PackedLinearArray::from_vars(borrowed.vars.clone(), shape.clone());
                array.combine(rhs, if op == '+' { 1.0 } else { -1.0 });
                drop(arr);
                drop(borrowed);
                return make(array);
            }
        }
        return Ok(None);
    }
    if let Some(v) = packed_scalar_number(other) {
        let mut array = PackedLinearArray::from_vars(borrowed.vars.clone(), shape.clone());
        match op {
            '+' => array.add_scalar(v),
            '-' => array.add_scalar(-v),
            '*' => array.scale(v),
            '/' => {
                if v == 0.0 {
                    return Err(InvalidModelError::new_err("division by zero"));
                }
                array.scale(1.0 / v);
            }
            _ => return Ok(None),
        }
        drop(borrowed);
        return make(array);
    }
    if op == '*' {
        if let Some(values) = packed_dense_number(py, other, &shape, opname)? {
            let mut array = PackedLinearArray::from_vars(borrowed.vars.clone(), shape.clone());
            array.scale_dense(&values);
            drop(borrowed);
            return make(array);
        }
    }
    Ok(None)
}

/// Packed fast path for `scalar - VarArray` / `array - VarArray` reversal.
fn packed_vararray_rsub(
    py: Python<'_>,
    slf: &Bound<'_, VarArray>,
    other: &Bound<'_, PyAny>,
) -> PyResult<Option<ExprArray>> {
    if let Some(v) = packed_scalar_number(other) {
        let borrowed = slf.borrow();
        let owner = borrowed.owner.clone_ref(py);
        let shape = borrowed.shape.clone();
        let mut array = PackedLinearArray::from_vars(borrowed.vars.clone(), shape.clone());
        drop(borrowed);
        array.scale(-1.0);
        array.add_scalar(v);
        return Ok(Some(ExprArray {
            owner,
            shape,
            repr: ExprArrayRepr::Packed(array),
        }));
    }
    Ok(None)
}

#[pymethods]
impl VarArray {
    #[getter]
    fn shape<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        shape_tuple(py, &self.shape)
    }

    fn __len__(&self) -> usize {
        self.shape.first().copied().unwrap_or(1)
    }

    fn __repr__(&self) -> String {
        format!("VarArray({:?}, shape={:?})", self.base_name, self.shape)
    }

    fn __bool__(&self) -> PyResult<bool> {
        Err(pyo3::exceptions::PyTypeError::new_err(
            "the truth value of an array is ambiguous; pass comparisons to m.add(...)",
        ))
    }

    fn __hash__(&self) -> PyResult<isize> {
        Err(pyo3::exceptions::PyTypeError::new_err(
            "arrays are unhashable",
        ))
    }

    fn __getitem__(slf: &Bound<'_, Self>, index: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let py = index.py();
        let borrowed = slf.borrow();
        let (flat, result_shape, scalar) = normalize_index(&borrowed.shape, &index)?;
        if scalar {
            let id = borrowed.vars[flat[0]];
            // Root ordinal, not the view-relative offset: `x[2:7][0]` is
            // `x[2]`. Roots carry no side vector (`None` = identity).
            let ordinal = borrowed
                .ordinals
                .as_ref()
                .map(|o| o[flat[0]])
                .unwrap_or(flat[0]);
            let var = Var {
                owner: borrowed.owner.clone_ref(py),
                id,
                name: element_name(&borrowed.base_name, ordinal),
            };
            Ok(var.into_pyobject(py)?.into_any().unbind())
        } else {
            let vars = flat.iter().map(|f| borrowed.vars[*f]).collect();
            let ordinals = Some(match &borrowed.ordinals {
                Some(o) => flat.iter().map(|f| o[*f]).collect(),
                None => flat.clone(),
            });
            let view = var_view(
                py,
                &borrowed.owner,
                &borrowed.base_name,
                result_shape,
                vars,
                ordinals,
            );
            Ok(view.into_pyobject(py)?.into_any().unbind())
        }
    }

    fn __neg__(slf: &Bound<'_, Self>) -> PyResult<ExprArray> {
        let py = slf.py();
        let borrowed = slf.borrow();
        let mut array = PackedLinearArray::from_vars(borrowed.vars.clone(), borrowed.shape.clone());
        let owner = borrowed.owner.clone_ref(py);
        let shape = borrowed.shape.clone();
        drop(borrowed);
        array.scale(-1.0);
        Ok(ExprArray {
            owner,
            shape,
            repr: ExprArrayRepr::Packed(array),
        })
    }

    fn __add__(slf: &Bound<'_, Self>, other: Bound<'_, PyAny>) -> PyResult<ExprArray> {
        let py = slf.py();
        if let Some(out) = packed_vararray_op(py, slf, &other, '+', "addition")? {
            return Ok(out);
        }
        let borrowed = slf.borrow();
        let own: Vec<Affine> = borrowed
            .vars
            .iter()
            .map(|v| affine_of_var(py, &borrowed.owner, *v))
            .collect();
        let shape = borrowed.shape.clone();
        let owner = borrowed.owner.clone_ref(py);
        drop(borrowed);
        elementwise(py, &owner, shape, own, other, '+', "addition")
    }

    fn __radd__(slf: &Bound<'_, Self>, other: Bound<'_, PyAny>) -> PyResult<ExprArray> {
        Self::__add__(slf, other)
    }

    fn __sub__(slf: &Bound<'_, Self>, other: Bound<'_, PyAny>) -> PyResult<ExprArray> {
        let py = slf.py();
        if let Some(out) = packed_vararray_op(py, slf, &other, '-', "subtraction")? {
            return Ok(out);
        }
        let borrowed = slf.borrow();
        let own: Vec<Affine> = borrowed
            .vars
            .iter()
            .map(|v| affine_of_var(py, &borrowed.owner, *v))
            .collect();
        let shape = borrowed.shape.clone();
        let owner = borrowed.owner.clone_ref(py);
        drop(borrowed);
        elementwise(py, &owner, shape, own, other, '-', "subtraction")
    }

    fn __rsub__(slf: &Bound<'_, Self>, other: Bound<'_, PyAny>) -> PyResult<ExprArray> {
        let py = slf.py();
        if let Some(out) = packed_vararray_rsub(py, slf, &other)? {
            return Ok(out);
        }
        let borrowed = slf.borrow();
        let own: Vec<Affine> = borrowed
            .vars
            .iter()
            .map(|v| affine_of_var(py, &borrowed.owner, *v))
            .collect();
        let shape = borrowed.shape.clone();
        let owner = borrowed.owner.clone_ref(py);
        drop(borrowed);
        // Scalar/array minus self: negate self, then add the left operand.
        let neg: Vec<Affine> = own
            .iter()
            .map(|a| {
                apply_binary(
                    py,
                    &owner,
                    a,
                    &Affine {
                        owner: owner.clone_ref(py),
                        terms: Vec::new(),
                        constant: ValueExpr::constant(-1.0),
                    },
                    '*',
                )
            })
            .collect::<PyResult<_>>()?;
        let other_norm = normalize_operand(py, &owner, &other, &shape, "subtraction")?;
        let left = match other_norm {
            Operand::Scalar(s) => vec![s; neg.len()],
            Operand::Vector(_, v) => {
                if v.len() != neg.len() {
                    return Err(ShapeError::new_err("subtraction: array shapes differ"));
                }
                v
            }
        };
        let mut exprs = Vec::with_capacity(neg.len());
        for (l, n) in left.iter().zip(neg.iter()) {
            exprs.push(apply_binary(py, &owner, l, n, '+')?);
        }
        Ok(ExprArray {
            owner,
            shape,
            repr: ExprArrayRepr::Materialized(exprs),
        })
    }

    fn __mul__(slf: &Bound<'_, Self>, other: Bound<'_, PyAny>) -> PyResult<ExprArray> {
        let py = slf.py();
        if let Some(out) = packed_vararray_op(py, slf, &other, '*', "multiplication")? {
            return Ok(out);
        }
        let borrowed = slf.borrow();
        let own: Vec<Affine> = borrowed
            .vars
            .iter()
            .map(|v| affine_of_var(py, &borrowed.owner, *v))
            .collect();
        let shape = borrowed.shape.clone();
        let owner = borrowed.owner.clone_ref(py);
        drop(borrowed);
        elementwise(py, &owner, shape, own, other, '*', "multiplication")
    }

    fn __rmul__(slf: &Bound<'_, Self>, other: Bound<'_, PyAny>) -> PyResult<ExprArray> {
        Self::__mul__(slf, other)
    }

    fn __truediv__(slf: &Bound<'_, Self>, other: Bound<'_, PyAny>) -> PyResult<ExprArray> {
        let py = slf.py();
        if let Some(v) = packed_scalar_number(&other) {
            if v == 0.0 {
                return Err(InvalidModelError::new_err("division by zero"));
            }
            let borrowed = slf.borrow();
            let mut array =
                PackedLinearArray::from_vars(borrowed.vars.clone(), borrowed.shape.clone());
            let owner = borrowed.owner.clone_ref(py);
            let shape = borrowed.shape.clone();
            drop(borrowed);
            array.scale(1.0 / v);
            return Ok(ExprArray {
                owner,
                shape,
                repr: ExprArrayRepr::Packed(array),
            });
        }
        let divisor = numeric_scalar(&other, "division")?;
        if divisor == 0.0 {
            return Err(InvalidModelError::new_err("division by zero"));
        }
        let borrowed = slf.borrow();
        let mut exprs = Vec::with_capacity(borrowed.vars.len());
        for v in &borrowed.vars {
            let mut affine = affine_of_var(py, &borrowed.owner, *v);
            affine.terms[0].coeff = ValueExpr::constant(1.0 / divisor);
            exprs.push(affine);
        }
        Ok(ExprArray {
            owner: borrowed.owner.clone_ref(py),
            shape: borrowed.shape.clone(),
            repr: ExprArrayRepr::Materialized(exprs),
        })
    }

    fn __le__(slf: &Bound<'_, Self>, other: Bound<'_, PyAny>) -> PyResult<ComparisonArray> {
        let py = slf.py();
        let borrowed = slf.borrow();
        let shape = borrowed.shape.clone();
        let owner = borrowed.owner.clone_ref(py);
        let lhs = PackedLinearArray::from_vars(borrowed.vars.clone(), shape.clone());
        drop(borrowed);
        if let Some(out) = packed_compare(py, &owner, &shape, lhs, &other, PackedSense::Le)? {
            return Ok(out);
        }
        let borrowed = slf.borrow();
        let own: Vec<Affine> = borrowed
            .vars
            .iter()
            .map(|v| affine_of_var(py, &borrowed.owner, *v))
            .collect();
        drop(borrowed);
        compare_elementwise(
            py,
            &owner,
            shape,
            own,
            other,
            BoundSide::Upper(ValueExpr::constant(0.0)),
            "comparison",
        )
    }

    fn __ge__(slf: &Bound<'_, Self>, other: Bound<'_, PyAny>) -> PyResult<ComparisonArray> {
        let py = slf.py();
        let borrowed = slf.borrow();
        let shape = borrowed.shape.clone();
        let owner = borrowed.owner.clone_ref(py);
        let lhs = PackedLinearArray::from_vars(borrowed.vars.clone(), shape.clone());
        drop(borrowed);
        if let Some(out) = packed_compare(py, &owner, &shape, lhs, &other, PackedSense::Ge)? {
            return Ok(out);
        }
        let borrowed = slf.borrow();
        let own: Vec<Affine> = borrowed
            .vars
            .iter()
            .map(|v| affine_of_var(py, &borrowed.owner, *v))
            .collect();
        drop(borrowed);
        compare_elementwise(
            py,
            &owner,
            shape,
            own,
            other,
            BoundSide::Lower(ValueExpr::constant(0.0)),
            "comparison",
        )
    }

    fn __eq__(slf: &Bound<'_, Self>, other: Bound<'_, PyAny>) -> PyResult<ComparisonArray> {
        let py = slf.py();
        let borrowed = slf.borrow();
        let shape = borrowed.shape.clone();
        let owner = borrowed.owner.clone_ref(py);
        let lhs = PackedLinearArray::from_vars(borrowed.vars.clone(), shape.clone());
        drop(borrowed);
        if let Some(out) = packed_compare(py, &owner, &shape, lhs, &other, PackedSense::Eq)? {
            return Ok(out);
        }
        let borrowed = slf.borrow();
        let own: Vec<Affine> = borrowed
            .vars
            .iter()
            .map(|v| affine_of_var(py, &borrowed.owner, *v))
            .collect();
        drop(borrowed);
        compare_elementwise(
            py,
            &owner,
            shape,
            own,
            other,
            BoundSide::Eq(ValueExpr::constant(0.0)),
            "comparison",
        )
    }
}

/// Shared arithmetic engine: elementwise op over an affine vector.
fn array_binary(
    py: Python<'_>,
    owner: &pyo3::Py<Model>,
    shape: Vec<usize>,
    own: Vec<Affine>,
    other: Bound<'_, PyAny>,
    op: char,
    opname: &str,
) -> PyResult<ExprArray> {
    elementwise(py, owner, shape, own, other, op, opname)
}

/// Packed fast path for array comparisons (P1C-1): both sides must be
/// packed-representable (owned `VarArray`, packed `ExprArray`, or finite
/// numeric scalar) with identical shapes and owners, and the bound side
/// numeric. Returns `None` when the general path must run (which owns all
/// error behavior for shape mismatches, dense/parameter bounds, and mixed
/// operands).
fn packed_compare(
    py: Python<'_>,
    owner: &pyo3::Py<Model>,
    shape: &[usize],
    lhs: PackedLinearArray,
    other: &Bound<'_, PyAny>,
    sense: PackedSense,
) -> PyResult<Option<ComparisonArray>> {
    let wrap = |array: PackedLinearArray, bound: f64| {
        Ok(Some(ComparisonArray {
            owner: owner.clone_ref(py),
            shape: shape.to_vec(),
            repr: ComparisonArrayRepr::Packed(PackedComparison {
                shape: shape.to_vec(),
                array,
                sense,
                bound,
            }),
        }))
    };
    // Numeric scalar bound: the common BESS case.
    if let Some(v) = packed_scalar_number(other) {
        return wrap(lhs, v);
    }
    // Owned variable vector of identical shape.
    if let Ok(arr) = other.cast::<VarArray>() {
        let arr = arr.borrow();
        owners_match(&arr.owner, owner)?;
        if arr.shape.is_empty() || arr.shape != shape {
            // 0-d broadcast and shape mismatches run generally.
            return Ok(None);
        }
        let mut array = lhs;
        array.combine(
            &PackedLinearArray::from_vars(arr.vars.clone(), arr.shape.clone()),
            -1.0,
        );
        drop(arr);
        return wrap(array, 0.0);
    }
    // Packed expression vector of identical shape.
    if let Ok(arr) = other.cast::<ExprArray>() {
        let arr = arr.borrow();
        owners_match(&arr.owner, owner)?;
        if let ExprArrayRepr::Packed(rhs) = &arr.repr {
            if rhs.shape == shape {
                let mut array = lhs;
                array.combine(rhs, -1.0);
                drop(arr);
                return wrap(array, 0.0);
            }
        }
        return Ok(None);
    }
    Ok(None)
}

/// Shared comparison engine.
fn array_compare(
    py: Python<'_>,
    owner: &pyo3::Py<Model>,
    shape: Vec<usize>,
    own: Vec<Affine>,
    other: Bound<'_, PyAny>,
    sense: BoundSide,
) -> PyResult<ComparisonArray> {
    compare_elementwise(py, owner, shape, own, other, sense, "comparison")
}

/// Shared negation engine.
fn array_neg(
    py: Python<'_>,
    owner: &pyo3::Py<Model>,
    shape: Vec<usize>,
    own: Vec<Affine>,
) -> PyResult<ExprArray> {
    let neg = Affine {
        owner: owner.clone_ref(py),
        terms: Vec::new(),
        constant: ValueExpr::constant(-1.0),
    };
    let pairs = expand_pair(own, Operand::Scalar(neg), "negation")?;
    let mut exprs = Vec::with_capacity(pairs.len());
    for (a, b) in &pairs {
        exprs.push(apply_binary(py, owner, a, b, '*')?);
    }
    Ok(ExprArray {
        owner: owner.clone_ref(py),
        shape,
        repr: ExprArrayRepr::Materialized(exprs),
    })
}

/// Shared reversed-subtraction engine: scalar/array minus self.
fn array_rsub(
    py: Python<'_>,
    owner: &pyo3::Py<Model>,
    shape: Vec<usize>,
    own: Vec<Affine>,
    other: Bound<'_, PyAny>,
) -> PyResult<ExprArray> {
    let negated = array_neg(py, owner, shape.clone(), own)?;
    let other_norm = normalize_operand(py, owner, &other, &shape, "subtraction")?;
    let left = match other_norm {
        Operand::Scalar(s) => vec![s; negated.numel()],
        Operand::Vector(_, v) => {
            if v.len() != negated.numel() {
                return Err(ShapeError::new_err("subtraction: array shapes differ"));
            }
            v
        }
    };
    let negated_exprs = negated.materialize(py);
    let mut exprs = Vec::with_capacity(negated_exprs.len());
    for (l, n) in left.iter().zip(negated_exprs.iter()) {
        exprs.push(apply_binary(py, owner, l, n, '+')?);
    }
    Ok(ExprArray {
        owner: owner.clone_ref(py),
        shape,
        repr: ExprArrayRepr::Materialized(exprs),
    })
}

/// Packed fast path for `ExprArray` arithmetic (P1C-1). The receiver is
/// already packed with `shape`/`owner`; returns `None` when the general
/// path must run (which owns all error behavior for shape mismatches,
/// nonlinear products, bools, and parameterized/mixed operands).
fn packed_expr_op(
    py: Python<'_>,
    owner: &pyo3::Py<Model>,
    shape: &[usize],
    packed: &PackedLinearArray,
    other: &Bound<'_, PyAny>,
    op: char,
    opname: &str,
) -> PyResult<Option<ExprArray>> {
    let wrap = |array: PackedLinearArray| {
        Ok(Some(ExprArray {
            owner: owner.clone_ref(py),
            shape: shape.to_vec(),
            repr: ExprArrayRepr::Packed(array),
        }))
    };
    if let Ok(arr) = other.cast::<VarArray>() {
        let arr = arr.borrow();
        owners_match(&arr.owner, owner)?;
        // Same-shape variable vectors combine as unit terms for `+`/`-`
        // (0-d broadcast and products run generally).
        if arr.shape == shape && (op == '+' || op == '-') {
            let mut array = packed.clone();
            array.combine(
                &PackedLinearArray::from_vars(arr.vars.clone(), arr.shape.clone()),
                if op == '+' { 1.0 } else { -1.0 },
            );
            drop(arr);
            return wrap(array);
        }
        return Ok(None);
    }
    if let Ok(arr) = other.cast::<ExprArray>() {
        let arr = arr.borrow();
        owners_match(&arr.owner, owner)?;
        if let ExprArrayRepr::Packed(rhs) = &arr.repr {
            if rhs.shape == shape && (op == '+' || op == '-') {
                let mut array = packed.clone();
                array.combine(rhs, if op == '+' { 1.0 } else { -1.0 });
                drop(arr);
                return wrap(array);
            }
        }
        return Ok(None);
    }
    if let Some(v) = packed_scalar_number(other) {
        let mut array = packed.clone();
        match op {
            '+' => array.add_scalar(v),
            '-' => array.add_scalar(-v),
            '*' => array.scale(v),
            '/' => {
                if v == 0.0 {
                    return Err(InvalidModelError::new_err("division by zero"));
                }
                array.scale(1.0 / v);
            }
            _ => return Ok(None),
        }
        return wrap(array);
    }
    if op == '*' {
        if let Some(values) = packed_dense_number(py, other, shape, opname)? {
            // A nonzero scalar constant would densify under elementwise
            // scaling; those rare cases run generally.
            if packed.constant != 0.0 {
                return Ok(None);
            }
            let mut array = packed.clone();
            array.scale_dense(&values);
            return wrap(array);
        }
    }
    Ok(None)
}

/// Packed fast path for `scalar - ExprArray`.
fn packed_expr_rsub(
    py: Python<'_>,
    owner: &pyo3::Py<Model>,
    shape: &[usize],
    packed: &PackedLinearArray,
    other: &Bound<'_, PyAny>,
) -> PyResult<Option<ExprArray>> {
    if let Some(v) = packed_scalar_number(other) {
        let mut array = packed.clone();
        array.scale(-1.0);
        array.add_scalar(v);
        return Ok(Some(ExprArray {
            owner: owner.clone_ref(py),
            shape: shape.to_vec(),
            repr: ExprArrayRepr::Packed(array),
        }));
    }
    Ok(None)
}

/// Shared division-by-constant engine.
fn array_div(
    py: Python<'_>,
    owner: &pyo3::Py<Model>,
    shape: Vec<usize>,
    mut own: Vec<Affine>,
    other: Bound<'_, PyAny>,
) -> PyResult<ExprArray> {
    let divisor = numeric_scalar(&other, "division")?;
    if divisor == 0.0 {
        return Err(InvalidModelError::new_err("division by zero"));
    }
    let factor = 1.0 / divisor;
    for affine in &mut own {
        for term in &mut affine.terms {
            term.coeff = term.coeff.clone() * factor;
        }
        affine.constant = affine.constant.clone() * factor;
    }
    Ok(ExprArray {
        owner: owner.clone_ref(py),
        shape,
        repr: ExprArrayRepr::Materialized(own),
    })
}

fn param_affines(py: Python<'_>, owner: &pyo3::Py<Model>, params: &[roml::ParamId]) -> Vec<Affine> {
    params
        .iter()
        .map(|p| Affine {
            owner: owner.clone_ref(py),
            terms: Vec::new(),
            constant: ValueExpr::param(*p),
        })
        .collect()
}

#[pymethods]
impl ParamArray {
    #[getter]
    fn shape<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        shape_tuple(py, &self.shape)
    }

    fn __len__(&self) -> usize {
        self.shape.first().copied().unwrap_or(1)
    }

    fn __repr__(&self) -> String {
        format!("ParamArray({:?}, shape={:?})", self.base_name, self.shape)
    }

    fn __bool__(&self) -> PyResult<bool> {
        Err(pyo3::exceptions::PyTypeError::new_err(
            "the truth value of an array is ambiguous; pass comparisons to m.add(...)",
        ))
    }

    fn __hash__(&self) -> PyResult<isize> {
        Err(pyo3::exceptions::PyTypeError::new_err(
            "arrays are unhashable",
        ))
    }

    fn __getitem__(slf: &Bound<'_, Self>, index: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let py = index.py();
        let borrowed = slf.borrow();
        let (flat, result_shape, scalar) = normalize_index(&borrowed.shape, &index)?;
        if scalar {
            let id = borrowed.params[flat[0]];
            let param = super::handles::Param {
                owner: borrowed.owner.clone_ref(py),
                id,
                name: element_name(&borrowed.base_name, flat[0]),
            };
            Ok(param.into_pyobject(py)?.into_any().unbind())
        } else {
            let params = flat.iter().map(|f| borrowed.params[*f]).collect();
            let view = ParamArray {
                owner: borrowed.owner.clone_ref(py),
                shape: result_shape,
                params,
                base_name: borrowed.base_name.clone(),
            };
            Ok(view.into_pyobject(py)?.into_any().unbind())
        }
    }

    fn __neg__(slf: &Bound<'_, Self>) -> PyResult<ExprArray> {
        let py = slf.py();
        let borrowed = slf.borrow();
        let own = param_affines(py, &borrowed.owner, &borrowed.params);
        let shape = borrowed.shape.clone();
        let owner = borrowed.owner.clone_ref(py);
        drop(borrowed);
        array_neg(py, &owner, shape, own)
    }

    fn __add__(slf: &Bound<'_, Self>, other: Bound<'_, PyAny>) -> PyResult<ExprArray> {
        let py = slf.py();
        let borrowed = slf.borrow();
        let own = param_affines(py, &borrowed.owner, &borrowed.params);
        let shape = borrowed.shape.clone();
        let owner = borrowed.owner.clone_ref(py);
        drop(borrowed);
        array_binary(py, &owner, shape, own, other, '+', "addition")
    }

    fn __radd__(slf: &Bound<'_, Self>, other: Bound<'_, PyAny>) -> PyResult<ExprArray> {
        Self::__add__(slf, other)
    }

    fn __sub__(slf: &Bound<'_, Self>, other: Bound<'_, PyAny>) -> PyResult<ExprArray> {
        let py = slf.py();
        let borrowed = slf.borrow();
        let own = param_affines(py, &borrowed.owner, &borrowed.params);
        let shape = borrowed.shape.clone();
        let owner = borrowed.owner.clone_ref(py);
        drop(borrowed);
        array_binary(py, &owner, shape, own, other, '-', "subtraction")
    }

    fn __rsub__(slf: &Bound<'_, Self>, other: Bound<'_, PyAny>) -> PyResult<ExprArray> {
        let py = slf.py();
        let borrowed = slf.borrow();
        let own = param_affines(py, &borrowed.owner, &borrowed.params);
        let shape = borrowed.shape.clone();
        let owner = borrowed.owner.clone_ref(py);
        drop(borrowed);
        array_rsub(py, &owner, shape, own, other)
    }

    fn __mul__(slf: &Bound<'_, Self>, other: Bound<'_, PyAny>) -> PyResult<ExprArray> {
        let py = slf.py();
        let borrowed = slf.borrow();
        let own = param_affines(py, &borrowed.owner, &borrowed.params);
        let shape = borrowed.shape.clone();
        let owner = borrowed.owner.clone_ref(py);
        drop(borrowed);
        array_binary(py, &owner, shape, own, other, '*', "multiplication")
    }

    fn __rmul__(slf: &Bound<'_, Self>, other: Bound<'_, PyAny>) -> PyResult<ExprArray> {
        Self::__mul__(slf, other)
    }

    fn __truediv__(slf: &Bound<'_, Self>, other: Bound<'_, PyAny>) -> PyResult<ExprArray> {
        let py = slf.py();
        let borrowed = slf.borrow();
        let own = param_affines(py, &borrowed.owner, &borrowed.params);
        let shape = borrowed.shape.clone();
        let owner = borrowed.owner.clone_ref(py);
        drop(borrowed);
        array_div(py, &owner, shape, own, other)
    }

    fn __le__(slf: &Bound<'_, Self>, other: Bound<'_, PyAny>) -> PyResult<ComparisonArray> {
        let py = slf.py();
        let borrowed = slf.borrow();
        let own = param_affines(py, &borrowed.owner, &borrowed.params);
        let shape = borrowed.shape.clone();
        let owner = borrowed.owner.clone_ref(py);
        drop(borrowed);
        array_compare(
            py,
            &owner,
            shape,
            own,
            other,
            BoundSide::Upper(ValueExpr::constant(0.0)),
        )
    }

    fn __ge__(slf: &Bound<'_, Self>, other: Bound<'_, PyAny>) -> PyResult<ComparisonArray> {
        let py = slf.py();
        let borrowed = slf.borrow();
        let own = param_affines(py, &borrowed.owner, &borrowed.params);
        let shape = borrowed.shape.clone();
        let owner = borrowed.owner.clone_ref(py);
        drop(borrowed);
        array_compare(
            py,
            &owner,
            shape,
            own,
            other,
            BoundSide::Lower(ValueExpr::constant(0.0)),
        )
    }

    fn __eq__(slf: &Bound<'_, Self>, other: Bound<'_, PyAny>) -> PyResult<ComparisonArray> {
        let py = slf.py();
        let borrowed = slf.borrow();
        let own = param_affines(py, &borrowed.owner, &borrowed.params);
        let shape = borrowed.shape.clone();
        let owner = borrowed.owner.clone_ref(py);
        drop(borrowed);
        array_compare(
            py,
            &owner,
            shape,
            own,
            other,
            BoundSide::Eq(ValueExpr::constant(0.0)),
        )
    }
}

#[pymethods]
impl ExprArray {
    #[getter]
    fn shape<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        shape_tuple(py, &self.shape)
    }

    fn __len__(&self) -> usize {
        self.shape.first().copied().unwrap_or(1)
    }

    fn __repr__(&self) -> String {
        format!("ExprArray(shape={:?})", self.shape)
    }

    fn __bool__(&self) -> PyResult<bool> {
        Err(pyo3::exceptions::PyTypeError::new_err(
            "the truth value of an array is ambiguous; pass comparisons to m.add(...)",
        ))
    }

    fn __hash__(&self) -> PyResult<isize> {
        Err(pyo3::exceptions::PyTypeError::new_err(
            "arrays are unhashable",
        ))
    }

    fn __getitem__(slf: &Bound<'_, Self>, index: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let py = index.py();
        let borrowed = slf.borrow();
        let (flat, result_shape, scalar) = normalize_index(&borrowed.shape, &index)?;
        if scalar {
            let expr = super::expressions::Expr {
                inner: super::expressions::Scalar::Lazy(super::expressions::Lazy::flat(
                    borrowed.owner.clone_ref(py),
                    borrowed.materialize(py)[flat[0]].clone().into(),
                )),
            };
            Ok(expr.into_pyobject(py)?.into_any().unbind())
        } else {
            let repr = match &borrowed.repr {
                ExprArrayRepr::Packed(packed) => {
                    ExprArrayRepr::Packed(packed.select(&flat, result_shape.clone()))
                }
                ExprArrayRepr::Materialized(exprs) => {
                    ExprArrayRepr::Materialized(flat.iter().map(|f| exprs[*f].clone()).collect())
                }
            };
            let view = ExprArray {
                owner: borrowed.owner.clone_ref(py),
                shape: result_shape,
                repr,
            };
            Ok(view.into_pyobject(py)?.into_any().unbind())
        }
    }

    fn __neg__(slf: &Bound<'_, Self>) -> PyResult<ExprArray> {
        let py = slf.py();
        let borrowed = slf.borrow();
        let shape = borrowed.shape.clone();
        let owner = borrowed.owner.clone_ref(py);
        if let ExprArrayRepr::Packed(packed) = &borrowed.repr {
            let mut array = packed.clone();
            drop(borrowed);
            array.scale(-1.0);
            return Ok(ExprArray {
                owner,
                shape,
                repr: ExprArrayRepr::Packed(array),
            });
        }
        let own = borrowed.materialize(py);
        drop(borrowed);
        array_neg(py, &owner, shape, own)
    }

    fn __add__(slf: &Bound<'_, Self>, other: Bound<'_, PyAny>) -> PyResult<ExprArray> {
        let py = slf.py();
        let borrowed = slf.borrow();
        let shape = borrowed.shape.clone();
        let owner = borrowed.owner.clone_ref(py);
        if let ExprArrayRepr::Packed(packed) = &borrowed.repr {
            let packed = packed.clone();
            drop(borrowed);
            if let Some(out) = packed_expr_op(py, &owner, &shape, &packed, &other, '+', "addition")?
            {
                return Ok(out);
            }
            let own = packed.materialize(py, &owner);
            return array_binary(py, &owner, shape, own, other, '+', "addition");
        }
        let own = borrowed.materialize(py);
        drop(borrowed);
        array_binary(py, &owner, shape, own, other, '+', "addition")
    }

    fn __radd__(slf: &Bound<'_, Self>, other: Bound<'_, PyAny>) -> PyResult<ExprArray> {
        Self::__add__(slf, other)
    }

    fn __sub__(slf: &Bound<'_, Self>, other: Bound<'_, PyAny>) -> PyResult<ExprArray> {
        let py = slf.py();
        let borrowed = slf.borrow();
        let shape = borrowed.shape.clone();
        let owner = borrowed.owner.clone_ref(py);
        if let ExprArrayRepr::Packed(packed) = &borrowed.repr {
            let packed = packed.clone();
            drop(borrowed);
            if let Some(out) =
                packed_expr_op(py, &owner, &shape, &packed, &other, '-', "subtraction")?
            {
                return Ok(out);
            }
            let own = packed.materialize(py, &owner);
            return array_binary(py, &owner, shape, own, other, '-', "subtraction");
        }
        let own = borrowed.materialize(py);
        drop(borrowed);
        array_binary(py, &owner, shape, own, other, '-', "subtraction")
    }

    fn __rsub__(slf: &Bound<'_, Self>, other: Bound<'_, PyAny>) -> PyResult<ExprArray> {
        let py = slf.py();
        let borrowed = slf.borrow();
        let shape = borrowed.shape.clone();
        let owner = borrowed.owner.clone_ref(py);
        if let ExprArrayRepr::Packed(packed) = &borrowed.repr {
            let packed = packed.clone();
            drop(borrowed);
            if let Some(out) = packed_expr_rsub(py, &owner, &shape, &packed, &other)? {
                return Ok(out);
            }
            let own = packed.materialize(py, &owner);
            return array_rsub(py, &owner, shape, own, other);
        }
        let own = borrowed.materialize(py);
        drop(borrowed);
        array_rsub(py, &owner, shape, own, other)
    }

    fn __mul__(slf: &Bound<'_, Self>, other: Bound<'_, PyAny>) -> PyResult<ExprArray> {
        let py = slf.py();
        let borrowed = slf.borrow();
        let shape = borrowed.shape.clone();
        let owner = borrowed.owner.clone_ref(py);
        if let ExprArrayRepr::Packed(packed) = &borrowed.repr {
            let packed = packed.clone();
            drop(borrowed);
            if let Some(out) =
                packed_expr_op(py, &owner, &shape, &packed, &other, '*', "multiplication")?
            {
                return Ok(out);
            }
            let own = packed.materialize(py, &owner);
            return array_binary(py, &owner, shape, own, other, '*', "multiplication");
        }
        let own = borrowed.materialize(py);
        drop(borrowed);
        array_binary(py, &owner, shape, own, other, '*', "multiplication")
    }

    fn __rmul__(slf: &Bound<'_, Self>, other: Bound<'_, PyAny>) -> PyResult<ExprArray> {
        Self::__mul__(slf, other)
    }

    fn __truediv__(slf: &Bound<'_, Self>, other: Bound<'_, PyAny>) -> PyResult<ExprArray> {
        let py = slf.py();
        let borrowed = slf.borrow();
        let shape = borrowed.shape.clone();
        let owner = borrowed.owner.clone_ref(py);
        if let ExprArrayRepr::Packed(packed) = &borrowed.repr {
            let packed = packed.clone();
            drop(borrowed);
            if let Some(v) = packed_scalar_number(&other) {
                if v == 0.0 {
                    return Err(InvalidModelError::new_err("division by zero"));
                }
                let mut array = packed;
                array.scale(1.0 / v);
                return Ok(ExprArray {
                    owner,
                    shape,
                    repr: ExprArrayRepr::Packed(array),
                });
            }
            let own = packed.materialize(py, &owner);
            return array_div(py, &owner, shape, own, other);
        }
        let own = borrowed.materialize(py);
        drop(borrowed);
        array_div(py, &owner, shape, own, other)
    }

    fn __le__(slf: &Bound<'_, Self>, other: Bound<'_, PyAny>) -> PyResult<ComparisonArray> {
        let py = slf.py();
        let borrowed = slf.borrow();
        let shape = borrowed.shape.clone();
        let owner = borrowed.owner.clone_ref(py);
        if let ExprArrayRepr::Packed(packed) = &borrowed.repr {
            let packed = packed.clone();
            drop(borrowed);
            if let Some(out) = packed_compare(py, &owner, &shape, packed, &other, PackedSense::Le)?
            {
                return Ok(out);
            }
            let slf2 = slf.borrow();
            let own = slf2.materialize(py);
            drop(slf2);
            return array_compare(
                py,
                &owner,
                shape,
                own,
                other,
                BoundSide::Upper(ValueExpr::constant(0.0)),
            );
        }
        let own = borrowed.materialize(py);
        drop(borrowed);
        array_compare(
            py,
            &owner,
            shape,
            own,
            other,
            BoundSide::Upper(ValueExpr::constant(0.0)),
        )
    }

    fn __ge__(slf: &Bound<'_, Self>, other: Bound<'_, PyAny>) -> PyResult<ComparisonArray> {
        let py = slf.py();
        let borrowed = slf.borrow();
        let shape = borrowed.shape.clone();
        let owner = borrowed.owner.clone_ref(py);
        if let ExprArrayRepr::Packed(packed) = &borrowed.repr {
            let packed = packed.clone();
            drop(borrowed);
            if let Some(out) = packed_compare(py, &owner, &shape, packed, &other, PackedSense::Ge)?
            {
                return Ok(out);
            }
            let slf2 = slf.borrow();
            let own = slf2.materialize(py);
            drop(slf2);
            return array_compare(
                py,
                &owner,
                shape,
                own,
                other,
                BoundSide::Lower(ValueExpr::constant(0.0)),
            );
        }
        let own = borrowed.materialize(py);
        drop(borrowed);
        array_compare(
            py,
            &owner,
            shape,
            own,
            other,
            BoundSide::Lower(ValueExpr::constant(0.0)),
        )
    }

    fn __eq__(slf: &Bound<'_, Self>, other: Bound<'_, PyAny>) -> PyResult<ComparisonArray> {
        let py = slf.py();
        let borrowed = slf.borrow();
        let shape = borrowed.shape.clone();
        let owner = borrowed.owner.clone_ref(py);
        if let ExprArrayRepr::Packed(packed) = &borrowed.repr {
            let packed = packed.clone();
            drop(borrowed);
            if let Some(out) = packed_compare(py, &owner, &shape, packed, &other, PackedSense::Eq)?
            {
                return Ok(out);
            }
            let slf2 = slf.borrow();
            let own = slf2.materialize(py);
            drop(slf2);
            return array_compare(
                py,
                &owner,
                shape,
                own,
                other,
                BoundSide::Eq(ValueExpr::constant(0.0)),
            );
        }
        let own = borrowed.materialize(py);
        drop(borrowed);
        array_compare(
            py,
            &owner,
            shape,
            own,
            other,
            BoundSide::Eq(ValueExpr::constant(0.0)),
        )
    }
}

#[pymethods]
impl ComparisonArray {
    #[getter]
    fn shape<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        shape_tuple(py, &self.shape)
    }

    fn __len__(&self) -> usize {
        self.shape.first().copied().unwrap_or(1)
    }

    fn __repr__(&self) -> String {
        format!("ComparisonArray(shape={:?})", self.shape)
    }

    fn __bool__(&self) -> PyResult<bool> {
        Err(pyo3::exceptions::PyTypeError::new_err(
            "symbolic comparisons have no truth value; pass them to m.add(...)",
        ))
    }

    fn __hash__(&self) -> PyResult<isize> {
        Err(pyo3::exceptions::PyTypeError::new_err(
            "comparisons are unhashable",
        ))
    }

    fn __getitem__(slf: &Bound<'_, Self>, index: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let py = index.py();
        let borrowed = slf.borrow();
        let (flat, result_shape, scalar) = normalize_index(&borrowed.shape, &index)?;
        if scalar {
            let (affine, side) = match &borrowed.repr {
                ComparisonArrayRepr::Packed(packed) => {
                    let affines = packed.array.materialize(py, &borrowed.owner);
                    let rhs = match packed.sense {
                        PackedSense::Le => BoundSide::Upper(ValueExpr::constant(packed.bound)),
                        PackedSense::Ge => BoundSide::Lower(ValueExpr::constant(packed.bound)),
                        PackedSense::Eq => BoundSide::Eq(ValueExpr::constant(packed.bound)),
                    };
                    (affines[flat[0]].clone(), rhs)
                }
                ComparisonArrayRepr::Materialized(items) => items[flat[0]].clone(),
            };
            let comp = Comparison {
                owner: borrowed.owner.clone_ref(py),
                expr: super::expressions::ComparisonExpr::General(affine),
                rhs: side,
            };
            Ok(comp.into_pyobject(py)?.into_any().unbind())
        } else {
            let repr = match &borrowed.repr {
                ComparisonArrayRepr::Packed(packed) => {
                    ComparisonArrayRepr::Packed(PackedComparison {
                        shape: result_shape.clone(),
                        array: packed.array.select(&flat, result_shape.clone()),
                        sense: packed.sense,
                        bound: packed.bound,
                    })
                }
                ComparisonArrayRepr::Materialized(items) => ComparisonArrayRepr::Materialized(
                    flat.iter().map(|f| items[*f].clone()).collect(),
                ),
            };
            let view = ComparisonArray {
                owner: borrowed.owner.clone_ref(py),
                shape: result_shape,
                repr,
            };
            Ok(view.into_pyobject(py)?.into_any().unbind())
        }
    }
}

#[pymethods]
impl ConstraintArray {
    #[getter]
    fn shape<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        shape_tuple(py, &self.shape)
    }

    fn __len__(&self) -> usize {
        self.shape.first().copied().unwrap_or(1)
    }

    fn __repr__(&self) -> String {
        format!("ConstraintArray(shape={:?})", self.shape)
    }

    fn __hash__(&self) -> PyResult<isize> {
        Err(pyo3::exceptions::PyTypeError::new_err(
            "arrays are unhashable",
        ))
    }

    fn __getitem__(slf: &Bound<'_, Self>, index: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let py = index.py();
        let borrowed = slf.borrow();
        let (flat, result_shape, scalar) = normalize_index(&borrowed.shape, &index)?;
        if scalar {
            let con = super::handles::Constraint {
                owner: borrowed.owner.clone_ref(py),
                id: borrowed.cons[flat[0]],
                name: None,
            };
            Ok(con.into_pyobject(py)?.into_any().unbind())
        } else {
            let cons = flat.iter().map(|f| borrowed.cons[*f]).collect();
            let view = ConstraintArray {
                owner: borrowed.owner.clone_ref(py),
                shape: result_shape,
                cons,
            };
            Ok(view.into_pyobject(py)?.into_any().unbind())
        }
    }
}

/// Scalar reduction implemented in Rust: sums all elements. Empty
/// reductions are numeric zero. A fully constant result (no variables,
/// no parameters) folds to a Python float.
///
/// P0: summing a nonempty `VarArray` returns a packed vector form instead
/// of a million-term `Affine`, so `minimize`/`maximize` can take the core
/// bulk path with no per-term normalization. All other inputs keep the
/// existing fold.
#[pyfunction]
pub(crate) fn sum(obj: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let py = obj.py();
    if let Ok(arr) = obj.cast::<VarArray>() {
        let arr = arr.borrow();
        if !arr.vars.is_empty() {
            let packed = PackedVars {
                owner: arr.owner.clone_ref(py),
                array: PackedLinearArray::from_vars(arr.vars.clone(), arr.shape.clone()),
            };
            return Ok(super::expressions::Expr {
                inner: Scalar::Packed(packed),
            }
            .into_pyobject(py)?
            .into_any()
            .unbind());
        }
    }
    let (owner, affines) = array_affines(&obj)?;
    let folded = fold_affines(
        py,
        &owner,
        &affines.iter().map(|a| (a, 1.0)).collect::<Vec<_>>(),
    );
    finish_scalar(py, folded)
}

/// A folded scalar result: a Python float when fully constant (no
/// variables and no parameter dependencies), otherwise a symbolic `Expr`.
/// Constants stay symbolic while any parameter can still move them.
fn finish_scalar(py: Python<'_>, folded: Affine) -> PyResult<Py<PyAny>> {
    if folded.terms.is_empty() && folded.constant.dependencies().is_empty() {
        let v = folded.constant.eval(|_| 0.0);
        return Ok(v.into_pyobject(py)?.into_any().unbind());
    }
    Ok(super::expressions::Expr {
        inner: super::expressions::Scalar::Lazy(super::expressions::Lazy::flat(
            folded.owner.clone_ref(py),
            folded.into(),
        )),
    }
    .into_pyobject(py)?
    .into_any()
    .unbind())
}

/// Collect (owner, element affines) from any supported sum/dot operand.
fn array_affines(obj: &Bound<'_, PyAny>) -> PyResult<(pyo3::Py<Model>, Vec<Affine>)> {
    let py = obj.py();
    if let Ok(arr) = obj.cast::<VarArray>() {
        let arr = arr.borrow();
        let owner = arr.owner.clone_ref(py);
        let affines = arr
            .vars
            .iter()
            .map(|v| affine_of_var(py, &owner, *v))
            .collect();
        return Ok((owner, affines));
    }
    if let Ok(arr) = obj.cast::<ParamArray>() {
        let arr = arr.borrow();
        let owner = arr.owner.clone_ref(py);
        let affines = param_affines(py, &owner, &arr.params);
        return Ok((owner, affines));
    }
    if let Ok(arr) = obj.cast::<ExprArray>() {
        let arr = arr.borrow();
        return Ok((arr.owner.clone_ref(py), arr.materialize(py)));
    }
    if let Ok(var) = obj.cast::<Var>() {
        let var = var.borrow();
        let owner = var.owner.clone_ref(py);
        let affine = affine_of_var(py, &owner, var.id);
        return Ok((owner, vec![affine]));
    }
    if let Ok(param) = obj.cast::<super::handles::Param>() {
        let param = param.borrow();
        let owner = param.owner.clone_ref(py);
        return Ok((
            owner.clone_ref(py),
            vec![Affine {
                owner,
                terms: Vec::new(),
                constant: ValueExpr::param(param.id),
            }],
        ));
    }
    if let Ok(expr) = obj.cast::<super::expressions::Expr>() {
        let expr = expr.borrow();
        let inner = expr.inner.materialize(py);
        return Ok((inner.owner.clone_ref(py), vec![inner]));
    }
    Err(InvalidModelError::new_err(
        "sum/dot operands must be arrays, variables, parameters, or expressions",
    ))
}

/// One analyzed dot-product left coefficient (P1C-2 phase 1 keeps the
/// full expression, exactly as the general fold would).
#[derive(Clone, Debug)]
enum DotLeft {
    Num(f64),
    Sym(DotSym),
}

/// A parameter-only coefficient in original form.
#[derive(Clone, Debug)]
enum DotSym {
    /// Bare parameter reference.
    Bare(ParamId),
    /// Original parameter-only expression.
    Expr(ValueExpr),
}

/// Structural dot-product lowering (P1C-2 phase 1).
///
/// The right side normalizes to packed form with no per-element `Affine`
/// objects; the left side analyzes to per-element coefficients with no
/// per-element `ValueExpr` vectors. An all-numeric result stays packed
/// (P0 bulk objective path); a symbolic result builds one `Affine` with
/// direct term pushes (no `HashMap` fold; the balanced-constant tree runs
/// only when the right constant is nonzero, mirroring the general fold).
/// Returns `None` when the general path must run (materialized or empty
/// right side, or a left shape the general dense parser must reject).
/// Every error raised here matches the general path exactly.
fn dot_structural(
    py: Python<'_>,
    coefficients: &Bound<'_, PyAny>,
    expressions: &Bound<'_, PyAny>,
) -> PyResult<Option<Py<PyAny>>> {
    use super::expressions::Expr as PyExpr;
    // Right normalization: owned variable vector or packed array, nonempty.
    let (owner, right): (pyo3::Py<Model>, PackedLinearArray) =
        if let Ok(arr) = expressions.cast::<VarArray>() {
            let arr = arr.borrow();
            if arr.vars.is_empty() {
                return Ok(None);
            }
            (
                arr.owner.clone_ref(py),
                PackedLinearArray::from_vars(arr.vars.clone(), arr.shape.clone()),
            )
        } else if let Ok(arr) = expressions.cast::<ExprArray>() {
            let arr = arr.borrow();
            match &arr.repr {
                ExprArrayRepr::Packed(packed) if packed.numel() > 0 => {
                    (arr.owner.clone_ref(py), packed.clone())
                }
                _ => return Ok(None),
            }
        } else {
            return Ok(None);
        };
    let n = right.numel();
    if coefficients.cast::<VarArray>().is_ok() || coefficients.cast::<Var>().is_ok() {
        return Err(super::errors::UnsupportedExpressionError::new_err(
            "dot coefficients must be numeric or parameter-only; decision-dependent coefficients multiplying decision expressions are nonlinear",
        ));
    }
    // Left analysis (disjoint operand kinds; relative order is free).
    let mut left: Vec<DotLeft> = Vec::with_capacity(n);
    if let Ok(arr) = coefficients.cast::<ParamArray>() {
        let arr = arr.borrow();
        owners_match(&arr.owner, &owner)?;
        if arr.shape != right.shape {
            return Err(ShapeError::new_err(format!(
                "dot: coefficient shape {:?} does not match expression shape {:?}",
                arr.shape, right.shape
            )));
        }
        left.extend(arr.params.iter().map(|p| DotLeft::Sym(DotSym::Bare(*p))));
    } else if let Ok(param) = coefficients.cast::<super::handles::Param>() {
        let param = param.borrow();
        owners_match(&param.owner, &owner)?;
        left.extend(std::iter::repeat_n(DotLeft::Sym(DotSym::Bare(param.id)), n));
    } else if let Ok(arr) = coefficients.cast::<ExprArray>() {
        let arr = arr.borrow();
        owners_match(&arr.owner, &owner)?;
        if arr.shape != right.shape {
            return Err(ShapeError::new_err(format!(
                "dot: coefficient shape {:?} does not match expression shape {:?}",
                arr.shape, right.shape
            )));
        }
        match &arr.repr {
            // Packed left arrays always carry decision variables.
            ExprArrayRepr::Packed(_) => {
                return Err(super::errors::UnsupportedExpressionError::new_err(
                    "dot coefficients must be numeric or parameter-only; decision-dependent coefficients multiplying decision expressions are nonlinear",
                ));
            }
            ExprArrayRepr::Materialized(exprs) => {
                for e in exprs.iter() {
                    if !e.terms.is_empty() {
                        return Err(super::errors::UnsupportedExpressionError::new_err(
                            "dot coefficients must be numeric or parameter-only; decision-dependent coefficients multiplying decision expressions are nonlinear",
                        ));
                    }
                    if e.constant.dependencies().is_empty() {
                        left.push(DotLeft::Num(e.constant.eval(|_| 0.0)));
                    } else {
                        left.push(DotLeft::Sym(DotSym::Expr(e.constant.clone())));
                    }
                }
            }
        }
    } else if let Ok(expr) = coefficients.cast::<PyExpr>() {
        let expr = expr.borrow();
        let inner = expr.inner.materialize(py);
        owners_match(&inner.owner, &owner)?;
        if !inner.terms.is_empty() {
            return Err(super::errors::UnsupportedExpressionError::new_err(
                "dot coefficients must be numeric or parameter-only; decision-dependent coefficients multiplying decision expressions are nonlinear",
            ));
        }
        if inner.constant.dependencies().is_empty() {
            left.extend(std::iter::repeat_n(
                DotLeft::Num(inner.constant.eval(|_| 0.0)),
                n,
            ));
        } else {
            left.extend(std::iter::repeat_n(
                DotLeft::Sym(DotSym::Expr(inner.constant.clone())),
                n,
            ));
        }
    } else if let Some(v) = packed_scalar_number(coefficients) {
        left.extend(std::iter::repeat_n(DotLeft::Num(v), n));
    } else if is_numpy_array(coefficients) || coefficients.cast::<PySequence>().is_ok() {
        let parsed = parse_numeric(py, coefficients, NumericMode::Finite, "dot coefficients")?;
        if parsed.shape != right.shape {
            return Err(ShapeError::new_err(format!(
                "dot: coefficient shape {:?} does not match expression shape {:?}",
                parsed.shape, right.shape
            )));
        }
        left.extend(parsed.values.into_iter().map(DotLeft::Num));
    } else {
        // Anything else (including junk the general dense parser rejects):
        // the general path owns the error.
        return Ok(None);
    }
    debug_assert_eq!(left.len(), n);
    let left_as_expr = |c: &DotLeft| -> ValueExpr {
        match c {
            DotLeft::Num(v) => ValueExpr::constant(*v),
            DotLeft::Sym(DotSym::Bare(p)) => ValueExpr::param(*p),
            DotLeft::Sym(DotSym::Expr(e)) => e.clone(),
        }
    };
    if left.iter().all(|c| matches!(c, DotLeft::Num(_))) {
        // All-numeric result stays packed (P0 bulk objective path).
        let mut result = PackedLinearArray {
            shape: right.shape.clone(),
            terms: Vec::with_capacity(right.terms.len()),
            constant: 0.0,
        };
        for term in &right.terms {
            let mut vals = Vec::with_capacity(n);
            for i in 0..n {
                let rc = match &term.coeffs {
                    PackedCoeffs::One => 1.0,
                    PackedCoeffs::Scalar(c) => *c,
                    PackedCoeffs::Dense(v) => v[i],
                };
                let lc = match &left[i] {
                    DotLeft::Num(v) => *v,
                    DotLeft::Sym(_) => unreachable!(),
                };
                vals.push(rc * lc);
            }
            result.terms.push(PackedArrayTerm {
                vars: term.vars.clone(),
                coeffs: PackedCoeffs::Dense(vals),
            });
        }
        // Mirror the general fold's constant handling exactly (per-element
        // products through the same balanced tree); the common
        // zero-constant case folds to zero with no work and no NaN hazard.
        result.constant = if right.constant == 0.0 {
            0.0
        } else {
            balanced_sum(
                left.iter()
                    .map(|c| match c {
                        DotLeft::Num(v) => {
                            ValueExpr::constant(right.constant) * ValueExpr::constant(*v)
                        }
                        DotLeft::Sym(_) => unreachable!(),
                    })
                    .collect(),
            )
            .eval(|_| 0.0)
        };
        return Ok(Some(
            PyExpr {
                inner: Scalar::Packed(PackedVars {
                    owner: owner.clone_ref(py),
                    array: result,
                }),
            }
            .into_pyobject(py)?
            .into_any()
            .unbind(),
        ));
    }
    // Symbolic result. When every coefficient is trivially representable
    // (bare parameter or scaled parameter), keep the packed-symbolic form:
    // three flat buffers into `set_linear_objective_param_bulk`, no
    // per-term `Affine` expansion. Right-side numeric factors fold into the
    // scales. Anything else (numeric mixes, general parameter expressions)
    // keeps the direct-`Affine` lowering below with identical semantics.
    // The constant mirrors the general fold (`right.constant * left`,
    // balanced); it is parameter-dependent exactly when the scalar path's
    // would be, so `minimize`/`maximize` accept or reject identically.
    let mut sym_vars: Vec<VarId> = Vec::new();
    let mut sym_params: Vec<ParamId> = Vec::new();
    let mut sym_scales: Vec<f64> = Vec::new();
    let mut sym_ok = true;
    for term in &right.terms {
        for (i, var) in term.vars.iter().enumerate() {
            let rc = match &term.coeffs {
                PackedCoeffs::One => 1.0,
                PackedCoeffs::Scalar(c) => *c,
                PackedCoeffs::Dense(v) => v[i],
            };
            let (param, scale) = match &left[i] {
                DotLeft::Sym(DotSym::Bare(p)) => (*p, rc),
                DotLeft::Sym(DotSym::Expr(e)) => match as_scaled_param(e) {
                    Some((p, s)) => (p, rc * s),
                    None => {
                        sym_ok = false;
                        break;
                    }
                },
                DotLeft::Num(_) => {
                    sym_ok = false;
                    break;
                }
            };
            if !scale.is_finite() {
                sym_ok = false;
                break;
            }
            sym_vars.push(*var);
            sym_params.push(param);
            sym_scales.push(scale);
        }
        if !sym_ok {
            break;
        }
    }
    // A zero right factor kills the dependency (the scalar fold simplifies
    // to a numeric); that cell cannot pack.
    if sym_ok && sym_scales.contains(&0.0) {
        sym_ok = false;
    }
    if sym_ok {
        let constant = if right.constant == 0.0 {
            ValueExpr::constant(0.0)
        } else {
            balanced_sum(
                (0..n)
                    .map(|i| ValueExpr::constant(right.constant) * left_as_expr(&left[i]))
                    .collect(),
            )
        };
        return Ok(Some(
            PyExpr {
                inner: Scalar::PackedSymbolic(crate::expressions::PackedSymbolic {
                    owner,
                    vars: sym_vars,
                    params: sym_params,
                    scales: sym_scales,
                    constant,
                }),
            }
            .into_pyobject(py)?
            .into_any()
            .unbind(),
        ));
    }
    // General symbolic result: direct term pushes, no HashMap fold.
    // Coefficient construction mirrors the general fold exactly
    // (`right * left`, simplified per term).
    let mut terms = Vec::with_capacity(n * right.terms.len().max(1));
    for term in &right.terms {
        for (i, var) in term.vars.iter().enumerate() {
            let rc = match &term.coeffs {
                PackedCoeffs::One => 1.0,
                PackedCoeffs::Scalar(c) => *c,
                PackedCoeffs::Dense(v) => v[i],
            };
            terms.push(ExprTerm {
                var: *var,
                coeff: simplify_value(ValueExpr::constant(rc) * left_as_expr(&left[i])),
            });
        }
    }
    let constant = if right.constant == 0.0 {
        ValueExpr::constant(0.0)
    } else {
        balanced_sum(
            (0..n)
                .map(|i| ValueExpr::constant(right.constant) * left_as_expr(&left[i]))
                .collect(),
        )
    };
    Ok(Some(
        PyExpr {
            inner: Scalar::Lazy(crate::expressions::Lazy::flat(
                owner,
                crate::expressions::FlatTerms { terms, constant },
            )),
        }
        .into_pyobject(py)?
        .into_any()
        .unbind(),
    ))
}

/// Scalar dot product of identical-shape arrays in C order: numeric or
/// parameter-only coefficients times a `VarArray` or affine `ExprArray`.
/// Two decision-dependent inputs reject as nonlinear.
#[pyfunction]
pub(crate) fn dot(
    coefficients: Bound<'_, PyAny>,
    expressions: Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    let py = coefficients.py();
    // Owners first: a foreign-model handle on either side is a mismatch,
    // even when the affinity rule would also reject it.
    {
        let left_owner = coefficient_owner(&coefficients)?;
        let right_owner = expression_owner(&expressions)?;
        if let (Some(l), Some(r)) = (left_owner, right_owner) {
            if l.as_ptr() != r.as_ptr() {
                return Err(super::errors::ModelMismatchError::new_err(
                    "dot operands belong to different models",
                ));
            }
        }
    }
    // P0 packed path: numeric coefficients over a nonempty `VarArray`
    // right side bypass affine folding entirely. Anything else
    // (parameters, decision-tailed coefficients, shape errors) falls
    // through to the general path, which owns all error behavior.
    if let Ok(arr) = expressions.cast::<VarArray>() {
        let arr = arr.borrow();
        if !arr.vars.is_empty() {
            if let Some(coeffs) = packed_dot_coefficients(py, &coefficients, &arr.shape)? {
                let packed = PackedVars {
                    owner: arr.owner.clone_ref(py),
                    array: PackedLinearArray {
                        shape: arr.shape.clone(),
                        terms: vec![PackedArrayTerm {
                            vars: arr.vars.clone(),
                            coeffs,
                        }],
                        constant: 0.0,
                    },
                };
                return Ok(super::expressions::Expr {
                    inner: Scalar::Packed(packed),
                }
                .into_pyobject(py)?
                .into_any()
                .unbind());
            }
        }
    }
    // P1C-2 structural path: normalize the right side to packed form
    // without per-element Affines, analyze the left side without
    // per-element ValueExpr vectors, and lower directly. Falls through to
    // the general path below (untouched) whenever the shape is not
    // structural; every error raised here matches the general path.
    if let Some(out) = dot_structural(py, &coefficients, &expressions)? {
        return Ok(out);
    }
    // Right side: decision expressions with a concrete shape.
    let (owner, right_shape, right) = {
        if let Ok(arr) = expressions.cast::<VarArray>() {
            let arr = arr.borrow();
            let owner = arr.owner.clone_ref(py);
            let affines = arr
                .vars
                .iter()
                .map(|v| affine_of_var(py, &owner, *v))
                .collect();
            (owner, arr.shape.clone(), affines)
        } else if let Ok(arr) = expressions.cast::<ExprArray>() {
            let arr = arr.borrow();
            (
                arr.owner.clone_ref(py),
                arr.shape.clone(),
                arr.materialize(py),
            )
        } else {
            return Err(InvalidModelError::new_err(
                "dot expressions must be a VarArray or an affine ExprArray",
            ));
        }
    };
    // Left side: numeric or parameter-only coefficients, identical shape
    // (scalar broadcast of a number/Param/param-expr is also accepted).
    // A decision-bearing coefficient is a nonlinear rejection, not a
    // generic input error.
    let left: Vec<ValueExpr> = if let Ok(arr) = coefficients.cast::<ParamArray>() {
        let arr = arr.borrow();
        owners_match(&arr.owner, &owner)?;
        if arr.shape != right_shape {
            return Err(ShapeError::new_err(format!(
                "dot: coefficient shape {:?} does not match expression shape {:?}",
                arr.shape, right_shape
            )));
        }
        arr.params.iter().map(|p| ValueExpr::param(*p)).collect()
    } else if coefficients.cast::<VarArray>().is_ok() || coefficients.cast::<Var>().is_ok() {
        return Err(super::errors::UnsupportedExpressionError::new_err(
            "dot coefficients must be numeric or parameter-only; decision-dependent coefficients multiplying decision expressions are nonlinear",
        ));
    } else if let Ok(arr) = coefficients.cast::<ExprArray>() {
        let arr = arr.borrow();
        owners_match(&arr.owner, &owner)?;
        if arr.shape != right_shape {
            return Err(ShapeError::new_err(format!(
                "dot: coefficient shape {:?} does not match expression shape {:?}",
                arr.shape, right_shape
            )));
        }
        let arr_exprs = arr.materialize(py);
        let mut out = Vec::with_capacity(arr_exprs.len());
        for e in &arr_exprs {
            if !e.terms.is_empty() {
                return Err(super::errors::UnsupportedExpressionError::new_err(
                    "dot coefficients must be numeric or parameter-only; decision-dependent coefficients multiplying decision expressions are nonlinear",
                ));
            }
            out.push(e.constant.clone());
        }
        out
    } else if let Ok(expr) = coefficients.cast::<super::expressions::Expr>() {
        let expr = expr.borrow();
        let inner = expr.inner.materialize(py);
        owners_match(&inner.owner, &owner)?;
        if !inner.terms.is_empty() {
            return Err(super::errors::UnsupportedExpressionError::new_err(
                "dot coefficients must be numeric or parameter-only; decision-dependent coefficients multiplying decision expressions are nonlinear",
            ));
        }
        let c = inner.constant.clone();
        vec![c; right.len()]
    } else if let Ok(param) = coefficients.cast::<super::handles::Param>() {
        let param = param.borrow();
        owners_match(&param.owner, &owner)?;
        vec![ValueExpr::param(param.id); right.len()]
    } else if is_scalar_number(&coefficients)? {
        let c = scalar_coefficient(py, &owner, &coefficients)?;
        vec![c; right.len()]
    } else {
        // Dense numeric array input (NumPy or nested sequences).
        let parsed = parse_numeric(py, &coefficients, NumericMode::Finite, "dot coefficients")?;
        if parsed.shape != right_shape {
            return Err(ShapeError::new_err(format!(
                "dot: coefficient shape {:?} does not match expression shape {:?}",
                parsed.shape, right_shape
            )));
        }
        parsed.values.into_iter().map(ValueExpr::constant).collect()
    };
    // Fold pairwise in Rust: no Python element loop. Constants combine
    // through a balanced tree (logarithmic depth, not linear nesting).
    let mut terms: std::collections::HashMap<VarId, ValueExpr> = std::collections::HashMap::new();
    let mut consts: Vec<ValueExpr> = Vec::with_capacity(left.len());
    for (c, affine) in left.iter().zip(right.iter()) {
        for term in &affine.terms {
            terms
                .entry(term.var)
                .and_modify(|e| *e = simplify_value(e.clone() + term.coeff.clone() * c.clone()))
                .or_insert_with(|| simplify_value(term.coeff.clone() * c.clone()));
        }
        consts.push(affine.constant.clone() * c.clone());
    }
    let constant = balanced_sum(consts);
    let mut terms: Vec<ExprTerm> = terms
        .into_iter()
        .map(|(var, coeff)| ExprTerm { var, coeff })
        .collect();
    terms.sort_by_key(|t| t.var);
    let folded = Affine {
        owner,
        terms,
        constant,
    };
    finish_scalar(py, folded)
}

/// Packed-coefficient extraction for the `dot` fast path.
///
/// Returns `Some` only for finite-numeric left operands whose shape matches
/// `right_shape` (scalar broadcast or dense array); every other left
/// operand — parameters, decision-bearing handles, bools, shape mismatches —
/// yields `None` so the general path (which owns all error behavior) runs.
fn packed_dot_coefficients(
    py: Python<'_>,
    coefficients: &Bound<'_, PyAny>,
    right_shape: &[usize],
) -> PyResult<Option<PackedCoeffs>> {
    if is_scalar_number(coefficients)? {
        let v: f64 = coefficients
            .extract()
            .map_err(|_| InvalidModelError::new_err("dot: unsupported coefficient type"))?;
        return Ok(Some(PackedCoeffs::Scalar(v)));
    }
    if is_numpy_array(coefficients) || coefficients.cast::<PySequence>().is_ok() {
        let parsed = parse_numeric(py, coefficients, NumericMode::Finite, "dot coefficients")?;
        if parsed.shape == right_shape {
            return Ok(Some(PackedCoeffs::Dense(parsed.values)));
        }
    }
    Ok(None)
}

/// True for plain numbers (finite): broadcastable scalar coefficients.
/// `Param` and parameter-only `Expr` are handled by the caller before this
/// is consulted; anything else here is a dense array or an input error.
fn is_scalar_number(obj: &Bound<'_, PyAny>) -> PyResult<bool> {
    if is_numpy_array(obj) {
        return Ok(false);
    }
    if obj.is_instance_of::<PyBool>() {
        return Err(InvalidModelError::new_err(
            "dot: bools are not accepted as numeric values",
        ));
    }
    match obj.extract::<f64>() {
        Ok(v) if v.is_finite() => Ok(true),
        Ok(_) => Err(InvalidModelError::new_err(
            "dot: coefficient must be finite",
        )),
        Err(_) => Ok(false),
    }
}

/// Scalar number coefficient as a `ValueExpr`.
fn scalar_coefficient(
    _py: Python<'_>,
    _owner: &pyo3::Py<Model>,
    obj: &Bound<'_, PyAny>,
) -> PyResult<ValueExpr> {
    let v: f64 = obj
        .extract()
        .map_err(|_| InvalidModelError::new_err("dot: unsupported coefficient type"))?;
    Ok(ValueExpr::constant(v))
}

/// Parse a bound argument: a scalar (broadcast) or an exact-shape numeric
/// array. Infinities are allowed (bounds mode); NaN rejects.
/// Parse a bound argument: a scalar (broadcast) or an exact-shape numeric
/// array. Infinities are allowed only on the matching open side
/// (`is_lower`: -inf only; upper: +inf only); NaN always rejects.
/// Array inputs pass through bounds-mode parsing and are then checked
/// per element against the same side rule.
pub(crate) fn parse_bound_array(
    py: Python<'_>,
    obj: &Bound<'_, PyAny>,
    shape: &[usize],
    what: &str,
    is_lower: bool,
) -> PyResult<Vec<f64>> {
    let check_side = |v: f64| -> PyResult<f64> {
        if v.is_nan() {
            return Err(InvalidModelError::new_err(format!(
                "{what} must not be NaN"
            )));
        }
        if !v.is_finite() {
            let ok = if is_lower {
                v == f64::NEG_INFINITY
            } else {
                v == f64::INFINITY
            };
            if !ok {
                return Err(InvalidModelError::new_err(format!(
                    "{what} must be a real number, -inf (lower only), or +inf (upper only)"
                )));
            }
        }
        Ok(v)
    };
    let n = numel(shape);
    // Scalar broadcast: plain numbers only (not arrays, sequences, bools).
    if !is_numpy_array(obj) && obj.cast::<PySequence>().is_err() {
        if obj.is_instance_of::<PyBool>() {
            return Err(InvalidModelError::new_err(format!(
                "{what}: bools are not accepted as bounds"
            )));
        }
        if let Ok(v) = obj.extract::<f64>() {
            return Ok(vec![check_side(v)?; n]);
        }
    }
    let parsed = parse_numeric(py, obj, NumericMode::Bounds, what)?;
    if parsed.shape != shape {
        return Err(ShapeError::new_err(format!(
            "{what}: bound shape {:?} does not match array shape {:?}",
            parsed.shape, shape
        )));
    }
    parsed.values.into_iter().map(check_side).collect()
}

/// True when the operand is one of the array classes.
pub(crate) fn is_array_operand(obj: &Bound<'_, PyAny>) -> bool {
    obj.cast::<VarArray>().is_ok()
        || obj.cast::<ParamArray>().is_ok()
        || obj.cast::<ExprArray>().is_ok()
}

/// Owner of a coefficient-side operand, when it has one (arrays, handles,
/// and parameter-only expressions carry owners; dense numerics do not).
fn coefficient_owner(obj: &Bound<'_, PyAny>) -> PyResult<Option<pyo3::Py<Model>>> {
    let py = obj.py();
    if let Ok(arr) = obj.cast::<VarArray>() {
        return Ok(Some(arr.borrow().owner.clone_ref(py)));
    }
    if let Ok(arr) = obj.cast::<ParamArray>() {
        return Ok(Some(arr.borrow().owner.clone_ref(py)));
    }
    if let Ok(arr) = obj.cast::<ExprArray>() {
        return Ok(Some(arr.borrow().owner.clone_ref(py)));
    }
    if let Ok(var) = obj.cast::<Var>() {
        return Ok(Some(var.borrow().owner.clone_ref(py)));
    }
    if let Ok(param) = obj.cast::<super::handles::Param>() {
        return Ok(Some(param.borrow().owner.clone_ref(py)));
    }
    if let Ok(expr) = obj.cast::<super::expressions::Expr>() {
        return Ok(Some(expr.borrow().inner.owner_ref(py)));
    }
    Ok(None)
}

/// Owner of an expression-side operand, when it has one.
fn expression_owner(obj: &Bound<'_, PyAny>) -> PyResult<Option<pyo3::Py<Model>>> {
    let py = obj.py();
    if let Ok(arr) = obj.cast::<VarArray>() {
        return Ok(Some(arr.borrow().owner.clone_ref(py)));
    }
    if let Ok(arr) = obj.cast::<ExprArray>() {
        return Ok(Some(arr.borrow().owner.clone_ref(py)));
    }
    Ok(None)
}

/// True for NumPy boolean scalars (`np.True_`): they implement `__index__`
/// but must not silently become 0/1 in shapes or indices.
fn is_numpy_bool_scalar(obj: &Bound<'_, PyAny>) -> bool {
    if obj.is_instance_of::<PyBool>() {
        return false;
    }
    obj.getattr("dtype")
        .and_then(|dtype| dtype.getattr("kind"))
        .and_then(|kind| kind.extract::<String>())
        .map(|kind| kind == "b")
        .unwrap_or(false)
}
