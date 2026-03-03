//! Hand-written FFI bindings for HiGHS C API.
//!
//! Only binding limited functions needed by HiGHSAdapter. Lets add later as needed.

#![allow(non_snake_case)]

use std::ffi::{c_char, c_double, c_int, c_void};

/// HiGHS integer type. HiGHS can be compiled with 32- or 64-bit indexing.
/// We declare it as i32 and validate at runtime via Highs_getSizeofHighsInt.
/// HiPO work with 64??
pub type HighsInt = c_int;

extern "C" {
    // ── Lifecycle ──────────────────────────────────────────────────────────
    pub fn Highs_create() -> *mut c_void;
    pub fn Highs_destroy(highs: *mut c_void);
    pub fn Highs_clearModel(highs: *mut c_void) -> HighsInt;
    pub fn Highs_getSizeofHighsInt(highs: *const c_void) -> HighsInt;

    // ── Solve ──────────────────────────────────────────────────────────────
    pub fn Highs_run(highs: *mut c_void) -> HighsInt;
    pub fn Highs_getModelStatus(highs: *const c_void) -> HighsInt;
    pub fn Highs_getSolution(
        highs: *const c_void,
        col_value: *mut c_double,
        col_dual: *mut c_double,
        row_value: *mut c_double,
        row_dual: *mut c_double,
    ) -> HighsInt;
    pub fn Highs_getObjectiveValue(highs: *const c_void) -> c_double;
    pub fn Highs_getInfinity(highs: *const c_void) -> c_double;
    pub fn Highs_getNumCol(highs: *const c_void) -> HighsInt;
    pub fn Highs_getNumRow(highs: *const c_void) -> HighsInt;

    // ── Adding ─────────────────────────────────────────────────────────────
    pub fn Highs_addVar(highs: *mut c_void, lower: c_double, upper: c_double) -> HighsInt;
    pub fn Highs_addRow(
        highs: *mut c_void,
        lower: c_double,
        upper: c_double,
        num_new_nz: HighsInt,
        index: *const HighsInt,
        value: *const c_double,
    ) -> HighsInt;

    // ── Modifying ──────────────────────────────────────────────────────────
    pub fn Highs_changeColBounds(
        highs: *mut c_void,
        col: HighsInt,
        lower: c_double,
        upper: c_double,
    ) -> HighsInt;
    pub fn Highs_changeRowBounds(
        highs: *mut c_void,
        row: HighsInt,
        lower: c_double,
        upper: c_double,
    ) -> HighsInt;
    pub fn Highs_changeColCost(highs: *mut c_void, col: HighsInt, cost: c_double) -> HighsInt;
    pub fn Highs_changeColIntegrality(
        highs: *mut c_void,
        col: HighsInt,
        integrality: HighsInt,
    ) -> HighsInt;
    pub fn Highs_changeCoeff(
        highs: *mut c_void,
        row: HighsInt,
        col: HighsInt,
        value: c_double,
    ) -> HighsInt;
    pub fn Highs_changeObjectiveSense(highs: *mut c_void, sense: HighsInt) -> HighsInt;

    // ── Deleting ───────────────────────────────────────────────────────────
    pub fn Highs_deleteColsByRange(
        highs: *mut c_void,
        from_col: HighsInt,
        to_col: HighsInt,
    ) -> HighsInt;
    pub fn Highs_deleteRowsByRange(
        highs: *mut c_void,
        from_row: HighsInt,
        to_row: HighsInt,
    ) -> HighsInt;

    // ── Options ────────────────────────────────────────────────────────────
    pub fn Highs_setBoolOptionValue(
        highs: *mut c_void,
        option: *const c_char,
        value: HighsInt,
    ) -> HighsInt;

    pub fn Highs_setIntOptionValue(
        highs: *mut c_void,
        option: *const c_char,
        value: HighsInt,
    ) -> HighsInt;

    pub fn Highs_setDoubleOptionValue(
        highs: *mut c_void,
        option: *const c_char,
        value: c_double,
    ) -> HighsInt;

    pub fn Highs_setStringOptionValue(
        highs: *mut c_void,
        option: *const c_char,
        value: *const c_char,
    ) -> HighsInt;
}

// ── Model status constants ─────────────────────────────────────────────────
// From HiGHS source: kModelStatusOptimal = 7, etc.
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

// ── Variable integrality constants ─────────────────────────────────────────
pub const VAR_TYPE_CONTINUOUS: HighsInt = 0;
pub const VAR_TYPE_INTEGER: HighsInt = 1;

// ── Objective sense constants ──────────────────────────────────────────────
pub const OBJ_SENSE_MINIMIZE: HighsInt = 1;
pub const OBJ_SENSE_MAXIMIZE: HighsInt = -1;

// ── Return status ──────────────────────────────────────────────────────────
pub const STATUS_OK: HighsInt = 0;
