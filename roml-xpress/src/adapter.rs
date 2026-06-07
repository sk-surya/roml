//! FICO Xpress solver adapter implementing roml's `SolverAdapter` trait.
//!
//! # Design
//!
//! - All Xpress state is owned by an opaque `XPRSprob` handle.
//! - `col_map` / `row_map` maintain Id → Xpress-index bidirectional maps.
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

use roml::id::{ConId, ObjId, VarId};
use roml::model::changelog::Change;
use roml::model::coefficient::CoefficientTarget;
use roml::model::objective::Sense;
use roml::model::variable::VarType;
use roml::solver::{SolverAdapter, SolverError, SolverStatus};

use crate::ffi;
use crate::index_map::IndexMap;

// ── Library-level init (call XPRSinit exactly once per process) ────────────

static XPRESS_INIT: OnceLock<()> = OnceLock::new();

fn ensure_xpress_init() {
    XPRESS_INIT.get_or_init(|| {
        // XPRESS_DIR env var: path to xpressmp directory.
        // We pass its bin/ subdirectory to XPRSinit so it finds xpauth.xpr.
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

// ── Helper: map constraint (lb, ub) → Xpress row type + rhs + range ────────

/// Returns `(rowtype_byte, rhs, rng)` for an Xpress row representing `lb ≤ ax ≤ ub`.
///
/// Xpress range rows use: `rhs - rng ≤ ax ≤ rhs` when `rng > 0`.
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

/// Clamp Rust ±infinity to Xpress ±1e20.
fn xprs_bound(v: f64) -> f64 {
    if v == f64::INFINITY      { ffi::XPRS_PLUSINFINITY  }
    else if v == f64::NEG_INFINITY { ffi::XPRS_MINUSINFINITY }
    else { v }
}

// ── Options ────────────────────────────────────────────────────────────────

/// Configuration forwarded to Xpress at adapter creation time.
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

// ── Adapter struct ─────────────────────────────────────────────────────────

pub struct XpressAdapter {
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

    status:          SolverStatus,
    solution:        Option<HashMap<VarId, f64>>,
    objective_value: Option<f64>,
    duals:           Option<HashMap<ConId, f64>>,
    reduced_costs:   Option<HashMap<VarId, f64>>,
}

// SAFETY: XPRSprob is a C pointer. We never share it across threads.
unsafe impl Send for XpressAdapter {}

// ── Construction helpers ───────────────────────────────────────────────────

fn make_prob(opts: &XpressOptions) -> ffi::XPRSprob {
    let mut prob: ffi::XPRSprob = std::ptr::null_mut();
    let ret = unsafe { ffi::XPRScreateprob(&mut prob) };
    assert!(ret == 0 && !prob.is_null(), "XPRScreateprob failed (code {ret})");

    let output_level: i32 = if opts.console_output { 1 } else { opts.log_level };
    set_int_control(prob, ffi::XPRS_OUTPUTLOG, output_level, "OUTPUTLOG");
    // Keep presolve configurable because disabling it allows incremental
    // LP reoptimization to retain and extend the existing basis.
    set_int_control(
        prob,
        ffi::XPRS_PRESOLVE,
        i32::from(opts.presolve),
        "PRESOLVE",
    );
    if let Some(t) = opts.threads {
        set_int_control(prob, ffi::XPRS_THREADS, t, "THREADS");
    }
    if let Some(secs) = opts.max_time {
        set_double_control(prob, ffi::XPRS_TIMELIMIT, secs, "TIMELIMIT");
    }
    unsafe {
        // Always install message callback (overhead is negligible; OUTPUTLOG=0 suppresses messages)
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

/// C-callable message callback — forwards Xpress output to stdout.
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

// ── SolverError helpers ────────────────────────────────────────────────────

fn xprs_err(msg: impl Into<String>) -> SolverError {
    SolverError::InternalError(msg.into())
}

fn check(ret: ffi::XprsRes, op: &str) -> Result<(), SolverError> {
    if ret == 0 { Ok(()) } else { Err(xprs_err(format!("{op} returned {ret}"))) }
}

// ── impl XpressAdapter ─────────────────────────────────────────────────────

impl XpressAdapter {
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
            status:          SolverStatus::NotSolved,
            solution:        None,
            objective_value: None,
            duals:           None,
            reduced_costs:   None,
        }
    }

    fn is_mip(&self) -> bool {
        !self.integer_vars.is_empty()
    }

    fn supports_bulk_additive_sync(changes: &[Change]) -> bool {
        !changes.iter().any(|change| {
            matches!(
                change,
                Change::VariableRemoved { .. }
                    | Change::VariableActivityChanged { .. }
                    | Change::ConstraintRemoved { .. }
                    | Change::ConstraintActivityChanged { .. }
                    | Change::CoefficientRemoved { .. }
                    | Change::CoefficientValueChanged { .. }
                    | Change::ObjectiveRemoved { .. }
            )
        })
    }

    fn reload_active_objective(&mut self) -> Result<(), SolverError> {
        let ncols = self.col_map.len();
        if ncols == 0 {
            return Ok(());
        }

        let mut cols = Vec::with_capacity(ncols);
        let mut values = Vec::with_capacity(ncols);
        let costs = self.active_obj.and_then(|obj| self.obj_costs.get(&obj));
        for (var, col) in self.col_map.iter() {
            cols.push(col);
            values.push(costs.and_then(|entries| entries.get(&var)).copied().unwrap_or(0.0));
        }
        check(
            unsafe {
                ffi::XPRSchgobj(
                    self.prob,
                    ncols as i32,
                    cols.as_ptr(),
                    values.as_ptr(),
                )
            },
            "XPRSchgobj (bulk objective reload)",
        )?;

        if let Some(obj) = self.active_obj {
            if let Some(&sense) = self.obj_senses.get(&obj) {
                check(
                    unsafe { ffi::XPRSchgobjsense(self.prob, sense_to_xprs(sense)) },
                    "XPRSchgobjsense (bulk objective reload)",
                )?;
            }
        }
        Ok(())
    }

    fn apply_bulk_additive_changes(&mut self, changes: &[Change]) -> Result<(), SolverError> {
        let mut new_vars = Vec::new();
        let mut new_var_state = HashMap::new();
        let mut new_cons = Vec::new();
        let mut new_con_bounds = HashMap::new();

        for change in changes {
            match change {
                Change::VariableAdded { var, bounds, var_type } => {
                    new_vars.push(*var);
                    new_var_state.insert(*var, (*bounds, *var_type));
                }
                Change::VariableBoundsChanged { var, new, .. } => {
                    if let Some((bounds, _)) = new_var_state.get_mut(var) {
                        *bounds = *new;
                    }
                }
                Change::VariableTypeChanged { var, new, .. } => {
                    if let Some((_, var_type)) = new_var_state.get_mut(var) {
                        *var_type = *new;
                    }
                }
                Change::ConstraintAdded { con, bounds } => {
                    new_cons.push(*con);
                    new_con_bounds.insert(*con, *bounds);
                }
                Change::ConstraintBoundsChanged { con, new, .. } => {
                    if let Some(bounds) = new_con_bounds.get_mut(con) {
                        *bounds = *new;
                    }
                }
                _ => {}
            }
        }

        if !new_vars.is_empty() {
            let first_col = unsafe {
                let mut count = 0;
                ffi::XPRSgetintattrib(self.prob, ffi::XPRS_COLS, &mut count);
                count
            };
            let obj = vec![0.0; new_vars.len()];
            let starts = vec![0; new_vars.len()];
            let mut lower = Vec::with_capacity(new_vars.len());
            let mut upper = Vec::with_capacity(new_vars.len());
            for var in &new_vars {
                let (bounds, _) = new_var_state[var];
                lower.push(xprs_bound(bounds.lower));
                upper.push(xprs_bound(bounds.upper));
            }
            check(
                unsafe {
                    ffi::XPRSaddcols(
                        self.prob,
                        new_vars.len() as i32,
                        0,
                        obj.as_ptr(),
                        starts.as_ptr(),
                        std::ptr::null(),
                        std::ptr::null(),
                        lower.as_ptr(),
                        upper.as_ptr(),
                    )
                },
                "XPRSaddcols (bulk)",
            )?;

            let mut integer_cols = Vec::new();
            let mut integer_types = Vec::new();
            for (offset, var) in new_vars.iter().enumerate() {
                let (bounds, var_type) = new_var_state[var];
                let col = first_col + offset as i32;
                self.col_map.insert(*var, col);
                self.var_bounds.insert(*var, (bounds.lower, bounds.upper));
                match var_type {
                    VarType::Continuous => {}
                    VarType::Integer => {
                        self.integer_vars.insert(*var);
                        integer_cols.push(col);
                        integer_types.push(b'I' as i8);
                    }
                    VarType::Binary => {
                        self.integer_vars.insert(*var);
                        integer_cols.push(col);
                        integer_types.push(b'B' as i8);
                    }
                }
            }
            if !integer_cols.is_empty() {
                check(
                    unsafe {
                        ffi::XPRSchgcoltype(
                            self.prob,
                            integer_cols.len() as i32,
                            integer_cols.as_ptr(),
                            integer_types.as_ptr(),
                        )
                    },
                    "XPRSchgcoltype (bulk)",
                )?;
            }
        }

        if !new_cons.is_empty() {
            let first_row = unsafe {
                let mut count = 0;
                ffi::XPRSgetintattrib(self.prob, ffi::XPRS_ROWS, &mut count);
                count
            };
            let starts = vec![0; new_cons.len()];
            let mut row_types = Vec::with_capacity(new_cons.len());
            let mut rhs = Vec::with_capacity(new_cons.len());
            let mut ranges = Vec::with_capacity(new_cons.len());
            for con in &new_cons {
                let bounds = new_con_bounds[con];
                let (row_type, row_rhs, range) = xprs_row(bounds.lower, bounds.upper);
                row_types.push(row_type as i8);
                rhs.push(row_rhs);
                ranges.push(range);
            }
            check(
                unsafe {
                    ffi::XPRSaddrows(
                        self.prob,
                        new_cons.len() as i32,
                        0,
                        row_types.as_ptr(),
                        rhs.as_ptr(),
                        ranges.as_ptr(),
                        starts.as_ptr(),
                        std::ptr::null(),
                        std::ptr::null(),
                    )
                },
                "XPRSaddrows (bulk)",
            )?;
            for (offset, con) in new_cons.iter().enumerate() {
                let bounds = new_con_bounds[con];
                self.row_map.insert(*con, first_row + offset as i32);
                self.con_bounds.insert(*con, (bounds.lower, bounds.upper));
            }
        }

        let new_var_set: HashSet<_> = new_vars.iter().copied().collect();
        let new_con_set: HashSet<_> = new_cons.iter().copied().collect();
        let mut matrix_rows = Vec::new();
        let mut matrix_cols = Vec::new();
        let mut matrix_values = Vec::new();
        let mut objective_dirty = false;

        for change in changes {
            match change {
                Change::VariableAdded { .. } | Change::ConstraintAdded { .. } => {}
                Change::VariableBoundsChanged { var, .. }
                | Change::VariableTypeChanged { var, .. }
                    if new_var_set.contains(var) => {}
                Change::ConstraintBoundsChanged { con, .. } if new_con_set.contains(con) => {}
                Change::CoefficientAdded { var, target, value, .. } => match target {
                    CoefficientTarget::Constraint(con) => {
                        let row = self
                            .row_map
                            .get(*con)
                            .ok_or_else(|| xprs_err(format!("missing row for {con:?}")))?;
                        let col = self
                            .col_map
                            .get(*var)
                            .ok_or_else(|| xprs_err(format!("missing column for {var:?}")))?;
                        matrix_rows.push(row);
                        matrix_cols.push(col);
                        matrix_values.push(*value);
                    }
                    CoefficientTarget::Objective(obj) => {
                        self.obj_costs.entry(*obj).or_default().insert(*var, *value);
                        objective_dirty |= self.active_obj == Some(*obj);
                    }
                },
                Change::ActiveObjectiveChanged { new, .. } => {
                    self.active_obj = *new;
                    objective_dirty = true;
                }
                _ => self.apply_one(change)?,
            }
        }

        if !matrix_rows.is_empty() {
            check(
                unsafe {
                    ffi::XPRSchgmcoef(
                        self.prob,
                        matrix_rows.len() as i32,
                        matrix_rows.as_ptr(),
                        matrix_cols.as_ptr(),
                        matrix_values.as_ptr(),
                    )
                },
                "XPRSchgmcoef (bulk)",
            )?;
        }
        if objective_dirty {
            self.reload_active_objective()?;
        }
        Ok(())
    }

    // ── Row-type application ───────────────────────────────────────────────

    fn apply_row_bounds(&self, row: i32, lb: f64, ub: f64) -> Result<(), SolverError> {
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

    // ── Apply a single change ──────────────────────────────────────────────

    fn apply_one(&mut self, change: &Change) -> Result<(), SolverError> {
        match change {
            // ── Variable Added ────────────────────────────────────────────
            Change::VariableAdded { var, bounds, var_type } => {
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
            Change::VariableRemoved { var } => {
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

            // ── Variable Bounds Changed ────────────────────────────────────
            Change::VariableBoundsChanged { var, new, .. } => {
                if let Some(col) = self.col_map.get(*var) {
                    let lb = xprs_bound(new.lower);
                    let ub = xprs_bound(new.upper);
                    let cols  = [col, col];
                    let types = [b'L' as i8, b'U' as i8];
                    let vals  = [lb, ub];
                    check(
                        unsafe { ffi::XPRSchgbounds(self.prob, 2, cols.as_ptr(), types.as_ptr(), vals.as_ptr()) },
                        "XPRSchgbounds (var bounds)",
                    )?;
                    self.var_bounds.insert(*var, (new.lower, new.upper));
                }
            }

            // ── Variable Type Changed ──────────────────────────────────────
            Change::VariableTypeChanged { var, new, .. } => {
                if let Some(col) = self.col_map.get(*var) {
                    let ct = if self.semicontinuous_vars.contains(var) {
                        // Semi-continuous + integer → 'R'
                        match new {
                            VarType::Continuous => { self.integer_vars.remove(var); }
                            _ => { self.integer_vars.insert(*var); }
                        }
                        b'R' as i8
                    } else {
                        match new {
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

            // ── Variable Semi-Continuous Bound Changed ─────────────────────
            Change::SemiContinuousBoundChanged { var, .. } => {
                if let Some(col) = self.col_map.get(*var) {
                    self.semicontinuous_vars.insert(*var);
                    let ct = if self.integer_vars.contains(var) {
                        b'R' as i8  // SC + integer
                    } else {
                        b'S' as i8  // SC + continuous
                    };
                    check(
                        unsafe { ffi::XPRSchgcoltype(self.prob, 1, &col, &ct) },
                        "XPRSchgcoltype (semi-continuous)",
                    )?;
                }
            }

            // ── Variable Activity Changed ──────────────────────────────────
            Change::VariableActivityChanged { var, active } => {
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

            // ── Constraint Added ───────────────────────────────────────────
            Change::ConstraintAdded { con, bounds } => {
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

            // ── Constraint Removed ─────────────────────────────────────────
            Change::ConstraintRemoved { con } => {
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

            // ── Constraint Bounds Changed ──────────────────────────────────
            Change::ConstraintBoundsChanged { con, new, .. } => {
                if let Some(row) = self.row_map.get(*con) {
                    self.apply_row_bounds(row, new.lower, new.upper)?;
                    self.con_bounds.insert(*con, (new.lower, new.upper));
                }
            }

            // ── Constraint Activity Changed ────────────────────────────────
            Change::ConstraintActivityChanged { con, active } => {
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

            // ── Coefficient Added ──────────────────────────────────────────
            Change::CoefficientAdded { var, target, value, .. } => {
                match target {
                    CoefficientTarget::Constraint(con) => {
                        if let (Some(row), Some(col)) =
                            (self.row_map.get(*con), self.col_map.get(*var))
                        {
                            check(
                                unsafe { ffi::XPRSchgcoef(self.prob, row, col, *value) },
                                "XPRSchgcoef (add)",
                            )?;
                        }
                    }
                    CoefficientTarget::Objective(obj) => {
                        self.obj_costs.entry(*obj).or_default().insert(*var, *value);
                        if Some(*obj) == self.active_obj {
                            if let Some(col) = self.col_map.get(*var) {
                                check(
                                    unsafe { ffi::XPRSchgobj(self.prob, 1, &col, value) },
                                    "XPRSchgobj (coeff add)",
                                )?;
                            }
                        }
                    }
                }
            }

            // ── Coefficient Removed ────────────────────────────────────────
            Change::CoefficientRemoved { var, target, .. } => {
                match target {
                    CoefficientTarget::Constraint(con) => {
                        if let (Some(row), Some(col)) =
                            (self.row_map.get(*con), self.col_map.get(*var))
                        {
                            check(
                                unsafe { ffi::XPRSchgcoef(self.prob, row, col, 0.0) },
                                "XPRSchgcoef (remove)",
                            )?;
                        }
                    }
                    CoefficientTarget::Objective(obj) => {
                        if let Some(costs) = self.obj_costs.get_mut(obj) {
                            costs.remove(var);
                        }
                        if Some(*obj) == self.active_obj {
                            if let Some(col) = self.col_map.get(*var) {
                                let zero = 0.0f64;
                                check(
                                    unsafe { ffi::XPRSchgobj(self.prob, 1, &col, &zero) },
                                    "XPRSchgobj (coeff remove)",
                                )?;
                            }
                        }
                    }
                }
            }

            // ── Coefficient Value Changed ──────────────────────────────────
            Change::CoefficientValueChanged { var, target, new, .. } => {
                match target {
                    CoefficientTarget::Constraint(con) => {
                        if let (Some(row), Some(col)) =
                            (self.row_map.get(*con), self.col_map.get(*var))
                        {
                            check(
                                unsafe { ffi::XPRSchgcoef(self.prob, row, col, *new) },
                                "XPRSchgcoef (update)",
                            )?;
                        }
                    }
                    CoefficientTarget::Objective(obj) => {
                        if let Some(costs) = self.obj_costs.get_mut(obj) {
                            costs.insert(*var, *new);
                        }
                        if Some(*obj) == self.active_obj {
                            if let Some(col) = self.col_map.get(*var) {
                                check(
                                    unsafe { ffi::XPRSchgobj(self.prob, 1, &col, new) },
                                    "XPRSchgobj (coeff update)",
                                )?;
                            }
                        }
                    }
                }
            }

            // ── Objective Added ────────────────────────────────────────────
            Change::ObjectiveAdded { obj, sense } => {
                self.obj_senses.insert(*obj, *sense);
                self.obj_costs.entry(*obj).or_default();
            }

            // ── Objective Removed ──────────────────────────────────────────
            Change::ObjectiveRemoved { obj } => {
                self.obj_costs.remove(obj);
                self.obj_senses.remove(obj);
                if self.active_obj == Some(*obj) {
                    self.active_obj = None;
                }
            }

            // ── Objective Sense Changed ────────────────────────────────────
            Change::ObjectiveSenseChanged { obj, new, .. } => {
                self.obj_senses.insert(*obj, *new);
                if Some(*obj) == self.active_obj {
                    check(
                        unsafe { ffi::XPRSchgobjsense(self.prob, sense_to_xprs(*new)) },
                        "XPRSchgobjsense",
                    )?;
                }
            }

            // ── Active Objective Changed ───────────────────────────────────
            Change::ActiveObjectiveChanged { new, .. } => {
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

                if let Some(new_obj) = new {
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

            // ── Parameter Value Changed ────────────────────────────────────
            // No-op: coefficient delta events follow as CoefficientValueChanged.
            Change::ParameterValueChanged { .. } => {}
        }
        Ok(())
    }
}

fn sense_to_xprs(sense: Sense) -> ffi::XprsInt {
    match sense {
        Sense::Minimize => ffi::OBJ_MINIMIZE,
        Sense::Maximize => ffi::OBJ_MAXIMIZE,
    }
}

impl Drop for XpressAdapter {
    fn drop(&mut self) {
        if !self.prob.is_null() {
            unsafe { ffi::XPRSdestroyprob(self.prob) };
            self.prob = std::ptr::null_mut();
        }
    }
}

// ── SolverAdapter implementation ───────────────────────────────────────────

impl SolverAdapter for XpressAdapter {
    fn apply_changes(&mut self, changes: &[Change]) -> Result<(), SolverError> {
        if Self::supports_bulk_additive_sync(changes) {
            return self.apply_bulk_additive_changes(changes);
        }

        // Batch ConstraintAdded + its CoefficientAdded events into a single
        // XPRSaddrows call with all coefficients, rather than adding the row
        // empty and then calling XPRSchgcoef N times.
        let mut i = 0;
        while i < changes.len() {
            if let Change::ConstraintAdded { con, bounds } = &changes[i] {
                let (rt, rhs, rng) = xprs_row(bounds.lower, bounds.upper);

                let mut cols: Vec<i32> = Vec::new();
                let mut vals: Vec<f64> = Vec::new();
                let mut j = i + 1;
                while j < changes.len() {
                    if let Change::CoefficientAdded {
                        var,
                        target: CoefficientTarget::Constraint(target_con),
                        value,
                        ..
                    } = &changes[j]
                    {
                        if target_con == con {
                            if let Some(col) = self.col_map.get(*var) {
                                cols.push(col);
                                vals.push(*value);
                            }
                            j += 1;
                            continue;
                        }
                    }
                    break;
                }

                let row_idx = unsafe {
                    let mut n = 0i32;
                    ffi::XPRSgetintattrib(self.prob, ffi::XPRS_ROWS, &mut n);
                    n
                };

                let ncoefs = cols.len() as i32;
                let start  = [0i32];
                let ret = if cols.is_empty() {
                    unsafe {
                        ffi::XPRSaddrows(
                            self.prob,
                            1, 0,
                            &(rt as i8), &rhs, &rng,
                            start.as_ptr(),
                            std::ptr::null(), std::ptr::null(),
                        )
                    }
                } else {
                    unsafe {
                        ffi::XPRSaddrows(
                            self.prob,
                            1, ncoefs,
                            &(rt as i8), &rhs, &rng,
                            start.as_ptr(),
                            cols.as_ptr(),
                            vals.as_ptr(),
                        )
                    }
                };
                check(ret, "XPRSaddrows (batched)")?;
                self.row_map.insert(*con, row_idx);
                self.con_bounds.insert(*con, (bounds.lower, bounds.upper));

                i = j;
                continue;
            }

            self.apply_one(&changes[i])?;
            i += 1;
        }
        Ok(())
    }

    fn solve(&mut self) -> Result<SolverStatus, SolverError> {
        // Xpress can preserve and extend the basis between sequential LP solves
        // when callers explicitly disable presolve.
        self.solution        = None;
        self.objective_value = None;
        self.duals           = None;
        self.reduced_costs   = None;

        let empty_flags = CString::new("").unwrap();

        let ret = if self.is_mip() {
            unsafe { ffi::XPRSmipoptimize(self.prob, empty_flags.as_ptr()) }
        } else {
            unsafe { ffi::XPRSlpoptimize(self.prob, empty_flags.as_ptr()) }
        };
        if ret != 0 {
            return Err(xprs_err(format!("XPRSoptimize returned {ret}")));
        }

        // Read solver status.
        let status = if self.is_mip() {
            let mut mip_status = 0i32;
            unsafe { ffi::XPRSgetintattrib(self.prob, ffi::XPRS_MIPSTATUS, &mut mip_status) };
            match mip_status {
                ffi::MIP_OPTIMAL  => SolverStatus::Optimal,
                ffi::MIP_INFEAS   => SolverStatus::Infeasible,
                ffi::MIP_UNBOUNDED => SolverStatus::Unbounded,
                _ => SolverStatus::Error,
            }
        } else {
            let mut lp_status = 0i32;
            unsafe { ffi::XPRSgetintattrib(self.prob, ffi::XPRS_LPSTATUS, &mut lp_status) };
            match lp_status {
                ffi::LP_OPTIMAL   => SolverStatus::Optimal,
                ffi::LP_INFEAS    => SolverStatus::Infeasible,
                ffi::LP_UNBOUNDED => SolverStatus::Unbounded,
                _ => SolverStatus::Error,
            }
        };
        self.status = status;

        if status == SolverStatus::Optimal {
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
                self.objective_value = Some(obj_val);
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
                self.objective_value = Some(obj_val);

                // Dual values: row_idx → dual
                let mut dual_map = HashMap::new();
                for (con, row) in self.row_map.iter() {
                    dual_map.insert(con, duals[row as usize]);
                }
                self.duals = Some(dual_map);

                // Reduced costs: col_idx → reduced cost
                let mut rc_map = HashMap::new();
                for (var, col) in self.col_map.iter() {
                    rc_map.insert(var, djs[col as usize]);
                }
                self.reduced_costs = Some(rc_map);
            }

            // Primal solution map.
            let mut sol = HashMap::new();
            for (var, col) in self.col_map.iter() {
                sol.insert(var, x[col as usize]);
            }
            self.solution = Some(sol);
        }

        Ok(status)
    }

    fn status(&self) -> SolverStatus {
        self.status
    }

    fn solution_values(&self) -> Option<HashMap<VarId, f64>> {
        self.solution.clone()
    }

    fn objective_value_raw(&self) -> Option<f64> {
        self.objective_value
    }

    fn dual_values(&self) -> Option<HashMap<ConId, f64>> {
        self.duals.clone()
    }

    fn reduced_costs_raw(&self) -> Option<HashMap<VarId, f64>> {
        self.reduced_costs.clone()
    }

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
        self.obj_costs       = HashMap::new();
        self.obj_senses      = HashMap::new();
        self.active_obj      = None;
        self.status          = SolverStatus::NotSolved;
        self.solution        = None;
        self.objective_value = None;
        self.duals           = None;
        self.reduced_costs   = None;
    }

    fn set_console_output(&mut self, enabled: bool) -> Result<(), SolverError> {
        self.opts.console_output = enabled;
        let level: i32 = if enabled { 1 } else { 0 };
        unsafe { ffi::XPRSsetintcontrol(self.prob, ffi::XPRS_OUTPUTLOG, level); }
        Ok(())
    }

    fn supports_incremental(&self, _change: &Change) -> bool {
        true
    }

    fn supports_semi_continuous(&self) -> bool {
        true
    }
}
