//! Minimal hand-written FFI bindings for the FICO Xpress C API.
//!
//! Only the functions needed by XpressAdapter are bound here.

#![allow(non_snake_case, non_camel_case_types, dead_code)]

use std::ffi::{c_char, c_double, c_int, c_void};

// ── Opaque handle type ─────────────────────────────────────────────────────

/// XPRSprob — opaque problem handle.
pub type XPRSprob = *mut c_void;

// ── Scalar types ───────────────────────────────────────────────────────────

pub type XprsInt = c_int;
pub type XprsReal = c_double;
pub type XprsRes = c_int;

// ── Useful constants ───────────────────────────────────────────────────────

pub const XPRS_PLUSINFINITY: XprsReal = 1.0e+20;
pub const XPRS_MINUSINFINITY: XprsReal = -1.0e+20;

// ── Objective sense ────────────────────────────────────────────────────────

pub const OBJ_MINIMIZE: XprsInt = 1; // XPRS_OBJ_MINIMIZE
pub const OBJ_MAXIMIZE: XprsInt = -1; // XPRS_OBJ_MAXIMIZE

// ── LP status (XPRS_LPSTATUS attribute values) ─────────────────────────────

pub const LP_UNSTARTED: XprsInt = 0;
pub const LP_OPTIMAL: XprsInt = 1;
pub const LP_INFEAS: XprsInt = 2;
pub const LP_CUTOFF: XprsInt = 3;
pub const LP_UNFINISHED: XprsInt = 4;
pub const LP_UNBOUNDED: XprsInt = 5;
pub const LP_UNSOLVED: XprsInt = 7;

// ── MIP status (XPRS_MIPSTATUS attribute values) ───────────────────────────

pub const MIP_NOT_LOADED: XprsInt = 0;
pub const MIP_LP_NOT_OPTIMAL: XprsInt = 1;
pub const MIP_LP_OPTIMAL: XprsInt = 2;
pub const MIP_NO_SOL_FOUND: XprsInt = 3;
pub const MIP_SOLUTION: XprsInt = 4;
pub const MIP_INFEAS: XprsInt = 5;
pub const MIP_OPTIMAL: XprsInt = 6;
pub const MIP_UNBOUNDED: XprsInt = 7;

// ── Integer attributes (XPRSgetintattrib) ─────────────────────────────────

pub const XPRS_ROWS: XprsInt = 1001;
pub const XPRS_COLS: XprsInt = 1018;
pub const XPRS_LPSTATUS: XprsInt = 1010;
pub const XPRS_MIPSTATUS: XprsInt = 1011;

// ── Double attributes (XPRSgetdblattrib) ──────────────────────────────────

pub const XPRS_LPOBJVAL: XprsInt = 2001;
pub const XPRS_MIPOBJVAL: XprsInt = 2003;

// ── Integer controls (XPRSsetintcontrol) ──────────────────────────────────

pub const XPRS_OUTPUTLOG: XprsInt = 8035;
pub const XPRS_THREADS: XprsInt = 8278;
pub const XPRS_TIMELIMIT: XprsInt = 7158;
pub const XPRS_PRESOLVE: XprsInt = 8011;
pub const XPRS_DEFAULTALG: XprsInt = 7997;

// Algorithm values for XPRS_DEFAULTALG
pub const ALG_AUTOMATIC: XprsInt = 0;
pub const ALG_PRIMAL_SIMPLEX: XprsInt = 1;
pub const ALG_DUAL_SIMPLEX: XprsInt = 2;
pub const ALG_NETWORK_SIMPLEX: XprsInt = 3;
pub const ALG_BARRIER: XprsInt = 4;

// ── Extern C declarations ──────────────────────────────────────────────────

