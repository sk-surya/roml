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

use super::errors::{InvalidHandleError, InvalidModelError};
use super::expressions::{to_affine, Affine, Comparison};
use super::handles::{Constraint, Objective, Param, Var};

/// A parameter-derived constraint bound: the numeric bound installed in the
/// core plus the symbolic expression it tracks. Re-evaluated on `update`.
#[derive(Clone, Debug)]
pub(crate) struct BoundDep {
    pub con: ConId,
    pub lower: Option<ValueExpr>,
    pub upper: Option<ValueExpr>,
}

/// Stored objective template for parameter-dependent constants: the full
/// lowering template plus the symbolic constant. Coefficients update
/// natively in the core; the constant is re-applied only when it changes.
#[derive(Clone, Debug)]
pub(crate) struct ObjectiveTemplate {
    pub terms: Vec<(VarId, ValueExpr)>,
    pub constant: ValueExpr,
}

pub(crate) struct ModelState {
    pub model: CoreModel,
    pub name: String,
    pub var_names: HashMap<String, VarId>,
    pub param_names: HashMap<String, ParamId>,
    pub con_names: HashMap<String, ConId>,
    pub bound_deps: Vec<BoundDep>,
    pub obj_templates: HashMap<ObjId, ObjectiveTemplate>,
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
                obj_templates: HashMap::new(),
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
        if state.var_names.contains_key(name) || state.param_names.contains_key(name) {
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
        if state.var_names.contains_key(name) || state.param_names.contains_key(name) {
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
        comparison: Bound<'_, Comparison>,
        name: Option<&str>,
    ) -> PyResult<Constraint> {
        if let Some(n) = name {
            if n.is_empty() {
                return Err(InvalidModelError::new_err(
                    "constraint name must be a nonempty string",
                ));
            }
        }
        let comp = comparison.borrow();
        comp.owner_check(slf)?;
        let borrowed = slf.borrow();
        let mut state = borrowed.state.lock().map_err(|_| invalid_state())?;
        if let Some(n) = name {
            if state.con_names.contains_key(n) {
                return Err(InvalidModelError::new_err(format!(
                    "duplicate constraint name {n:?}"
                )));
            }
        }
        // Lower the affine expression: numeric constants fold into bounds
        // (core behavior); parameter-dependent parts become tracked bound
        // expressions evaluated at current values.
        let mut lin = LinExpr::new();
        for term in &comp.expr.terms {
            if state.model.variable_bounds(term.var).is_none() {
                return Err(InvalidHandleError::new_err(
                    "expression references an unknown variable",
                ));
            }
            lin = lin.term(TermCoeff::from(term.coeff.clone()), term.var);
        }
        let const_expr = comp.expr.constant.clone();
        let (lower_expr, upper_expr) = comp.bound_exprs();
        let eval_bound = |e: &ValueExpr| -> Result<(f64, Option<ValueExpr>), ModelError> {
            // Subtract the lhs constant: bound' = bound - const.
            let shifted = e.clone() - const_expr.clone();
            if shifted.dependencies().is_empty() {
                let v = shifted.eval(|_| 0.0);
                if !v.is_finite() {
                    return Err(ModelError::NonFiniteValue("constraint bound"));
                }
                Ok((v, None))
            } else {
                let v = eval_expr(&state, &shifted)?;
                Ok((v, Some(shifted)))
            }
        };
        let (lower_val, lower_sym) = match lower_expr {
            Some(e) => {
                let (v, s) = eval_bound(&e).map_err(map_model_error)?;
                (v, s)
            }
            None => (f64::NEG_INFINITY, None),
        };
        let (upper_val, upper_sym) = match upper_expr {
            Some(e) => {
                let (v, s) = eval_bound(&e).map_err(map_model_error)?;
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
        let spec = ConstraintSpec::new(lin, bounds);
        let con = state.model.add_constraint(spec).map_err(map_model_error)?;
        if lower_sym.is_some() || upper_sym.is_some() {
            state.bound_deps.push(BoundDep {
                con,
                lower: lower_sym,
                upper: upper_sym,
            });
        }
        if let Some(n) = name {
            state.con_names.insert(n.to_string(), con);
        }
        state.pending = true;
        state.py_revision += 1;
        Ok(Constraint {
            owner: slf.clone().unbind(),
            id: con,
            name: name.map(str::to_string),
        })
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
        let mut batch: Vec<(ParamId, f64)> = Vec::new();
        for (key, value) in values.iter() {
            let name: String = key
                .extract()
                .map_err(|_| InvalidModelError::new_err("parameter names must be strings"))?;
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
        // Validate derived objective constants.
        let mut obj_updates: Vec<(ObjId, LinExpr)> = Vec::new();
        for (obj, template) in &state.obj_templates {
            if template.constant.dependencies().is_empty() {
                continue;
            }
            if !template
                .constant
                .dependencies()
                .iter()
                .any(|p| batch.iter().any(|(id, _)| id == p))
            {
                continue;
            }
            let new_const = eval_proposed(&template.constant).map_err(map_model_error)?;
            let mut lin = LinExpr::new();
            for (var, coeff) in &template.terms {
                lin = lin.term(TermCoeff::from(coeff.clone()), *var);
            }
            lin = lin.constant(new_const);
            obj_updates.push((*obj, lin));
        }
        // Phase 2: install. All validation passed; core setters cannot fail
        // on pre-validated live identities and finite values.
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
        for (obj, lin) in obj_updates {
            state
                .model
                .set_objective_expr(obj, lin)
                .map_err(map_model_error)?;
        }
        state.pending = true;
        state.py_revision += 1;
        Ok(())
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
        // Split the constant: numeric part lowers natively; a
        // parameter-dependent part is evaluated now and re-applied on
        // update via the stored template.
        let mut lin = LinExpr::new();
        for term in &e.terms {
            lin = lin.term(TermCoeff::from(term.coeff.clone()), term.var);
        }
        let template = ObjectiveTemplate {
            terms: e.terms.iter().map(|t| (t.var, t.coeff.clone())).collect(),
            constant: e.constant.clone(),
        };
        let const_now = eval_expr(&state, &e.constant).map_err(map_model_error)?;
        lin = lin.constant(const_now);
        let obj = match sense {
            Sense::Minimize => state.model.minimize(lin),
            Sense::Maximize => state.model.maximize(lin),
        }
        .map_err(map_model_error)?;
        if !template.constant.dependencies().is_empty() {
            state.obj_templates.insert(obj, template);
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
