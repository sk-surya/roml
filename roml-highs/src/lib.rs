//! HiGHS solver backend for roml.
//!
//! This crate provides a [`BackendSession`] implementation backed by the
//! HiGHS mixed-integer linear programming solver, using authoritative
//! [`highs-sys`] bindings for FFI.
//!
//! # Module Structure
//!
//! - [`bindings`]: Re-exports from `highs-sys` plus ROML constant aliases.
//! - [`error`]: `BackendError` construction helpers for HiGHS failures.
//! - [`lifecycle`]: [`HighsSession`] construction, ownership, and Drop.
//! - [`projection`]: Snapshot-to-HiGHS rebuild and delta application.
//! - [`session`]: `BackendSession` trait implementation (thin delegation).
//! - [`solution`]: Status mapping and solution extraction.
//! - [`callback`]: Callback bridge for MIP lazy constraints/interrupts.
//! - [`index_map`]: Dense index bookkeeping (kept from original adapter).
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use roml_highs::HighsSession;
//!
//! let session = HighsSession::try_new().expect("Failed to create HiGHS session");
//! ```
//!
//! # Build Configuration
//!
//! The crate supports two build modes via Cargo features:
//!
//! - `bundled` (default): Builds HiGHS from source via `highs-sys`'s cmake.
//! - `system`: Discovers a system-installed HiGHS library.

mod bindings;
mod error;
mod index_map;
mod lifecycle;
mod projection;
mod session;
mod solution;
mod callback;

pub use error::HighsError;
pub use lifecycle::HighsSession;

/// Re-export key types from `highs-sys` for caller convenience.
pub use bindings::HighsInt;
