//! FICO Xpress solver backend implementing roml's `BackendSession` trait.
//!
//! # Design
//!
//! - All Xpress state is owned by an opaque `XPRSprob` handle.
//! - `col_map` / `row_map` maintain Id to Xpress-index bidirectional maps.
//! - Variable/constraint bounds are cached locally so they can be restored
//!   when deactivated entities are reactivated.
//! - Objective costs per objective are cached so switching active objective
//!   only requires zeroing the current costs and loading the new ones.
//! - Inactive variables are fixed at [0, 0]; inactive constraints become
//!   non-binding (row type 'N').
//! - When a column/row is deleted, `reindex_after_delete` keeps maps in sync
//!   with Xpress's dense integer addressing.

use std::collections::{HashMap, HashSet};
use std::ffi::CString;
use std::sync::OnceLock;

use log::info;

use roml::delta::{DeltaBatch, ModelOp};
use roml::id::{ConId, ObjId, VarId};
use roml::model::coefficient::CoefficientTarget;
use roml::model::objective::Sense;
use roml::model::variable::VarType;
use roml::revision::ModelRevision;
use roml::snapshot::ModelSnapshot;
use roml::solver::backend::{BackendCapabilities, BackendError, ErrorCategory, HealthEffect, TerminationStatus};
use roml::solver::request::{ConfigRejection, EffectiveConfig, LpAlgorithm, SolveRequest, SolveResult, SolveSolution};
use roml::solver::session::{BackendMetadata, BackendSession, SessionHealth, SolutionView, SyncReceipt, Synchronization};
use roml::sync::AdapterCursor;

use crate::ffi;
use crate::index_map::IndexMap;

// ── Library-level init (call XPRSinit exactly once per process) ───────────

static XPRESS_INIT: OnceLock<()> = OnceLock::new();

fn ensure_xpress_init() {
    XPRESS_INIT.get_or_init(|| {
        let ret = if let Ok(dir) = std::env::var("XPRESS_DIR") {
            let bin = format!("{}/bin", dir);
            let c = CString::new(bin).expect("XPRESS_DIR must not contain null bytes");
            unsafe { ffi::XPRSinit(c.as_ptr()) }
        } else {
            unsafe { ffi::XPRSinit(std::ptr::null()) }
        };
        if ret != 0 {
            panic!("XPRSinit failed with code {ret}; set XPRESS_DIR to the xpressmp directory");
        }
    });
}

// ── Helper: map constraint (lb, ub) to Xpress row type + rhs + range ──────

/// Returns `(rowtype_byte, rhs, rng)` for an Xpress row representing `lb <= ax <= ub`.
///
/// Xpress range rows use: `rhs - rng <= ax <= rhs` when `rng > 0`.
fn xprs_row(lb: f64, ub: f64) -> (u8, f64, f64) {
    let lb_fin = lb.is_finite();
    let ub_fin = ub.is_finite();
    match (lb_fin, ub_fin) {
        (false, false) => (b'N', 0.0, 0.0),
        (false, true)  => (b'L', ub,  0.0),
        (true,  false) => (b'G', lb,  0.0),
        (true,  true) if (lb - ub).abs() < f64::EPSILON => (b'E', lb, 0.0),
        (true,  true)  => (b'R', ub,  ub - lb),
    }
}

/// Clamp Rust +/-infinity to Xpress +/-1e20.
fn xprs_bound(v: f64) -> f64 {
    if v == f64::INFINITY      { ffi::XPRS_PLUSINFINITY  }
    else if v == f64::NEG_INFINITY { ffi::XPRS_MINUSINFINITY }
    else { v }
}

// ── Options ────────────────────────────────────────────────────────────────

/// Configuration forwarded to Xpress at session creation time.
#[derive(Debug, Clone)]
pub struct XpressOptions {
    threads:        Option<i32>,
    log_level:      i32,
    max_time:       Option<f64>,
    presolve:       bool,
    console_output: bool,
}

impl Default for XpressOptions {
    fn default() -> Self {
        Self {
            threads: None,
            log_level: 5,
            max_time: None,
            presolve: true,
            console_output: false,
        }
    }
}

impl XpressOptions {
    pub fn threads(mut self, n: i32) -> Self {
        self.threads = Some(n);
        self
    }

    pub fn log_level(mut self, n: i32) -> Self {
        self.log_level = n;
        self
    }

    pub fn console_output(mut self, enabled: bool) -> Self {
        self.console_output = enabled;
        self
    }

    pub fn max_time(mut self, seconds: f64) -> Self {
        self.max_time = Some(seconds);
        self
    }

    /// Enable or disable Xpress presolve. Enabled by default.
    pub fn presolve(mut self, enabled: bool) -> Self {
        self.presolve = enabled;
        self
    }
}

// ── Session struct ─────────────────────────────────────────────────────────

pub struct XpressSession {
    prob: ffi::XPRSprob,
    opts: XpressOptions,

    col_map: IndexMap<VarId>,
    row_map: IndexMap<ConId>,

    var_bounds: HashMap<VarId, (f64, f64)>,
    con_bounds: HashMap<ConId, (f64, f64)>,

