//! MOSEK solver adapter implementing roml's `SolverAdapter` trait.
//!
//! # Design
//!
//! - One MOSEK environment is owned per adapter; a fresh Task is re-created on
//!   `reset()`.
//! - `col_map` / `row_map` maintain Id → MOSEK-index bidirectional maps.
//! - Variable/constraint bounds are cached so they can be restored when
//!   deactivated entities are reactivated.
//! - Objective costs per objective are cached so switching the active objective
//!   only requires zeroing the previous costs and loading the new ones.
//! - Inactive variables are fixed at [0, 0]; inactive constraints become free
//!   rows (FR bound key).
//! - When a column/row is deleted, `reindex_after_delete` keeps maps in sync
//!   with MOSEK's dense integer addressing.

#![allow(dead_code)]

use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use roml::delta::ModelOp;
use roml::id::{ConId, ObjId, VarId};
use roml::model::changelog::Change;
use roml::model::coefficient::CoefficientTarget;
use roml::model::objective::Sense;
use roml::model::variable::VarType;
use roml::revision::ModelRevision;
use roml::snapshot::ModelSnapshot;
use roml::solver::backend::{BackendCapabilities, BackendError, ErrorCategory, HealthEffect, TerminationStatus};
use roml::solver::callback::{CallbackAction, CallbackData, CallbackHandler};
use roml::solver::request::{ConfigAdjustment, ConfigRejection, EffectiveConfig, LpAlgorithm, SolveRequest, SolveResult, SolveSolution};
use roml::solver::session::{BackendMetadata, BackendSession, CallbackSession, SessionHealth, SolutionView, SyncReceipt, Synchronization};
use roml::sync::{AdapterCursor, AdapterHealth};
use log::{info, warn};

use crate::ffi::{self, MosekEnv, MosekTask};
use crate::index_map::IndexMap;

// ── Error helpers ──────────────────────────────────────────────────────────

fn mosek_err(msg: impl Into<String>) -> BackendError {
    BackendError::new(msg, ErrorCategory::Internal, HealthEffect::Recoverable)
}

fn check(ret: ffi::MosekRes, op: &str) -> Result<(), BackendError> {
    if ret == ffi::RES_OK {
        Ok(())
    } else {
        Err(mosek_err(format!("{op} returned code {ret}")))
    }
}

// ── Bound-key helper ───────────────────────────────────────────────────────

fn mosek_bounds(lb: f64, ub: f64) -> (ffi::MosekInt, f64, f64) {
    match (lb.is_finite(), ub.is_finite()) {
        (false, false) => (ffi::BK_FR, 0.0, 0.0),
        (true, false)  => (ffi::BK_LO, lb, 0.0),
        (false, true)  => (ffi::BK_UP, 0.0, ub),
        (true, true) if (lb - ub).abs() < f64::EPSILON => (ffi::BK_FX, lb, ub),
        (true, true)   => (ffi::BK_RA, lb, ub),
    }
}

// ── Options ────────────────────────────────────────────────────────────────

/// Which LP algorithm MOSEK should use.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MosekOptimizer {
    /// Let MOSEK choose (interior point for large LP, simplex otherwise).
    #[default]
    Free,
    /// Interior-point method.
    InteriorPoint,
    /// Legacy dual simplex.
    DualSimplex,
    /// Revised dual simplex introduced in MOSEK 11 (generally faster).
    NewDualSimplex,
    /// MOSEK picks between primal and dual simplex.
    FreeSimplex,
}

/// Simplex hotstart mode.
#[derive(Debug, Clone, Copy, Default)]
pub enum MosekSimHotstart {
    /// No hotstart — always solve from scratch.
    None,
    /// Let MOSEK decide whether to hotstart.
    #[default]
    Free,
    /// Reuse previous basis status keys.
    StatusKeys,
}

/// Options forwarded to MOSEK at adapter creation time.
#[derive(Debug, Clone, Default)]
pub struct MosekOptions {
    num_threads: Option<i32>,
    pub log_level: i32,
    max_time: Option<f64>,
    optimizer: MosekOptimizer,
    sim_hotstart: MosekSimHotstart,
    /// Also reuse the LU factorization from the previous solve (on top of status keys).
    sim_hotstart_lu: bool,
    /// Enable solver console output (iteration log). Default: false.
    pub console_output: bool,
}

impl MosekOptions {
    pub fn mio_max_time(mut self, seconds: f64) -> Self {
        self.max_time = Some(seconds);
        self
    }

    pub fn threads(mut self, n: i32) -> Self {
        self.num_threads = Some(n);
        self
    }

    /// Log verbosity; 0 = silent (default), 1+ = solver output to stdout.
    pub fn log_level(mut self, level: i32) -> Self {
        self.log_level = level;
        self
    }

    pub fn optimizer(mut self, opt: MosekOptimizer) -> Self {
        self.optimizer = opt;
        self
    }

    pub fn sim_hotstart(mut self, hs: MosekSimHotstart) -> Self {
        self.sim_hotstart = hs;
        self
    }

    pub fn sim_hotstart_lu(mut self, enabled: bool) -> Self {
        self.sim_hotstart_lu = enabled;
        self
    }

    pub fn console_output(mut self, enabled: bool) -> Self {
        self.console_output = enabled;
        self
    }
}

// ── Adapter struct ─────────────────────────────────────────────────────────

pub struct MosekAdapter {
    env:  MosekEnv,
    task: MosekTask,

    opts: MosekOptions,

    col_map: IndexMap<VarId>,
    row_map: IndexMap<ConId>,

    var_bounds:   HashMap<VarId, (f64, f64)>,
    con_bounds:   HashMap<ConId, (f64, f64)>,
    integer_vars: HashSet<VarId>,
    semicontinuous_vars: HashSet<VarId>,

    obj_costs:  HashMap<ObjId, HashMap<VarId, f64>>,
    obj_senses: HashMap<ObjId, Sense>,
    active_obj: Option<ObjId>,

    /// Synchronization cursor tracking applied revision and health.
    cursor: AdapterCursor,

    /// Termination status from the most recent solve.
    last_status: Option<TerminationStatus>,

    /// Solution from the most recent solve, if available.
    current_solution: Option<SolveSolution>,

    /// Optional callback handler for MIP lazy constraints / cutting planes.
    callback_handler: Option<Box<dyn CallbackHandler>>,

    /// Per-solve optimizer override. Set via request, consumed in `solve()`.
    next_optimizer: Option<MosekOptimizer>,
}

// SAFETY: MOSEK is a C library; we never share the handles across threads.
unsafe impl Send for MosekAdapter {}

impl MosekAdapter {
    pub fn new() -> Self {
        Self::with_options(MosekOptions::default())
    }

    pub fn with_options(opts: MosekOptions) -> Self {
        let mut env: MosekEnv = std::ptr::null_mut();
        let ret = unsafe { ffi::MSK_makeenv(&mut env, std::ptr::null()) };
        assert!(ret == ffi::RES_OK, "MSK_makeenv failed with code {ret}");

        let task = make_task(env, &opts);

        Self {
            env,
            task,
            opts,
            col_map: IndexMap::new(),
            row_map: IndexMap::new(),
            var_bounds: HashMap::new(),
            con_bounds: HashMap::new(),
            integer_vars: HashSet::new(),
            semicontinuous_vars: HashSet::new(),
            obj_costs: HashMap::new(),
            obj_senses: HashMap::new(),
            active_obj: None,
            cursor: AdapterCursor::new(),
            last_status: None,
            current_solution: None,
            callback_handler: None,
            next_optimizer: None,
        }
    }

