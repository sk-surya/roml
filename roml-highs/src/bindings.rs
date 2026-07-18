//! Authoritative HiGHS C API bindings via `highs-sys`.
//!
//! This module re-exports all types, functions, and constants from the
//! `highs-sys` crate, providing ROML-specific constant aliases that bridge
//! naming conventions. No handwritten `extern "C"` declarations exist in
//! this crate — `highs-sys` is the sole ABI owner.
//!
//! # Safety
//!
//! All FFI calls through `highs-sys` are inherently `unsafe`. The safety
//! invariants for each call are documented at the call site in the
//! downstream modules (lifecycle, projection, solution, etc.).

#![allow(non_snake_case)]

pub use highs_sys::*;

// ── Return status ──────────────────────────────────────────────────────────
// Verified against highs-sys 1.15.0 generated values.

pub const STATUS_OK: HighsInt = 0;
pub const STATUS_WARNING: HighsInt = 1;
pub const STATUS_ERROR: HighsInt = -1;

pub const kHighsStatusOk: HighsInt = 0;
pub const kHighsStatusWarning: HighsInt = 1;
pub const kHighsStatusError: HighsInt = -1;

// ── Model status constants ─────────────────────────────────────────────────
// Verified against highs-sys 1.15.0 generated values (HiGHS C API constants).

pub const MODEL_STATUS_NOT_SET: HighsInt = 0;
pub const MODEL_STATUS_LOAD_ERROR: HighsInt = 1;
pub const MODEL_STATUS_MODEL_ERROR: HighsInt = 2;
pub const MODEL_STATUS_PRESOLVE_ERROR: HighsInt = 3;
pub const MODEL_STATUS_SOLVE_ERROR: HighsInt = 4;
pub const MODEL_STATUS_POSTSOLVE_ERROR: HighsInt = 5;
pub const MODEL_STATUS_MODEL_EMPTY: HighsInt = 6;
pub const MODEL_STATUS_OPTIMAL: HighsInt = 7;
pub const MODEL_STATUS_INFEASIBLE: HighsInt = 8;
pub const MODEL_STATUS_UNBOUNDED_OR_INFEASIBLE: HighsInt = 9;
pub const MODEL_STATUS_UNBOUNDED: HighsInt = 10;
pub const MODEL_STATUS_OBJECTIVE_BOUND: HighsInt = 11;
pub const MODEL_STATUS_OBJECTIVE_TARGET: HighsInt = 12;
pub const MODEL_STATUS_TIME_LIMIT: HighsInt = 13;
pub const MODEL_STATUS_ITERATION_LIMIT: HighsInt = 14;
pub const MODEL_STATUS_UNKNOWN: HighsInt = 15;

// ── Callback type constants ────────────────────────────────────────────────
// Verified against highs-sys 1.15.0 generated values.

pub const kCallbackMipLogging: HighsInt = 1;
pub const kCallbackMipInterrupt: HighsInt = 2;
pub const kCallbackMipSolution: HighsInt = 3;
pub const kCallbackMipImprovingSolution: HighsInt = 4;
pub const kCallbackMipGetCutPool: HighsInt = 7;
pub const kCallbackMipDefineLazyConstraints: HighsInt = 8;

// ── Infinity ────────────────────────────────────────────────────────────────
// Note: kHighsInfinity is a runtime value (typically 1e30) obtained from
// Highs_getInfinity(). This constant provides the typical value for
// compile-time reference; runtime code should call Highs_getInfinity().
pub const kHighsInfinity: f64 = 1e30;
