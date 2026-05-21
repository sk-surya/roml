//! HiGHS solver adapter for roml.
//!
//! Provides [`HighsAdapter`], a concrete implementation of roml's
//! [`SolverAdapter`] trait backed by the HiGHS mixed-integer linear
//! programming solver.
//!
//! # Build Configuration
//!
//! The adapter's build script supports two configuration modes:
//!
//! 1. Link an existing HiGHS install by setting `HIGHS_ROOT` or `HIGHS_LIB_DIR`.
//! 2. Build HiGHS from source by setting `HIGHS_SOURCE_DIR`.
//!
//! Optional environment variables:
//!
//! - `HIGHS_EXTRA_LIB_DIRS` for additional library search directories.
//! - `HIGHS_EXTRA_LIBS` for extra link libraries such as `openblas,z`.
//! - `HIGHS_BUILD_SHARED=ON|OFF` when building from source.
//!
//! # Example
//!
//! ```rust,ignore
//! use roml::prelude::*;
//! use roml::{constrain, set_objective};
//! use roml_highs::HighsAdapter;
//!
//! fn solve_with_highs() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut model = Model::new();
//!     let x = model.add_var();
//!     let y = model.add_var();
//!
//!     constrain!(model, x + y <= 4.0)?;
//!     constrain!(model, x <= 3.0)?;
//!     constrain!(model, y <= 3.0)?;
//!
//!     let obj = set_objective!(model, maximize: x + y + 2.0)?;
//!
//!     let mut adapter = HighsAdapter::new();
//!     let solution = adapter.solve_model(&mut model)?;
//!     assert!(solution.is_optimal());
//!     assert_eq!(model.objective_constant(obj), Some(2.0));
//!     Ok(())
//! }
//! ```

mod ffi;
mod index_map;
pub mod adapter;

pub use adapter::{HighsAdapter, HighsOptions};
pub use roml::solver::{SolverError, SolverModelExt, SolverStatus};