    // ── Internal helpers ───────────────────────────────────────────────────

    fn sense_to_mosek(sense: Sense) -> ffi::MosekInt {
        match sense {
            Sense::Minimize => ffi::OBJ_SENSE_MINIMIZE,
            Sense::Maximize => ffi::OBJ_SENSE_MAXIMIZE,
        }
    }

    fn var_type_to_mosek(vt: VarType) -> ffi::MosekInt {
        match vt {
            VarType::Continuous => ffi::VAR_TYPE_CONT,
            VarType::Integer | VarType::Binary => ffi::VAR_TYPE_INT,
        }
    }

    fn is_mip(&self) -> bool {
        !self.integer_vars.is_empty()
    }

    fn soltype(&self) -> ffi::MosekInt {
        if self.is_mip() { ffi::SOL_ITG } else { ffi::SOL_BAS }
    }

    fn num_vars(&self) -> i32 {
        let mut n: ffi::MosekInt = 0;
        unsafe { ffi::MSK_getnumvar(self.task, &mut n) };
        n
    }

    fn num_cons(&self) -> i32 {
        let mut n: ffi::MosekInt = 0;
        unsafe { ffi::MSK_getnumcon(self.task, &mut n) };
        n
    }

    fn map_termination_status(&self, prosta: ffi::MosekInt, solsta: ffi::MosekInt) -> TerminationStatus {
        // Optimality is in solution status.
        if solsta == ffi::SOL_STA_OPTIMAL || solsta == ffi::SOL_STA_INTEGER_OPTIMAL {
            return TerminationStatus::Optimal;
        }
        // Infeasibility / unboundedness from problem status.
        match prosta {
            ffi::PRO_STA_PRIM_INFEAS
            | ffi::PRO_STA_PRIM_AND_DUAL_INFEAS => TerminationStatus::Infeasible,
            ffi::PRO_STA_PRIM_INFEAS_OR_UNBOUNDED => TerminationStatus::InfeasibleOrUnbounded,
            ffi::PRO_STA_DUAL_INFEAS => TerminationStatus::Unbounded,
            _ => match solsta {
                ffi::SOL_STA_PRIM_INFEAS_CER => TerminationStatus::Infeasible,
                ffi::SOL_STA_DUAL_INFEAS_CER => TerminationStatus::Unbounded,
                _ => TerminationStatus::Error,
            },
        }
    }

