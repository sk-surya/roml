//! Immutable solution snapshots with provenance (DESIGN §6, preliminary).
//!
//! A `Solution` owns copied values and frozen provenance (backend,
//! model instance/revision at solve time). Old results keep answering for
//! their original handles after later edits. Missing values never default
//! to zero: `value` raises `NoSolutionError` without a primal and
//! `MissingValueError` for absent entries.

use std::collections::HashMap;

use pyo3::prelude::*;
use pyo3::types::PyDict;
use roml::{ModelInstanceId, ModelRevision, SolveStatus as CoreStatus, VarId};

use super::errors::{InvalidModelError, MissingValueError, NoSolutionError};
use super::handles::Var;
use super::model::Model;

#[pyclass(frozen, eq, eq_int, name = "SolveStatus", skip_from_py_object)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SolveStatus {
    Unknown,
    Optimal,
    Feasible,
    Infeasible,
    Unbounded,
    InfeasibleOrUnbounded,
    TimeLimit,
    IterationLimit,
    NodeLimit,
    Interrupted,
    Numerical,
    Error,
}

impl From<CoreStatus> for SolveStatus {
    fn from(status: CoreStatus) -> Self {
        match status {
            CoreStatus::Unknown => Self::Unknown,
            CoreStatus::Optimal => Self::Optimal,
            CoreStatus::Feasible => Self::Feasible,
            CoreStatus::Infeasible => Self::Infeasible,
            CoreStatus::Unbounded => Self::Unbounded,
            CoreStatus::InfeasibleOrUnbounded => Self::InfeasibleOrUnbounded,
            CoreStatus::TimeLimit => Self::TimeLimit,
            CoreStatus::IterationLimit => Self::IterationLimit,
            CoreStatus::NodeLimit => Self::NodeLimit,
            CoreStatus::Interrupted => Self::Interrupted,
            CoreStatus::Numerical => Self::Numerical,
            CoreStatus::Error => Self::Error,
        }
    }
}

pub(crate) struct Snapshot {
    pub status: CoreStatus,
    pub objective: Option<f64>,
    pub values: HashMap<VarId, f64>,
    /// Whether the backend reported a candidate solution object. Primal
    /// access gates on this — never on status alone and never on a
    /// non-empty map — so limits with incumbents expose values while
    /// infeasible/unbounded outcomes without candidates do not.
    /// (Preliminary rule: HiGHS reports values only with a candidate;
    /// MPY-04 governs the exact termination-based evidence contract.)
    pub has_candidate: bool,
    pub backend: String,
    pub instance: ModelInstanceId,
    /// Whether the solved model contained discrete variables, recorded at
    /// solve time so later model edits cannot change old diagnostics.
    pub discrete: bool,
    pub lineage: roml::ModelLineageId,
    pub revision: ModelRevision,
    pub py_revision: u64,
    /// Native dual values by constraint id, when the backend reported
    /// valid LP dual evidence.
    pub duals: Option<HashMap<roml::ConId, f64>>,
    /// Native reduced costs by variable id, when reported.
    pub reduced_costs: Option<HashMap<VarId, f64>>,
    /// Effective per-call time limit in seconds (constructor default with
    /// per-call override applied), if any.
    pub effective_time_limit: Option<f64>,
    /// Total wall-clock seconds for the solve call measured in the binding.
    pub wall_seconds: f64,
    /// Warm-start disposition: `none` (no start requested), `applied` (the
    /// backend's effective plan records the start), or
    /// `requested_not_applied`.
    pub warm_start: WarmStart,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WarmStart {
    None,
    Applied,
    RequestedNotApplied,
}

impl WarmStart {
    fn as_str(self) -> &'static str {
        match self {
            WarmStart::None => "none",
            WarmStart::Applied => "applied",
            WarmStart::RequestedNotApplied => "requested_not_applied",
        }
    }
}

#[pyclass(frozen, name = "Solution")]
pub struct Solution {
    pub(crate) snapshot: Snapshot,
}

#[pymethods]
impl Solution {
    #[getter]
    fn status(&self) -> SolveStatus {
        SolveStatus::from(self.snapshot.status)
    }

