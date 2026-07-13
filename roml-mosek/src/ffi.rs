//! Minimal hand-written FFI bindings for the MOSEK C API.
//!
//! Only the functions needed by MosekAdapter are bound here.

#![allow(non_snake_case, non_camel_case_types, dead_code)]

use std::ffi::{c_char, c_double, c_int, c_void};

// ── Opaque handle types ────────────────────────────────────────────────────

pub type MosekEnv  = *mut c_void;   // MSKenv_t
pub type MosekTask = *mut c_void;   // MSKtask_t

// ── Scalar types ───────────────────────────────────────────────────────────

pub type MosekInt  = c_int;         // MSKint32t
pub type MosekReal = c_double;      // MSKrealt
pub type MosekRes  = c_int;         // MSKrescodee

// ── Enum value aliases (i32 constants matching the C enum values) ──────────

// Bound keys (MSKboundkeye)
pub const BK_LO: MosekInt = 0;  // lower bound only
pub const BK_UP: MosekInt = 1;  // upper bound only
pub const BK_FX: MosekInt = 2;  // fixed
pub const BK_FR: MosekInt = 3;  // free (no bounds)
pub const BK_RA: MosekInt = 4;  // range (both bounds)

// Objective sense (MSKobjsensee)
pub const OBJ_SENSE_MINIMIZE: MosekInt = 0;
pub const OBJ_SENSE_MAXIMIZE: MosekInt = 1;

// Variable type (MSKvariabletypee)
pub const VAR_TYPE_CONT:       MosekInt = 0;
pub const VAR_TYPE_INT:        MosekInt = 1;
pub const VAR_TYPE_SEMICONT:   MosekInt = 2;
pub const VAR_TYPE_SEMICONT_INT: MosekInt = 3;

// Solution type (MSKsoltypee)
pub const SOL_ITR: MosekInt = 0;  // interior-point
pub const SOL_BAS: MosekInt = 1;  // basic (simplex)
pub const SOL_ITG: MosekInt = 2;  // integer

// Solution status (MSKsolstae)
pub const SOL_STA_UNKNOWN:          MosekInt = 0;
pub const SOL_STA_OPTIMAL:          MosekInt = 1;
pub const SOL_STA_PRIM_FEAS:        MosekInt = 2;
pub const SOL_STA_DUAL_FEAS:        MosekInt = 3;
pub const SOL_STA_PRIM_AND_DUAL_FEAS: MosekInt = 4;
pub const SOL_STA_PRIM_INFEAS_CER:  MosekInt = 5;
pub const SOL_STA_DUAL_INFEAS_CER:  MosekInt = 6;
pub const SOL_STA_INTEGER_OPTIMAL:  MosekInt = 9;

// Problem status (MSKprostae)
pub const PRO_STA_UNKNOWN:                  MosekInt = 0;
pub const PRO_STA_PRIM_AND_DUAL_FEAS:       MosekInt = 1;
pub const PRO_STA_PRIM_FEAS:                MosekInt = 2;
pub const PRO_STA_DUAL_FEAS:                MosekInt = 3;
pub const PRO_STA_PRIM_INFEAS:              MosekInt = 4;
pub const PRO_STA_DUAL_INFEAS:              MosekInt = 5;
pub const PRO_STA_PRIM_AND_DUAL_INFEAS:     MosekInt = 6;
pub const PRO_STA_ILL_POSED:                MosekInt = 7;
pub const PRO_STA_PRIM_INFEAS_OR_UNBOUNDED: MosekInt = 8;

// Stream type constants (MSKstreamtypee)
pub const STREAM_LOG: MosekInt = 0;

// Double params (MSKdparame) — subset we use
pub const DPAR_MIO_MAX_TIME: MosekInt = 42;

// Integer params (MSKiparame) — subset we use
pub const IPAR_LOG:              MosekInt = 34;
pub const IPAR_NUM_THREADS:      MosekInt = 100;
pub const IPAR_OPTIMIZER:        MosekInt = 110;
pub const IPAR_SIM_HOTSTART:     MosekInt = 145;
pub const IPAR_SIM_HOTSTART_LU:  MosekInt = 146;
pub const IPAR_INTPNT_HOTSTART:  MosekInt = 19;
pub const MSK_ON:  MosekInt = 1;
pub const MSK_OFF: MosekInt = 0;