    integer_vars: HashSet<VarId>,
    semicontinuous_vars: HashSet<VarId>,

    obj_costs:  HashMap<ObjId, HashMap<VarId, f64>>,
    obj_senses: HashMap<ObjId, Sense>,
    active_obj: Option<ObjId>,

    current_solution: Option<SolveSolution>,
    last_status: Option<TerminationStatus>,

    cursor: AdapterCursor,
}

// SAFETY: XPRSprob is a C pointer. We never share it across threads.
unsafe impl Send for XpressSession {}

// ── Construction helpers ───────────────────────────────────────────────────

fn make_prob(opts: &XpressOptions) -> ffi::XPRSprob {
    let mut prob: ffi::XPRSprob = std::ptr::null_mut();
    let ret = unsafe { ffi::XPRScreateprob(&mut prob) };
    assert!(ret == 0 && !prob.is_null(), "XPRScreateprob failed (code {ret})");

    let output_level: i32 = if opts.console_output { 1 } else { opts.log_level };
    set_int_control(prob, ffi::XPRS_OUTPUTLOG, output_level, "OUTPUTLOG");
    set_int_control(prob, ffi::XPRS_PRESOLVE, i32::from(opts.presolve), "PRESOLVE");
    if let Some(t) = opts.threads {
        set_int_control(prob, ffi::XPRS_THREADS, t, "THREADS");
    }
    if let Some(secs) = opts.max_time {
        set_double_control(prob, ffi::XPRS_TIMELIMIT, secs, "TIMELIMIT");
    }
    unsafe {
        ffi::XPRSaddcbmessage(prob, Some(xpress_msg_cb), std::ptr::null_mut(), 0);
    }
    prob
}

fn set_int_control(prob: ffi::XPRSprob, control: i32, value: i32, name: &str) {
    let ret = unsafe { ffi::XPRSsetintcontrol(prob, control, value) };
    assert_eq!(ret, 0, "XPRSsetintcontrol({name}) failed with code {ret}");
}

fn set_double_control(prob: ffi::XPRSprob, control: i32, value: f64, name: &str) {
    let ret = unsafe { ffi::XPRSsetdblcontrol(prob, control, value) };
    assert_eq!(ret, 0, "XPRSsetdblcontrol({name}) failed with code {ret}");
}

/// C-callable message callback -- forwards Xpress output to stderr.
unsafe extern "C" fn xpress_msg_cb(
    _prob:   ffi::XPRSprob,
    _data:   *mut std::ffi::c_void,
    msg:     *const std::ffi::c_char,
    msglen:  std::ffi::c_int,
    _msgtype: std::ffi::c_int,
) {
    if msg.is_null() || msglen <= 0 { return; }
    let bytes = unsafe { std::slice::from_raw_parts(msg as *const u8, msglen as usize) };
    if let Ok(s) = std::str::from_utf8(bytes) {
        use std::io::Write;
        let mut stderr = std::io::stderr();
        let _ = stderr.write_all(s.as_bytes());
        let _ = stderr.write_all(b"\n");
        let _ = stderr.flush();
    }
}

// ── BackendError helpers ──────────────────────────────────────────────────

fn xprs_err(msg: impl Into<String>) -> BackendError {
    BackendError::new(msg, ErrorCategory::Internal, HealthEffect::RequiresRebuild)
}

fn check(ret: ffi::XprsRes, op: &str) -> Result<(), BackendError> {
    if ret == 0 { Ok(()) } else { Err(xprs_err(format!("{op} returned {ret}"))) }
}

// ── impl XpressSession ─────────────────────────────────────────────────────

impl XpressSession {
    pub fn new() -> Self {
        Self::with_options(XpressOptions::default())
    }

    pub fn with_options(opts: XpressOptions) -> Self {
        ensure_xpress_init();
        let prob = make_prob(&opts);
        Self {
            prob,
            opts,
            col_map:         IndexMap::new(),
            row_map:         IndexMap::new(),
            var_bounds:      HashMap::new(),
            con_bounds:      HashMap::new(),
            integer_vars:    HashSet::new(),
            semicontinuous_vars: HashSet::new(),
            obj_costs:       HashMap::new(),
            obj_senses:      HashMap::new(),
            active_obj:      None,
            current_solution: None,
            last_status:     None,
            cursor:          AdapterCursor::new(),
        }
    }

    fn is_mip(&self) -> bool {
        !self.integer_vars.is_empty()
    }

    // ── Row-type application ──────────────────────────────────────────────

    fn apply_row_bounds(&self, row: i32, lb: f64, ub: f64) -> Result<(), BackendError> {
        let (rt, rhs, rng) = xprs_row(lb, ub);
        let row_i = row;
        unsafe {
            check(
                ffi::XPRSchgrowtype(self.prob, 1, &row_i, &(rt as i8)),
                "XPRSchgrowtype",
            )?;
            check(ffi::XPRSchgrhs(self.prob, 1, &row_i, &rhs), "XPRSchgrhs")?;
            check(ffi::XPRSchgrhsrange(self.prob, 1, &row_i, &rng), "XPRSchgrhsrange")?;
        }
        Ok(())
    }

