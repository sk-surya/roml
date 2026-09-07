//! Python model owner and atomic mutation boundary (DESIGN §5).
//!
//! One `Model` owns one canonical `roml::Model`, the Python-level namespace
//! registry, parameter-derived bound/objective tracking, and the
//! pending-mutation flag behind a `Mutex`. Handles carry `Py<Model>` owner
//! references plus typed Rust identities; raw integer IDs are never usable
//! authority.

use std::collections::HashMap;
use std::sync::Mutex;

use pyo3::prelude::*;
use pyo3::types::PyDict;
use roml::expr::TermCoeff;
use roml::{
    ConId, ConstraintBounds, ConstraintSpec, LinExpr, Model as CoreModel, ModelError, ObjId,
    ParamId, Sense, ValueExpr, VarId, VarType,
};

use super::errors::{InvalidHandleError, InvalidModelError, ShapeError};
use super::expressions::{simplify_value, to_affine, Affine, Comparison};
use super::handles::{Constraint, Objective, Param, Var};

/// A parameter-derived constraint bound: the numeric bound installed in the
/// core plus the symbolic expression it tracks. Re-evaluated on `update`.
#[derive(Clone, Debug)]
pub(crate) struct BoundDep {
    pub con: ConId,
    pub lower: Option<ValueExpr>,
    pub upper: Option<ValueExpr>,
}

pub(crate) struct ModelState {
    pub model: CoreModel,
    pub name: String,
    pub var_names: HashMap<String, VarId>,
    pub param_names: HashMap<String, ParamId>,
    pub con_names: HashMap<String, ConId>,
    pub bound_deps: Vec<BoundDep>,
    /// Reserved array base names (including empty arrays, which contribute
    /// no element entries). Checked alongside the entity namespaces.
    pub array_names: std::collections::HashSet<String>,
    /// Parameter array base names with their immutable shapes.
    pub param_array_shapes: HashMap<String, Vec<usize>>,
    /// Coefficient templates for derived-overflow pre-validation on update:
    /// every parameter-dependent coefficient the binding lowered, keyed by
    /// target. Coefficients update natively in the core; these copies exist
    /// only to evaluate proposed environments before mutation.
    pub obj_coeffs: HashMap<ObjId, Vec<ValueExpr>>,
    pub con_coeffs: HashMap<ConId, Vec<ValueExpr>>,
    pub pending: bool,
    pub py_revision: u64,
}

/// Reject bools, accept ints/floats, require finiteness.
pub(crate) fn py_numeric(value: &Bound<'_, PyAny>, what: &str) -> PyResult<f64> {
    if value.is_instance_of::<pyo3::types::PyBool>() {
        return Err(InvalidModelError::new_err(format!(
            "{what} must be a real number, got bool"
        )));
    }
    let v: f64 = value
        .extract()
        .map_err(|_| InvalidModelError::new_err(format!("{what} must be a real number")))?;
    if !v.is_finite() {
        return Err(InvalidModelError::new_err(format!(
            "{what} must be finite, got {v}"
        )));
    }
    Ok(v)
}

pub(crate) fn map_model_error(err: ModelError) -> PyErr {
    match err {
        ModelError::VariableNotFound(_)
        | ModelError::ConstraintNotFound(_)
        | ModelError::ObjectiveNotFound(_)
        | ModelError::ParameterNotFound(_)
        | ModelError::CoefficientNotFound(_)
        | ModelError::ConstructNotFound(_) => InvalidHandleError::new_err(err.to_string()),
        ModelError::ContinuousTimesContinuousProduct => {
            super::errors::UnsupportedExpressionError::new_err(err.to_string())
        }
        _ => InvalidModelError::new_err(err.to_string()),
    }
}

/// Evaluate a bound/value expression against current parameter values.
pub(crate) fn eval_expr(state: &ModelState, expr: &ValueExpr) -> Result<f64, ModelError> {
    let values: HashMap<ParamId, f64> = state
        .param_names
        .values()
        .map(|id| (*id, state.model.parameter_value(*id).unwrap_or(0.0)))
        .collect();
    for dep in expr.dependencies() {
        if !values.contains_key(&dep) {
            return Err(ModelError::ParameterNotFound(dep));
        }
    }
    let v = expr.eval(|p| values.get(&p).copied().unwrap_or(0.0));
    if !v.is_finite() {
        return Err(ModelError::NonFiniteValue("evaluated parameter expression"));
    }
    Ok(v)
}

fn invalid_state() -> PyErr {
    InvalidModelError::new_err("model state is invalid")
}

#[pyclass(frozen, name = "Model")]
pub struct Model {
    pub(crate) state: Mutex<ModelState>,
}

#[pymethods]
impl Model {
    #[new]
    #[pyo3(signature = (name = ""))]
    fn new(name: &str) -> PyResult<Self> {
        Ok(Self {
            state: Mutex::new(ModelState {
                model: CoreModel::new(),
                name: name.to_string(),
                var_names: HashMap::new(),
                param_names: HashMap::new(),
                con_names: HashMap::new(),
                bound_deps: Vec::new(),
                array_names: std::collections::HashSet::new(),
                param_array_shapes: HashMap::new(),
                obj_coeffs: HashMap::new(),
                con_coeffs: HashMap::new(),
                pending: true,
                py_revision: 0,
            }),
        })
    }

    #[getter]
    fn name(slf: &Bound<'_, Self>) -> String {
        slf.borrow()
            .state
            .lock()
            .map(|state| state.name.clone())
            .unwrap_or_default()
    }

