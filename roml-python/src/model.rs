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
use super::expressions::{simplify_value, to_scalar, Comparison, PackedCoeffs, Scalar};
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
    /// Structural variable-array reservations: base name to element count
    /// (P2A). Implicit generated names `base[i]` for `i < len` exist
    /// without eagerly allocated strings; see [`crate::namespace`].
    pub var_array_lens: HashMap<String, usize>,
    /// Occupied generated-looking names that are NOT implicit variable
    /// elements: explicit scalars, eager parameter elements, and array
    /// bases of any kind, keyed by base. Constraint names excluded by
    /// design (variable paths never consulted them).
    pub explicit_indices: crate::namespace::ExplicitIndices,
    pub bound_deps: Vec<BoundDep>,
    /// Reserved array base names (including empty arrays, which contribute
    /// no element entries). Checked alongside the entity namespaces.
    pub array_names: std::collections::HashSet<String>,
    /// Parameter array base names with their immutable shapes.
    pub param_array_shapes: HashMap<String, Vec<usize>>,
    /// Parameter array base names with their element identities in
    /// C order, so batch updates address elements without reformatting
    /// names or re-hashing per element.
    pub param_array_ids: HashMap<String, Vec<ParamId>>,
    /// Coefficient templates for derived-overflow pre-validation on update:
    /// every parameter-dependent coefficient the binding lowered, keyed by
    /// target. Coefficients update natively in the core; these copies exist
    /// only to evaluate proposed environments before mutation.
    pub obj_coeffs: HashMap<ObjId, Vec<ValueExpr>>,
    pub con_coeffs: HashMap<ConId, Vec<ValueExpr>>,
    /// True once any recorded template holds a non-lone-parameter
    /// expression. Guards the lazy proposed-environment build: without
    /// complex dependents, updates skip the full parameter scan.
    pub has_complex_deps: bool,
    /// True once any integer/binary variable exists. Duals and reduced
    /// costs are LP-only diagnostics; on discrete models they raise
    /// `UnavailableDiagnosticError` instead of advertising relaxation
    /// values as economic marginals.
    pub has_discrete: bool,
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

/// Snapshot of committed parameter values for preflight evaluation.
///
/// Built ONCE per lowering call and shared across all coefficient/bound
/// evaluations within it: rebuilding it per term is quadratic in the
/// parameter count (P1C-2 instrumentation). Values cannot change mid-call
/// (the model lock is held throughout), so sharing is exact.
pub(crate) fn param_values(state: &ModelState) -> HashMap<ParamId, f64> {
    state
        .param_names
        .values()
        .map(|id| (*id, state.model.parameter_value(*id).unwrap_or(0.0)))
        .collect()
}

/// Evaluate a value expression against a prebuilt parameter snapshot.
/// Committed values are always fresh: update() commits accepted batches
/// before returning, so no queued-but-uncommitted state can exist outside
/// update/solve internals.
pub(crate) fn eval_expr(
    values: &HashMap<ParamId, f64>,
    expr: &ValueExpr,
) -> Result<f64, ModelError> {
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

/// GIL-independent shared model state. The `Arc` (not the `Py<Model>`
/// wrapper) crosses the PyO3 detach boundary: it is `Send`/`Sync` while
/// carrying no Python object. Locks are always acquired and released
/// inside the detached closure; no guard ever crosses it.
#[derive(Clone)]
pub(crate) struct SharedModel {
    state: std::sync::Arc<Mutex<ModelState>>,
    poison: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

#[pyclass(frozen, name = "Model")]
pub struct Model {
    pub(crate) shared: SharedModel,
}

/// GIL-free lock outcome for detached native work.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ModelLockFail {
    Busy,
    Poisoned,
}

/// Nonblocking model-state acquisition without Python errors (usable with
/// the GIL released). Contention reports `Busy`; poisoning fuses the model
/// and reports `Poisoned`.
pub(crate) fn try_model_state(
    shared: &SharedModel,
) -> Result<std::sync::MutexGuard<'_, ModelState>, ModelLockFail> {
    use std::sync::atomic::Ordering;
    if shared.poison.load(Ordering::SeqCst) {
        return Err(ModelLockFail::Poisoned);
    }
    match shared.state.try_lock() {
        Ok(guard) => Ok(guard),
        Err(std::sync::TryLockError::WouldBlock) => Err(ModelLockFail::Busy),
        Err(std::sync::TryLockError::Poisoned(_)) => {
            shared.poison.store(true, Ordering::SeqCst);
            Err(ModelLockFail::Poisoned)
        }
    }
}

