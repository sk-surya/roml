//! HiGHS solver adapter for roml.
//!
//! Provides [`HighsAdapter`], a concrete implementation of roml's
//! [`SolverAdapter`] trait backed by the HiGHS mixed-integer linear
//! programming solver.
//!
//! # Example
//!
//! ```rust,ignore
//! use roml::{Model, Bounds, ConstraintBounds, Sense};
//! use roml_highs::HighsAdapter;
//! use roml::solver::SolverAdapter;
//!
//! let mut model = Model::new();
//! let x = model.add_variable(Bounds::NON_NEGATIVE, Default::default()).unwrap();
//! let y = model.add_variable(Bounds::NON_NEGATIVE, Default::default()).unwrap();
//!
//! // Add objective: minimize x + 2y
//! let obj = model.add_objective(Sense::Minimize).unwrap();
//! model.set_active_objective(Some(obj)).unwrap();
//! // ... add coefficients and constraints ...
//!
//! let mut adapter = HighsAdapter::new();
//! let changes = model.drain_changelog();
//! adapter.apply_changes(&changes).unwrap();
//! let status = adapter.solve().unwrap();
//! ```

mod ffi;
mod index_map;
pub mod adapter;

pub use adapter::HighsAdapter;
pub use roml::solver::{SolverError, SolverStatus};