    // ── Apply a single ModelOp ────────────────────────────────────────────

    fn apply_model_op(&mut self, op: &ModelOp) -> Result<(), BackendError> {
        match op {
            // ── Variable Added ────────────────────────────────────────────
            ModelOp::AddVariable { var, bounds, var_type } => {
                let lb = xprs_bound(bounds.lower);
                let ub = xprs_bound(bounds.upper);
                let objc = [0.0f64];
                let start = [0i32];
                let col_idx = unsafe {
                    let mut n = 0i32;
                    ffi::XPRSgetintattrib(self.prob, ffi::XPRS_COLS, &mut n);
                    n
                };
                check(
                    unsafe {
                        ffi::XPRSaddcols(
                            self.prob,
                            1, 0,
                            objc.as_ptr(),
                            start.as_ptr(),
                            std::ptr::null(),
                            std::ptr::null(),
                            &lb, &ub,
                        )
                    },
                    "XPRSaddcols",
                )?;
                self.col_map.insert(*var, col_idx);
                self.var_bounds.insert(*var, (bounds.lower, bounds.upper));

                if matches!(var_type, VarType::Integer | VarType::Binary) {
                    let ct = if *var_type == VarType::Binary { b'B' as i8 } else { b'I' as i8 };
                    check(
                        unsafe { ffi::XPRSchgcoltype(self.prob, 1, &col_idx, &ct) },
                        "XPRSchgcoltype",
                    )?;
                    self.integer_vars.insert(*var);
                }
            }

            // ── Variable Removed ──────────────────────────────────────────
            ModelOp::RemoveVariable { var } => {
                let col = match self.col_map.remove(*var) {
                    Some(c) => c,
                    None    => return Ok(()),
                };
                self.var_bounds.remove(var);
                self.integer_vars.remove(var);
                for costs in self.obj_costs.values_mut() {
                    costs.remove(var);
                }
                check(
                    unsafe { ffi::XPRSdelcols(self.prob, 1, &col) },
                    "XPRSdelcols",
                )?;
                self.col_map.reindex_after_delete(col);
            }

            // ── Variable Bounds Changed ──────────────────────────────────
            ModelOp::SetVariableBounds { var, bounds } => {
                if let Some(col) = self.col_map.get(*var) {
                    let lb = xprs_bound(bounds.lower);
                    let ub = xprs_bound(bounds.upper);
                    let cols  = [col, col];
                    let types = [b'L' as i8, b'U' as i8];
                    let vals  = [lb, ub];
                    check(
                        unsafe { ffi::XPRSchgbounds(self.prob, 2, cols.as_ptr(), types.as_ptr(), vals.as_ptr()) },
                        "XPRSchgbounds (var bounds)",
                    )?;
                    self.var_bounds.insert(*var, (bounds.lower, bounds.upper));
                }
            }

            // ── Variable Type Changed ────────────────────────────────────
            ModelOp::SetVariableType { var, var_type } => {
                if let Some(col) = self.col_map.get(*var) {
                    let ct = if self.semicontinuous_vars.contains(var) {
                        match var_type {
                            VarType::Continuous => { self.integer_vars.remove(var); }
                            _ => { self.integer_vars.insert(*var); }
                        }
                        b'R' as i8
                    } else {
                        match var_type {
                            VarType::Continuous => { self.integer_vars.remove(var); b'C' as i8 }
                            VarType::Integer    => { self.integer_vars.insert(*var); b'I' as i8 }
                            VarType::Binary     => { self.integer_vars.insert(*var); b'B' as i8 }
                        }
                    };
                    check(
                        unsafe { ffi::XPRSchgcoltype(self.prob, 1, &col, &ct) },
                        "XPRSchgcoltype (type change)",
                    )?;
                }
            }

            // ── Variable Activity Changed ────────────────────────────────
            ModelOp::SetVariableActive { var, active } => {
                if let Some(col) = self.col_map.get(*var) {
                    let (lb, ub) = if *active {
                        let (orig_lb, orig_ub) = self.var_bounds.get(var).copied().unwrap_or((0.0, ffi::XPRS_PLUSINFINITY));
                        (xprs_bound(orig_lb), xprs_bound(orig_ub))
                    } else {
                        (0.0, 0.0)
                    };
                    let cols  = [col, col];
                    let types = [b'L' as i8, b'U' as i8];
                    let vals  = [lb, ub];
                    check(
                        unsafe { ffi::XPRSchgbounds(self.prob, 2, cols.as_ptr(), types.as_ptr(), vals.as_ptr()) },
                        "XPRSchgbounds (activity)",
                    )?;
                }
            }

            // ── Constraint Added ──────────────────────────────────────────
            ModelOp::AddConstraint { con, bounds } => {
                let (rt, rhs, rng) = xprs_row(bounds.lower, bounds.upper);
                let row_idx = unsafe {
                    let mut n = 0i32;
                    ffi::XPRSgetintattrib(self.prob, ffi::XPRS_ROWS, &mut n);
                    n
                };
                let start = [0i32];
                check(
                    unsafe {
                        ffi::XPRSaddrows(
                            self.prob,
                            1, 0,
                            &(rt as i8),
                            &rhs, &rng,
                            start.as_ptr(),
                            std::ptr::null(),
                            std::ptr::null(),
                        )
                    },
                    "XPRSaddrows",
                )?;
                self.row_map.insert(*con, row_idx);
                self.con_bounds.insert(*con, (bounds.lower, bounds.upper));
            }

            // ── Constraint Removed ────────────────────────────────────────
            ModelOp::RemoveConstraint { con } => {
                let row = match self.row_map.remove(*con) {
                    Some(r) => r,
                    None    => return Ok(()),
                };
                self.con_bounds.remove(con);
                check(
                    unsafe { ffi::XPRSdelrows(self.prob, 1, &row) },
                    "XPRSdelrows",
                )?;
                self.row_map.reindex_after_delete(row);
            }

            // ── Constraint Bounds Changed ────────────────────────────────
            ModelOp::SetConstraintBounds { con, bounds } => {
                if let Some(row) = self.row_map.get(*con) {
                    self.apply_row_bounds(row, bounds.lower, bounds.upper)?;
                    self.con_bounds.insert(*con, (bounds.lower, bounds.upper));
                }
            }

            // ── Constraint Activity Changed ──────────────────────────────
            ModelOp::SetConstraintActive { con, active } => {
                if let Some(row) = self.row_map.get(*con) {
                    if *active {
                        let (orig_lb, orig_ub) = self.con_bounds
                            .get(con)
                            .copied()
                            .unwrap_or((f64::NEG_INFINITY, f64::INFINITY));
                        self.apply_row_bounds(row, orig_lb, orig_ub)?;
                    } else {
                        check(
                            unsafe { ffi::XPRSchgrowtype(self.prob, 1, &row, &(b'N' as i8)) },
                            "XPRSchgrowtype (deactivate)",
                        )?;
                    }
                }
            }

            // ── Coefficient Cell Set ──────────────────────────────────────
            ModelOp::SetCell { cell_key, evaluated_value, .. } => {
                let (target, var_id) = cell_key;
                match target {
                    CoefficientTarget::Constraint(con_id) => {
                        if let (Some(row), Some(col)) =
                            (self.row_map.get(*con_id), self.col_map.get(*var_id))
                        {
                            check(
                                unsafe { ffi::XPRSchgcoef(self.prob, row, col, *evaluated_value) },
                                "XPRSchgcoef (set cell)",
                            )?;
                        }
                    }
                    CoefficientTarget::Objective(_) => {
                        return Err(BackendError::unsupported(
                            "SetCell with Objective target -- use SetObjectiveCell instead",
                        ));
                    }
                }
            }

            // ── Coefficient Cell Removed ─────────────────────────────────
            ModelOp::RemoveCell { cell_key } => {
                let (target, var_id) = cell_key;
                match target {
                    CoefficientTarget::Constraint(con_id) => {
                        if let (Some(row), Some(col)) =
                            (self.row_map.get(*con_id), self.col_map.get(*var_id))
                        {
                            check(
                                unsafe { ffi::XPRSchgcoef(self.prob, row, col, 0.0) },
                                "XPRSchgcoef (remove cell)",
                            )?;
                        }
                    }
                    CoefficientTarget::Objective(obj_id) => {
                        if let Some(costs) = self.obj_costs.get_mut(obj_id) {
                            costs.remove(var_id);
                        }
                        if Some(*obj_id) == self.active_obj {
                            if let Some(col) = self.col_map.get(*var_id) {
                                let zero = 0.0f64;
                                check(
                                    unsafe { ffi::XPRSchgobj(self.prob, 1, &col, &zero) },
                                    "XPRSchgobj (remove obj cell)",
                                )?;
                            }
                        }
                    }
                }
            }

            // ── Objective Added ───────────────────────────────────────────
            ModelOp::AddObjective { obj, sense } => {
                self.obj_senses.insert(*obj, *sense);
                self.obj_costs.entry(*obj).or_default();
            }

            // ── Objective Removed ─────────────────────────────────────────
            ModelOp::RemoveObjective { obj } => {
                self.obj_costs.remove(obj);
                self.obj_senses.remove(obj);
                if self.active_obj == Some(*obj) {
                    self.active_obj = None;
                }
            }

            // ── Active Objective Changed ──────────────────────────────────
            ModelOp::SetActiveObjective { obj } => {
                // Zero all current column costs.
                let ncols = unsafe {
                    let mut n = 0i32;
                    ffi::XPRSgetintattrib(self.prob, ffi::XPRS_COLS, &mut n);
                    n
                };
                if ncols > 0 {
                    let cols:   Vec<i32> = (0..ncols).collect();
                    let zeros:  Vec<f64> = vec![0.0; ncols as usize];
                    check(
                        unsafe { ffi::XPRSchgobj(self.prob, ncols, cols.as_ptr(), zeros.as_ptr()) },
                        "XPRSchgobj (zero for obj switch)",
                    )?;
                }

                if let Some(new_obj) = obj {
                    if let Some(costs) = self.obj_costs.get(new_obj).cloned() {
                        for (var, cost) in &costs {
                            if let Some(col) = self.col_map.get(*var) {
                                check(
                                    unsafe { ffi::XPRSchgobj(self.prob, 1, &col, cost) },
                                    "XPRSchgobj (obj switch load)",
                                )?;
                            }
                        }
                    }
                    if let Some(&sense) = self.obj_senses.get(new_obj) {
                        check(
                            unsafe { ffi::XPRSchgobjsense(self.prob, sense_to_xprs(sense)) },
                            "XPRSchgobjsense (obj switch)",
                        )?;
                    }
                    self.active_obj = Some(*new_obj);
                } else {
                    self.active_obj = None;
                }
            }

            // ── Objective Coefficient Cell Set ────────────────────────────
            ModelOp::SetObjectiveCell { cell_key, evaluated_value, .. } => {
                let (target, var_id) = cell_key;
                let obj_id = match target {
                    CoefficientTarget::Objective(obj) => obj,
                    CoefficientTarget::Constraint(_) => {
                        return Err(BackendError::new(
                            "SetObjectiveCell with Constraint target -- use SetCell instead",
                            ErrorCategory::InvalidInput,
                            HealthEffect::Recoverable,
                        ));
                    }
                };

                self.obj_costs
                    .entry(*obj_id)
                    .or_default()
                    .insert(*var_id, *evaluated_value);

                if Some(*obj_id) == self.active_obj {
                    if let Some(col) = self.col_map.get(*var_id) {
                        check(
                            unsafe { ffi::XPRSchgobj(self.prob, 1, &col, evaluated_value) },
                            "XPRSchgobj (set obj cell)",
                        )?;
                    }
                }
            }

            // ── Parameter Value Changed ───────────────────────────────────
            ModelOp::SetParameter { .. } => {}
        }
        Ok(())
    }