/// Nonblocking model-state acquisition: contention fails deterministically
/// with `ModelBusyError` instead of waiting; poisoning fuses the model.
pub(crate) fn lock_state(model: &Model) -> PyResult<std::sync::MutexGuard<'_, ModelState>> {
    try_model_state(&model.shared).map_err(|fail| match fail {
        ModelLockFail::Busy => {
            super::errors::ModelBusyError::new_err("model is busy with another operation")
        }
        ModelLockFail::Poisoned => {
            InvalidModelError::new_err("model is unusable after an operational failure")
        }
    })
}

#[pymethods]
impl Model {
    #[new]
    #[pyo3(signature = (name = ""))]
    fn new(name: &str) -> PyResult<Self> {
        Ok(Self {
            shared: SharedModel {
                state: std::sync::Arc::new(Mutex::new(ModelState {
                    model: CoreModel::new(),
                    name: name.to_string(),
                    var_names: HashMap::new(),
                    param_names: HashMap::new(),
                    con_names: HashMap::new(),
                    var_array_lens: HashMap::new(),
                    explicit_indices: HashMap::new(),
                    bound_deps: Vec::new(),
                    array_names: std::collections::HashSet::new(),
                    param_array_shapes: HashMap::new(),
                    param_array_ids: HashMap::new(),
                    obj_coeffs: HashMap::new(),
                    con_coeffs: HashMap::new(),
                    has_complex_deps: false,
                    has_discrete: false,
                    pending: true,
                    py_revision: 0,
                })),
                poison: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            },
        })
    }

    #[getter]
    fn name(slf: &Bound<'_, Self>) -> PyResult<String> {
        lock_state(&slf.borrow()).map(|state| state.name.clone())
    }