// Optimizer type constants (MSKoptimizertype)
pub const OPTIMIZER_FREE:             MosekInt = 2; // let MOSEK choose
pub const OPTIMIZER_INTPNT:           MosekInt = 4; // interior point
pub const OPTIMIZER_DUAL_SIMPLEX:     MosekInt = 1; // legacy dual simplex
pub const OPTIMIZER_NEW_DUAL_SIMPLEX: MosekInt = 6; // revised dual simplex (11.x)
pub const OPTIMIZER_FREE_SIMPLEX:     MosekInt = 3; // MOSEK picks primal/dual simplex

// Simplex hotstart (MSKsimhotstart_enum)
pub const SIM_HOTSTART_NONE:        MosekInt = 0;
pub const SIM_HOTSTART_FREE:        MosekInt = 1;
pub const SIM_HOTSTART_STATUS_KEYS: MosekInt = 2; // reuse previous basis

// Interior-point hotstart (MSKintpnthotstart_enum)
pub const INTPNT_HOTSTART_NONE:         MosekInt = 0;
pub const INTPNT_HOTSTART_PRIMAL_DUAL:  MosekInt = 3; // feed both primal+dual from prior solve

// Interior-point basis / crossover (MSKiparame)
pub const IPAR_INTPNT_BASIS: MosekInt = 31;
pub const BI_NEVER:          MosekInt = 0; // no basis after IPM
pub const BI_IF_FEASIBLE:    MosekInt = 1; // compute basis if feasible (MOSEK default)
pub const BI_ALWAYS:         MosekInt = 2; // always compute basis after IPM

// Return codes
pub const RES_OK: MosekRes = 0;
pub const RES_TRM_USER_CALLBACK: MosekRes = 100007;

// ── Callback code constants (MSKcallbackcodee) ────────────────────────────
pub const CALLBACK_BEGIN_MIO: MosekInt = 17;
pub const CALLBACK_END_MIO: MosekInt = 54;
pub const CALLBACK_IM_MIO: MosekInt = 80;
pub const CALLBACK_NEW_INT_MIO: MosekInt = 91;

// ── MIO info indices (for douinf / intinf arrays in callbacks) ───────────
pub const DINF_MIO_OBJ_ABS_GAP: MosekInt = 35;
pub const DINF_MIO_OBJ_BOUND: MosekInt = 36;
pub const DINF_MIO_OBJ_INT: MosekInt = 37;
pub const DINF_MIO_OBJ_REL_GAP: MosekInt = 38;
pub const IINF_MIO_NUM_INT_SOLUTIONS: MosekInt = 47;

// ── Extern C declarations ──────────────────────────────────────────────────