    #[getter]
    fn has_primal(&self) -> bool {
        self.snapshot.has_candidate
    }

    #[getter]
    fn is_optimal(&self) -> bool {
        matches!(self.snapshot.status, CoreStatus::Optimal)
    }

    #[getter]
    fn objective(&self) -> Option<f64> {
        self.snapshot.objective
    }

    fn value(&self, var: Bound<'_, Var>) -> PyResult<f64> {
        if !self.has_primal() {
            return Err(NoSolutionError::new_err(
                "this result carries no primal solution",
            ));
        }
        let var = var.borrow();
        check_solution_owner(&var.owner, &self.snapshot)?;
        self.snapshot
            .values
            .get(&var.id)
            .copied()
            .ok_or_else(|| MissingValueError::new_err("no value reported for this variable"))
    }

    /// Owned NumPy float64 result over a `VarArray`, same shape and C
    /// order; a copy, never a view into live native buffers.
    fn values(&self, var_array: Bound<'_, super::arrays::VarArray>) -> PyResult<Py<PyAny>> {
        use numpy::{IxDyn, PyArray1, PyArrayMethods};
        if !self.has_primal() {
            return Err(NoSolutionError::new_err(
                "this result carries no primal solution",
            ));
        }
        let arr = var_array.borrow();
        check_solution_owner(&arr.owner, &self.snapshot)?;
        let mut data = Vec::with_capacity(arr.vars.len());
        for (i, var) in arr.vars.iter().enumerate() {
            data.push(self.snapshot.values.get(var).copied().ok_or_else(|| {
                MissingValueError::new_err(format!("no value reported for array element {i}"))
            })?);
        }
        let shape = arr.shape.clone();
        drop(arr);
        Python::attach(|py| {
            let flat = PyArray1::from_vec(py, data);
            let shaped = flat.reshape(IxDyn(&shape)).map_err(|_| {
                InvalidModelError::new_err("internal error: result shape does not match data")
            })?;
            Ok(shaped.into_any().unbind())
        })
    }

    /// Native dual value for one constraint: LP evidence only.
    /// Raises `UnavailableDiagnosticError` when the backend reported no
    /// valid dual evidence (including MILP models, whose duals are not
    /// economic marginal values).
    fn dual(&self, constraint: Bound<'_, super::handles::Constraint>) -> PyResult<f64> {
        use super::errors::UnavailableDiagnosticError;
        if !self.has_primal() {
            return Err(NoSolutionError::new_err(
                "this result carries no primal solution",
            ));
        }
        let con = constraint.borrow();
        check_solution_owner(&con.owner, &self.snapshot)?;
        require_lp(self.snapshot.discrete)?;
        match &self.snapshot.duals {
            Some(duals) => duals.get(&con.id).copied().ok_or_else(|| {
                super::errors::MissingValueError::new_err("no dual reported for this constraint")
            }),
            None => Err(UnavailableDiagnosticError::new_err(
                "the backend reported no valid dual evidence for this result",
            )),
        }
    }

    /// Owned NumPy float64 duals over a `ConstraintArray`, same shape and
    /// C order. Any missing entry raises instead of fabricating a value.
    fn duals(
        &self,
        constraint_array: Bound<'_, super::arrays::ConstraintArray>,
    ) -> PyResult<Py<PyAny>> {
        use super::errors::UnavailableDiagnosticError;
        use numpy::{IxDyn, PyArray1, PyArrayMethods};
        if !self.has_primal() {
            return Err(NoSolutionError::new_err(
                "this result carries no primal solution",
            ));
        }
        let arr = constraint_array.borrow();
        check_solution_owner(&arr.owner, &self.snapshot)?;
        require_lp(self.snapshot.discrete)?;
        let duals = self.snapshot.duals.as_ref().ok_or_else(|| {
            UnavailableDiagnosticError::new_err(
                "the backend reported no valid dual evidence for this result",
            )
        })?;
        let mut data = Vec::with_capacity(arr.cons.len());
        for (i, con) in arr.cons.iter().enumerate() {
            data.push(duals.get(con).copied().ok_or_else(|| {
                super::errors::MissingValueError::new_err(format!(
                    "no dual reported for array element {i}"
                ))
            })?);
        }
        let shape = arr.shape.clone();
        drop(arr);
        Python::attach(|py| {
            let flat = PyArray1::from_vec(py, data);
            let shaped = flat.reshape(IxDyn(&shape)).map_err(|_| {
                InvalidModelError::new_err("internal error: result shape does not match data")
            })?;
            Ok(shaped.into_any().unbind())
        })
    }