extern "C" {
    // ── Library lifecycle ─────────────────────────────────────────────────
    pub fn XPRSinit(path: *const c_char) -> XprsRes;
    pub fn XPRSfree() -> XprsRes;

    // ── Problem lifecycle ─────────────────────────────────────────────────
    pub fn XPRScreateprob(p_prob: *mut XPRSprob) -> XprsRes;
    pub fn XPRSdestroyprob(prob: XPRSprob) -> XprsRes;

    // ── Controls and attributes ───────────────────────────────────────────
    pub fn XPRSsetintcontrol(prob: XPRSprob, control: XprsInt, value: XprsInt) -> XprsRes;
    pub fn XPRSsetdblcontrol(prob: XPRSprob, control: XprsInt, value: XprsReal) -> XprsRes;
    pub fn XPRSgetintattrib(prob: XPRSprob, attrib: XprsInt, p_value: *mut XprsInt) -> XprsRes;
    pub fn XPRSgetdblattrib(prob: XPRSprob, attrib: XprsInt, p_value: *mut XprsReal) -> XprsRes;

    // ── Adding columns (variables) ────────────────────────────────────────
    /// Add ncols new columns.
    /// start[i] is the offset in rowind/rowcoef where col i's entries begin.
    pub fn XPRSaddcols(
        prob: XPRSprob,
        ncols: XprsInt,
        ncoefs: XprsInt,
        objcoef: *const XprsReal,
        start: *const XprsInt,
        rowind: *const XprsInt,
        rowcoef: *const XprsReal,
        lb: *const XprsReal,
        ub: *const XprsReal,
    ) -> XprsRes;

    /// Delete columns indexed in colind[0..ncols-1].
    pub fn XPRSdelcols(prob: XPRSprob, ncols: XprsInt, colind: *const XprsInt) -> XprsRes;

    // ── Column bounds & type ──────────────────────────────────────────────
    /// Change bounds for nbounds entries.
    /// bndtype[i] is 'L' (lower), 'U' (upper), or 'B' (both/fixed).
    pub fn XPRSchgbounds(
        prob: XPRSprob,
        nbounds: XprsInt,
        colind: *const XprsInt,
        bndtype: *const c_char,
        bndval: *const XprsReal,
    ) -> XprsRes;

    /// Change column types. coltype[i] is 'C', 'I', or 'B'.
    pub fn XPRSchgcoltype(
        prob: XPRSprob,
        ncols: XprsInt,
        colind: *const XprsInt,
        coltype: *const c_char,
    ) -> XprsRes;

    // ── Objective ─────────────────────────────────────────────────────────
    /// Change objective coefficients for ncols columns.
    pub fn XPRSchgobj(
        prob: XPRSprob,
        ncols: XprsInt,
        colind: *const XprsInt,
        objcoef: *const XprsReal,
    ) -> XprsRes;

    /// Change objective sense.
    pub fn XPRSchgobjsense(prob: XPRSprob, objsense: XprsInt) -> XprsRes;

    // ── Adding rows (constraints) ─────────────────────────────────────────
    /// Add nrows new rows.
    /// rowtype[i] is 'L', 'G', 'E', 'R', or 'N'.
    /// rhs[i] is the right-hand side; rng[i] is the range (for type 'R').
    /// start[i] is the offset in colind/rowcoef for row i's coefficients.
    pub fn XPRSaddrows(
        prob: XPRSprob,
        nrows: XprsInt,
        ncoefs: XprsInt,
        rowtype: *const c_char,
        rhs: *const XprsReal,
        rng: *const XprsReal,
        start: *const XprsInt,
        colind: *const XprsInt,
        rowcoef: *const XprsReal,
    ) -> XprsRes;

    /// Delete rows indexed in rowind[0..nrows-1].
    pub fn XPRSdelrows(prob: XPRSprob, nrows: XprsInt, rowind: *const XprsInt) -> XprsRes;

    // ── Row bound changes ─────────────────────────────────────────────────
    pub fn XPRSchgrowtype(
        prob: XPRSprob,
        nrows: XprsInt,
        rowind: *const XprsInt,
        rowtype: *const c_char,
    ) -> XprsRes;

    pub fn XPRSchgrhs(
        prob: XPRSprob,
        nrows: XprsInt,
        rowind: *const XprsInt,
        rhs: *const XprsReal,
    ) -> XprsRes;

    pub fn XPRSchgrhsrange(
        prob: XPRSprob,
        nrows: XprsInt,
        rowind: *const XprsInt,
        rng: *const XprsReal,
    ) -> XprsRes;

    // ── Constraint matrix ─────────────────────────────────────────────────
    pub fn XPRSchgcoef(prob: XPRSprob, row: XprsInt, col: XprsInt, coef: XprsReal) -> XprsRes;

    /// Change multiple matrix coefficients in one call.
    pub fn XPRSchgmcoef(
        prob: XPRSprob,
        ncoefs: XprsInt,
        rowind: *const XprsInt,
        colind: *const XprsInt,
        rowcoef: *const XprsReal,
    ) -> XprsRes;

    // ── Solve ─────────────────────────────────────────────────────────────
    pub fn XPRSlpoptimize(prob: XPRSprob, flags: *const c_char) -> XprsRes;
    pub fn XPRSmipoptimize(prob: XPRSprob, flags: *const c_char) -> XprsRes;

    // ── Basis ──────────────────────────────────────────────────────────────
    /// Get current basis status arrays. rowstatus[0..ROWS-1], colstatus[0..COLS-1].
    /// Status: 0=at lower bound, 1=basic, 2=at upper bound, 3=super-basic.
    pub fn XPRSgetbasis(
        prob: XPRSprob,
        rowstatus: *mut XprsInt,
        colstatus: *mut XprsInt,
    ) -> XprsRes;
    /// Load basis from arrays (must match current ROWS/COLS).
    pub fn XPRSloadbasis(
        prob: XPRSprob,
        rowstatus: *const XprsInt,
        colstatus: *const XprsInt,
    ) -> XprsRes;

    // ── Solution queries ──────────────────────────────────────────────────
    /// Get LP solution. Pass NULL for arrays you don't need.
    pub fn XPRSgetlpsol(
        prob: XPRSprob,
        x: *mut XprsReal,     // primal (ncols)
        slack: *mut XprsReal, // slack  (nrows)
        duals: *mut XprsReal, // dual multipliers (nrows)
        djs: *mut XprsReal,   // reduced costs (ncols)
    ) -> XprsRes;

    /// Get best MIP solution.
    pub fn XPRSgetmipsol(
        prob: XPRSprob,
        x: *mut XprsReal,     // primal (ncols)
        slack: *mut XprsReal, // slack  (nrows)
    ) -> XprsRes;

    // ── Message callback ──────────────────────────────────────────────────
    pub fn XPRSaddcbmessage(
        prob: XPRSprob,
        message: Option<unsafe extern "C" fn(XPRSprob, *mut c_void, *const c_char, c_int, c_int)>,
        data: *mut c_void,
        priority: XprsInt,
    ) -> XprsRes;
}
