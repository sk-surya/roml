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

use std::collections::{HashMap, HashSet};
use roml::id::{ConId, ObjId, VarId};
use roml::model::changelog::Change;
use roml::model::coefficient::CoefficientTarget;
use roml::model::objective::Sense;
use roml::model::variable::VarType;
use roml::solver::{SolverAdapter, SolverError, SolverStatus};
use log::{info, warn};

use crate::ffi::{self, MosekEnv, MosekTask};
use crate::index_map::IndexMap;

// ── Error helpers ──────────────────────────────────────────────────────────

fn mosek_err(msg: impl Into<String>) -> SolverError {
    SolverError::InternalError(msg.into())
}

fn check(ret: ffi::MosekRes, op: &str) -> Result<(), SolverError> {
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

/// Options forwarded to MOSEK at adapter creation time.
#[derive(Debug, Clone, Default)]
pub struct MosekOptions {
    num_threads: Option<i32>,
    log_level: i32,
}

impl MosekOptions {
    pub fn threads(mut self, n: i32) -> Self {
        self.num_threads = Some(n);
        self
    }

    /// Log verbosity; 0 = silent (default), higher = more output.
    pub fn log_level(mut self, level: i32) -> Self {
        self.log_level = level;
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

    obj_costs:  HashMap<ObjId, HashMap<VarId, f64>>,
    obj_senses: HashMap<ObjId, Sense>,
    active_obj: Option<ObjId>,

    status:          SolverStatus,
    solution:        Option<HashMap<VarId, f64>>,
    objective_value: Option<f64>,
    duals:           Option<HashMap<ConId, f64>>,
    reduced_costs:   Option<HashMap<VarId, f64>>,
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
            obj_costs: HashMap::new(),
            obj_senses: HashMap::new(),
            active_obj: None,
            status: SolverStatus::NotSolved,
            solution: None,
            objective_value: None,
            duals: None,
            reduced_costs: None,
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

    fn map_status(&self, prosta: ffi::MosekInt, solsta: ffi::MosekInt) -> SolverStatus {
        // Optimality is in solution status.
        if solsta == ffi::SOL_STA_OPTIMAL || solsta == ffi::SOL_STA_INTEGER_OPTIMAL {
            return SolverStatus::Optimal;
        }
        // Infeasibility / unboundedness from problem status.
        match prosta {
            ffi::PRO_STA_PRIM_INFEAS
            | ffi::PRO_STA_PRIM_AND_DUAL_INFEAS
            | ffi::PRO_STA_PRIM_INFEAS_OR_UNBOUNDED => SolverStatus::Infeasible,
            ffi::PRO_STA_DUAL_INFEAS => SolverStatus::Unbounded,
            _ => match solsta {
                ffi::SOL_STA_PRIM_INFEAS_CER => SolverStatus::Infeasible,
                ffi::SOL_STA_DUAL_INFEAS_CER => SolverStatus::Unbounded,
                _ => SolverStatus::Error,
            },
        }
    }

    fn apply_one(&mut self, change: &Change) -> Result<(), SolverError> {
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
                    check(
                        unsafe {
                            ffi::MSK_putvartype(self.task, col, Self::var_type_to_mosek(*new))
                        },
                        "MSK_putvartype (type change)",
                    )?;
                    if matches!(new, VarType::Integer | VarType::Binary) {
                        self.integer_vars.insert(*var);
                    } else {
                        self.integer_vars.remove(var);
                    }
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

// ── SolverAdapter implementation ───────────────────────────────────────────

impl SolverAdapter for MosekAdapter {
    fn apply_changes(&mut self, changes: &[Change]) -> Result<(), SolverError> {
        for change in changes {
            self.apply_one(change)?;
        }
        self.solution = None;
        self.objective_value = None;
        self.duals = None;
        self.reduced_costs = None;
        self.status = SolverStatus::NotSolved;
        Ok(())
    }

    fn solve(&mut self) -> Result<SolverStatus, SolverError> {
        info!(
            "Starting MOSEK solve: {} vars, {} cons, mip={}",
            self.col_map.len(),
            self.row_map.len(),
            self.is_mip(),
        );

        if self.active_obj.is_none() {
            warn!("Solving with no active objective.");
        }

        let ret = unsafe { ffi::MSK_optimize(self.task) };
        // TRM codes >= 100000 mean early termination (time/iter limit) — not
        // errors; we proceed to inspect solution status.
        if ret > 0 && ret < 100000 {
            return Err(mosek_err(format!("MSK_optimize error code {ret}")));
        }

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

        let solver_status = self.map_status(prosta, solsta);
        self.status = solver_status;

        if matches!(solver_status, SolverStatus::Optimal) {
            let nv = self.num_vars() as usize;
            let nc = self.num_cons() as usize;

            // Primal variable values.
            let mut xx = vec![0.0f64; nv];
            check(
                unsafe { ffi::MSK_getxx(self.task, soltype, xx.as_mut_ptr()) },
                "MSK_getxx",
            )?;

            // Primal objective value.
            let mut pobj = 0.0f64;
            check(
                unsafe { ffi::MSK_getprimalobj(self.task, soltype, &mut pobj) },
                "MSK_getprimalobj",
            )?;
            self.objective_value = Some(pobj);

            let mut sol: HashMap<VarId, f64> = HashMap::new();
            for (var, col) in self.col_map.iter() {
                if let Some(v) = xx.get(col as usize) {
                    sol.insert(var, *v);
                }
            }
            self.solution = Some(sol);

            // Dual values and reduced costs — only available for LP (BAS solution).
            if !self.is_mip() {
                let mut y   = vec![0.0f64; nc];
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

                if y_ok {
                    let mut duals: HashMap<ConId, f64> = HashMap::new();
                    for (con, row) in self.row_map.iter() {
                        if let Some(v) = y.get(row as usize) {
                            duals.insert(con, *v);
                        }
                    }
                    self.duals = Some(duals);
                }

                if rc_ok {
                    let mut rc: HashMap<VarId, f64> = HashMap::new();
                    for (var, col) in self.col_map.iter() {
                        let c = col as usize;
                        let v = slx.get(c).copied().unwrap_or(0.0)
                            - sux.get(c).copied().unwrap_or(0.0);
                        rc.insert(var, v);
                    }
                    self.reduced_costs = Some(rc);
                }
            }
        } else {
            self.solution = None;
            self.objective_value = None;
            self.duals = None;
            self.reduced_costs = None;
        }

        Ok(solver_status)
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
        unsafe { ffi::MSK_deletetask(&mut self.task) };
        self.task = make_task(self.env, &self.opts);

        self.col_map = IndexMap::new();
        self.row_map = IndexMap::new();
        self.var_bounds.clear();
        self.con_bounds.clear();
        self.integer_vars.clear();
        self.obj_costs.clear();
        self.obj_senses.clear();
        self.active_obj = None;
        self.status = SolverStatus::NotSolved;
        self.solution = None;
        self.objective_value = None;
        self.duals = None;
        self.reduced_costs = None;
    }

    fn supports_incremental(&self, _change: &Change) -> bool {
        true
    }
}

// ── Private helper ─────────────────────────────────────────────────────────

fn make_task(env: MosekEnv, opts: &MosekOptions) -> MosekTask {
    let mut task: MosekTask = std::ptr::null_mut();
    let ret = unsafe { ffi::MSK_maketask(env, 0, 0, &mut task) };
    assert!(ret == ffi::RES_OK, "MSK_maketask failed with code {ret}");

    unsafe { ffi::MSK_putintparam(task, ffi::IPAR_LOG, opts.log_level) };

    if let Some(threads) = opts.num_threads {
        unsafe { ffi::MSK_putintparam(task, ffi::IPAR_NUM_THREADS, threads) };
    }

    task
}
