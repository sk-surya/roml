//! Owner-bearing entity handles and identity checks (DESIGN §5).
//!
//! Every handle carries its owning `Py<Model>` (keeping the model alive)
//! and a typed Rust identity. Handles are frozen, unhashable, and never
//! expose raw integer IDs. A foreign-model handle always fails, even if
//! slot numbers coincide.

use pyo3::prelude::*;
use roml::{ConId, ObjId, ParamId, VarId};

use super::errors::ModelMismatchError;
use super::model::Model;

fn owner_mismatch() -> PyErr {
    ModelMismatchError::new_err("variable, parameter, or expression from a different model")
}

pub(crate) fn check_owner(owner: &Py<Model>, model: &Bound<'_, Model>) -> PyResult<()> {
    if owner.as_ptr() == model.as_ptr() {
        Ok(())
    } else {
        Err(owner_mismatch())
    }
}

#[pyclass(frozen, name = "Var")]
pub struct Var {
    pub owner: Py<Model>,
    pub id: VarId,
    pub name: String,
}

#[pyclass(frozen, name = "Param")]
pub struct Param {
    pub owner: Py<Model>,
    pub id: ParamId,
    pub name: String,
}

#[pyclass(frozen, name = "Constraint")]
pub struct Constraint {
    pub owner: Py<Model>,
    pub id: ConId,
    pub name: Option<String>,
}

#[pymethods]
impl Constraint {
    fn __repr__(&self) -> String {
        match &self.name {
            Some(name) => format!("Constraint({name:?})"),
            None => "Constraint(<unnamed>)".to_string(),
        }
    }

    fn __hash__(&self) -> PyResult<isize> {
        Err(pyo3::exceptions::PyTypeError::new_err(
            "constraints are unhashable",
        ))
    }
}

#[pyclass(frozen, name = "Objective")]
pub struct Objective {
    pub owner: Py<Model>,
    pub id: ObjId,
}

#[pymethods]
impl Objective {
    fn __repr__(&self) -> String {
        "Objective(<scalar>)".to_string()
    }

    fn __hash__(&self) -> PyResult<isize> {
        Err(pyo3::exceptions::PyTypeError::new_err(
            "objectives are unhashable",
        ))
    }
}