    fn apply_one(&mut self, change: &Change) -> Result<(), BackendError> {
        match change {
            // ── Variable Added ────────────────────────────────────────────
            Change::VariableAdded { var, bounds, var_type } => {
                let col_idx = self.num_vars();
                check(unsafe { ffi::MSK_appendvars(self.task, 1) }, "MSK_appendvars")?;

                let (bk, lb, ub) = mosek_bounds(bounds.lower, bounds.upper);
                check(
                    unsafe { ffi::MSK_putvarbound(self.task, col_idx, bk, lb, ub) },
                    "MSK_putvarbound",
                )?;

                self.col_map.insert(*var, col_idx);
                self.var_bounds.insert(*var, (bounds.lower, bounds.upper));

                if matches!(var_type, VarType::Integer | VarType::Binary) {
                    check(
                        unsafe {
                            ffi::MSK_putvartype(
                                self.task,
                                col_idx,
                                Self::var_type_to_mosek(*var_type),
                            )
                        },
                        "MSK_putvartype",
                    )?;
                    self.integer_vars.insert(*var);
                }
            }

            // ── Variable Removed ──────────────────────────────────────────
            Change::VariableRemoved { var } => {
                let col = match self.col_map.remove(*var) {
                    Some(c) => c,
                    None => return Ok(()),
                };
                self.var_bounds.remove(var);
                self.integer_vars.remove(var);
                for costs in self.obj_costs.values_mut() {
                    costs.remove(var);
                }
                check(
                    unsafe { ffi::MSK_removevars(self.task, 1, &col) },
                    "MSK_removevars",
                )?;
                self.col_map.reindex_after_delete(col);
            }

            // ── Variable Bounds Changed ────────────────────────────────────
            Change::VariableBoundsChanged { var, new, .. } => {
                if let Some(col) = self.col_map.get(*var) {
                    let (bk, lb, ub) = mosek_bounds(new.lower, new.upper);
                    check(
                        unsafe { ffi::MSK_putvarbound(self.task, col, bk, lb, ub) },
                        "MSK_putvarbound (bounds change)",
                    )?;
                    self.var_bounds.insert(*var, (new.lower, new.upper));
                }
            }

    // ── Variable Type Changed ──────────────────────────────────────
    Change::VariableTypeChanged { var, new, .. } => {
        if let Some(col) = self.col_map.get(*var) {
            let msk_type = if self.semicontinuous_vars.contains(var) {
                // If variable is SC, use SEMICONT_INT when setting to integer/binary
                match new {
                    VarType::Integer | VarType::Binary => {
                        self.integer_vars.insert(*var);
                        ffi::VAR_TYPE_SEMICONT_INT
                    }
                    VarType::Continuous => {
                        self.integer_vars.remove(var);
                        ffi::VAR_TYPE_SEMICONT
                    }
                }
            } else {
                Self::var_type_to_mosek(*new)
            };
            check(
                unsafe { ffi::MSK_putvartype(self.task, col, msk_type) },
                "MSK_putvartype (type change)",
            )?;
            if matches!(new, VarType::Integer | VarType::Binary) {
                self.integer_vars.insert(*var);
            } else {
                self.integer_vars.remove(var);
            }
        }
    }

    // ── Variable Semi-Continuous Bound Changed ─────────────────────
    Change::SemiContinuousBoundChanged { var, .. } => {
        if let Some(col) = self.col_map.get(*var) {
            self.semicontinuous_vars.insert(*var);
            let msk_type = if self.integer_vars.contains(var) {
                ffi::VAR_TYPE_SEMICONT_INT
            } else {
                ffi::VAR_TYPE_SEMICONT
            };
            check(
                unsafe { ffi::MSK_putvartype(self.task, col, msk_type) },
                "MSK_putvartype (semi-continuous)",
            )?;
        }
    }

            // ── Variable Activity Changed ──────────────────────────────────
            Change::VariableActivityChanged { var, active } => {
                if let Some(col) = self.col_map.get(*var) {
                    let (bk, lb, ub) = if *active {
                        let (orig_lb, orig_ub) =
                            self.var_bounds.get(var).copied().unwrap_or((0.0, f64::INFINITY));
                        mosek_bounds(orig_lb, orig_ub)
                    } else {
                        (ffi::BK_FX, 0.0, 0.0)
                    };
                    check(
                        unsafe { ffi::MSK_putvarbound(self.task, col, bk, lb, ub) },
                        "MSK_putvarbound (activity)",
                    )?;
                }
            }

            // ── Constraint Added ───────────────────────────────────────────
            Change::ConstraintAdded { con, bounds } => {
                let row_idx = self.num_cons();
                check(unsafe { ffi::MSK_appendcons(self.task, 1) }, "MSK_appendcons")?;

                let (bk, lb, ub) = mosek_bounds(bounds.lower, bounds.upper);
                check(
                    unsafe { ffi::MSK_putconbound(self.task, row_idx, bk, lb, ub) },
                    "MSK_putconbound",
                )?;

                self.row_map.insert(*con, row_idx);
                self.con_bounds.insert(*con, (bounds.lower, bounds.upper));
            }

            // ── Constraint Removed ─────────────────────────────────────────
            Change::ConstraintRemoved { con } => {
                let row = match self.row_map.remove(*con) {
                    Some(r) => r,
                    None => return Ok(()),
                };
                self.con_bounds.remove(con);
                check(
                    unsafe { ffi::MSK_removecons(self.task, 1, &row) },
                    "MSK_removecons",
                )?;
                self.row_map.reindex_after_delete(row);
            }

            // ── Constraint Bounds Changed ──────────────────────────────────
            Change::ConstraintBoundsChanged { con, new, .. } => {
                if let Some(row) = self.row_map.get(*con) {
                    let (bk, lb, ub) = mosek_bounds(new.lower, new.upper);
                    check(
                        unsafe { ffi::MSK_putconbound(self.task, row, bk, lb, ub) },
                        "MSK_putconbound (bounds change)",
                    )?;
                    self.con_bounds.insert(*con, (new.lower, new.upper));
                }
            }

            // ── Constraint Activity Changed ────────────────────────────────
            Change::ConstraintActivityChanged { con, active } => {
                if let Some(row) = self.row_map.get(*con) {
                    let (bk, lb, ub) = if *active {
                        let (orig_lb, orig_ub) = self
                            .con_bounds
                            .get(con)
                            .copied()
                            .unwrap_or((f64::NEG_INFINITY, f64::INFINITY));
                        mosek_bounds(orig_lb, orig_ub)
                    } else {
                        (ffi::BK_FR, 0.0, 0.0)
                    };
                    check(
                        unsafe { ffi::MSK_putconbound(self.task, row, bk, lb, ub) },
                        "MSK_putconbound (activity)",
                    )?;
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
                                unsafe { ffi::MSK_putaij(self.task, row, col, *value) },
                                "MSK_putaij (add)",
                            )?;
                        }
                    }
                    CoefficientTarget::Objective(obj) => {
                        self.obj_costs.entry(*obj).or_default().insert(*var, *value);
                        if Some(*obj) == self.active_obj {
                            if let Some(col) = self.col_map.get(*var) {
                                check(
                                    unsafe { ffi::MSK_putcj(self.task, col, *value) },
                                    "MSK_putcj (add)",
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
                                unsafe { ffi::MSK_putaij(self.task, row, col, 0.0) },
                                "MSK_putaij (remove)",
                            )?;
                        }
                    }
                    CoefficientTarget::Objective(obj) => {
                        if let Some(costs) = self.obj_costs.get_mut(obj) {
                            costs.remove(var);
                        }
                        if Some(*obj) == self.active_obj {
                            if let Some(col) = self.col_map.get(*var) {
                                check(
                                    unsafe { ffi::MSK_putcj(self.task, col, 0.0) },
                                    "MSK_putcj (remove)",
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
                                unsafe { ffi::MSK_putaij(self.task, row, col, *new) },
                                "MSK_putaij (update)",
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
                                    unsafe { ffi::MSK_putcj(self.task, col, *new) },
                                    "MSK_putcj (update)",
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
                        unsafe {
                            ffi::MSK_putobjsense(self.task, Self::sense_to_mosek(*new))
                        },
                        "MSK_putobjsense (sense change)",
                    )?;
                }
            }

            // ── Active Objective Changed ───────────────────────────────────
            Change::ActiveObjectiveChanged { new, .. } => {
                // Zero out old objective coefficients.
                if let Some(old_obj) = self.active_obj {
                    if let Some(old_costs) = self.obj_costs.get(&old_obj).cloned() {
                        for var in old_costs.keys() {
                            if let Some(col) = self.col_map.get(*var) {
                                unsafe { ffi::MSK_putcj(self.task, col, 0.0) };
                            }
                        }
                    }
                }

                self.active_obj = *new;

                if let Some(new_obj) = new {
                    // Load new costs.
                    if let Some(costs) = self.obj_costs.get(new_obj).cloned() {
                        for (var, cost) in &costs {
                            if let Some(col) = self.col_map.get(*var) {
                                unsafe { ffi::MSK_putcj(self.task, col, *cost) };
                            }
                        }
                    }
                    // Apply new sense.
                    if let Some(&sense) = self.obj_senses.get(new_obj) {
                        unsafe {
                            ffi::MSK_putobjsense(
                                self.task,
                                Self::sense_to_mosek(sense),
                            )
                        };
                    }
                }
            }

            // ── Parameter Value Changed ────────────────────────────────────
            Change::ParameterValueChanged { .. } => {}
        }
        Ok(())
    }
}

impl Drop for MosekAdapter {
    fn drop(&mut self) {
        unsafe {
            ffi::MSK_deletetask(&mut self.task);
            ffi::MSK_deleteenv(&mut self.env);
        }
    }
}

// ── Callback bridge ────────────────────────────────────────────────────────

/// State accessible from the MOSEK C callback trampoline.
struct MosekCallbackState {
    col_to_var: HashMap<ffi::MosekInt, VarId>,
    var_to_col: HashMap<VarId, ffi::MosekInt>,
    #[allow(dead_code)]
    task: MosekTask,
    handler: Box<dyn CallbackHandler>,
    /// Number of SEC cuts added during this solve.
    cuts_added: usize,
}

/// C callback trampoline registered with MOSEK via MSK_putcallbackfunc.
///
/// MOSEK calls this function during optimization for various events.
/// We handle `CALLBACK_NEW_INT_MIO` (new integer solution found):
/// extract the current solution, invoke the user's CallbackHandler,
/// and if cuts are returned, add them to the model and return non-zero
/// to terminate the solve (triggering a re-optimize cycle).
unsafe extern "C" fn mosek_callback_trampoline(
    task: MosekTask,
    usrptr: *mut c_void,
    caller: ffi::MosekInt,
    douinf: *const ffi::MosekReal,
    _intinf: *const ffi::MosekInt,
    _lintinf: *const i64,
) -> ffi::MosekInt {
    if caller != ffi::CALLBACK_NEW_INT_MIO {
        return 0; // not an event we care about — continue
    }

    let state = &mut *(usrptr as *mut MosekCallbackState);

    // Extract variable values from the current integer solution
    let mut num_var: ffi::MosekInt = 0;
    if ffi::MSK_getnumvar(task, &mut num_var) != ffi::RES_OK || num_var == 0 {
        return 0;
    }

    let mut xx = vec![0.0f64; num_var as usize];
    if ffi::MSK_getxx(task, ffi::SOL_ITG, xx.as_mut_ptr()) != ffi::RES_OK {
        return 0;
    }

    // Map MOSEK column indices to VarIds
    let mut var_values = std::collections::HashMap::with_capacity(state.col_to_var.len());
    for (&col, &var_id) in &state.col_to_var {
        if let Some(&val) = xx.get(col as usize) {
            var_values.insert(var_id, val);
        }
    }

    // Build CallbackData from MOSEK info arrays
    let primal_bound = if !douinf.is_null() {
        *douinf.add(ffi::DINF_MIO_OBJ_INT as usize)
    } else {
        f64::INFINITY
    };
    let dual_bound = if !douinf.is_null() {
        *douinf.add(ffi::DINF_MIO_OBJ_BOUND as usize)
    } else {
        f64::NEG_INFINITY
    };
    let mip_gap = if !douinf.is_null() && primal_bound.is_finite() && primal_bound != 0.0 {
        (primal_bound - dual_bound).abs() / primal_bound.abs()
    } else {
        f64::INFINITY
    };

    let cb_data = CallbackData {
        var_values,
        primal_bound,
        dual_bound,
        mip_gap,
    };

    // Invoke user handler
    match state.handler.on_candidate(&cb_data) {
        CallbackAction::Accept => 0, // continue — solution is feasible
        CallbackAction::AddCuts(cuts) => {
            let base_row: ffi::MosekInt = {
                let mut num = 0;
                ffi::MSK_getnumcon(task, &mut num);
                num
            };

            if cuts.is_empty() {
                return 0;
            }

            // Append rows for each cut
            let n_cuts = cuts.len() as ffi::MosekInt;
            if ffi::MSK_appendcons(task, n_cuts) != ffi::RES_OK {
                return -1;
            }

            for (offset, cut) in cuts.iter().enumerate() {
                let row = base_row + offset as ffi::MosekInt;

                // Set constraint bound: lower <= sum coeff*var <= upper
                let (bkc, blc, buc) = if cut.lower.is_finite() && cut.upper.is_finite() {
                    if (cut.lower - cut.upper).abs() < 1e-12 {
                        (ffi::BK_FX, cut.lower, cut.upper)
                    } else {
                        (ffi::BK_RA, cut.lower, cut.upper)
                    }
                } else if cut.lower.is_finite() {
                    (ffi::BK_LO, cut.lower, 0.0)
                } else if cut.upper.is_finite() {
                    (ffi::BK_UP, 0.0, cut.upper)
                } else {
                    (ffi::BK_FR, 0.0, 0.0)
                };

                if ffi::MSK_putconbound(task, row, bkc, blc, buc) != ffi::RES_OK {
                    continue;
                }

                // Add coefficients
                let mut subj: Vec<ffi::MosekInt> = Vec::with_capacity(cut.terms.len());
                let mut valij: Vec<ffi::MosekReal> = Vec::with_capacity(cut.terms.len());
                for (var_id, coeff) in &cut.terms {
                    if let Some(&col) = state.var_to_col.get(var_id) {
                        subj.push(col);
                        valij.push(*coeff);
                    }
                }

                if !subj.is_empty() {
                    let subi = vec![row; subj.len()];
                    ffi::MSK_putaijlist(task, subj.len() as ffi::MosekInt, subi.as_ptr(), subj.as_ptr(), valij.as_ptr());
                }
            }

            state.cuts_added += cuts.len();

            // Return non-zero to terminate MSK_optimize — cuts were added
            -1
        }
    }
}

// ── BackendSession implementation ─────────────────────────────────────────────

impl BackendSession for MosekAdapter {
    /// Apply a [`Synchronization`] — either a full rebuild from snapshot or
    /// an incremental delta batch.
    ///
    /// On success, invalidates any cached solution and returns the
    /// updated cursor and health.
    fn synchronize(&mut self, sync: Synchronization) -> Result<SyncReceipt, BackendError> {
        match sync {
            Synchronization::Rebuild(snapshot) => {
                let revision = snapshot.revision;
                info!(
                    "Rebuilding MOSEK session from snapshot at revision {}",
                    revision
                );

                let result = self.rebuild_from_snapshot(&snapshot);

                match result {
                    Ok(()) => {
                        self.cursor.mark_ready(revision);
                        // Invalidate stale solution after model mutation.
                        self.current_solution = None;
                        info!("Rebuild complete, cursor at revision {}", revision);
                    }
                    Err(e) => {
                        match e.health_effect {
                            HealthEffect::Terminal => self.cursor.mark_terminal(),
                            _ => self.cursor.mark_rebuild(),
                        }
                        return Err(e);
                    }
                }
            }

            Synchronization::DeltaBatch(batch) => {
                info!(
                    "Applying delta batch r{} -> r{} ({} ops)",
                    batch.from,
                    batch.to,
                    batch.operations.len()
                );

                for op in &batch.operations {
                    if let Err(e) = self.apply_model_op(op) {
                        self.cursor.mark_rebuild();
                        return Err(e);
                    }
                }

                self.cursor.advance(&batch).map_err(|e| {
                    BackendError::new(
                        format!("cursor failed to advance after delta: {}", e),
                        ErrorCategory::Internal,
                        HealthEffect::Terminal,
                    )
                })?;
                // Invalidate stale solution after model mutation.
                self.current_solution = None;
                info!("Delta batch applied, cursor at revision {}", batch.to);
            }
        }

        Ok(SyncReceipt {
            cursor: self.cursor.clone(),
            health: self.cursor.health,
        })
    }

