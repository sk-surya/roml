#![allow(non_snake_case, dead_code)]
//! HiGHS C API bindings — re-exported from the maintained `highs-sys` crate.
//!
//! Prior to M1.2, this file contained handwritten ABI declarations (252 lines
//! of `extern "C"`, manually-copied struct layouts, and hardcoded constants).
//! M1.2 migrates to `rust-or/highs-sys`, generated from the official
//! `highs_c_api.h` header.
//!
//! ROML-specific constant aliases use the known numerical values from the
//! HiGHS 1.14 C API, verified against both the old handwritten declarations
//! and the `highs-sys` 1.15.0 generated bindings.

// Re-export the authoritative types and functions from highs-sys.
// This includes: Highs_create, Highs_destroy, Highs_run, Highs_addVar,
// Highs_addRow, Highs_changeColBounds, Highs_changeCoeff, Highs_setCallback,
// HighsCallbackDataOut, HighsCallbackDataIn, HighsInt, etc.
pub use highs_sys::*;

// ── ROML constant aliases ────────────────────────────────────────────────────
//
// highs-sys uses kHighs* naming (e.g., kHighsModelStatusOptimal).
// ROML's adapter uses MODEL_STATUS_* naming. These aliases bridge the two.
// Numerical values are from the HiGHS C API and verified against highs-sys 1.15.

// Return status
pub const STATUS_OK: HighsInt = 0;

// Model status (HiGHS kHighsModelStatus enum)
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

// Variable integrality (HiGHS kHighsVarType enum)
pub const VAR_TYPE_CONTINUOUS: HighsInt = 0;
pub const VAR_TYPE_INTEGER: HighsInt = 1;
pub const VAR_TYPE_SEMI_CONTINUOUS: HighsInt = 2;
pub const VAR_TYPE_SEMI_INTEGER: HighsInt = 3;

// Objective sense
pub const OBJ_SENSE_MINIMIZE: HighsInt = 1;
pub const OBJ_SENSE_MAXIMIZE: HighsInt = -1;

// Callback type constants
pub const CALLBACK_MIP_LOGGING: HighsInt = 1;
pub const CALLBACK_MIP_INTERRUPT: HighsInt = 2;
pub const CALLBACK_MIP_SOLUTION: HighsInt = 3;
pub const CALLBACK_MIP_IMPROVING_SOLUTION: HighsInt = 4;
pub const CALLBACK_MIP_GET_CUT_POOL: HighsInt = 7;
pub const CALLBACK_MIP_DEFINE_LAZY_CONSTRAINTS: HighsInt = 8;

// ── Callback type alias ──────────────────────────────────────────────────────
pub type HighsCCallback = highs_sys::HighsCCallbackType;