    fn __repr__(slf: &Bound<'_, Self>) -> String {
        match slf.borrow().state.lock() {
            Ok(state) => format!(
                "Model({} vars, {} params, {} constraints)",
                state.var_names.len(),
                state.param_names.len(),
                state.con_names.len()
            ),
            Err(_) => "Model(<invalid>)".to_string(),
        }
    }

    #[pyo3(signature = (name, *, lb = None, ub = None, kind = "continuous"))]
    fn var(
        slf: &Bound<'_, Self>,
        name: &str,
        lb: Option<Bound<'_, PyAny>>,
        ub: Option<Bound<'_, PyAny>>,
        kind: &str,
    ) -> PyResult<Var> {
        if name.is_empty() {
            return Err(InvalidModelError::new_err(
                "variable name must be a nonempty string",
            ));
        }
        let mut lower = match lb {
            Some(v) => bound_value(&v, "lb", true)?,
            None => 0.0,
        };
        let mut upper = match ub {
            Some(v) => bound_value(&v, "ub", false)?,
            None => f64::INFINITY,
        };
        if lower > upper {
            return Err(InvalidModelError::new_err(format!(
                "empty domain: lb {lower} > ub {upper}"
            )));
        }
        let var_type = match kind {
            "continuous" => VarType::Continuous,
            "integer" => VarType::Integer,
            "binary" => VarType::Binary,
            _ => {
                return Err(InvalidModelError::new_err(format!(
                    "kind must be continuous, integer, or binary, got {kind:?}"
                )))
            }
        };
        if kind == "binary" {
            // Effective bounds intersect [0, 1]: omitted bounds produce the
            // standard binary domain; an empty intersection rejects.
            lower = lower.max(0.0);
            upper = upper.min(1.0);
            if lower > upper {
                return Err(InvalidModelError::new_err(
                    "binary bounds are disjoint from [0, 1]",
                ));
            }
        }
        let borrowed = slf.borrow();
        let mut state = borrowed.state.lock().map_err(|_| invalid_state())?;
        if state.var_names.contains_key(name)
            || state.param_names.contains_key(name)
            || state.array_names.contains(name)
        {
            return Err(InvalidModelError::new_err(format!(
                "duplicate name {name:?}: names are unique across variables and parameters"
            )));
        }
        let def = match var_type {
            VarType::Continuous => roml::continuous().bounds(lower, upper),
            VarType::Integer => roml::integer().bounds(lower, upper),
            VarType::Binary => roml::binary().bounds(lower, upper),
        };
        let id = state.model.add_variable(def).map_err(map_model_error)?;
        state.var_names.insert(name.to_string(), id);
        state.pending = true;
        state.py_revision += 1;
        Ok(Var {
            owner: slf.clone().unbind(),
            id,
            name: name.to_string(),
        })
    }

    fn param(slf: &Bound<'_, Self>, name: &str, value: Bound<'_, PyAny>) -> PyResult<Param> {
        if name.is_empty() {
            return Err(InvalidModelError::new_err(
                "parameter name must be a nonempty string",
            ));
        }
        let v = py_numeric(&value, "parameter value")?;
        let borrowed = slf.borrow();
        let mut state = borrowed.state.lock().map_err(|_| invalid_state())?;
        if state.var_names.contains_key(name)
            || state.param_names.contains_key(name)
            || state.array_names.contains(name)
        {
            return Err(InvalidModelError::new_err(format!(
                "duplicate name {name:?}: names are unique across variables and parameters"
            )));
        }
        let id = state.model.add_parameter(v).map_err(map_model_error)?;
        state.param_names.insert(name.to_string(), id);
        state.pending = true;
        state.py_revision += 1;
        Ok(Param {
            owner: slf.clone().unbind(),
            id,
            name: name.to_string(),
        })
    }

    #[pyo3(signature = (comparison, *, name = None))]
    fn add(
        slf: &Bound<'_, Self>,
        comparison: Bound<'_, PyAny>,
        name: Option<&str>,
    ) -> PyResult<Py<PyAny>> {
        Self::add_any(slf, &comparison, name)
    }

    fn minimize(slf: &Bound<'_, Self>, expr: Bound<'_, PyAny>) -> PyResult<Objective> {
        let affine = to_affine(slf, &expr)?;
        Self::set_objective_impl(slf, affine, Sense::Minimize)
    }

    fn maximize(slf: &Bound<'_, Self>, expr: Bound<'_, PyAny>) -> PyResult<Objective> {
        let affine = to_affine(slf, &expr)?;
        Self::set_objective_impl(slf, affine, Sense::Maximize)
    }