    fn __repr__(slf: &Bound<'_, Self>) -> String {
        // Nonblocking read: a busy model renders a placeholder instead of
        // blocking the caller (repr must never hang); poison still shows.
        match slf.borrow().shared.state.try_lock() {
            Ok(state) => format!(
                "Model({} vars, {} params, {} constraints)",
                state.variable_count(),
                state.param_names.len(),
                state.con_names.len()
            ),
            Err(std::sync::TryLockError::WouldBlock) => "Model(<busy>)".to_string(),
            Err(std::sync::TryLockError::Poisoned(_)) => "Model(<invalid>)".to_string(),
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
        let mut state = lock_state(&borrowed)?;
        if state.explicit_name_conflicts(name) {
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
        if var_type != VarType::Continuous {
            state.has_discrete = true;
        }
        state.var_names.insert(name.to_string(), id);
        state.index_explicit_name(name);
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
        let mut state = lock_state(&borrowed)?;
        if state.explicit_name_conflicts(name) {
            return Err(InvalidModelError::new_err(format!(
                "duplicate name {name:?}: names are unique across variables and parameters"
            )));
        }
        let id = state.model.add_parameter(v).map_err(map_model_error)?;
        state.param_names.insert(name.to_string(), id);
        state.index_explicit_name(name);
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
        let scalar = to_scalar(slf, &expr)?;
        Self::set_objective_impl(slf, scalar, Sense::Minimize)
    }

    fn maximize(slf: &Bound<'_, Self>, expr: Bound<'_, PyAny>) -> PyResult<Objective> {
        let scalar = to_scalar(slf, &expr)?;
        Self::set_objective_impl(slf, scalar, Sense::Maximize)
    }

    /// Debug-only namespace cardinality probe (P2A evidence).
    ///
    /// Present in debug builds only; release wheels expose no such
    /// surface. Returns `(var reservations, explicit var names,
    /// explicit index entries)`: after `vars("x", 1M)` this reads
    /// `(1, 0, 0)`, i.e. one structural reservation and zero stored
    /// element strings anywhere in the namespace.
    #[cfg(debug_assertions)]
    fn _debug_namespace_counts(slf: &Bound<'_, Self>) -> PyResult<(usize, usize, usize)> {
        let borrowed = slf.borrow();
        let state = lock_state(&borrowed)?;
        Ok((
            state.var_array_lens.len(),
            state.var_names.len(),
            state.explicit_indices.values().map(|s| s.len()).sum(),
        ))
    }

    #[pyo3(signature = (**values))]
    fn update(slf: &Bound<'_, Self>, values: Option<&Bound<'_, PyDict>>) -> PyResult<()> {
        let borrowed = slf.borrow();
        let mut state = lock_state(&borrowed)?;
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
                use super::arrays::{numel, parse_numeric, NumericMode};
                // Zero-dimensional arrays accept a scalar (or 0-d input):
                // there is exactly one element, so scalar broadcast is
                // unambiguous. Anything else keeps exact-shape discipline.
                let parsed = if shape.is_empty() {
                    match scalar_or_zerod(slf.py(), &value, &format!("value for {name:?}"))? {
                        Some(v) => super::arrays::NumericInput {
                            shape: Vec::new(),
                            values: vec![v],
                        },
                        None => parse_numeric(
                            slf.py(),
                            &value,
                            NumericMode::Finite,
                            &format!("value for {name:?}"),
                        )?,
                    }
                } else {
                    parse_numeric(
                        slf.py(),
                        &value,
                        NumericMode::Finite,
                        &format!("value for {name:?}"),
                    )?
                };
                if parsed.shape != shape {
                    return Err(ShapeError::new_err(format!(
                        "value for {name:?} has shape {:?}, expected array shape {:?}",
                        parsed.shape, shape
                    )));
                }
                debug_assert_eq!(parsed.values.len(), numel(&shape));
                let ids = state.param_array_ids.get(&name).ok_or_else(|| {
                    InvalidModelError::new_err(format!("unknown parameter {name:?}"))
                })?;
                debug_assert_eq!(ids.len(), parsed.values.len());
                for (id, v) in ids.iter().zip(parsed.values.iter()) {
                    batch.push((*id, *v));
                }
                continue;
            }
            if state.var_names.contains_key(&name)
                || state.array_names.contains(&name)
                || state.implicit_var_element(&name)
            {
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
        // Proposed environment for derived validation, built lazily:
        // batches touching no tracked bounds or complex coefficients skip
        // the full parameter scan.
        let needs_proposed = !state.bound_deps.is_empty() || state.has_complex_deps;
        let mut proposed: HashMap<ParamId, f64> = HashMap::new();
        if needs_proposed {
            // Base is committed values (fresh: every accepted update
            // commits before returning), overlaid with the proposed batch.
            proposed = state
                .param_names
                .values()
                .map(|id| (*id, state.model.parameter_value(*id).unwrap_or(0.0)))
                .collect();
            for (id, v) in &batch {
                proposed.insert(*id, *v);
            }
        }
        let eval_proposed = |e: &ValueExpr| -> Result<f64, ModelError> {
            // Fast path: lone parameters read straight from the proposed
            // environment (the common coefficient shape).
            if let ValueExpr::Param(p) = e {
                return proposed
                    .get(p)
                    .copied()
                    .ok_or(ModelError::ParameterNotFound(*p));
            }
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
        // Only templates touching updated parameters are evaluated; lone
        // parameters read straight from the proposed environment (their
        // values are finite by input validation, so they cannot overflow).
        let batch_set: std::collections::HashSet<ParamId> =
            batch.iter().map(|(id, _)| *id).collect();
        for coeffs in state.obj_coeffs.values().chain(state.con_coeffs.values()) {
            for coeff in coeffs {
                // Lone parameters equal proposed values, which Phase 1
                // already proved finite: nothing to validate.
                if matches!(coeff, ValueExpr::Param(_)) {
                    continue;
                }
                if coeff.dependencies().iter().any(|p| batch_set.contains(p))
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
        // Commit accepted values immediately: preflight (above) and any
        // later core insertion (add/minimize/solve) then evaluate the
        // SAME parameter state, so a validated batch can never fail at
        // installation time. Commit is infallible for pre-validated
        // finite values on live identities.
        state.model.commit().map_err(map_model_error)?;
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
        use super::arrays::{numel, parse_bound_array, parse_shape};
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
        let mut state = lock_state(&borrowed)?;
        // Reserve the base structurally (P2A): the base itself goes
        // through the unified explicit-name check (exact occupancy or an
        // implicit element of another reservation, e.g. base "x[5]"), and
        // the reverse index answers prospective-element collisions without
        // formatting N strings. All validation runs before any mutation,
        // preserving the old atomicity (no partial reservation, variables,
        // or revision changes on rejection).
        if state.explicit_name_conflicts(name) {
            return Err(InvalidModelError::new_err(format!(
                "duplicate name {name:?}: names are unique across variables and parameters"
            )));
        }
        if let Some(ename) = state.prospective_array_conflicts(name, n) {
            return Err(InvalidModelError::new_err(format!(
                "namespace collision for array element {ename:?}"
            )));
        }
        state.array_names.insert(name.to_string());
        state.index_explicit_name(name);
        state.var_array_lens.insert(name.to_string(), n);
        let mut vars = Vec::with_capacity(n);
        // Domains pre-validated above; core insertion cannot fail on them.
        // (A residual internal failure would leave partial state; core
        // setters have no documented failure mode here.)
        // No element strings are formatted, hashed, or stored: implicit
        // names materialize on demand at handle creation.
        for (lo, hi) in domains.iter().copied() {
            let def = match var_type {
                VarType::Continuous => roml::continuous().bounds(lo, hi),
                VarType::Integer => roml::integer().bounds(lo, hi),
                VarType::Binary => roml::binary().bounds(lo, hi),
            };
            let id = state.model.add_variable(def).map_err(map_model_error)?;
            vars.push(id);
            if var_type != VarType::Continuous {
                state.has_discrete = true;
            }
        }
        state.pending = true;
        state.py_revision += 1;
        Ok(super::arrays::VarArray {
            owner: slf.clone().unbind(),
            shape,
            vars,
            base_name: name.to_string(),
            // Root arrays own the identity mapping: no side vector.
            ordinals: None,
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
        let mut state = lock_state(&borrowed)?;
        if state.explicit_name_conflicts(name) {
            return Err(InvalidModelError::new_err(format!(
                "duplicate name {name:?}: names are unique across variables and parameters"
            )));
        }
        if let Some(ename) = state.prospective_array_conflicts(name, n) {
            return Err(InvalidModelError::new_err(format!(
                "namespace collision for array element {ename:?}"
            )));
        }
        let mut params = Vec::with_capacity(n);
        for (i, v) in parsed.values.iter().enumerate() {
            let id = state.model.add_parameter(*v).map_err(map_model_error)?;
            let ename = element_name(name, i);
            state.param_names.insert(ename.clone(), id);
            // Parameter elements stay eager, but their generated-looking
            // names join the reverse index so prospective variable arrays
            // see them without string scans.
            state.index_explicit_name(&ename);
            params.push(id);
        }
        state.array_names.insert(name.to_string());
        state.index_explicit_name(name);
        state
            .param_array_shapes
            .insert(name.to_string(), parsed.shape.clone());
        state
            .param_array_ids
            .insert(name.to_string(), params.clone());
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
    fn set_objective_impl(slf: &Bound<'_, Self>, e: Scalar, sense: Sense) -> PyResult<Objective> {
        // Packed constant-coefficient vectors bypass the term-by-term
        // lowering entirely and go straight to the core bulk primitive
        // (P0). Everything else keeps the existing general path.
        if let Scalar::Packed(packed) = e {
            return Self::set_objective_packed(slf, packed, sense);
        }
        if let Scalar::PackedSymbolic(sym) = e {
            return Self::set_objective_param_bulk(slf, sym, sense);
        }
        let Scalar::Lazy(lazy) = e else {
            return Err(InvalidModelError::new_err("unsupported objective form"));
        };
        // P1E: classify once into the cheapest primitive. Numeric trees
        // reach the constant bulk primitive, scaled-parameter trees the
        // parametric one; only genuinely general trees pay the general
        // Affine lowering below.
        match lazy.classify(slf.py()) {
            super::expressions::LoweredScalar::Numeric {
                vars,
                coeffs,
                constant,
            } => Self::set_objective_numeric_bulk(slf, sense, &vars, &coeffs, constant),
            super::expressions::LoweredScalar::Parametric {
                vars,
                params,
                scales,
                constant,
            } => Self::set_objective_param_bulk(
                slf,
                super::expressions::PackedSymbolic {
                    owner: slf.clone().unbind(),
                    vars,
                    params,
                    scales,
                    constant,
                },
                sense,
            ),
            super::expressions::LoweredScalar::General(e) => {
                Self::set_objective_general(slf, e, sense)
            }
        }
    }

    /// General `Affine` objective insertion: the fallback for genuinely
    /// symbolic coefficients that admit no bulk primitive. Unchanged
    /// lowering (preflight, LinExpr build, template recording).
    fn set_objective_general(
        slf: &Bound<'_, Self>,
        e: super::expressions::Affine,
        sense: Sense,
    ) -> PyResult<Objective> {
        let borrowed = slf.borrow();
        let mut state = lock_state(&borrowed)?;
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
        let values = param_values(&state);
        let mut lin = LinExpr::new();
        for term in &e.terms {
            let coeff = simplify_value(term.coeff.clone());
            // Preflight mirroring lower_affine: non-finite coefficients
            // fail here, not inside core insertion.
            eval_expr(&values, &coeff).map_err(map_model_error)?;
            lin = lin.term(TermCoeff::from(coeff), term.var);
        }
        let const_now = eval_expr(&values, &const_simp).map_err(map_model_error)?;
        lin = lin.constant(const_now);
        let obj = match sense {
            Sense::Minimize => state.model.minimize(lin),
            Sense::Maximize => state.model.maximize(lin),
        }
        .map_err(map_model_error)?;
        record_obj_coeffs(&mut state, obj, &e.terms);
        state.pending = true;
        state.py_revision += 1;
        Ok(Objective {
            owner: slf.clone().unbind(),
            id: obj,
        })
    }

    /// Packed constant-coefficient objective insertion (P0 bulk path).
    ///
    /// `rm.sum(VarArray)` / `rm.dot(numeric, VarArray)` arrive here without
    /// any per-term `Affine` expansion. Coefficients are finite numerics by
    /// construction; the core re-validates liveness/finiteness/uniqueness
    /// (stale variables surface as `InvalidHandleError` through the shared
    /// error mapping, exactly like the general path's preflight). No
    /// parameter templates exist to record: packed coefficients admit no
    /// parameter dependencies.
    fn set_objective_packed(
        slf: &Bound<'_, Self>,
        packed: super::expressions::PackedVars,
        sense: Sense,
    ) -> PyResult<Objective> {
        use super::expressions::PackedCoeffs;
        if !packed.array.constant.is_finite() {
            return Err(InvalidModelError::new_err(
                "objective constant must be finite",
            ));
        }
        // Flatten term blocks into parallel buffers for the core bulk
        // primitive (single-term inputs stay a single contiguous run).
        let mut vars: Vec<VarId> = Vec::new();
        let mut coeffs: Vec<f64> = Vec::new();
        for term in &packed.array.terms {
            match &term.coeffs {
                PackedCoeffs::One => {
                    vars.extend_from_slice(&term.vars);
                    coeffs.extend(std::iter::repeat_n(1.0, term.vars.len()));
                }
                PackedCoeffs::Scalar(v) => {
                    vars.extend_from_slice(&term.vars);
                    coeffs.extend(std::iter::repeat_n(*v, term.vars.len()));
                }
                PackedCoeffs::Dense(values) => {
                    vars.extend_from_slice(&term.vars);
                    coeffs.extend_from_slice(values);
                }
            }
        }
        Self::set_objective_numeric_bulk(slf, sense, &vars, &coeffs, packed.array.constant)
    }

    /// Numeric bulk objective insertion (P1E classifier path).
    ///
    /// Classified numeric trees arrive here as parallel buffers, straight
    /// into the core constant bulk primitive — no per-term `Affine`
    /// expansion, no `LinExpr` build. Coefficients are finite numerics by
    /// classification; the core re-validates liveness/finiteness/uniqueness
    /// atomically (stale variables surface as `InvalidHandleError` through
    /// the shared error mapping, exactly like the general path's
    /// preflight). No parameter templates exist to record: numeric
    /// coefficients admit no parameter dependencies.
    fn set_objective_numeric_bulk(
        slf: &Bound<'_, Self>,
        sense: Sense,
        vars: &[VarId],
        coeffs: &[f64],
        constant: f64,
    ) -> PyResult<Objective> {
        let borrowed = slf.borrow();
        let mut state = lock_state(&borrowed)?;
        let core_sense = match sense {
            Sense::Minimize => roml::Sense::Minimize,
            Sense::Maximize => roml::Sense::Maximize,
        };
        let obj = state
            .model
            .set_linear_objective_bulk(core_sense, vars, coeffs, constant)
            .map_err(map_model_error)?;
        state.pending = true;
        state.py_revision += 1;
        Ok(Objective {
            owner: slf.clone().unbind(),
            id: obj,
        })
    }

    /// Packed scaled-parameter objective insertion (P1C-2 bulk path).
    ///
    /// `rm.dot` with structurally cheap parameter-only coefficients arrives
    /// here as three flat buffers, straight into the core parametric bulk
    /// primitive — no per-term `Affine` expansion, no `HashMap` fold. The
    /// constant must be numeric (parameter-dependent constants reject with
    /// the exact scalar-path error); scales are finite by construction and
    /// the core re-validates liveness/finiteness atomically. Update-time
    /// derived-coefficient validation sees the same templates the scalar
    /// path would record (canonical scaled-parameter forms), so `update()`
    /// accepts or rejects identically.
    fn set_objective_param_bulk(
        slf: &Bound<'_, Self>,
        sym: super::expressions::PackedSymbolic,
        sense: Sense,
    ) -> PyResult<Objective> {
        // Mirror the scalar path exactly: simplify first so degenerate
        // `0 * p` folds accept, then reject genuine parameter dependence
        // with the identical error.
        let const_simp = simplify_value(sym.constant.clone());
        if !const_simp.dependencies().is_empty() {
            return Err(super::errors::UnsupportedExpressionError::new_err(
                "parameter-dependent objective constants are not supported; move the parameter into a coefficient or a constraint bound",
            ));
        }
        let const_now = const_simp.eval(|_| 0.0);
        if !const_now.is_finite() {
            return Err(InvalidModelError::new_err(
                "objective constant must be finite",
            ));
        }
        let borrowed = slf.borrow();
        let mut state = lock_state(&borrowed)?;
        let core_sense = match sense {
            Sense::Minimize => roml::Sense::Minimize,
            Sense::Maximize => roml::Sense::Maximize,
        };
        let obj = state
            .model
            .set_linear_objective_param_bulk(
                core_sense,
                &sym.vars,
                &sym.params,
                &sym.scales,
                const_now,
            )
            .map_err(map_model_error)?;
        // Same update-validation templates the scalar path records: one
        // canonical scaled-parameter expression per cell (lone parameters
        // keep the `Param` fast path; scaled ones set `has_complex_deps`
        // exactly as simplified scalar coefficients would).
        let terms: Vec<super::expressions::ExprTerm> = sym
            .vars
            .iter()
            .zip(sym.params.iter())
            .zip(sym.scales.iter())
            .map(|((var, param), scale)| super::expressions::ExprTerm {
                var: *var,
                coeff: ValueExpr::scaled_param(*scale, *param),
            })
            .collect();
        record_obj_coeffs(&mut state, obj, &terms);
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
            let body = borrowed_cmp.expr.clone();
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
            let mut state = lock_state(&borrowed)?;
            if let Some(n) = name {
                if state.con_names.contains_key(n)
                    || state.var_names.contains_key(n)
                    || state.param_names.contains_key(n)
                    || state.array_names.contains(n)
                    || state.implicit_var_element(n)
                {
                    return Err(InvalidModelError::new_err(format!(
                        "duplicate constraint name {n:?}"
                    )));
                }
            }
            let con = match body {
                super::expressions::ComparisonExpr::General(affine) => {
                    insert_affine_comparison(&mut state, &affine, &side)?
                }
                super::expressions::ComparisonExpr::BulkRow(row) => {
                    Self::insert_bulk_row(&mut state, &row)?
                }
            };
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
    /// commit all rows. Packed comparisons bypass per-element lowering and
    /// insert through the core bulk primitive in one call.
    pub(crate) fn add_array(
        slf: &Bound<'_, Self>,
        comparison: &Bound<'_, PyAny>,
        name: Option<&str>,
    ) -> PyResult<super::arrays::ConstraintArray> {
        use super::arrays::{
            element_name, numel, ComparisonArray, ComparisonArrayRepr, ConstraintArray,
        };
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
        let nelems = numel(&shape);
        // Clone the packed form out (if any) before taking the model lock.
        let packed = match &borrowed_arr.repr {
            ComparisonArrayRepr::Packed(p) => Some(p.clone()),
            ComparisonArrayRepr::Materialized(_) => None,
        };
        let items = match &borrowed_arr.repr {
            ComparisonArrayRepr::Materialized(items) => Some(items.clone()),
            ComparisonArrayRepr::Packed(_) => None,
        };
        drop(borrowed_arr);
        let borrowed = slf.borrow();
        let mut state = lock_state(&borrowed)?;
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
            for i in 0..nelems {
                let ename = element_name(n, i);
                if state.con_names.contains_key(&ename)
                    || state.var_names.contains_key(&ename)
                    || state.param_names.contains_key(&ename)
                    || state.array_names.contains(&ename)
                    || state.implicit_var_element(&ename)
                {
                    return Err(InvalidModelError::new_err(format!(
                        "namespace collision for constraint {ename:?}"
                    )));
                }
            }
        }
        if let Some(packed) = packed {
            let cons = Self::insert_packed_comparison(&mut state, &packed)?;
            if let Some(n) = name {
                state.array_names.insert(n.to_string());
                state.index_explicit_name(n);
                for (i, con) in cons.iter().enumerate() {
                    state.con_names.insert(element_name(n, i), *con);
                }
            }
            state.pending = true;
            state.py_revision += 1;
            return Ok(ConstraintArray {
                owner: slf.clone().unbind(),
                shape,
                cons,
            });
        }
        let items = items.expect("packed xor materialized");
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
            record_con_coeffs(&mut state, con, affine);
            cons.push(con);
        }
        if let Some(n) = name {
            state.array_names.insert(n.to_string());
            state.index_explicit_name(n);
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

    /// Insert one numeric scalar-comparison row through the core bulk row
    /// primitive (P1E, batch size 1): no per-cell general insertion. The
    /// bound already folds the tree constant (compare_operand mirrors the
    /// general path's shifting); the core canonicalizes the row exactly
    /// like scalar insertion (duplicate accumulation, zero drop) and
    /// validates liveness/finiteness atomically. Numeric rows carry no
    /// parameter templates, so no bound/coefficient bookkeeping applies.
    fn insert_bulk_row(
        state: &mut ModelState,
        row: &super::expressions::BulkRow,
    ) -> PyResult<ConId> {
        let row_ptr = vec![0, row.vars.len() as u32];
        let mut cons = state
            .model
            .add_linear_rows_bulk(&row_ptr, &row.vars, &row.coeffs, &[row.bound])
            .map_err(map_model_error)?;
        cons.pop()
            .ok_or_else(|| InvalidModelError::new_err("bulk row insertion produced no constraint"))
    }

    /// Insert a packed numeric comparison array through the core bulk row
    /// primitive (P1C-1): one flat row block, no per-element lowering.
    /// Bounds fold the packed scalar constant exactly like `lower_affine`
    /// (`bound - constant`, finiteness-checked); the core canonicalizes
    /// each row exactly like scalar insertion.
    fn insert_packed_comparison(
        state: &mut ModelState,
        packed: &super::arrays::PackedComparison,
    ) -> PyResult<Vec<ConId>> {
        use super::arrays::{numel, PackedSense};
        let n = numel(&packed.shape);
        if n == 0 {
            return Ok(Vec::new());
        }
        let mut row_ptr: Vec<u32> = Vec::with_capacity(n + 1);
        let mut flat_vars: Vec<VarId> = Vec::with_capacity(packed.array.terms.len() * n);
        let mut flat_values: Vec<f64> = Vec::with_capacity(packed.array.terms.len() * n);
        let mut row_bounds: Vec<ConstraintBounds> = Vec::with_capacity(n);
        row_ptr.push(0);
        for i in 0..n {
            for term in &packed.array.terms {
                let c = match &term.coeffs {
                    PackedCoeffs::One => 1.0,
                    PackedCoeffs::Scalar(c) => *c,
                    PackedCoeffs::Dense(v) => v[i],
                };
                flat_vars.push(term.vars[i]);
                flat_values.push(c);
            }
            let shifted = packed.bound - packed.array.constant;
            if !shifted.is_finite() {
                return Err(map_model_error(ModelError::NonFiniteValue(
                    "constraint bound",
                )));
            }
            let (lower, upper) = match packed.sense {
                PackedSense::Le => (f64::NEG_INFINITY, shifted),
                PackedSense::Ge => (shifted, f64::INFINITY),
                PackedSense::Eq => (shifted, shifted),
            };
            row_bounds.push(ConstraintBounds { lower, upper });
            row_ptr.push(flat_vars.len() as u32);
        }
        state
            .model
            .add_linear_rows_bulk(&row_ptr, &flat_vars, &flat_values, &row_bounds)
            .map_err(map_model_error)
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
        let mut state = lock_state(&borrowed)?;
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
        // Pack rows flat in CSR order and insert once through the core bulk
        // primitive (P1B). Per-row canonicalization (sorted variables,
        // duplicate accumulation, zero drop, merged-overflow rejection)
        // happens inside the core exactly like the scalar row path; no
        // per-row Affine/LinExpr objects are built here.
        let mut row_ptr: Vec<u32> = Vec::with_capacity(nrows + 1);
        let mut flat_vars: Vec<VarId> = Vec::with_capacity(indices.len());
        let mut flat_values: Vec<f64> = Vec::with_capacity(data.values.len());
        row_ptr.push(0);
        for i in 0..nrows {
            let (start, end) = (indptr[i] as usize, indptr[i + 1] as usize);
            for k in start..end {
                flat_vars.push(columns[indices[k] as usize]);
                flat_values.push(data.values[k]);
            }
            row_ptr.push(flat_vars.len() as u32);
        }
        let row_bounds: Vec<ConstraintBounds> = (0..nrows)
            .map(|i| ConstraintBounds {
                lower: lower_vals[i],
                upper: upper_vals[i],
            })
            .collect();
        let cons = state
            .model
            .add_linear_rows_bulk(&row_ptr, &flat_vars, &flat_values, &row_bounds)
            .map_err(map_model_error)?;
        if let Some(n) = name {
            state.array_names.insert(n.to_string());
            state.index_explicit_name(n);
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
        use numpy::{IxDyn, PyArray, PyArrayMethods};
        let values: Vec<i64> = flat
            .call_method0("ravel")?
            .cast::<PyArray<i64, IxDyn>>()
            .map_err(|_| InvalidModelError::new_err(format!("{what}: cannot read data")))?
            .to_vec()
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
/// Record parameter-dependent coefficient templates for derived-overflow
/// pre-validation on update, and flag complex dependents so the update
/// path builds the proposed environment. Single registration point for
/// every lowering site (scalar, array, objective): a site that forgets
/// to call this silently disables overflow validation for its rows.
fn record_templates(
    state: &mut ModelState,
    terms: &[super::expressions::ExprTerm],
) -> Vec<ValueExpr> {
    let param_coeffs: Vec<ValueExpr> = terms
        .iter()
        .map(|t| simplify_value(t.coeff.clone()))
        .filter(|c| !c.dependencies().is_empty())
        .collect();
    if param_coeffs
        .iter()
        .any(|c| !matches!(c, ValueExpr::Param(_)))
    {
        state.has_complex_deps = true;
    }
    param_coeffs
}

fn record_con_coeffs(state: &mut ModelState, con: ConId, affine: &super::expressions::Affine) {
    let param_coeffs = record_templates(state, &affine.terms);
    if !param_coeffs.is_empty() {
        state.con_coeffs.insert(con, param_coeffs);
    }
}

fn record_obj_coeffs(state: &mut ModelState, obj: ObjId, terms: &[super::expressions::ExprTerm]) {
    let param_coeffs = record_templates(state, terms);
    if !param_coeffs.is_empty() {
        state.obj_coeffs.insert(obj, param_coeffs);
    }
}

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
    record_con_coeffs(state, con, affine);
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
    let values = param_values(state);
    let mut lin = LinExpr::new();
    for term in &affine.terms {
        if state.model.variable_bounds(term.var).is_none() {
            return Err(InvalidHandleError::new_err(
                "expression references an unknown variable",
            ));
        }
        let coeff = simplify_value(term.coeff.clone());
        // Preflight: a coefficient that is non-finite at current values
        // would fail core insertion mid-batch. Reject the whole batch
        // before installing any row.
        eval_expr(&values, &coeff).map_err(map_model_error)?;
        lin = lin.term(TermCoeff::from(coeff), term.var);
    }
    let const_expr = affine.constant.clone();
    let eval_bound = |e: &ValueExpr| -> Result<(f64, Option<ValueExpr>), ModelError> {
        let shifted = e.clone() - const_expr.clone();
        if shifted.dependencies().is_empty() {
            let v = shifted.eval(|_| 0.0);
            if !v.is_finite() {
                return Err(ModelError::NonFiniteValue("constraint bound"));
            }
            Ok((v, None))
        } else {
            let v = eval_expr(&values, &shifted)?;
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
    Ok((lin, bounds, lower_sym, upper_sym))
}

/// Scalar-or-nothing probe for 0-d parameter updates: plain numbers yield
/// `Some(value)`; bools reject; anything else yields `None` so the caller
/// falls through to dense array parsing (which enforces exact shape).
fn scalar_or_zerod(_py: Python<'_>, value: &Bound<'_, PyAny>, what: &str) -> PyResult<Option<f64>> {
    use pyo3::types::PyBool;
    if value.is_instance_of::<PyBool>() {
        return Err(InvalidModelError::new_err(format!(
            "{what}: bools are not accepted as parameter values"
        )));
    }
    // NumPy scalars/arrays and sequences fall through to array parsing.
    if value.hasattr("dtype").unwrap_or(false) || value.cast::<pyo3::types::PySequence>().is_ok() {
        return Ok(None);
    }
    match value.extract::<f64>() {
        Ok(v) => {
            if !v.is_finite() {
                return Err(InvalidModelError::new_err(format!("{what} must be finite")));
            }
            Ok(Some(v))
        }
        Err(_) => Err(InvalidModelError::new_err(format!(
            "{what} must be a real number or an array"
        ))),
    }
}

#[cfg(test)]
mod lock_tests {
    use super::*;

    /// Contention on model state fails deterministically instead of
    /// waiting: holding the guard makes a second acquisition report Busy.
    #[test]
    fn model_try_lock_contention_is_busy() {
        let shared = SharedModel {
            state: std::sync::Arc::new(Mutex::new(ModelState {
                model: CoreModel::new(),
                name: String::new(),
                var_names: HashMap::new(),
                param_names: HashMap::new(),
                con_names: HashMap::new(),
                var_array_lens: HashMap::new(),
                explicit_indices: HashMap::new(),
                array_names: std::collections::HashSet::new(),
                param_array_shapes: HashMap::new(),
                param_array_ids: HashMap::new(),
                obj_coeffs: HashMap::new(),
                con_coeffs: HashMap::new(),
                has_complex_deps: false,
                bound_deps: Vec::new(),
                has_discrete: false,
                pending: false,
                py_revision: 0,
            })),
            poison: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        };
        let _held = try_model_state(&shared).expect("first acquisition succeeds");
        assert!(matches!(try_model_state(&shared), Err(ModelLockFail::Busy)));
    }
}
