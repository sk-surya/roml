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

use super::errors::{InvalidHandleError, MissingValueError, NoSolutionError};
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
        self.snapshot
            .values
            .get(&var.id)
            .copied()
            .ok_or_else(|| MissingValueError::new_err("no value reported for this variable"))
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