    // ── Snapshot rebuild ──────────────────────────────────────────────────

    fn rebuild_from_snapshot(&mut self, snapshot: &ModelSnapshot) -> Result<(), BackendError> {
        self.reset();

        let revision = snapshot.revision;
        info!("Rebuilding Xpress session from snapshot at revision {}", revision);

        // Step 1: Add all variables.
        for var in &snapshot.variables {
            let lb = xprs_bound(var.bounds.lower);
            let ub = xprs_bound(var.bounds.upper);
            let objc = [0.0f64];
            let start = [0i32];
            let col_idx = unsafe {
                let mut n = 0i32;
                ffi::XPRSgetintattrib(self.prob, ffi::XPRS_COLS, &mut n);
                n
            };
            check(
                unsafe {
                    ffi::XPRSaddcols(
                        self.prob, 1, 0,
                        objc.as_ptr(), start.as_ptr(),
                        std::ptr::null(), std::ptr::null(),
                        &lb, &ub,
                    )
                },
                "XPRSaddcols (rebuild)",
            )?;
            self.col_map.insert(var.id, col_idx);

            // Set integrality for integer/binary.
            match var.var_type {
                VarType::Continuous => {}
                VarType::Integer => {
                    let ct = b'I' as i8;
                    check(
                        unsafe { ffi::XPRSchgcoltype(self.prob, 1, &col_idx, &ct) },
                        "XPRSchgcoltype (rebuild integer)",
                    )?;
                    self.integer_vars.insert(var.id);
                }
                VarType::Binary => {
                    let ct = b'B' as i8;
                    check(
                        unsafe { ffi::XPRSchgcoltype(self.prob, 1, &col_idx, &ct) },
                        "XPRSchgcoltype (rebuild binary)",
                    )?;
                    self.integer_vars.insert(var.id);
                }
            }

            // Handle semi-continuous.
            if let Some(_sclb) = var.semicontinuous_lower {
                self.semicontinuous_vars.insert(var.id);
                let ct = if self.integer_vars.contains(&var.id) {
                    b'R' as i8   // Semi-continuous + integer
                } else {
                    b'S' as i8   // Semi-continuous + continuous
                };
                check(
                    unsafe { ffi::XPRSchgcoltype(self.prob, 1, &col_idx, &ct) },
                    "XPRSchgcoltype (rebuild semicontinuous)",
                )?;
            }

            // Inactive variables: fix to [0, 0].
            if !var.active {
                let cols  = [col_idx, col_idx];
                let types = [b'L' as i8, b'U' as i8];
                let vals  = [0.0, 0.0];
                check(
                    unsafe {
                        ffi::XPRSchgbounds(self.prob, 2, cols.as_ptr(), types.as_ptr(), vals.as_ptr())
                    },
                    "XPRSchgbounds (rebuild inactive var)",
                )?;
            }

            self.var_bounds.insert(var.id, (var.bounds.lower, var.bounds.upper));
        }

        // Step 2: Add all constraints (empty rows).
        for con in &snapshot.constraints {
            let (rt, rhs, rng) = xprs_row(con.bounds.lower, con.bounds.upper);
            let row_idx = unsafe {
                let mut n = 0i32;
                ffi::XPRSgetintattrib(self.prob, ffi::XPRS_ROWS, &mut n);
                n
            };
            let start = [0i32];
            check(
                unsafe {
                    ffi::XPRSaddrows(
                        self.prob, 1, 0,
                        &(rt as i8), &rhs, &rng,
                        start.as_ptr(),
                        std::ptr::null(), std::ptr::null(),
                    )
                },
                "XPRSaddrows (rebuild)",
            )?;
            self.row_map.insert(con.id, row_idx);
            self.con_bounds.insert(con.id, (con.bounds.lower, con.bounds.upper));
        }

        // Step 3: Add constraint coefficient cells.
        for cell in &snapshot.cells {
            let (target, var_id) = cell.cell_key;
            if let Some(col) = self.col_map.get(var_id) {
                match target {
                    CoefficientTarget::Constraint(con_id) => {
                        if let Some(row) = self.row_map.get(con_id) {
                            check(
                                unsafe { ffi::XPRSchgcoef(self.prob, row, col, cell.evaluated_value) },
                                "XPRSchgcoef (rebuild cell)",
                            )?;
                        }
                    }
                    CoefficientTarget::Objective(_) => {
                        // Processed in step 4.
                    }
                }
            }
        }

        // Step 4: Add objectives and objective coefficient cells.
        for obj in &snapshot.objectives {
            self.obj_senses.insert(obj.id, obj.sense);
            let mut costs: HashMap<VarId, f64> = HashMap::new();

            for cell in &snapshot.cells {
                let (target, var_id) = cell.cell_key;
                if let CoefficientTarget::Objective(obj_id) = target {
                    if obj_id == obj.id {
                        costs.insert(var_id, cell.evaluated_value);
                    }
                }
            }

            if obj.active {
                self.active_obj = Some(obj.id);

                // Load costs for active objective.
                for (&vid, &cost) in &costs {
                    if let Some(col) = self.col_map.get(vid) {
                        check(
                            unsafe { ffi::XPRSchgobj(self.prob, 1, &col, &cost) },
                            "XPRSchgobj (rebuild obj cost)",
                        )?;
                    }
                }

                // Set sense.
                check(
                    unsafe { ffi::XPRSchgobjsense(self.prob, sense_to_xprs(obj.sense)) },
                    "XPRSchgobjsense (rebuild)",
                )?;
            }

            self.obj_costs.insert(obj.id, costs);
        }

        // Step 5: Handle inactive constraints -- set to non-binding.
        for con in &snapshot.constraints {
            if !con.active {
                if let Some(row) = self.row_map.get(con.id) {
                    check(
                        unsafe { ffi::XPRSchgrowtype(self.prob, 1, &row, &(b'N' as i8)) },
                        "XPRSchgrowtype (rebuild inactive con)",
                    )?;
                }
            }
        }

        info!("Rebuild complete at revision {}", revision);
        Ok(())
    }