    #[pyo3(signature = (**values))]
    fn update(slf: &Bound<'_, Self>, values: Option<&Bound<'_, PyDict>>) -> PyResult<()> {
        let borrowed = slf.borrow();
        let mut state = borrowed.state.lock().map_err(|_| invalid_state())?;
        let values = values
            .ok_or_else(|| InvalidModelError::new_err("update requires keyword arguments"))?;
        // Phase 1: resolve + validate the entire batch before mutation.
        // Scalar names set one parameter; parameter-array base names take
        // an exact-shape array (no partial updates).
        let mut batch: Vec<(ParamId, f64)> = Vec::new();
        for (key, value) in values.iter() {
            let name: String = key
                .extract()
                .map_err(|_| InvalidModelError::new_err("parameter names must be strings"))?;
            if let Some(shape) = state.param_array_shapes.get(&name).cloned() {
                use super::arrays::{element_name, numel, parse_numeric, NumericMode};
                let parsed = parse_numeric(
                    slf.py(),
                    &value,
                    NumericMode::Finite,
                    &format!("value for {name:?}"),
                )?;
                if parsed.shape != shape {
                    return Err(ShapeError::new_err(format!(
                        "value for {name:?} has shape {:?}, expected array shape {:?}",
                        parsed.shape, shape
                    )));
                }
                debug_assert_eq!(parsed.values.len(), numel(&shape));
                for (i, v) in parsed.values.iter().enumerate() {
                    let ename = element_name(&name, i);
                    let id = *state.param_names.get(&ename).ok_or_else(|| {
                        InvalidModelError::new_err(format!("unknown parameter {ename:?}"))
                    })?;
                    batch.push((id, *v));
                }
                continue;
            }
            if state.var_names.contains_key(&name) || state.array_names.contains(&name) {
                return Err(InvalidModelError::new_err(format!(
                    "unknown parameter {name:?} (only parameters can be updated)"
                )));
            }
            let id = *state
                .param_names
                .get(&name)
                .ok_or_else(|| InvalidModelError::new_err(format!("unknown parameter {name:?}")))?;
            let v = py_numeric(&value, &format!("value for {name:?}"))?;
            batch.push((id, v));
        }
        if batch.is_empty() {
            return Err(InvalidModelError::new_err(
                "update requires at least one parameter",
            ));
        }
        // Proposed environment for derived validation.
        let mut proposed: HashMap<ParamId, f64> = state
            .param_names
            .values()
            .map(|id| (*id, state.model.parameter_value(*id).unwrap_or(0.0)))
            .collect();
        for (id, v) in &batch {
            proposed.insert(*id, *v);
        }
        let eval_proposed = |e: &ValueExpr| -> Result<f64, ModelError> {
            for dep in e.dependencies() {
                if !proposed.contains_key(&dep) {
                    return Err(ModelError::ParameterNotFound(dep));
                }
            }
            let v = e.eval(|p| proposed.get(&p).copied().unwrap_or(0.0));
            if !v.is_finite() {
                return Err(ModelError::NonFiniteValue(
                    "parameter update produces a non-finite derived value",
                ));
            }
            Ok(v)
        };
        // Validate derived bounds for affected constraints.
        let mut bound_updates: Vec<(ConId, f64, f64)> = Vec::new();
        for dep in &state.bound_deps {
            let mut touched = false;
            let mut lower_val: Option<f64> = None;
            let mut upper_val: Option<f64> = None;
            if let Some(e) = &dep.lower {
                if e.dependencies()
                    .iter()
                    .any(|p| batch.iter().any(|(id, _)| id == p))
                {
                    touched = true;
                    lower_val = Some(eval_proposed(e).map_err(map_model_error)?);
                }
            }
            if let Some(e) = &dep.upper {
                if e.dependencies()
                    .iter()
                    .any(|p| batch.iter().any(|(id, _)| id == p))
                {
                    touched = true;
                    upper_val = Some(eval_proposed(e).map_err(map_model_error)?);
                }
            }
            if touched {
                let current = state
                    .model
                    .constraint_bounds(dep.con)
                    .ok_or(ModelError::ConstraintNotFound(dep.con))
                    .map_err(map_model_error)?;
                let lo = lower_val.unwrap_or(current.lower);
                let hi = upper_val.unwrap_or(current.upper);
                if lo > hi {
                    return Err(InvalidModelError::new_err(format!(
                        "update would invert a constraint domain ({lo} > {hi}); batch rejected"
                    )));
                }
                bound_updates.push((dep.con, lo, hi));
            }
        }
        // Validate derived objective/constraint coefficients: a finite
        // update that drives any parameter-dependent coefficient
        // non-finite rejects the whole batch (same discipline as bounds).
        // Only templates touching updated parameters are evaluated.
        let batch_params: Vec<ParamId> = batch.iter().map(|(id, _)| *id).collect();
        for coeffs in state.obj_coeffs.values().chain(state.con_coeffs.values()) {
            for coeff in coeffs {
                if coeff
                    .dependencies()
                    .iter()
                    .any(|p| batch_params.contains(p))
                    && eval_proposed(coeff).is_err()
                {
                    return Err(InvalidModelError::new_err(
                        "update would make a parameter-dependent coefficient non-finite; batch rejected",
                    ));
                }
            }
        }
        // Phase 2: install. All validation passed; core setters cannot fail
        // on pre-validated live identities and finite values. Infallibility
        // reliance (documented, not proven): a residual core failure here
        // would leave a partially installed batch. The setters consulted
        // (`set_parameter`: stale/non-finite inputs; `set_constraint_bounds`:
        // stale identities) have no other documented failure mode for
        // pre-validated inputs, and identities cannot go stale under the
        // held model lock.
        for (id, v) in &batch {
            state
                .model
                .set_parameter(*id, *v)
                .map_err(map_model_error)?;
        }
        for (con, lo, hi) in bound_updates {
            state
                .model
                .set_constraint_bounds(
                    con,
                    ConstraintBounds {
                        lower: lo,
                        upper: hi,
                    },
                )
                .map_err(map_model_error)?;
        }
        state.pending = true;
        state.py_revision += 1;
        Ok(())
    }

