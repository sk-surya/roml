//! MOSEK solver adapter for roml.
//!
//! Provides [`MosekAdapter`], a concrete implementation of roml's
//! [`SolverAdapter`] trait backed by the MOSEK mixed-integer linear
//! programming solver.
//!
//! # Build Configuration
//!
//! Set `MOSEK_BINDIR` to the directory containing `libmosek64.dylib` (macOS)
//! or `libmosek64.so` (Linux), e.g.:
//!
//! ```sh
//! export MOSEK_BINDIR=/path/to/mosek/11.2/tools/platform/osxaarch64/bin
//! ```
//!
//! Alternatively, set `MOSEK_ROOT` to the MOSEK installation root and the
//! build script will locate the binary directory automatically.
//!
//! A valid MOSEK license (`mosek.lic`) must be available; MOSEK searches
//! `~/mosek/mosek.lic` by default, or the path set in `MOSEKLM_LICENSE_FILE`.

mod ffi;
mod index_map;
pub mod adapter;

pub use adapter::{MosekAdapter, MosekOptimizer, MosekOptions, MosekSimHotstart};
pub use roml::solver::{SolverError, SolverModelExt, SolverStatus};
