//! FICO Xpress solver backend for roml.
//!
//! Provides [`XpressSession`], a concrete implementation of roml's
//! [`BackendSession`] trait backed by the FICO Xpress MILP solver.
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

mod ffi;
mod index_map;
pub mod adapter;

pub use adapter::{XpressOptions, XpressSession};
pub use roml::solver::SolverError;