    /// Solve the current model with the given [`SolveRequest`].
    ///
    /// Flow:
    /// 1. Apply solve options from the request (algorithm, time limit, etc.).
    /// 2. Set up callback handler if one is registered.
    /// 3. Call MOSEK optimize.
    /// 4. Map termination status.
    /// 5. Extract solution data if available.
    fn solve(&mut self, request: &SolveRequest) -> Result<SolveResult, BackendError> {
        info!(
            "Starting MOSEK solve: {} vars, {} cons, mip={}",
            self.col_map.len(),
            self.row_map.len(),
            self.is_mip(),
        );

        if self.active_obj.is_none() {
            warn!("Solving with no active objective.");
        }

        // Step 1: Apply solve options from the request.
        let effective_config = self.negotiate_options(request)?;

        // ── Optional callback loop ──
        if let Some(handler) = self.callback_handler.take() {
            let col_to_var: HashMap<ffi::MosekInt, VarId> = self.col_map.reverse_map();
            let var_to_col: HashMap<VarId, ffi::MosekInt> =
                self.col_map.iter().map(|(v, c)| (v, c)).collect();

            let state = Box::new(MosekCallbackState {
                col_to_var,
                var_to_col,
                task: self.task,
                handler,
                cuts_added: 0,
            });
            let state_ptr: *mut MosekCallbackState = Box::into_raw(state);

            // Register callback
            unsafe {
                ffi::MSK_putcallbackfunc(self.task, Some(mosek_callback_trampoline), state_ptr as *mut c_void);
            }

            // Loop: optimize → if cuts added → re-optimize
            loop {
                let ret = unsafe { ffi::MSK_optimize(self.task) };
                let state = unsafe { &mut *state_ptr };

                if state.cuts_added > 0 {
                    // Cuts were added; the callback terminated the solve.
                    // Re-optimize with the tightened model.
                    state.cuts_added = 0;
                    continue;
                }

                // No cuts added in the last solve — check for normal termination
                if ret > 0 && ret < ffi::RES_TRM_USER_CALLBACK {
                    // Real error (not user-callback termination)
                    unsafe {
                        let _ = Box::from_raw(state_ptr);
                    }
                    return Err(mosek_err(format!("MSK_optimize error code {ret}")));
                }
                break;
            };

            // Unregister callback and reclaim state
            unsafe {
                ffi::MSK_putcallbackfunc(self.task, None, std::ptr::null_mut());
                let state = Box::from_raw(state_ptr);
                self.callback_handler = Some(state.handler);
            }

        } else {
            // ── No callback — plain optimize ──
            let ret = unsafe { ffi::MSK_optimize(self.task) };
            if ret > 0 && ret < 100000 {
                return Err(mosek_err(format!("MSK_optimize error code {ret}")));
            }
        }

        // Step 4: Map status.
        let soltype = self.soltype();

        let mut solsta: ffi::MosekInt = ffi::SOL_STA_UNKNOWN;
        check(
            unsafe { ffi::MSK_getsolsta(self.task, soltype, &mut solsta) },
            "MSK_getsolsta",
        )?;

        let mut prosta: ffi::MosekInt = ffi::PRO_STA_UNKNOWN;
        check(
            unsafe { ffi::MSK_getprosta(self.task, soltype, &mut prosta) },
            "MSK_getprosta",
        )?;

        let termination = self.map_termination_status(prosta, solsta);
        self.last_status = Some(termination);
        info!("Solve completed with status: {:?}", termination);

        // Step 5: Extract solution data.
        let solution = self.extract_solution_data(&termination, soltype);
        self.current_solution = solution.clone();

        Ok(SolveResult {
            effective_configuration: effective_config,
            termination,
            solution,
        })
    }

