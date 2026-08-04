//! HiGHS solver backend for roml.
//!
//! This crate provides a `BackendSession` implementation backed by the
//! HiGHS mixed-integer linear programming solver, using authoritative
//! `highs-sys` bindings for FFI.
//!
//! The golden-path user type is [`Highs`]: a thin façade exposing
//! `new`/`solve`/`solve_with`. Ordinary users need nothing else from this
//! crate. [`HighsSession`] remains public for framework authors and tests.
//!
//! # Feature Mutual Exclusion
//!
//! `bundled` and `system` are mutually exclusive — activating both is a
//! compile-time error.
//!
//! # Module Structure
//!
//! - `bindings`: Re-exports from `highs-sys` plus ROML constant aliases.
//! - `error`: `BackendError` construction helpers for HiGHS failures.
//! - `lifecycle`: [`HighsSession`] construction, ownership, and Drop.
//! - `compiler`: Backend IR → HiGHS native rebuild and delta application
//!   (P26 Task 7; the HiGHS session receives no canonical `ModelSnapshot`).
//! - `session`: `BackendSession` trait implementation (thin delegation).
//! - `solution`: Status mapping and solution extraction.
//! - `callback`: Callback bridge for MIP lazy constraints/interrupts.
//! - `index_map`: Dense index bookkeeping (kept from original adapter).
//! - `facade`: The golden-path [`Highs`] façade (D3).

#![warn(missing_docs)]
//!
//! # Quick Start
//!
//! Use the [`Highs`] façade — synchronization is automatic:
//!
//! ```rust
//! use roml::prelude::*;
//! use roml_highs::Highs;
//!
//! # fn run() -> Result<(), Box<dyn std::error::Error>> {
//! let mut model = Model::named("production");
//! let x = model.add_variable(continuous().named("x"))?;
//! let y = model.add_variable(integer().bounds(0.0, 10.0).named("y"))?;
//! model.add_constraint((x + y).le(4.0).named("capacity"))?;
//! model.maximize(3.0 * x + y)?;
//!
//! let mut highs = Highs::new()?;
//! let solution = highs.solve(&mut model)?;
//! assert!(solution.status().is_optimal());
//! # Ok(())
//! # }
//! # run().unwrap();
//! ```
//!
//! # Build Configuration
//!
//! The crate supports two build modes via Cargo features:
//!
//! - `bundled` (default): Builds HiGHS from source via `highs-sys`'s cmake.
//! - `system`: Discovers a system-installed HiGHS library.

/// Ensure bundled and system features are mutually exclusive.
#[cfg(all(feature = "bundled", feature = "system"))]
compile_error!("features `bundled` and `system` are mutually exclusive; activate at most one");

mod bindings;
mod callback;
mod compiler;
mod error;
mod facade;
mod index_map;
mod lifecycle;
mod session;
mod solution;
mod start;

pub use error::HighsError;
pub use facade::Highs;
pub use lifecycle::HighsSession;
pub use session::highs_capability_set;

/// Re-export key types from `highs-sys` for caller convenience.
pub use bindings::HighsInt;

// ── BackendFixture ────────────────────────────────────────────────────────────

/// Creates fresh [`HighsSession`] instances for parameterized tests.
///
/// Implements [`roml::solver::session::BackendFixture`] so that HiGHS can
/// run the shared conformance suite alongside ReferenceBackend.
pub struct HighsFixture;

impl roml::solver::session::BackendFixture for HighsFixture {
    type Session = HighsSession;

    fn new_session(&self) -> Result<Self::Session, roml::solver::backend::BackendError> {
        HighsSession::try_new().map_err(|e| {
            roml::solver::backend::BackendError::new(
                format!("HighsFixture: {}", e.message),
                roml::solver::backend::ErrorCategory::LibraryNotFound,
                roml::solver::backend::HealthEffect::Terminal,
            )
        })
    }

    fn backend_name(&self) -> &str {
        "HiGHS"
    }
}