    // ── Apply a delta batch ───────────────────────────────────────────────

    fn apply_delta_batch(&mut self, batch: &DeltaBatch) -> Result<(), BackendError> {
        info!(
            "Applying delta batch r{} -> r{} ({} ops)",
            batch.from,
            batch.to,
            batch.operations.len()
        );

        for op in &batch.operations {
            self.apply_model_op(op)?;
        }

        Ok(())
    }

    // ── Reset ─────────────────────────────────────────────────────────────

    fn reset(&mut self) {
        if !self.prob.is_null() {
            unsafe { ffi::XPRSdestroyprob(self.prob) };
        }
        self.prob = make_prob(&self.opts);

        self.col_map         = IndexMap::new();
        self.row_map         = IndexMap::new();
        self.var_bounds      = HashMap::new();
        self.con_bounds      = HashMap::new();
        self.integer_vars    = HashSet::new();
        self.semicontinuous_vars = HashSet::new();
        self.obj_costs       = HashMap::new();
        self.obj_senses      = HashMap::new();
        self.active_obj      = None;
        self.current_solution = None;
        self.last_status     = None;
    }

    // ── Solve request negotiation ─────────────────────────────────────────

    fn negotiate_options(&self, request: &SolveRequest) -> Result<EffectiveConfig, BackendError> {
        let mut effective = EffectiveConfig::default();

        // lp_algorithm
        if let Some(algo) = &request.lp_algorithm {
            match algo {
                LpAlgorithm::Automatic => {
                    effective.lp_algorithm = Some(LpAlgorithm::Automatic);
                }
                LpAlgorithm::PrimalSimplex => {
                    set_int_control(self.prob, ffi::XPRS_DEFAULTALG, ffi::ALG_PRIMAL_SIMPLEX, "DEFAULTALG (primal)");
                    effective.lp_algorithm = Some(LpAlgorithm::PrimalSimplex);
                }
                LpAlgorithm::DualSimplex => {
                    set_int_control(self.prob, ffi::XPRS_DEFAULTALG, ffi::ALG_DUAL_SIMPLEX, "DEFAULTALG (dual)");
                    effective.lp_algorithm = Some(LpAlgorithm::DualSimplex);
                }
                LpAlgorithm::Barrier => {
                    set_int_control(self.prob, ffi::XPRS_DEFAULTALG, ffi::ALG_BARRIER, "DEFAULTALG (barrier)");
                    effective.lp_algorithm = Some(LpAlgorithm::Barrier);
                }
            }
        }

        // time_limit_secs
        if let Some(t) = request.time_limit_secs {
            set_double_control(self.prob, ffi::XPRS_TIMELIMIT, t, "TIMELIMIT");
            effective.time_limit_secs = Some(t);
        }

        // mip_rel_gap
        if let Some(g) = request.mip_rel_gap {
            set_double_control(self.prob, ffi::XPRS_MIPRELSTOP, g, "MIPRELSTOP");
            effective.mip_rel_gap = Some(g);
        }

        // mip_abs_gap
        if let Some(g) = request.mip_abs_gap {
            set_double_control(self.prob, ffi::XPRS_MIPABSSTOP, g, "MIPABSSTOP");
            effective.mip_rel_gap = Some(g);
        }

        // threads
        if let Some(t) = request.threads {
            set_int_control(self.prob, ffi::XPRS_THREADS, t, "THREADS");
            effective.threads = Some(t);
        }

        // enable_output
        if let Some(enabled) = request.enable_output {
            let level: i32 = if enabled { 1 } else { 0 };
            set_int_control(self.prob, ffi::XPRS_OUTPUTLOG, level, "OUTPUTLOG");
            effective.enable_output = Some(enabled);
        }

        // random_seed -- not directly supported by Xpress controls
        if request.random_seed.is_some() {
            effective.rejections.push(ConfigRejection {
                key: "random_seed".into(),
                reason: "Xpress does not have a direct random_seed control".into(),
            });
        }

        // extra_options
        for (key, value) in &request.extra_options {
            // Xpress requires integer control codes, not string keys.
            // Reject all named extra options since we cannot map them.
            effective.rejections.push(ConfigRejection {
                key: key.clone(),
                reason: format!(
                    "Xpress options must be set by integer control code; cannot map '{}' = '{}'",
                    key, value
                ),
            });
        }

        Ok(effective)
    }
}