    /// Close the session, releasing native resources.
    ///
    /// Consumes `self` so that the [`Drop`] impl runs immediately, which
    /// calls MOSEK delete functions on the handles.
    fn close(self) -> Result<(), BackendError> {
        // Drop handles all cleanup: task and env deletion.
        info!("Closing MOSEK session");
        Ok(())
    }
}

// ── SessionHealth ───────────────────────────────────────────────────────────────

impl SessionHealth for MosekAdapter {
    fn health(&self) -> AdapterHealth {
        self.cursor.health
    }

    fn revision(&self) -> ModelRevision {
        self.cursor.applied_revision
    }
}

// ── BackendMetadata ─────────────────────────────────────────────────────────────

impl BackendMetadata for MosekAdapter {
    fn name(&self) -> &str {
        "MOSEK"
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            lp: true,
            mip: true,
            solution: true,
            duals: true,
            reduced_costs: true,
            callbacks: true,
            delete: true,
            add_variable: true,
            add_constraint: true,
            set_coefficient: true,
            set_bounds: true,
            set_objective: true,
            semicontinuous: true,
            semiinteger: true,
            parameter_update: false,
        }
    }
}

// ── CallbackSession ─────────────────────────────────────────────────────────────

impl CallbackSession for MosekAdapter {
    fn set_callback_handler(&mut self, handler: Box<dyn CallbackHandler>) -> Result<(), BackendError> {
        self.callback_handler = Some(handler);
        Ok(())
    }

    fn clear_callback_handler(&mut self) -> Result<(), BackendError> {
        self.callback_handler = None;
        Ok(())
    }
}

// ── SolutionView ────────────────────────────────────────────────────────────────

impl SolutionView for MosekAdapter {
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

// ── MosekAdapter methods (delta application, rebuild, option negotiation) ───────

impl MosekAdapter {
    /// Reset the adapter — delete and recreate the MOSEK task, clear all caches.
    fn reset(&mut self) {
        unsafe { ffi::MSK_deletetask(&mut self.task) };
        self.task = make_task(self.env, &self.opts);

        self.col_map = IndexMap::new();
        self.row_map = IndexMap::new();
        self.var_bounds.clear();
        self.con_bounds.clear();
        self.integer_vars.clear();
        self.semicontinuous_vars.clear();
        self.obj_costs.clear();
        self.obj_senses.clear();
        self.active_obj = None;
        self.cursor = AdapterCursor::new();
        self.last_status = None;
        self.current_solution = None;
        self.callback_handler = None;
    }