    #[pyo3(signature = (name, shape, *, lb = None, ub = None, kind = "continuous"))]
    fn vars(
        slf: &Bound<'_, Self>,
        name: &str,
        shape: Bound<'_, PyAny>,
        lb: Option<Bound<'_, PyAny>>,
        ub: Option<Bound<'_, PyAny>>,
        kind: &str,
    ) -> PyResult<super::arrays::VarArray> {
        use super::arrays::{element_name, numel, parse_bound_array, parse_shape};
        if name.is_empty() {
            return Err(InvalidModelError::new_err(
                "variable array name must be a nonempty string",
            ));
        }
        let shape = parse_shape(&shape)?;
        let n = numel(&shape);
        let var_type = match kind {
            "continuous" => VarType::Continuous,
            "integer" => VarType::Integer,
            "binary" => VarType::Binary,
            _ => {
                return Err(InvalidModelError::new_err(format!(
                    "kind must be continuous, integer, or binary, got {kind:?}"
                )))
            }
        };
        let py = slf.py();
        let lower = match lb {
            Some(v) => parse_bound_array(py, &v, &shape, "lb", true)?,
            None => vec![0.0; n],
        };
        let upper = match ub {
            Some(v) => parse_bound_array(py, &v, &shape, "ub", false)?,
            None => vec![f64::INFINITY; n],
        };
        // Validate every element domain (including binary intersection)
        // before taking the lock or mutating anything: a later-element
        // failure must leave no reserved names and no orphan variables.
        let mut domains: Vec<(f64, f64)> = Vec::with_capacity(n);
        for i in 0..n {
            let (mut lo, mut hi) = (lower[i], upper[i]);
            if kind == "binary" {
                lo = lo.max(0.0);
                hi = hi.min(1.0);
            }
            if lo > hi {
                return Err(InvalidModelError::new_err(format!(
                    "empty domain for element {name}[{i}]: {lo} > {hi}"
                )));
            }
            domains.push((lo, hi));
        }
        let borrowed = slf.borrow();
        let mut state = borrowed.state.lock().map_err(|_| invalid_state())?;
        // Reserve the base name and every element name before mutation.
        if state.var_names.contains_key(name)
            || state.param_names.contains_key(name)
            || state.array_names.contains(name)
        {
            return Err(InvalidModelError::new_err(format!(
                "duplicate name {name:?}: names are unique across variables and parameters"
            )));
        }
        let mut element_ids = Vec::with_capacity(n);
        for i in 0..n {
            let ename = element_name(name, i);
            if state.var_names.contains_key(&ename)
                || state.param_names.contains_key(&ename)
                || state.array_names.contains(&ename)
            {
                return Err(InvalidModelError::new_err(format!(
                    "namespace collision for array element {ename:?}"
                )));
            }
            element_ids.push(ename);
        }
        state.array_names.insert(name.to_string());
        let mut vars = Vec::with_capacity(n);
        // Domains pre-validated above; core insertion cannot fail on them.
        // (A residual internal failure would leave partial state; core
        // setters have no documented failure mode here.)
        for (i, ename) in element_ids.iter().enumerate() {
            let (lo, hi) = domains[i];
            let def = match var_type {
                VarType::Continuous => roml::continuous().bounds(lo, hi),
                VarType::Integer => roml::integer().bounds(lo, hi),
                VarType::Binary => roml::binary().bounds(lo, hi),
            };
            let id = state.model.add_variable(def).map_err(map_model_error)?;
            state.var_names.insert(ename.clone(), id);
            vars.push(id);
        }
        state.pending = true;
        state.py_revision += 1;
        Ok(super::arrays::VarArray {
            owner: slf.clone().unbind(),
            shape,
            vars,
            base_name: name.to_string(),
        })
    }

    /// Shaped parameter array; shape is inferred once and immutable.
    fn params(
        slf: &Bound<'_, Self>,
        name: &str,
        values: Bound<'_, PyAny>,
    ) -> PyResult<super::arrays::ParamArray> {
        use super::arrays::{element_name, numel, parse_numeric, NumericMode};
        if name.is_empty() {
            return Err(InvalidModelError::new_err(
                "parameter array name must be a nonempty string",
            ));
        }
        let py = slf.py();
        let parsed = if !super::arrays::is_numpy_array(&values)
            && values.cast::<pyo3::types::PySequence>().is_err()
        {
            // Python scalar (not bool): a 0-d parameter array.
            if values.is_instance_of::<pyo3::types::PyBool>() {
                return Err(InvalidModelError::new_err(
                    "parameter values: bools are not accepted",
                ));
            }
            match values.extract::<f64>() {
                Ok(v) => {
                    if !v.is_finite() {
                        return Err(InvalidModelError::new_err(
                            "parameter values must be finite",
                        ));
                    }
                    super::arrays::NumericInput {
                        shape: Vec::new(),
                        values: vec![v],
                    }
                }
                Err(_) => {
                    return Err(InvalidModelError::new_err(
                        "parameter values must be a real scalar or an array",
                    ))
                }
            }
        } else {
            parse_numeric(py, &values, NumericMode::Finite, "parameter values")?
        };
        let n = numel(&parsed.shape);
        debug_assert_eq!(parsed.values.len(), n);
        let borrowed = slf.borrow();
        let mut state = borrowed.state.lock().map_err(|_| invalid_state())?;
        if state.var_names.contains_key(name)
            || state.param_names.contains_key(name)
            || state.array_names.contains(name)
        {
            return Err(InvalidModelError::new_err(format!(
                "duplicate name {name:?}: names are unique across variables and parameters"
            )));
        }
        for i in 0..n {
            let ename = element_name(name, i);
            if state.var_names.contains_key(&ename)
                || state.param_names.contains_key(&ename)
                || state.array_names.contains(&ename)
            {
                return Err(InvalidModelError::new_err(format!(
                    "namespace collision for array element {ename:?}"
                )));
            }
        }
        let mut params = Vec::with_capacity(n);
        for (i, v) in parsed.values.iter().enumerate() {
            let id = state.model.add_parameter(*v).map_err(map_model_error)?;
            state.param_names.insert(element_name(name, i), id);
            params.push(id);
        }
        state.array_names.insert(name.to_string());
        state
            .param_array_shapes
            .insert(name.to_string(), parsed.shape.clone());
        state.pending = true;
        state.py_revision += 1;
        Ok(super::arrays::ParamArray {
            owner: slf.clone().unbind(),
            shape: parsed.shape,
            params,
            base_name: name.to_string(),
        })
    }