// ── Drop ──────────────────────────────────────────────────────────────────

impl Drop for XpressSession {
    fn drop(&mut self) {
        if !self.prob.is_null() {
            unsafe { ffi::XPRSdestroyprob(self.prob) };
            self.prob = std::ptr::null_mut();
        }
    }
}

// ── Helper: map Objective Sense to Xpress sense constants ─────────────────

fn sense_to_xprs(sense: Sense) -> ffi::XprsInt {
    match sense {
        Sense::Minimize => ffi::OBJ_MINIMIZE,
        Sense::Maximize => ffi::OBJ_MAXIMIZE,
    }
}

// ── BackendSession implementation ─────────────────────────────────────────

impl BackendSession for XpressSession {
    fn synchronize(&mut self, sync: Synchronization) -> Result<SyncReceipt, BackendError> {
        match sync {
            Synchronization::Rebuild(snapshot) => {
                let revision = snapshot.revision;
                self.rebuild_from_snapshot(&snapshot)?;
                self.cursor.mark_ready(revision);
                // T-11-18: Invalidate stale solution after model mutation.
                self.current_solution = None;
                self.last_status = None;
            }
            Synchronization::DeltaBatch(batch) => {
                self.apply_delta_batch(&batch)?;
                self.cursor.advance(&batch).map_err(|e| {
                    BackendError::new(
                        format!("cursor failed to advance after delta: {}", e),
                        ErrorCategory::Internal,
                        HealthEffect::Terminal,
                    )
                })?;
                // T-11-18: Invalidate stale solution after model mutation.
                self.current_solution = None;
                self.last_status = None;
            }
        }

        Ok(SyncReceipt {
            cursor: self.cursor.clone(),
            health: self.cursor.health,
        })
    }