    /// Apply a single [`ModelOp`] to the MOSEK task.
    fn apply_model_op(&mut self, op: &ModelOp) -> Result<(), BackendError> {
        match op {
            // ── Variable Added ──────────────────────────────────────────────
            ModelOp::AddVariable { var, bounds, var_type } => {
                let col_idx = self.num_vars();
                check(unsafe { ffi::MSK_appendvars(self.task, 1) }, "MSK_appendvars")?;

                let (bk, lb, ub) = mosek_bounds(bounds.lower, bounds.upper);
                check(
                    unsafe { ffi::MSK_putvarbound(self.task, col_idx, bk, lb, ub) },
                    "MSK_putvarbound",
                )?;

                self.col_map.insert(*var, col_idx);
                self.var_bounds.insert(*var, (bounds.lower, bounds.upper));

                if matches!(var_type, VarType::Integer | VarType::Binary) {
                    check(
                        unsafe { ffi::MSK_putvartype(self.task, col_idx, Self::var_type_to_mosek(*var_type)) },
                        "MSK_putvartype",
                    )?;
                    self.integer_vars.insert(*var);
                }
            }

            // ── Variable Removed ────────────────────────────────────────────
            ModelOp::RemoveVariable { var } => {
                let col = match self.col_map.remove(*var) {
                    Some(c) => c,
                    None => return Ok(()),
                };
                self.var_bounds.remove(var);
                self.integer_vars.remove(var);
                for costs in self.obj_costs.values_mut() {
                    costs.remove(var);
                }
                check(
                    unsafe { ffi::MSK_removevars(self.task, 1, &col) },
                    "MSK_removevars",
                )?;
                self.col_map.reindex_after_delete(col);
            }

            // ── Variable Bounds Changed ─────────────────────────────────────
            ModelOp::SetVariableBounds { var, bounds } => {
                if let Some(col) = self.col_map.get(*var) {
                    let (bk, lb, ub) = mosek_bounds(bounds.lower, bounds.upper);
                    check(
                        unsafe { ffi::MSK_putvarbound(self.task, col, bk, lb, ub) },
                        "MSK_putvarbound (bounds change)",
                    )?;
                    self.var_bounds.insert(*var, (bounds.lower, bounds.upper));
                }
            }

            // ── Variable Type Changed ───────────────────────────────────────
            ModelOp::SetVariableType { var, var_type } => {
                if let Some(col) = self.col_map.get(*var) {
                    let msk_type = if self.semicontinuous_vars.contains(var) {
                        match var_type {
                            VarType::Integer | VarType::Binary => {
                                self.integer_vars.insert(*var);
                                ffi::VAR_TYPE_SEMICONT_INT
                            }
                            VarType::Continuous => {
                                self.integer_vars.remove(var);
                                ffi::VAR_TYPE_SEMICONT
                            }
                        }
                    } else {
                        Self::var_type_to_mosek(*var_type)
                    };
                    check(
                        unsafe { ffi::MSK_putvartype(self.task, col, msk_type) },
                        "MSK_putvartype (type change)",
                    )?;
                    if matches!(var_type, VarType::Integer | VarType::Binary) {
                        self.integer_vars.insert(*var);
                    } else {
                        self.integer_vars.remove(var);
                    }
                }
            }

            // ── Variable Activity Changed ───────────────────────────────────
            ModelOp::SetVariableActive { var, active } => {
                if let Some(col) = self.col_map.get(*var) {
                    let (bk, lb, ub) = if *active {
                        let (orig_lb, orig_ub) =
                            self.var_bounds.get(var).copied().unwrap_or((0.0, f64::INFINITY));
                        mosek_bounds(orig_lb, orig_ub)
                    } else {
                        (ffi::BK_FX, 0.0, 0.0)
                    };
                    check(
                        unsafe { ffi::MSK_putvarbound(self.task, col, bk, lb, ub) },
                        "MSK_putvarbound (activity)",
                    )?;
                }
            }

            // ── Constraint Added ────────────────────────────────────────────
            ModelOp::AddConstraint { con, bounds } => {
                let row_idx = self.num_cons();
                check(unsafe { ffi::MSK_appendcons(self.task, 1) }, "MSK_appendcons")?;

                let (bk, lb, ub) = mosek_bounds(bounds.lower, bounds.upper);
                check(
                    unsafe { ffi::MSK_putconbound(self.task, row_idx, bk, lb, ub) },
                    "MSK_putconbound",
                )?;

                self.row_map.insert(*con, row_idx);
                self.con_bounds.insert(*con, (bounds.lower, bounds.upper));
            }

            // ── Constraint Removed ──────────────────────────────────────────
            ModelOp::RemoveConstraint { con } => {
                let row = match self.row_map.remove(*con) {
                    Some(r) => r,
                    None => return Ok(()),
                };
                self.con_bounds.remove(con);
                check(
                    unsafe { ffi::MSK_removecons(self.task, 1, &row) },
                    "MSK_removecons",
                )?;
                self.row_map.reindex_after_delete(row);
            }

            // ── Constraint Bounds Changed ───────────────────────────────────
            ModelOp::SetConstraintBounds { con, bounds } => {
                if let Some(row) = self.row_map.get(*con) {
                    let (bk, lb, ub) = mosek_bounds(bounds.lower, bounds.upper);
                    check(
                        unsafe { ffi::MSK_putconbound(self.task, row, bk, lb, ub) },
                        "MSK_putconbound (bounds change)",
                    )?;
                    self.con_bounds.insert(*con, (bounds.lower, bounds.upper));
                }
            }

            // ── Constraint Activity Changed ─────────────────────────────────
            ModelOp::SetConstraintActive { con, active } => {
                if let Some(row) = self.row_map.get(*con) {
                    let (bk, lb, ub) = if *active {
                        let (orig_lb, orig_ub) = self
                            .con_bounds
                            .get(con)
                            .copied()
                            .unwrap_or((f64::NEG_INFINITY, f64::INFINITY));
                        mosek_bounds(orig_lb, orig_ub)
                    } else {
                        (ffi::BK_FR, 0.0, 0.0)
                    };
                    check(
                        unsafe { ffi::MSK_putconbound(self.task, row, bk, lb, ub) },
                        "MSK_putconbound (activity)",
                    )?;
                }
            }

            // ── Set Coefficient Cell ────────────────────────────────────────
            ModelOp::SetCell { cell_key, evaluated_value, .. } => {
                let (target, var) = cell_key;
                match target {
                    CoefficientTarget::Constraint(con) => {
                        if let (Some(row), Some(col)) =
                            (self.row_map.get(*con), self.col_map.get(*var))
                        {
                            check(
                                unsafe { ffi::MSK_putaij(self.task, row, col, *evaluated_value) },
                                "MSK_putaij (set cell)",
                            )?;
                        }
                    }
                    CoefficientTarget::Objective(obj) => {
                        self.obj_costs.entry(*obj).or_default().insert(*var, *evaluated_value);
                        if Some(*obj) == self.active_obj {
                            if let Some(col) = self.col_map.get(*var) {
                                check(
                                    unsafe { ffi::MSK_putcj(self.task, col, *evaluated_value) },
                                    "MSK_putcj (set cell)",
                                )?;
                            }
                        }
                    }
                }
            }

            // ── Remove Coefficient Cell ─────────────────────────────────────
            ModelOp::RemoveCell { cell_key } => {
                let (target, var) = cell_key;
                match target {
                    CoefficientTarget::Constraint(con) => {
                        if let (Some(row), Some(col)) =
                            (self.row_map.get(*con), self.col_map.get(*var))
                        {
                            check(
                                unsafe { ffi::MSK_putaij(self.task, row, col, 0.0) },
                                "MSK_putaij (remove cell)",
                            )?;
                        }
                    }
                    CoefficientTarget::Objective(obj) => {
                        if let Some(costs) = self.obj_costs.get_mut(obj) {
                            costs.remove(var);
                        }
                        if Some(*obj) == self.active_obj {
                            if let Some(col) = self.col_map.get(*var) {
                                check(
                                    unsafe { ffi::MSK_putcj(self.task, col, 0.0) },
                                    "MSK_putcj (remove cell)",
                                )?;
                            }
                        }
                    }
                }
            }

            // ── Objective Added ─────────────────────────────────────────────
            ModelOp::AddObjective { obj, sense } => {
                self.obj_senses.insert(*obj, *sense);
                self.obj_costs.entry(*obj).or_default();
            }

            // ── Objective Removed ───────────────────────────────────────────
            ModelOp::RemoveObjective { obj } => {
                self.obj_costs.remove(obj);
                self.obj_senses.remove(obj);
                if self.active_obj == Some(*obj) {
                    self.active_obj = None;
                }
            }

            // ── Active Objective Changed ─────────────────────────────────────
            ModelOp::SetActiveObjective { obj } => {
                // Zero out old objective coefficients.
                if let Some(old_obj) = self.active_obj {
                    if let Some(old_costs) = self.obj_costs.get(&old_obj).cloned() {
                        for var in old_costs.keys() {
                            if let Some(col) = self.col_map.get(*var) {
                                unsafe { ffi::MSK_putcj(self.task, col, 0.0) };
                            }
                        }
                    }
                }

                self.active_obj = *obj;

                if let Some(new_obj) = obj {
                    // Load new costs.
                    if let Some(costs) = self.obj_costs.get(new_obj).cloned() {
                        for (var, cost) in &costs {
                            if let Some(col) = self.col_map.get(*var) {
                                unsafe { ffi::MSK_putcj(self.task, col, *cost) };
                            }
                        }
                    }
                    // Apply new sense.
                    if let Some(&sense) = self.obj_senses.get(new_obj) {
                        unsafe {
                            ffi::MSK_putobjsense(self.task, Self::sense_to_mosek(sense))
                        };
                    }
                }
            }

            // ── Set Objective Cell ──────────────────────────────────────────
            ModelOp::SetObjectiveCell { cell_key, evaluated_value, .. } => {
                let (_target, var) = cell_key;
                if let Some(col) = self.col_map.get(*var) {
                    check(
                        unsafe { ffi::MSK_putcj(self.task, col, *evaluated_value) },
                        "MSK_putcj (obj cell)",
                    )?;
                }
            }

            // ── Set Parameter ───────────────────────────────────────────────
            ModelOp::SetParameter { .. } => {
                // MOSEK does not have a parameter concept in the same way;
                // parameters are resolved to evaluated_values in cells.
            }
        }
        Ok(())
    }