    #[pyo3(signature = (indptr, indices, data, *, variables, lower, upper, name = None))]
    #[allow(clippy::too_many_arguments)]
    fn add_linear_rows(
        slf: &Bound<'_, Self>,
        indptr: Bound<'_, PyAny>,
        indices: Bound<'_, PyAny>,
        data: Bound<'_, PyAny>,
        variables: Bound<'_, super::arrays::VarArray>,
        lower: Bound<'_, PyAny>,
        upper: Bound<'_, PyAny>,
        name: Option<&str>,
    ) -> PyResult<super::arrays::ConstraintArray> {
        Self::add_linear_rows_impl(slf, indptr, indices, data, variables, lower, upper, name)
    }
}

impl Model {
    fn set_objective_impl(slf: &Bound<'_, Self>, e: Affine, sense: Sense) -> PyResult<Objective> {
        let borrowed = slf.borrow();
        let mut state = borrowed.state.lock().map_err(|_| invalid_state())?;
        for term in &e.terms {
            if state.model.variable_bounds(term.var).is_none() {
                return Err(InvalidHandleError::new_err(
                    "objective references an unknown variable",
                ));
            }
        }
        // The constant must be numeric: parameter-dependent objective
        // constants are rejected explicitly (the core has no replaceable
        // constant cell; coefficients update natively). The check runs on
        // the simplified constant so degenerate `0 * p` folds accept.
        let const_simp = simplify_value(e.constant.clone());
        if !const_simp.dependencies().is_empty() {
            return Err(super::errors::UnsupportedExpressionError::new_err(
                "parameter-dependent objective constants are not supported; move the parameter into a coefficient or a constraint bound",
            ));
        }
        let mut lin = LinExpr::new();
        for term in &e.terms {
            lin = lin.term(
                TermCoeff::from(simplify_value(term.coeff.clone())),
                term.var,
            );
        }
        let const_now = eval_expr(&state, &const_simp).map_err(map_model_error)?;
        lin = lin.constant(const_now);
        let obj = match sense {
            Sense::Minimize => state.model.minimize(lin),
            Sense::Maximize => state.model.maximize(lin),
        }
        .map_err(map_model_error)?;
        let param_coeffs: Vec<ValueExpr> = e
            .terms
            .iter()
            .map(|t| simplify_value(t.coeff.clone()))
            .filter(|c| !c.dependencies().is_empty())
            .collect();
        if !param_coeffs.is_empty() {
            state.obj_coeffs.insert(obj, param_coeffs);
        }
        state.pending = true;
        state.py_revision += 1;
        Ok(Objective {
            owner: slf.clone().unbind(),
            id: obj,
        })
    }
}

/// Bound arguments: numeric with -inf/+inf allowed only on the matching side.
fn bound_value(value: &Bound<'_, PyAny>, what: &str, is_lower: bool) -> PyResult<f64> {
    if value.is_instance_of::<pyo3::types::PyBool>() {
        return Err(InvalidModelError::new_err(format!(
            "{what} must be a real number, got bool"
        )));
    }
    let v: f64 = value
        .extract()
        .map_err(|_| InvalidModelError::new_err(format!("{what} must be a real number")))?;
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
}