    fn solve(&mut self, request: &SolveRequest) -> Result<SolveResult, BackendError> {
        info!("Solving with Xpress");

        // Invalidate previous solution.
        self.current_solution = None;
        self.last_status = None;

        // Apply request options.
        let effective_config = self.negotiate_options(request)?;

        // Clear algorithm override (not stored for next solve).
        let flags = CString::new("").unwrap();

        let ret = if self.is_mip() {
            unsafe { ffi::XPRSmipoptimize(self.prob, flags.as_ptr()) }
        } else {
            unsafe { ffi::XPRSlpoptimize(self.prob, flags.as_ptr()) }
        };
        if ret != 0 {
            return Err(xprs_err(format!("XPRSoptimize returned {ret}")));
        }

        // Map solver termination status.
        let status = if self.is_mip() {
            let mut mip_status = 0i32;
            unsafe { ffi::XPRSgetintattrib(self.prob, ffi::XPRS_MIPSTATUS, &mut mip_status) };
            match mip_status {
                ffi::MIP_OPTIMAL  => TerminationStatus::Optimal,
                ffi::MIP_INFEAS   => TerminationStatus::Infeasible,
                ffi::MIP_UNBOUNDED => TerminationStatus::Unbounded,
                _ => TerminationStatus::Error,
            }
        } else {
            let mut lp_status = 0i32;
            unsafe { ffi::XPRSgetintattrib(self.prob, ffi::XPRS_LPSTATUS, &mut lp_status) };
            match lp_status {
                ffi::LP_OPTIMAL   => TerminationStatus::Optimal,
                ffi::LP_INFEAS    => TerminationStatus::Infeasible,
                ffi::LP_UNBOUNDED => TerminationStatus::Unbounded,
                _ => TerminationStatus::Error,
            }
        };
        self.last_status = Some(status);
        info!("Solve completed with status: {:?}", status);

        // Extract solution data.
        let solution = if status == TerminationStatus::Optimal {
            let ncols = unsafe {
                let mut n = 0i32;
                ffi::XPRSgetintattrib(self.prob, ffi::XPRS_COLS, &mut n);
                n as usize
            };
            let nrows = unsafe {
                let mut n = 0i32;
                ffi::XPRSgetintattrib(self.prob, ffi::XPRS_ROWS, &mut n);
                n as usize
            };

            let mut x     = vec![0.0f64; ncols];
            let mut duals = vec![0.0f64; nrows];
            let mut djs   = vec![0.0f64; ncols];

            if self.is_mip() {
                unsafe {
                    ffi::XPRSgetmipsol(
                        self.prob,
                        x.as_mut_ptr(),
                        std::ptr::null_mut(),
                    )
                };

                let mut obj_val = 0.0f64;
                unsafe { ffi::XPRSgetdblattrib(self.prob, ffi::XPRS_MIPOBJVAL, &mut obj_val) };

                let variable_values: Vec<(VarId, f64)> = self.col_map.iter()
                    .map(|(var, col)| (var, x[col as usize]))
                    .collect();

                Some(SolveSolution {
                    variable_values,
                    objective_value: Some(obj_val),
                    dual_values: None,
                    reduced_costs: None,
                })
            } else {
                unsafe {
                    ffi::XPRSgetlpsol(
                        self.prob,
                        x.as_mut_ptr(),
                        std::ptr::null_mut(),
                        duals.as_mut_ptr(),
                        djs.as_mut_ptr(),
                    )
                };

                let mut obj_val = 0.0f64;
                unsafe { ffi::XPRSgetdblattrib(self.prob, ffi::XPRS_LPOBJVAL, &mut obj_val) };

                let variable_values: Vec<(VarId, f64)> = self.col_map.iter()
                    .map(|(var, col)| (var, x[col as usize]))
                    .collect();

                // Dual values: ConId -> dual
                let dual_values: Vec<(ConId, f64)> = self.row_map.iter()
                    .map(|(con, row)| (con, duals[row as usize]))
                    .collect();

                // Reduced costs: VarId -> reduced cost
                let reduced_costs: Vec<(VarId, f64)> = self.col_map.iter()
                    .map(|(var, col)| (var, djs[col as usize]))
                    .collect();

                Some(SolveSolution {
                    variable_values,
                    objective_value: Some(obj_val),
                    dual_values: Some(dual_values),
                    reduced_costs: Some(reduced_costs),
                })
            }
        } else {
            None
        };

        self.current_solution = solution.clone();

        Ok(SolveResult {
            effective_configuration: effective_config,
            termination: status,
            solution,
        })
    }