extern "C" {
    // ── Environment & task lifecycle ──────────────────────────────────────
    pub fn MSK_makeenv(env: *mut MosekEnv, dbgfile: *const c_char) -> MosekRes;
    pub fn MSK_deleteenv(env: *mut MosekEnv) -> MosekRes;
    pub fn MSK_maketask(
        env:       MosekEnv,
        maxnumcon: MosekInt,
        maxnumvar: MosekInt,
        task:      *mut MosekTask,
    ) -> MosekRes;
    pub fn MSK_deletetask(task: *mut MosekTask) -> MosekRes;

    // ── Parameters ────────────────────────────────────────────────────────
    pub fn MSK_putintparam(
        task:     MosekTask,
        param:    MosekInt,
        parvalue: MosekInt,
    ) -> MosekRes;
    pub fn MSK_putdouparam(
        task:     MosekTask,
        param:    MosekInt,
        parvalue: MosekReal,
    ) -> MosekRes;

    // ── Appending variables / constraints ─────────────────────────────────
    pub fn MSK_appendvars(task: MosekTask, num: MosekInt) -> MosekRes;
    pub fn MSK_appendcons(task: MosekTask, num: MosekInt) -> MosekRes;

    // ── Querying sizes ────────────────────────────────────────────────────
    pub fn MSK_getnumvar(task: MosekTask, numvar: *mut MosekInt) -> MosekRes;
    pub fn MSK_getnumcon(task: MosekTask, numcon: *mut MosekInt) -> MosekRes;

    // ── Bounds ────────────────────────────────────────────────────────────
    pub fn MSK_putvarbound(
        task: MosekTask,
        j:    MosekInt,
        bkx:  MosekInt,
        blx:  MosekReal,
        bux:  MosekReal,
    ) -> MosekRes;
    pub fn MSK_putconbound(
        task: MosekTask,
        i:    MosekInt,
        bkc:  MosekInt,
        blc:  MosekReal,
        buc:  MosekReal,
    ) -> MosekRes;

    // ── Objective ─────────────────────────────────────────────────────────
    pub fn MSK_putcj(task: MosekTask, j: MosekInt, cj: MosekReal) -> MosekRes;
    pub fn MSK_putobjsense(task: MosekTask, sense: MosekInt) -> MosekRes;

    // ── Constraint matrix ─────────────────────────────────────────────────
    pub fn MSK_putaij(
        task: MosekTask,
        i:    MosekInt,
        j:    MosekInt,
        aij:  MosekReal,
    ) -> MosekRes;

    // ── Variable type ─────────────────────────────────────────────────────
    pub fn MSK_putvartype(
        task:    MosekTask,
        j:       MosekInt,
        vartype: MosekInt,
    ) -> MosekRes;

    // ── Removing ──────────────────────────────────────────────────────────
    pub fn MSK_removevars(
        task:   MosekTask,
        num:    MosekInt,
        subset: *const MosekInt,
    ) -> MosekRes;
    pub fn MSK_removecons(
        task:   MosekTask,
        num:    MosekInt,
        subset: *const MosekInt,
    ) -> MosekRes;

    // ── Logging stream ────────────────────────────────────────────────────
    pub fn MSK_linkfunctotaskstream(
        task:        MosekTask,
        whichstream: MosekInt,
        handle:      *mut c_void,
        func:        Option<unsafe extern "C" fn(*mut c_void, *const c_char)>,
    ) -> MosekRes;

    // ── Optimize ──────────────────────────────────────────────────────────
    pub fn MSK_optimize(task: MosekTask) -> MosekRes;

    // ── Solution queries ──────────────────────────────────────────────────
    pub fn MSK_getsolsta(
        task:       MosekTask,
        whichsol:   MosekInt,
        solutionsta: *mut MosekInt,
    ) -> MosekRes;
    pub fn MSK_getprosta(
        task:       MosekTask,
        whichsol:   MosekInt,
        problemsta: *mut MosekInt,
    ) -> MosekRes;
    pub fn MSK_getxx(
        task:     MosekTask,
        whichsol: MosekInt,
        xx:       *mut MosekReal,
    ) -> MosekRes;
    pub fn MSK_gety(
        task:     MosekTask,
        whichsol: MosekInt,
        y:        *mut MosekReal,
    ) -> MosekRes;
    pub fn MSK_getslx(
        task:     MosekTask,
        whichsol: MosekInt,
        slx:      *mut MosekReal,
    ) -> MosekRes;
    pub fn MSK_getsux(
        task:     MosekTask,
        whichsol: MosekInt,
        sux:      *mut MosekReal,
    ) -> MosekRes;
    pub fn MSK_getprimalobj(
        task:      MosekTask,
        whichsol:  MosekInt,
        primalobj: *mut MosekReal,
    ) -> MosekRes;

    // ── Callbacks ──────────────────────────────────────────────────────────
    /// Register a callback function. The callback is invoked at various points
    /// during optimization (determined by the callback code).
    pub fn MSK_putcallbackfunc(
        task:   MosekTask,
        func:   Option<MosekCallbackFunc>,
        handle: *mut c_void,
    ) -> MosekRes;

    // ── Efficient matrix modification ──────────────────────────────────────
    /// Set multiple coefficients in the constraint matrix at once.
    pub fn MSK_putaijlist(
        task:   MosekTask,
        num:    MosekInt,
        subi:   *const MosekInt,
        subj:   *const MosekInt,
        valij:  *const MosekReal,
    ) -> MosekRes;

    // ── Solution existence check ───────────────────────────────────────────
    /// Check whether a solution of the given type exists.
    pub fn MSK_solutiondef(
        task:     MosekTask,
        whichsol: MosekInt,
        isdef:    *mut MosekInt,
    ) -> MosekRes;
}

// ── MOSEK callback function pointer type ──────────────────────────────────

/// Type of the callback function registered with `MSK_putcallbackfunc`.
/// Return 0 to continue, non-zero to terminate with `RES_TRM_USER_CALLBACK`.
pub type MosekCallbackFunc = unsafe extern "C" fn(
    task:   MosekTask,
    usrptr: *mut c_void,        // user data (MosekCallbackState)
    caller: MosekInt,           // event code (e.g., CALLBACK_NEW_INT_MIO)
    douinf: *const MosekReal,   // double info array
    intinf: *const MosekInt,    // int info array
    lintinf: *const i64,        // long int info array
) -> MosekInt;