impl Model {
    /// Expose scalar and array insertion through the single `add` vocabulary.
    pub(crate) fn add_any(
        slf: &Bound<'_, Self>,
        comparison: &Bound<'_, PyAny>,
        name: Option<&str>,
    ) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        if let Ok(scalar) = comparison.cast::<Comparison>() {
            let borrowed_cmp = scalar.borrow();
            borrowed_cmp.owner_check(slf)?;
            let affine = borrowed_cmp.expr.clone();
            let side = borrowed_cmp.rhs.clone();
            drop(borrowed_cmp);
            if let Some(n) = name {
                if n.is_empty() {
                    return Err(InvalidModelError::new_err(
                        "constraint name must be a nonempty string",
                    ));
                }
            }
            let borrowed = slf.borrow();
            let mut state = borrowed.state.lock().map_err(|_| invalid_state())?;
            if let Some(n) = name {
                if state.con_names.contains_key(n)
                    || state.var_names.contains_key(n)
                    || state.param_names.contains_key(n)
                    || state.array_names.contains(n)
                {
                    return Err(InvalidModelError::new_err(format!(
                        "duplicate constraint name {n:?}"
                    )));
                }
            }
            let con = insert_affine_comparison(&mut state, &affine, &side)?;
            if let Some(n) = name {
                state.con_names.insert(n.to_string(), con);
            }
            state.pending = true;
            state.py_revision += 1;
            let handle = Constraint {
                owner: slf.clone().unbind(),
                id: con,
                name: name.map(str::to_string),
            };
            return Ok(handle.into_pyobject(py)?.into_any().unbind());
        }
        if comparison.cast::<super::arrays::ComparisonArray>().is_ok() {
            let arr = Self::add_array(slf, comparison, name)?;
            return Ok(arr.into_pyobject(py)?.into_any().unbind());
        }
        Err(InvalidModelError::new_err(
            "add expects a comparison (x <= ...) or a comparison array",
        ))
    }

    /// Array insertion: lower every element first (all-or-none), then
    /// commit all rows.
    pub(crate) fn add_array(
        slf: &Bound<'_, Self>,
        comparison: &Bound<'_, PyAny>,
        name: Option<&str>,
    ) -> PyResult<super::arrays::ConstraintArray> {
        use super::arrays::{element_name, ComparisonArray, ConstraintArray};
        if let Some(n) = name {
            if n.is_empty() {
                return Err(InvalidModelError::new_err(
                    "constraint name must be a nonempty string",
                ));
            }
        }
        let arr = comparison.cast::<ComparisonArray>().map_err(|_| {
            InvalidModelError::new_err("add expects a comparison (x <= ...) or a comparison array")
        })?;
        let borrowed_arr = arr.borrow();
        super::handles::check_owner(&borrowed_arr.owner, slf)?;
        let shape = borrowed_arr.shape.clone();
        let items = borrowed_arr.items.clone();
        drop(borrowed_arr);
        let borrowed = slf.borrow();
        let mut state = borrowed.state.lock().map_err(|_| invalid_state())?;
        if let Some(n) = name {
            if state.con_names.contains_key(n)
                || state.var_names.contains_key(n)
                || state.param_names.contains_key(n)
                || state.array_names.contains(n)
            {
                return Err(InvalidModelError::new_err(format!(
                    "duplicate constraint name {n:?}"
                )));
            }
            for i in 0..items.len() {
                let ename = element_name(n, i);
                if state.con_names.contains_key(&ename)
                    || state.var_names.contains_key(&ename)
                    || state.param_names.contains_key(&ename)
                    || state.array_names.contains(&ename)
                {
                    return Err(InvalidModelError::new_err(format!(
                        "namespace collision for constraint {ename:?}"
                    )));
                }
            }
        }
        // Lower everything before inserting anything.
        let mut lowered = Vec::with_capacity(items.len());
        for (affine, side) in &items {
            lowered.push(lower_affine(&state, affine, side)?);
        }
        // Commit all rows, recording bound and coefficient templates.
        let mut cons = Vec::with_capacity(items.len());
        for ((affine, _), (lin, bounds, lower_sym, upper_sym)) in items.iter().zip(lowered) {
            let spec = ConstraintSpec::new(lin, bounds);
            let con = state.model.add_constraint(spec).map_err(map_model_error)?;
            if lower_sym.is_some() || upper_sym.is_some() {
                state.bound_deps.push(BoundDep {
                    con,
                    lower: lower_sym,
                    upper: upper_sym,
                });
            }
            let param_coeffs: Vec<ValueExpr> = affine
                .terms
                .iter()
                .map(|t| simplify_value(t.coeff.clone()))
                .filter(|c| !c.dependencies().is_empty())
                .collect();
            if !param_coeffs.is_empty() {
                state.con_coeffs.insert(con, param_coeffs);
            }
            cons.push(con);
        }
        if let Some(n) = name {
            state.array_names.insert(n.to_string());
            for (i, con) in cons.iter().enumerate() {
                state.con_names.insert(element_name(n, i), *con);
            }
        }
        state.pending = true;
        state.py_revision += 1;
        Ok(ConstraintArray {
            owner: slf.clone().unbind(),
            shape,
            cons,
        })
    }
}