    /// Rebuild the MOSEK task from a canonical [`ModelSnapshot`].
    ///
    /// Clears the MOSEK task, then adds all variables, constraints, cells,
    /// and objectives from the snapshot. Handles inactive entities and
    /// semi-continuous variables.
    fn rebuild_from_snapshot(&mut self, snapshot: &ModelSnapshot) -> Result<(), BackendError> {
        self.reset();

        // Add variables
        for var_entry in &snapshot.variables {
            let col_idx = self.num_vars();
            check(unsafe { ffi::MSK_appendvars(self.task, 1) }, "MSK_appendvars")?;

            let (bk, lb, ub) = mosek_bounds(var_entry.bounds.lower, var_entry.bounds.upper);
            check(
                unsafe { ffi::MSK_putvarbound(self.task, col_idx, bk, lb, ub) },
                "MSK_putvarbound",
            )?;

            self.col_map.insert(var_entry.id, col_idx);
            self.var_bounds.insert(var_entry.id, (var_entry.bounds.lower, var_entry.bounds.upper));

            if matches!(var_entry.var_type, VarType::Integer | VarType::Binary) {
                check(
                    unsafe { ffi::MSK_putvartype(self.task, col_idx, Self::var_type_to_mosek(var_entry.var_type)) },
                    "MSK_putvartype",
                )?;
                self.integer_vars.insert(var_entry.id);
            }

            if let Some(_sc_lower) = var_entry.semicontinuous_lower {
                self.semicontinuous_vars.insert(var_entry.id);
                let msk_type = if matches!(var_entry.var_type, VarType::Integer | VarType::Binary) {
                    ffi::VAR_TYPE_SEMICONT_INT
                } else {
                    ffi::VAR_TYPE_SEMICONT
                };
                check(
                    unsafe { ffi::MSK_putvartype(self.task, col_idx, msk_type) },
                    "MSK_putvartype (semi-continuous)",
                )?;
            }

            // Handle inactive variables
            if !var_entry.active {
                if let Some(col) = self.col_map.get(var_entry.id) {
                    check(
                        unsafe { ffi::MSK_putvarbound(self.task, col, ffi::BK_FX, 0.0, 0.0) },
                        "MSK_putvarbound (inactive)",
                    )?;
                }
            }
        }

        // Add constraints
        for con_entry in &snapshot.constraints {
            let row_idx = self.num_cons();
            check(unsafe { ffi::MSK_appendcons(self.task, 1) }, "MSK_appendcons")?;

            let (bk, lb, ub) = mosek_bounds(con_entry.bounds.lower, con_entry.bounds.upper);
            check(
                unsafe { ffi::MSK_putconbound(self.task, row_idx, bk, lb, ub) },
                "MSK_putconbound",
            )?;

            self.row_map.insert(con_entry.id, row_idx);
            self.con_bounds.insert(con_entry.id, (con_entry.bounds.lower, con_entry.bounds.upper));

            // Handle inactive constraints
            if !con_entry.active {
                if let Some(row) = self.row_map.get(con_entry.id) {
                    check(
                        unsafe { ffi::MSK_putconbound(self.task, row, ffi::BK_FR, 0.0, 0.0) },
                        "MSK_putconbound (inactive)",
                    )?;
                }
            }
        }

        // Add coefficient cells
        for cell in &snapshot.cells {
            match cell.cell_key.0 {
                CoefficientTarget::Constraint(con) => {
                    if let (Some(row), Some(col)) = (self.row_map.get(con), self.col_map.get(cell.cell_key.1)) {
                        check(
                            unsafe { ffi::MSK_putaij(self.task, row, col, cell.evaluated_value) },
                            "MSK_putaij (rebuild)",
                        )?;
                    }
                }
                CoefficientTarget::Objective(obj) => {
                    self.obj_costs.entry(obj).or_default().insert(cell.cell_key.1, cell.evaluated_value);
                }
            }
        }

        // Register objectives
        for obj_entry in &snapshot.objectives {
            self.obj_senses.insert(obj_entry.id, obj_entry.sense);
            self.obj_costs.entry(obj_entry.id).or_default();
            if obj_entry.active {
                self.active_obj = Some(obj_entry.id);
            }
        }

        // Set active objective
        if let Some(obj) = self.active_obj {
            // Apply sense
            if let Some(&sense) = self.obj_senses.get(&obj) {
                unsafe { ffi::MSK_putobjsense(self.task, Self::sense_to_mosek(sense)) };
            }
            // Apply costs
            if let Some(costs) = self.obj_costs.get(&obj) {
                for (var, cost) in costs {
                    if let Some(col) = self.col_map.get(*var) {
                        unsafe { ffi::MSK_putcj(self.task, col, *cost) };
                    }
                }
            }
        }

        Ok(())
    }

    /// Map solve request options to MOSEK parameters, returning the effective
    /// configuration. Each option is applied, adjusted, or rejected explicitly.
    fn negotiate_options(&self, request: &SolveRequest) -> Result<EffectiveConfig, BackendError> {
        let mut effective = EffectiveConfig::default();

        // ── lp_algorithm ────────────────────────────────────────────────────
        if let Some(algo) = &request.lp_algorithm {
            let mosek_opt = match algo {
                LpAlgorithm::Automatic     => ffi::OPTIMIZER_FREE,
                LpAlgorithm::PrimalSimplex => ffi::OPTIMIZER_DUAL_SIMPLEX,
                LpAlgorithm::DualSimplex   => ffi::OPTIMIZER_NEW_DUAL_SIMPLEX,
                LpAlgorithm::Barrier       => ffi::OPTIMIZER_INTPNT,
            };
            unsafe { ffi::MSK_putintparam(self.task, ffi::IPAR_OPTIMIZER, mosek_opt) };
            effective.lp_algorithm = Some(*algo);
        }

        // ── time_limit_secs ─────────────────────────────────────────────────
        if let Some(t) = request.time_limit_secs {
            unsafe { ffi::MSK_putdouparam(self.task, ffi::DPAR_MIO_MAX_TIME, t) };
            effective.time_limit_secs = Some(t);
        }

        // ── mip_rel_gap ─────────────────────────────────────────────────────
        if let Some(g) = request.mip_rel_gap {
            unsafe { ffi::MSK_putdouparam(self.task, ffi::DPAR_MIO_TOL_REL_GAP, g) };
            effective.mip_rel_gap = Some(g);
        }

        // ── mip_abs_gap ─────────────────────────────────────────────────────
        if let Some(g) = request.mip_abs_gap {
            unsafe { ffi::MSK_putdouparam(self.task, ffi::DPAR_MIO_TOL_ABS_GAP, g) };
            effective.adjustments.push(ConfigAdjustment {
                key: "mip_abs_gap".into(),
                requested: g.to_string(),
                applied: g.to_string(),
                reason: "set via MSK_putdouparam".into(),
            });
        }

        // ── threads ─────────────────────────────────────────────────────────
        if let Some(n) = request.threads {
            unsafe { ffi::MSK_putintparam(self.task, ffi::IPAR_NUM_THREADS, n) };
            effective.threads = Some(n);
        }

        // ── enable_output ───────────────────────────────────────────────────
        if let Some(enabled) = request.enable_output {
            let level: i32 = if enabled { 3 } else { 0 };
            unsafe { ffi::MSK_putintparam(self.task, ffi::IPAR_LOG, level) };
            effective.enable_output = Some(enabled);
        }

        // ── random_seed ─────────────────────────────────────────────────────
        if request.random_seed.is_some() {
            effective.rejections.push(ConfigRejection {
                key: "random_seed".into(),
                reason: "MOSEK random seed not supported via current FFI bindings".into(),
            });
        }

        // ── extra_options ───────────────────────────────────────────────────
        for (key, _value) in &request.extra_options {
            effective.rejections.push(ConfigRejection {
                key: key.clone(),
                reason: format!("MOSEK extra option '{}' not supported via current FFI bindings", key),
            });
        }

        Ok(effective)
    }