    fn close(self) -> Result<(), BackendError> {
        info!("Closing Xpress session");
        // Drop handles XPRSdestroyprob.
        Ok(())
    }
}

// ── SessionHealth implementation ─────────────────────────────────────────

impl SessionHealth for XpressSession {
    fn health(&self) -> roml::sync::AdapterHealth {
        self.cursor.health
    }

    fn revision(&self) -> ModelRevision {
        self.cursor.applied_revision
    }
}

// ── SolutionView implementation ──────────────────────────────────────────

impl SolutionView for XpressSession {
    fn value(&self, var: VarId) -> Option<f64> {
        self.current_solution
            .as_ref()
            .and_then(|sol| sol.variable_values.iter().find(|(id, _)| *id == var).map(|(_, v)| *v))
    }

    fn dual(&self, con: ConId) -> Option<f64> {
        self.current_solution.as_ref().and_then(|sol| {
            sol.dual_values
                .as_ref()
                .and_then(|duals| duals.iter().find(|(id, _)| *id == con).map(|(_, v)| *v))
        })
    }

    fn reduced_cost(&self, var: VarId) -> Option<f64> {
        self.current_solution.as_ref().and_then(|sol| {
            sol.reduced_costs
                .as_ref()
                .and_then(|costs| costs.iter().find(|(id, _)| *id == var).map(|(_, v)| *v))
        })
    }

    fn objective_value(&self) -> Option<f64> {
        self.current_solution.as_ref().and_then(|sol| sol.objective_value)
    }
}

// ── BackendMetadata implementation ──────────────────────────────────────

impl BackendMetadata for XpressSession {
    fn name(&self) -> &str {
        "Xpress (FICO)"
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            lp: true,
            mip: true,
            solution: true,
            duals: true,
            reduced_costs: true,
            semicontinuous: true,
            semiinteger: true,
            add_variable: true,
            add_constraint: true,
            set_coefficient: true,
            set_bounds: true,
            set_objective: true,
            delete: true,
            callbacks: false,
            parameter_update: false,
        }
    }
}