impl Model {
    /// Sparse CSR bulk rows: `variables` is the flattened column map
    /// (a `VarArray` in C order). Every invariant validates before any row
    /// is inserted; duplicate (row, column) entries accumulate
    /// algebraically, never last-write-wins.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn add_linear_rows_impl(
        slf: &Bound<'_, Self>,
        indptr: Bound<'_, PyAny>,
        indices: Bound<'_, PyAny>,
        data: Bound<'_, PyAny>,
        variables: Bound<'_, super::arrays::VarArray>,
        lower: Bound<'_, PyAny>,
        upper: Bound<'_, PyAny>,
        name: Option<&str>,
    ) -> PyResult<super::arrays::ConstraintArray> {
        use super::arrays::{element_name, parse_numeric, ConstraintArray, NumericMode};
        use std::collections::HashMap;
        if let Some(n) = name {
            if n.is_empty() {
                return Err(InvalidModelError::new_err(
                    "constraint name must be a nonempty string",
                ));
            }
        }
        let py = slf.py();
        let varr = variables.borrow();
        super::handles::check_owner(&varr.owner, slf)?;
        let columns: Vec<VarId> = varr.vars.clone();
        let ncols = columns.len();
        drop(varr);
        let indptr = parse_integer_vector(&indptr, "indptr")?;
        let indices = parse_integer_vector(&indices, "indices")?;
        let data = parse_numeric(py, &data, NumericMode::Finite, "data")?;
        if data.shape.len() != 1 {
            return Err(ShapeError::new_err("data must be one-dimensional"));
        }
        if indptr.is_empty() {
            return Err(ShapeError::new_err(
                "indptr must contain at least one entry",
            ));
        }
        if indptr[0] != 0 {
            return Err(ShapeError::new_err("indptr[0] must be 0"));
        }
        for w in indptr.windows(2) {
            if w[1] < w[0] {
                return Err(ShapeError::new_err("indptr must be non-decreasing"));
            }
        }
        let nrows = indptr.len() - 1;
        if *indptr.last().unwrap() as usize != data.values.len()
            || *indptr.last().unwrap() as usize != indices.len()
        {
            return Err(ShapeError::new_err(
                "indptr last entry must equal len(data) and len(indices)",
            ));
        }
        for (k, c) in indices.iter().enumerate() {
            if *c < 0 || *c as usize >= ncols {
                return Err(ShapeError::new_err(format!(
                    "column index {c} at position {k} is out of range for {ncols} variables"
                )));
            }
        }
        let lower_vals = parse_row_bounds(py, &lower, nrows, "lower")?;
        let upper_vals = parse_row_bounds(py, &upper, nrows, "upper")?;
        for i in 0..nrows {
            if lower_vals[i].is_nan() || upper_vals[i].is_nan() {
                return Err(InvalidModelError::new_err("bounds must not be NaN"));
            }
            if !lower_vals[i].is_finite() && lower_vals[i] != f64::NEG_INFINITY {
                return Err(InvalidModelError::new_err(
                    "lower bounds must be finite or -inf",
                ));
            }
            if !upper_vals[i].is_finite() && upper_vals[i] != f64::INFINITY {
                return Err(InvalidModelError::new_err(
                    "upper bounds must be finite or +inf",
                ));
            }
            if lower_vals[i] > upper_vals[i] {
                return Err(InvalidModelError::new_err(format!(
                    "empty row domain at row {i}: {} > {}",
                    lower_vals[i], upper_vals[i]
                )));
            }
        }
        let borrowed = slf.borrow();
        let mut state = borrowed.state.lock().map_err(|_| invalid_state())?;
        if let Some(n) = name {
            if state.con_names.contains_key(n)
                || state.var_names.contains_key(n)
                || state.param_names.contains_key(n)
                || state.array_names.contains(n)
            {
                return Err(InvalidModelError::new_err(format!(
                    "duplicate constraint name {n:?}"
                )));
            }
            for i in 0..nrows {
                let ename = element_name(n, i);
                if state.con_names.contains_key(&ename)
                    || state.var_names.contains_key(&ename)
                    || state.param_names.contains_key(&ename)
                    || state.array_names.contains(&ename)
                {
                    return Err(InvalidModelError::new_err(format!(
                        "namespace collision for constraint {ename:?}"
                    )));
                }
            }
        }
        for c in &indices {
            let var = columns[*c as usize];
            if state.model.variable_bounds(var).is_none() {
                return Err(InvalidHandleError::new_err(
                    "CSR column map references an unknown variable",
                ));
            }
        }
        // Lower all rows (algebraic duplicate accumulation), then commit.
        let mut lowered: Vec<(LinExpr, ConstraintBounds)> = Vec::with_capacity(nrows);
        for i in 0..nrows {
            let (start, end) = (indptr[i] as usize, indptr[i + 1] as usize);
            let mut cells: HashMap<VarId, f64> = HashMap::new();
            for k in start..end {
                let var = columns[indices[k] as usize];
                *cells.entry(var).or_insert(0.0) += data.values[k];
            }
            let mut lin = LinExpr::new();
            let mut order: Vec<VarId> = cells.keys().copied().collect();
            order.sort();
            for var in order {
                let coef = cells[&var];
                if !coef.is_finite() {
                    return Err(InvalidModelError::new_err(format!(
                        "accumulated CSR coefficient at row {i} is not finite"
                    )));
                }
                if coef != 0.0 {
                    lin = lin.term(coef, var);
                }
            }
            lowered.push((
                lin,
                ConstraintBounds {
                    lower: lower_vals[i],
                    upper: upper_vals[i],
                },
            ));
        }
        let mut cons = Vec::with_capacity(nrows);
        for (lin, bounds) in lowered {
            let spec = ConstraintSpec::new(lin, bounds);
            cons.push(state.model.add_constraint(spec).map_err(map_model_error)?);
        }
        if let Some(n) = name {
            state.array_names.insert(n.to_string());
            for (i, con) in cons.iter().enumerate() {
                state.con_names.insert(element_name(n, i), *con);
            }
        }
        state.pending = true;
        state.py_revision += 1;
        Ok(ConstraintArray {
            owner: slf.clone().unbind(),
            shape: vec![nrows],
            cons,
        })
    }
}

/// Strict integer vector parsing for CSR indptr/indices: NumPy integer
/// dtypes or sequences of ints. Bools, floats, and multi-dimensional
/// inputs reject; values must fit in `i64`.
fn parse_integer_vector(obj: &Bound<'_, PyAny>, what: &str) -> PyResult<Vec<i64>> {
    use pyo3::types::{PyBool, PySequence};
    if obj.hasattr("dtype").unwrap_or(false) {
        let kind: String = obj
            .getattr("dtype")?
            .getattr("kind")?
            .extract()
            .map_err(|_| InvalidModelError::new_err(format!("{what}: cannot read dtype")))?;
        if kind != "i" && kind != "u" {
            return Err(InvalidModelError::new_err(format!(
                "{what}: integer dtype required, got kind {kind:?}"
            )));
        }
        let py = obj.py();
        let shape: Vec<usize> = obj
            .getattr("shape")?
            .extract()
            .map_err(|_| InvalidModelError::new_err(format!("{what}: cannot read shape")))?;
        if shape.len() != 1 {
            return Err(ShapeError::new_err(format!(
                "{what}: index arrays must be one-dimensional, got shape {shape:?}"
            )));
        }
        let numpy = py.import("numpy")?;
        let flat = numpy.call_method(
            "ascontiguousarray",
            (obj,),
            Some(&{
                let kwargs = pyo3::types::PyDict::new(py);
                kwargs.set_item("dtype", numpy.getattr("int64")?)?;
                kwargs
            }),
        )?;
        let values: Vec<i64> = flat
            .call_method0("ravel")?
            .extract()
            .map_err(|_| InvalidModelError::new_err(format!("{what}: cannot read data")))?;
        return Ok(values);
    }
    if obj.is_instance_of::<PyBool>() {
        return Err(InvalidModelError::new_err(format!(
            "{what}: bools are not valid indices"
        )));
    }
    let seq = obj
        .cast::<PySequence>()
        .map_err(|_| InvalidModelError::new_err(format!("{what}: expected an integer sequence")))?;
    let mut out = Vec::with_capacity(seq.len()?);
    for i in 0..seq.len()? {
        let item = seq.get_item(i)?;
        if item.is_instance_of::<PyBool>() {
            return Err(InvalidModelError::new_err(format!(
                "{what}: bools are not valid indices"
            )));
        }
        let v: i64 = item
            .extract()
            .map_err(|_| InvalidModelError::new_err(format!("{what}: indices must be integers")))?;
        out.push(v);
    }
    Ok(out)
}

