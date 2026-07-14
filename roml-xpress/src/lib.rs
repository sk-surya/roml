//! FICO Xpress solver adapter for roml.
//!
//! Provides [`XpressAdapter`], a concrete implementation of roml's
//! [`SolverAdapter`] trait backed by the FICO Xpress MILP solver.
//!
//! # Build Configuration
//!
//! Set `XPRESS_DIR` to the xpressmp directory, e.g.:
//!
//! ```sh
//! export XPRESS_DIR="/Applications/FICO Xpress/Xpress Workbench.app/Contents/Resources/xpressmp"
//! ```
//!
//! The build script uses `$XPRESS_DIR/lib` for linking and embeds an rpath
//! so tests run without setting `DYLD_LIBRARY_PATH`.
//!
//! A valid Xpress license (`xpauth.xpr`) must be discoverable. The adapter
//! calls `XPRSinit` with `$XPRESS_DIR/bin` at startup. Alternatively, set
//! the standard `XPAUTH_PATH` environment variable.

pub mod adapter;
mod ffi;
mod index_map;

pub use adapter::{XpressAdapter, XpressOptions};
pub use roml::solver::{SolverError, SolverModelExt, SolverStatus};
