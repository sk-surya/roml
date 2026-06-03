//! Hand-written FFI bindings for HiGHS C API.
//!
//! Only binding limited functions needed by HighsAdapter. Lets add later as needed.

#![allow(non_snake_case)]

use std::ffi::{c_char, c_double, c_int, c_void};

/// HiGHS integer type. HiGHS can be compiled with 32- or 64-bit indexing.
/// We declare it as i32 and validate at runtime via Highs_getSizeofHighsInt.
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
    pub fn Highs_changeColsCostByRange(
        highs: *mut c_void,
        from_col: HighsInt,
        to_col: HighsInt,
        cost: *const c_double
    ) -> HighsInt;
    pub fn Highs_changeColsCostBySet(
        highs: *mut c_void,
        num_set_entries: HighsInt,
        set: *const HighsInt,
        cost: *const c_double
    ) -> HighsInt;
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

    // ── Callbacks ──────────────────────────────────────────────────────────
    /// Register a C callback function with HiGHS.
    /// The callback fires for enabled callback types during `Highs_run`.
    pub fn Highs_setCallback(
        highs: *mut c_void,
        user_callback: Option<HighsCCallback>,
        user_callback_data: *mut c_void,
    ) -> HighsInt;

    /// Enable a specific callback type.
    pub fn Highs_startCallback(highs: *mut c_void, callback_type: HighsInt) -> HighsInt;

    /// Disable a specific callback type.
    pub fn Highs_stopCallback(highs: *mut c_void, callback_type: HighsInt) -> HighsInt;
}

// ── Callback type ──────────────────────────────────────────────────────────

/// Signature for HiGHS C callback functions.
///
/// Matches `HighsCCallbackType` from the HiGHS C API header:
/// ```c
/// typedef void (*HighsCCallbackType)(int, const char*,
///                                    const HighsCallbackDataOut*,
///                                    HighsCallbackDataIn*, void*);
/// ```
pub type HighsCCallback = unsafe extern "C" fn(
    event_type: c_int,
    message: *const c_char,
    data_out: *const HighsCallbackDataOut,
    data_in: *mut HighsCallbackDataIn,
    user_data: *mut c_void,
);

// ── Callback structs ──────────────────────────────────────────────────────

/// Mirrors `HighsCallbackDataOut` from HiGHS 1.14.
///
/// IMPORTANT: This layout assumes `HighsInt = i32` (4 bytes). If HiGHS is
/// compiled with 64-bit indexing (`-DHIGHSINT64`), the struct layout shifts.
/// We validate at runtime via `Highs_getSizeofHighsInt` in the adapter.
#[repr(C)]
#[derive(Debug)]
pub struct HighsCallbackDataOut {
    pub cbdata: *mut c_void,                    // offset   0, size  8
    pub log_type: c_int,                         // offset   8, size  4
    //                                            // padding          4
    pub running_time: c_double,                  // offset  16, size  8
    pub simplex_iteration_count: HighsInt,        // offset  24, size  4
    pub ipm_iteration_count: HighsInt,            // offset  28, size  4
    pub pdlp_iteration_count: HighsInt,           // offset  32, size  4
    //                                            // padding          4
    pub objective_function_value: c_double,       // offset  40, size  8
    pub mip_node_count: i64,                      // offset  48, size  8
    pub mip_total_lp_iterations: i64,             // offset  56, size  8
    pub mip_primal_bound: c_double,               // offset  64, size  8
    pub mip_dual_bound: c_double,                 // offset  72, size  8
    pub mip_gap: c_double,                        // offset  80, size  8
    pub mip_solution: *mut c_double,              // offset  88, size  8
    pub mip_solution_size: HighsInt,              // offset  96, size  4
    pub cutpool_num_col: HighsInt,                // offset 100, size  4
    pub cutpool_num_cut: HighsInt,                // offset 104, size  4
    pub cutpool_num_nz: HighsInt,                 // offset 108, size  4
    pub cutpool_start: *mut HighsInt,             // offset 112, size  8
    pub cutpool_index: *mut HighsInt,             // offset 120, size  8
    pub cutpool_value: *mut c_double,             // offset 128, size  8
    pub cutpool_lower: *mut c_double,             // offset 136, size  8
    pub cutpool_upper: *mut c_double,             // offset 144, size  8
    pub external_solution_query_origin: HighsInt, // offset 152, size  4
    //                                            // padding          4
} //                                                total:  160 bytes

/// Compile-time assertion: verify the struct has the expected size on arm64
/// with 32-bit HighsInt. If HiGHS is upgraded or HighsInt size changes,
/// this assertion catches the mismatch before silent corruption.
const _: () = assert!(core::mem::size_of::<HighsCallbackDataOut>() == 160);

/// Mirrors `HighsCallbackDataIn` from HiGHS 1.14.
#[repr(C)]
#[derive(Debug)]
pub struct HighsCallbackDataIn {
    pub user_interrupt: c_int,                // offset   0, size  4
    //                                          // padding          4
    pub user_solution: *mut c_double,          // offset   8, size  8
    pub cbdata: *mut c_void,                   // offset  16, size  8
    pub user_has_solution: c_int,             // offset  24, size  4
    pub user_solution_size: HighsInt,          // offset  28, size  4
} //                                              total:   32 bytes

const _: () = assert!(core::mem::size_of::<HighsCallbackDataIn>() == 32);

// ── Callback type constants ───────────────────────────────────────────────

pub const CALLBACK_MIP_LOGGING: HighsInt = 1;
pub const CALLBACK_MIP_INTERRUPT: HighsInt = 2;
pub const CALLBACK_MIP_SOLUTION: HighsInt = 3;
pub const CALLBACK_MIP_IMPROVING_SOLUTION: HighsInt = 4;
pub const CALLBACK_MIP_GET_CUT_POOL: HighsInt = 7;
/// Callback fired when HiGHS needs to check lazy constraints for a MIP solution.
/// The callback can add violated constraints via `Highs_addRow`.
pub const CALLBACK_MIP_DEFINE_LAZY_CONSTRAINTS: HighsInt = 8;

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