    /// Extract solution data from MOSEK after a solve, building a
    /// [`SolveSolution`]. Returns `None` if the termination status does not
    /// indicate a solution is available or if the data cannot be retrieved.
    fn extract_solution_data(&self, termination: &TerminationStatus, soltype: ffi::MosekInt) -> Option<SolveSolution> {
        match termination {
            TerminationStatus::Optimal | TerminationStatus::Feasible => {}
            _ => return None,
        }

        let nv = self.num_vars() as usize;
        let nc = self.num_cons() as usize;

        if nv == 0 {
            return None;
        }

        // Primal variable values.
        let mut xx = vec![0.0f64; nv];
        if unsafe { ffi::MSK_getxx(self.task, soltype, xx.as_mut_ptr()) } != ffi::RES_OK {
            return None;
        }

        // Primal objective value.
        let mut pobj = 0.0f64;
        if unsafe { ffi::MSK_getprimalobj(self.task, soltype, &mut pobj) } != ffi::RES_OK {
            pobj = f64::NAN;
        }

        let rev_col = self.col_map.reverse_map();
        let rev_row = self.row_map.reverse_map();

        // Map column values to variable_values.
        let variable_values: Vec<(VarId, f64)> = (0..nv)
            .filter_map(|hi_idx| {
                let v = xx[hi_idx];
                if v.is_nan() || v.is_infinite() {
                    return None;
                }
                rev_col.get(&(hi_idx as i32)).copied().map(|var_id| (var_id, v))
            })
            .collect();

        // Dual values and reduced costs — only available for LP (BAS solution).
        let (dual_values, reduced_costs) = if !self.is_mip() {
            let mut y = vec![0.0f64; nc];
            let mut slx = vec![0.0f64; nv];
            let mut sux = vec![0.0f64; nv];

            let y_ok = unsafe {
                ffi::MSK_gety(self.task, soltype, y.as_mut_ptr())
            } == ffi::RES_OK;
            let rc_ok = unsafe {
                ffi::MSK_getslx(self.task, soltype, slx.as_mut_ptr())
            } == ffi::RES_OK
                && unsafe {
                    ffi::MSK_getsux(self.task, soltype, sux.as_mut_ptr())
                } == ffi::RES_OK;

            let duals = if y_ok {
                let d: Vec<(ConId, f64)> = (0..nc)
                    .filter_map(|hi_idx| {
                        let v = y[hi_idx];
                        if v.is_nan() || v.is_infinite() {
                            return None;
                        }
                        rev_row.get(&(hi_idx as i32)).copied().map(|con_id| (con_id, v))
                    })
                    .collect();
                if d.is_empty() { None } else { Some(d) }
            } else {
                None
            };

            let rcs = if rc_ok {
                let rc: Vec<(VarId, f64)> = (0..nv)
                    .filter_map(|hi_idx| {
                        let v = slx.get(hi_idx).copied().unwrap_or(0.0)
                            - sux.get(hi_idx).copied().unwrap_or(0.0);
                        rev_col.get(&(hi_idx as i32)).copied().map(|var_id| (var_id, v))
                    })
                    .collect();
                if rc.is_empty() { None } else { Some(rc) }
            } else {
                None
            };

            (duals, rcs)
        } else {
            (None, None)
        };

        if variable_values.is_empty() {
            return None;
        }

        Some(SolveSolution {
            variable_values,
            objective_value: if pobj.is_nan() { None } else { Some(pobj) },
            dual_values,
            reduced_costs,
        })
    }
}

// ── Private helpers ────────────────────────────────────────────────────────

/// C-callable stream callback that writes MOSEK log output to stderr (unbuffered).
unsafe extern "C" fn mosek_stdout_cb(_handle: *mut std::ffi::c_void, msg: *const std::ffi::c_char) {
    if msg.is_null() { return; }
    if let Ok(s) = unsafe { std::ffi::CStr::from_ptr(msg) }.to_str() {
        use std::io::Write;
        let mut stderr = std::io::stderr();
        let _ = stderr.write_all(s.as_bytes());
        let _ = stderr.write_all(b"\n");
        let _ = stderr.flush();
    }
}

fn make_task(env: MosekEnv, opts: &MosekOptions) -> MosekTask {
    let mut task: MosekTask = std::ptr::null_mut();
    let ret = unsafe { ffi::MSK_maketask(env, 0, 0, &mut task) };
    assert!(ret == ffi::RES_OK, "MSK_maketask failed with code {ret}");

    unsafe { ffi::MSK_putintparam(task, ffi::IPAR_LOG, opts.log_level) };

    if opts.log_level > 0 {
        unsafe {
            ffi::MSK_linkfunctotaskstream(
                task,
                ffi::STREAM_LOG,
                std::ptr::null_mut(),
                Some(mosek_stdout_cb),
            )
        };
    }

    if let Some(max_time) = opts.max_time {
        unsafe { ffi::MSK_putdouparam(task, ffi::DPAR_MIO_MAX_TIME, max_time) };
    }

    if let Some(threads) = opts.num_threads {
        unsafe { ffi::MSK_putintparam(task, ffi::IPAR_NUM_THREADS, threads) };
    }

    let optimizer_code = match opts.optimizer {
        MosekOptimizer::Free          => ffi::OPTIMIZER_FREE,
        MosekOptimizer::InteriorPoint => ffi::OPTIMIZER_INTPNT,
        MosekOptimizer::DualSimplex   => ffi::OPTIMIZER_DUAL_SIMPLEX,
        MosekOptimizer::NewDualSimplex => ffi::OPTIMIZER_NEW_DUAL_SIMPLEX,
        MosekOptimizer::FreeSimplex   => ffi::OPTIMIZER_FREE_SIMPLEX,
    };
    unsafe { ffi::MSK_putintparam(task, ffi::IPAR_OPTIMIZER, optimizer_code) };

    let sim_hs_code = match opts.sim_hotstart {
        MosekSimHotstart::None       => ffi::SIM_HOTSTART_NONE,
        MosekSimHotstart::Free       => ffi::SIM_HOTSTART_FREE,
        MosekSimHotstart::StatusKeys => ffi::SIM_HOTSTART_STATUS_KEYS,
    };
    unsafe { ffi::MSK_putintparam(task, ffi::IPAR_SIM_HOTSTART, sim_hs_code) };
    let lu_code = if opts.sim_hotstart_lu { ffi::MSK_ON } else { ffi::MSK_OFF };
    unsafe { ffi::MSK_putintparam(task, ffi::IPAR_SIM_HOTSTART_LU, lu_code) };
    unsafe { ffi::MSK_putintparam(task, ffi::IPAR_INTPNT_HOTSTART, ffi::INTPNT_HOTSTART_PRIMAL_DUAL) };

    task
}