/// Row bounds: a scalar (broadcast) or one value per row.
fn parse_row_bounds(
    py: Python<'_>,
    obj: &Bound<'_, PyAny>,
    nrows: usize,
    what: &str,
) -> PyResult<Vec<f64>> {
    use super::arrays::{parse_numeric, NumericMode};
    use pyo3::types::{PyBool, PySequence};
    if !obj.hasattr("dtype").unwrap_or(false) && obj.cast::<PySequence>().is_err() {
        if obj.is_instance_of::<PyBool>() {
            return Err(InvalidModelError::new_err(format!(
                "{what}: bools are not accepted as bounds"
            )));
        }
        if let Ok(v) = obj.extract::<f64>() {
            return Ok(vec![v; nrows]);
        }
    }
    let parsed = parse_numeric(py, obj, NumericMode::Bounds, what)?;
    if parsed.shape != [nrows] {
        return Err(ShapeError::new_err(format!(
            "{what}: expected a scalar or {nrows} values, got shape {:?}",
            parsed.shape
        )));
    }
    Ok(parsed.values)
}

/// Lower one affine comparison into the core: validate entities, fold
/// numeric constants into bounds, evaluate parameter-dependent bound parts
/// at current values, insert, and record bound dependencies. Shared by
/// scalar insertion (arrays pre-lower every element first, so a later
/// failure leaves all rows unadded).
pub(crate) fn insert_affine_comparison(
    state: &mut ModelState,
    affine: &super::expressions::Affine,
    side: &super::arrays::BoundSide,
) -> PyResult<ConId> {
    let (lin, bounds, lower_sym, upper_sym) = lower_affine(state, affine, side)?;
    let spec = ConstraintSpec::new(lin, bounds);
    let con = state.model.add_constraint(spec).map_err(map_model_error)?;
    if lower_sym.is_some() || upper_sym.is_some() {
        state.bound_deps.push(BoundDep {
            con,
            lower: lower_sym,
            upper: upper_sym,
        });
    }
    let param_coeffs: Vec<ValueExpr> = affine
        .terms
        .iter()
        .map(|t| simplify_value(t.coeff.clone()))
        .filter(|c| !c.dependencies().is_empty())
        .collect();
    if !param_coeffs.is_empty() {
        state.con_coeffs.insert(con, param_coeffs);
    }
    Ok(con)
}

/// Validate and lower one affine comparison without inserting: entity
/// liveness, bound evaluation, and domain checks.
#[allow(clippy::type_complexity)]
fn lower_affine(
    state: &ModelState,
    affine: &super::expressions::Affine,
    side: &super::arrays::BoundSide,
) -> PyResult<(
    LinExpr,
    ConstraintBounds,
    Option<ValueExpr>,
    Option<ValueExpr>,
)> {
    use super::arrays::BoundSide;
    let mut lin = LinExpr::new();
    for term in &affine.terms {
        if state.model.variable_bounds(term.var).is_none() {
            return Err(InvalidHandleError::new_err(
                "expression references an unknown variable",
            ));
        }
        lin = lin.term(
            TermCoeff::from(simplify_value(term.coeff.clone())),
            term.var,
        );
    }
    let const_expr = affine.constant.clone();
    let eval_bound =
        |state: &ModelState, e: &ValueExpr| -> Result<(f64, Option<ValueExpr>), ModelError> {
            let shifted = e.clone() - const_expr.clone();
            if shifted.dependencies().is_empty() {
                let v = shifted.eval(|_| 0.0);
                if !v.is_finite() {
                    return Err(ModelError::NonFiniteValue("constraint bound"));
                }
                Ok((v, None))
            } else {
                let v = eval_expr(state, &shifted)?;
                Ok((v, Some(simplify_value(shifted))))
            }
        };
    let (lower_expr, upper_expr) = match side {
        BoundSide::Upper(u) => (None, Some(u.clone())),
        BoundSide::Lower(l) => (Some(l.clone()), None),
        BoundSide::Eq(e) => (Some(e.clone()), Some(e.clone())),
    };
    let (lower_val, lower_sym) = match lower_expr {
        Some(e) => {
            let (v, s) = eval_bound(state, &e).map_err(map_model_error)?;
            (v, s)
        }
        None => (f64::NEG_INFINITY, None),
    };
    let (upper_val, upper_sym) = match upper_expr {
        Some(e) => {
            let (v, s) = eval_bound(state, &e).map_err(map_model_error)?;
            (v, s)
        }
        None => (f64::INFINITY, None),
    };
    if lower_val > upper_val {
        return Err(InvalidModelError::new_err(format!(
            "empty constraint domain: {lower_val} > {upper_val}"
        )));
    }
    let bounds = ConstraintBounds {
        lower: lower_val,
        upper: upper_val,
    };
    Ok((lin, bounds, lower_sym, upper_sym))
}
