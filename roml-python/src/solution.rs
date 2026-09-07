//! Immutable solution snapshots with provenance (DESIGN §6, preliminary).
//!
//! A `Solution` owns copied values and frozen provenance (backend,
//! model instance/revision at solve time). Old results keep answering for
//! their original handles after later edits. Missing values never default
//! to zero: `value` raises `NoSolutionError` without a primal and
//! `MissingValueError` for absent entries.

use std::collections::HashMap;

use pyo3::prelude::*;
use roml::{ModelInstanceId, ModelRevision, SolveStatus as CoreStatus, VarId};

use super::errors::{InvalidHandleError, InvalidModelError, MissingValueError, NoSolutionError};
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
    pub has_candidate: bool,
    pub backend: String,
    pub instance: ModelInstanceId,
    pub revision: ModelRevision,
    pub py_revision: u64,
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

    fn is_current(&self, model: Bound<'_, Model>) -> PyResult<bool> {
        let borrowed = model.borrow();
        let state = borrowed
            .state
            .lock()
            .map_err(|_| InvalidHandleError::new_err("model state is invalid"))?;
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
/// instance against the recorded solve provenance.
fn check_solution_owner(
    owner: &pyo3::Py<super::model::Model>,
    snapshot: &Snapshot,
) -> PyResult<()> {
    let owned = Python::attach(|py| {
        let bound = owner.bind(py);
        let borrowed = bound.borrow();
        let state = borrowed
            .state
            .lock()
            .map_err(|_| InvalidHandleError::new_err("model state is invalid"))?;
        Ok::<_, PyErr>(state.model.instance())
    })?;
    if owned != snapshot.instance {
        return Err(super::errors::ModelMismatchError::new_err(
            "this handle belongs to a different model than the solution",
        ));
    }
    Ok(())
}