    /// Native reduced cost for one variable; same diagnostic policy as duals.
    fn reduced_cost(&self, var: Bound<'_, Var>) -> PyResult<f64> {
        use super::errors::UnavailableDiagnosticError;
        if !self.has_primal() {
            return Err(NoSolutionError::new_err(
                "this result carries no primal solution",
            ));
        }
        let var = var.borrow();
        check_solution_owner(&var.owner, &self.snapshot)?;
        require_lp(self.snapshot.discrete)?;
        match &self.snapshot.reduced_costs {
            Some(costs) => costs.get(&var.id).copied().ok_or_else(|| {
                super::errors::MissingValueError::new_err(
                    "no reduced cost reported for this variable",
                )
            }),
            None => Err(UnavailableDiagnosticError::new_err(
                "the backend reported no valid reduced-cost evidence for this result",
            )),
        }
    }

    /// Solve metadata: backend, model identity/revision, effective options,
    /// synchronization mode, warm-start disposition, and measured timing.
    /// Missing native diagnostic evidence is `None`, never zero.
    #[getter]
    fn metadata<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        use pyo3::types::PyDict;
        let out = PyDict::new(py);
        out.set_item("backend", self.snapshot.backend.clone())?;
        out.set_item("lineage", format!("{:?}", self.snapshot.lineage))?;
        out.set_item("instance", format!("{:?}", self.snapshot.instance))?;
        out.set_item("revision", self.snapshot.revision.as_u64())?;
        out.set_item("py_revision", self.snapshot.py_revision)?;
        out.set_item("effective_time_limit", self.snapshot.effective_time_limit)?;
        out.set_item("wall_seconds", self.snapshot.wall_seconds)?;
        out.set_item("warm_start", self.snapshot.warm_start.as_str())?;
        out.set_item("has_primal", self.snapshot.has_candidate)?;
        Ok(out)
    }

    fn is_current(&self, model: Bound<'_, Model>) -> PyResult<bool> {
        let borrowed = model.borrow();
        let state = super::model::lock_state(&borrowed)?;
        if state.model.instance() != self.snapshot.instance {
            return Ok(false);
        }
        Ok(!state.pending && state.py_revision == self.snapshot.py_revision)
    }

    #[getter]
    fn backend(&self) -> String {
        self.snapshot.backend.clone()
    }

    #[getter]
    fn revision(&self) -> u64 {
        self.snapshot.revision.as_u64()
    }

    fn __repr__(&self) -> String {
        format!(
            "Solution(status={:?}, objective={:?})",
            SolveStatus::from(self.snapshot.status),
            self.snapshot.objective
        )
    }
}

/// A handle from another model (or instance) must never index into this
/// result: arena IDs can coincide across models. Compare the owner's live
/// instance against the recorded solve provenance. Returns the owner's
/// discreteness flag alongside for LP-only diagnostic gating.
fn check_solution_owner(
    owner: &pyo3::Py<super::model::Model>,
    snapshot: &Snapshot,
) -> PyResult<()> {
    let owned = Python::attach(|py| {
        let bound = owner.bind(py);
        let borrowed = bound.borrow();
        let state = super::model::lock_state(&borrowed)?;
        Ok::<_, PyErr>(state.model.instance())
    })?;
    if owned != snapshot.instance {
        return Err(super::errors::ModelMismatchError::new_err(
            "this handle belongs to a different model than the solution",
        ));
    }
    Ok(())
}

/// Duals and reduced costs are LP-only: on discrete models they raise
/// instead of advertising relaxation values as economic marginals.
fn require_lp(discrete: bool) -> PyResult<()> {
    if discrete {
        return Err(super::errors::UnavailableDiagnosticError::new_err(
            "duals and reduced costs are LP-only diagnostics and are not reported for mixed-integer models",
        ));
    }
    Ok(())
}
