//! HiGHS solver adapter implementing roml's `SolverAdapter` trait.
//!
//! # Design
//!
//! - All HiGHS state is owned by an opaque `*mut c_void` handle.
//! - `col_map` / `row_map` maintain Id → HiGHS-index bidirectional maps.
//! - Variable/constraint bounds are cached locally so we can restore them
//!   when deactivated entities are reactivated.
//! - Objective costs per objective are cached so switching active objective
//!   only requires zeroing the current costs and loading the new ones.
//! - Inactive variables are fixed at [0, 0]; inactive constraints become
//!   [-∞, +∞] (unconstrained rows).
//! - When a column/row is deleted, `reindex_after_delete` keeps maps in sync
//!   with HiGHS's dense integer addressing.

use std::collections::HashMap;
use roml::id::{ConId, ObjId, VarId};
use roml::model::changelog::Change;
use roml::model::coefficient::CoefficientTarget;
use roml::model::objective::Sense;
use roml::model::variable::VarType;
use roml::solver::{SolverAdapter, SolverError, SolverStatus};
use log::{info, warn};

use crate::ffi;
use crate::ffi::HighsInt;
use crate::index_map::IndexMap;

// ── Helper ─────────────────────────────────────────────────────────────────

fn highs_err(msg: impl Into<String>) -> SolverError {
    SolverError::InternalError(msg.into())
}

fn check_status(ret: HighsInt, op: &str) -> Result<(), SolverError> {
    if ret == ffi::STATUS_OK {
        Ok(())
    } else {
        Err(highs_err(format!("{op} returned status {ret}")))
    }
}

// ── Adapter struct ─────────────────────────────────────────────────────────

/// HiGHS solver adapter.
///
/// Create with `HighsAdapter::new()`, pass to `Model::with_solver()` or call
/// `apply_changes` / `solve` directly.
pub struct HighsAdapter {
    /// Opaque HiGHS instance handle.
    ptr: *mut std::ffi::c_void,

    /// VarId → HiGHS column index.
    col_map: IndexMap<VarId>,

    /// ConId → HiGHS row index.
    row_map: IndexMap<ConId>,

    /// Cached variable bounds (lb, ub) — used to restore after reactivation.
    var_bounds: HashMap<VarId, (f64, f64)>,

    /// Cached constraint bounds (lb, ub) — used to restore after reactivation.
    con_bounds: HashMap<ConId, (f64, f64)>,

    /// Per-objective stored costs: obj → (var → cost).
    /// Maintained so we can switch active objective efficiently.
    obj_costs: HashMap<ObjId, HashMap<VarId, f64>>,

    /// Sense for each known objective.
    obj_senses: HashMap<ObjId, Sense>,

    /// Currently active objective (if any).
    active_obj: Option<ObjId>,

    /// Last solver status.
    status: SolverStatus,

    /// Last solution: VarId → value.
    solution: Option<HashMap<VarId, f64>>,

    /// Objective value from the last solve.
    objective_value: Option<f64>,

    /// Dual values for constraints from the last solve (LP only).
    duals: Option<HashMap<ConId, f64>>,

    /// Reduced costs for variables from the last solve (LP only).
    reduced_costs: Option<HashMap<VarId, f64>>,

    /// HiGHS +∞ value (from Highs_getInfinity).
    inf: f64,
}

// SAFETY: HiGHS is a C library. We never share the pointer across threads.
// Users are responsible for not calling it from multiple threads simultaneously.
unsafe impl Send for HighsAdapter {}

/// Options forwarded to HiGHS at adapter creation time.
#[derive(Debug, Clone)]
pub struct HighsOptions {
    /// Number of solver threads. Defaults to 1 — LP dual simplex is sequential
    /// and extra threads only sleep on `HighsBinarySemaphore`.
    pub threads: i32,
}

impl Default for HighsOptions {
    fn default() -> Self {
        Self { threads: 1 }
    }
}

impl HighsAdapter {
    /// Create a new HiGHS adapter with default options.
    ///
    /// # Panics
    ///
    /// Panics if `Highs_create()` returns null or if HiGHS was compiled with
    /// 64-bit indexing (we require `HighsInt = i32`).
    pub fn new() -> Self {
        Self::with_options(HighsOptions::default())
    }

    /// Create a new HiGHS adapter with explicit options.
    pub fn with_options(opts: HighsOptions) -> Self {
        let ptr = unsafe { ffi::Highs_create() };
        assert!(!ptr.is_null(), "Highs_create() returned null");

        // Validate that HiGHS was compiled with 32-bit HighsInt.
        let sz = unsafe { ffi::Highs_getSizeofHighsInt(ptr) };
        assert_eq!(
            sz, 4,
            "Expected HighsInt = i32 (size 4 bytes), HiGHS reports size {sz}. \
             Rebuild HiGHS with -DHIGHS_INT64=OFF."
        );

        // HiGHS output ON/OFF.
        let output_flag = c"output_flag";
        unsafe {
            ffi::Highs_setBoolOptionValue(ptr, output_flag.as_ptr(), 1);
        }

        let threads_key = c"threads";
        unsafe {
            ffi::Highs_setIntOptionValue(ptr, threads_key.as_ptr(), opts.threads);
        }

        let inf = unsafe { ffi::Highs_getInfinity(ptr) };

        Self {
            ptr,
            col_map: IndexMap::new(),
            row_map: IndexMap::new(),
            var_bounds: HashMap::new(),
            con_bounds: HashMap::new(),
            obj_costs: HashMap::new(),
            obj_senses: HashMap::new(),
            active_obj: None,
            status: SolverStatus::NotSolved,
            solution: None,
            objective_value: None,
            duals: None,
            reduced_costs: None,
            inf,
        }
    }

    // ── Internal helpers ──────────────────────────────────────────────────

    fn sense_to_highs(sense: Sense) -> HighsInt {
        match sense {
            Sense::Minimize => ffi::OBJ_SENSE_MINIMIZE,
            Sense::Maximize => ffi::OBJ_SENSE_MAXIMIZE,
        }
    }

    fn var_type_to_highs(vt: VarType) -> HighsInt {
        match vt {
            VarType::Continuous => ffi::VAR_TYPE_CONTINUOUS,
            VarType::Integer | VarType::Binary => ffi::VAR_TYPE_INTEGER,
        }
    }

    fn map_status(raw: HighsInt) -> SolverStatus {
        match raw {
            ffi::MODEL_STATUS_OPTIMAL => SolverStatus::Optimal,
            ffi::MODEL_STATUS_INFEASIBLE | ffi::MODEL_STATUS_UNBOUNDED_OR_INFEASIBLE => {
                SolverStatus::Infeasible
            }
            ffi::MODEL_STATUS_UNBOUNDED => SolverStatus::Unbounded,
            ffi::MODEL_STATUS_TIME_LIMIT => SolverStatus::TimeLimit,
            ffi::MODEL_STATUS_ITERATION_LIMIT => SolverStatus::IterationLimit,
            _ => SolverStatus::Error,
        }
    }

    /// Apply a single change to HiGHS.
    fn apply_one(&mut self, change: &Change) -> Result<(), SolverError> {
        match change {
            // ── Variable Added ────────────────────────────────────────────
            Change::VariableAdded { var, bounds, var_type } => {
                let lb = if bounds.lower == f64::NEG_INFINITY { -self.inf } else { bounds.lower };
                let ub = if bounds.upper == f64::INFINITY { self.inf } else { bounds.upper };

                // Capture the next column index *before* calling addVar.
                // Highs_addVar returns a status code.
                // Pre-capture is robust even if it returns the index.
                let col_idx = unsafe { ffi::Highs_getNumCol(self.ptr) };
                let ret = unsafe { ffi::Highs_addVar(self.ptr, lb, ub) };
                if ret < 0 {
                    return Err(highs_err("Highs_addVar failed"));
                }
                self.col_map.insert(*var, col_idx);
                self.var_bounds.insert(*var, (bounds.lower, bounds.upper));

                if matches!(var_type, VarType::Integer | VarType::Binary) {
                    let ret = unsafe {
                        ffi::Highs_changeColIntegrality(
                            self.ptr,
                            col_idx,
                            Self::var_type_to_highs(*var_type),
                        )
                    };
                    check_status(ret, "Highs_changeColIntegrality")?;
                }
            }

            // ── Variable Removed ──────────────────────────────────────────
            Change::VariableRemoved { var } => {
                let col = match self.col_map.remove(*var) {
                    Some(c) => c,
                    None => return Ok(()), // already gone
                };
                self.var_bounds.remove(var);
                // Remove this var from all objective cost caches.
                for costs in self.obj_costs.values_mut() {
                    costs.remove(var);
                }
                let ret =
                    unsafe { ffi::Highs_deleteColsByRange(self.ptr, col, col) };
                check_status(ret, "Highs_deleteColsByRange")?;
                self.col_map.reindex_after_delete(col);
            }

            // ── Variable Bounds Changed ────────────────────────────────────
            Change::VariableBoundsChanged { var, new, .. } => {
                if let Some(col) = self.col_map.get(*var) {
                    let lb = if new.lower == f64::NEG_INFINITY { -self.inf } else { new.lower };
                    let ub = if new.upper == f64::INFINITY { self.inf } else { new.upper };
                    let ret = unsafe { ffi::Highs_changeColBounds(self.ptr, col, lb, ub) };
                    check_status(ret, "Highs_changeColBounds")?;
                    self.var_bounds.insert(*var, (new.lower, new.upper));
                }
            }

            // ── Variable Type Changed ──────────────────────────────────────
            Change::VariableTypeChanged { var, new, .. } => {
                if let Some(col) = self.col_map.get(*var) {
                    let ret = unsafe {
                        ffi::Highs_changeColIntegrality(
                            self.ptr,
                            col,
                            Self::var_type_to_highs(*new),
                        )
                    };
                    check_status(ret, "Highs_changeColIntegrality")?;
                }
            }

            // ── Variable Activity Changed ──────────────────────────────────
            Change::VariableActivityChanged { var, active } => {
                if let Some(col) = self.col_map.get(*var) {
                    let (lb, ub) = if *active {
                        // Restore original bounds.
                        let (orig_lb, orig_ub) = self.var_bounds.get(var).copied().unwrap_or((0.0, self.inf));
                        let lb = if orig_lb == f64::NEG_INFINITY { -self.inf } else { orig_lb };
                        let ub = if orig_ub == f64::INFINITY { self.inf } else { orig_ub };
                        (lb, ub)
                    } else {
                        // Fix at zero.
                        (0.0, 0.0)
                    };
                    let ret = unsafe { ffi::Highs_changeColBounds(self.ptr, col, lb, ub) };
                    check_status(ret, "Highs_changeColBounds (activity)")?;
                }
            }

            // ── Constraint Added ───────────────────────────────────────────
            Change::ConstraintAdded { con, bounds } => {
                let lb = if bounds.lower == f64::NEG_INFINITY { -self.inf } else { bounds.lower };
                let ub = if bounds.upper == f64::INFINITY { self.inf } else { bounds.upper };

                // Capture the next row index *before* calling addRow.
                // Highs_addRow may return the index or a status code depending
                // on HiGHS version; pre-capture is robust either way.
                let row_idx = unsafe { ffi::Highs_getNumRow(self.ptr) };
                let ret = unsafe {
                    ffi::Highs_addRow(self.ptr, lb, ub, 0, std::ptr::null(), std::ptr::null())
                };
                if ret < 0 {
                    return Err(highs_err("Highs_addRow failed"));
                }
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
                let ret =
                    unsafe { ffi::Highs_deleteRowsByRange(self.ptr, row, row) };
                check_status(ret, "Highs_deleteRowsByRange")?;
                self.row_map.reindex_after_delete(row);
            }

            // ── Constraint Bounds Changed ──────────────────────────────────
            Change::ConstraintBoundsChanged { con, new, .. } => {
                if let Some(row) = self.row_map.get(*con) {
                    let lb = if new.lower == f64::NEG_INFINITY { -self.inf } else { new.lower };
                    let ub = if new.upper == f64::INFINITY { self.inf } else { new.upper };
                    let ret = unsafe { ffi::Highs_changeRowBounds(self.ptr, row, lb, ub) };
                    check_status(ret, "Highs_changeRowBounds")?;
                    self.con_bounds.insert(*con, (new.lower, new.upper));
                }
            }

            // ── Constraint Activity Changed ────────────────────────────────
            Change::ConstraintActivityChanged { con, active } => {
                if let Some(row) = self.row_map.get(*con) {
                    let (lb, ub) = if *active {
                        let (orig_lb, orig_ub) = self.con_bounds.get(con).copied().unwrap_or((f64::NEG_INFINITY, f64::INFINITY));
                        let lb = if orig_lb == f64::NEG_INFINITY { -self.inf } else { orig_lb };
                        let ub = if orig_ub == f64::INFINITY { self.inf } else { orig_ub };
                        (lb, ub)
                    } else {
                        (-self.inf, self.inf)
                    };
                    let ret = unsafe { ffi::Highs_changeRowBounds(self.ptr, row, lb, ub) };
                    check_status(ret, "Highs_changeRowBounds (activity)")?;
                }
            }

            // ── Coefficient Added ──────────────────────────────────────────
            Change::CoefficientAdded { var, target, value, .. } => {
                match target {
                    CoefficientTarget::Constraint(con) => {
                        if let (Some(row), Some(col)) =
                            (self.row_map.get(*con), self.col_map.get(*var))
                        {
                            let ret = unsafe {
                                ffi::Highs_changeCoeff(self.ptr, row, col, *value)
                            };
                            check_status(ret, "Highs_changeCoeff (add)")?;
                        }
                    }
                    CoefficientTarget::Objective(obj) => {
                        self.obj_costs.entry(*obj).or_default().insert(*var, *value);
                        if Some(*obj) == self.active_obj {
                            if let Some(col) = self.col_map.get(*var) {
                                let ret =
                                    unsafe { ffi::Highs_changeColCost(self.ptr, col, *value) };
                                check_status(ret, "Highs_changeColCost (add)")?;
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
                            let ret =
                                unsafe { ffi::Highs_changeCoeff(self.ptr, row, col, 0.0) };
                            check_status(ret, "Highs_changeCoeff (remove)")?;
                        }
                    }
                    CoefficientTarget::Objective(obj) => {
                        if let Some(costs) = self.obj_costs.get_mut(obj) {
                            costs.remove(var);
                        }
                        if Some(*obj) == self.active_obj {
                            if let Some(col) = self.col_map.get(*var) {
                                let ret =
                                    unsafe { ffi::Highs_changeColCost(self.ptr, col, 0.0) };
                                check_status(ret, "Highs_changeColCost (remove)")?;
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
                            let ret =
                                unsafe { ffi::Highs_changeCoeff(self.ptr, row, col, *new) };
                            check_status(ret, "Highs_changeCoeff (update)")?;
                        }
                    }
                    CoefficientTarget::Objective(obj) => {
                        if let Some(costs) = self.obj_costs.get_mut(obj) {
                            costs.insert(*var, *new);
                        }
                        if Some(*obj) == self.active_obj {
                            if let Some(col) = self.col_map.get(*var) {
                                let ret =
                                    unsafe { ffi::Highs_changeColCost(self.ptr, col, *new) };
                                check_status(ret, "Highs_changeColCost (update)")?;
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
                    let ret = unsafe {
                        ffi::Highs_changeObjectiveSense(
                            self.ptr,
                            Self::sense_to_highs(*new),
                        )
                    };
                    check_status(ret, "Highs_changeObjectiveSense")?;
                }
            }

            // ── Active Objective Changed ───────────────────────────────────
            Change::ActiveObjectiveChanged { new, .. } => {
                // Zero out all current column costs.
                let num_cols = unsafe { ffi::Highs_getNumCol(self.ptr) };
                let zero_costs = vec![0.0f64; num_cols as usize];
                let ret = unsafe {
                    ffi::Highs_changeColsCostByRange(
                        self.ptr,
                        0,
                        num_cols - 1,
                        zero_costs.as_ptr(),
                    ) 
                };
                check_status(ret, "Highs_changeColsCostByRange (zero for obj switch)")?;

                if let Some(new_obj) = new {
                    // Load costs for the new objective.
                    if let Some(costs) = self.obj_costs.get(new_obj).cloned() {
                        let cols = costs.keys().filter_map(|var| self.col_map.get(*var)).collect::<Vec<_>>();
                        let costs = costs.values().copied().collect::<Vec<_>>();
                        assert!(cols.len() == costs.len());
                        let ret = unsafe {
                            ffi::Highs_changeColsCostBySet(
                                self.ptr,
                                cols.len() as HighsInt,
                                cols.as_ptr(),
                                costs.as_ptr(),
                            )
                        };
                        check_status(ret, "Highs_changeColsCostBySet (obj switch load)")?;
                    }
                    // Apply the new objective's sense.
                    if let Some(&sense) = self.obj_senses.get(new_obj) {
                        let ret = unsafe {
                            ffi::Highs_changeObjectiveSense(
                                self.ptr,
                                Self::sense_to_highs(sense),
                            )
                        };
                        check_status(ret, "Highs_changeObjectiveSense (obj switch)")?;
                    }
                    self.active_obj = Some(*new_obj);
                } else {
                    self.active_obj = None;
                }
            }

            // ── Parameter Value Changed ────────────────────────────────────
            // No-op: the coefficient deltas will follow as CoefficientValueChanged.
            Change::ParameterValueChanged { .. } => {}
        }
        Ok(())
    }
}

impl Drop for HighsAdapter {
    fn drop(&mut self) {
        unsafe { ffi::Highs_destroy(self.ptr) }
    }
}

// ── SolverAdapter implementation ───────────────────────────────────────────

impl SolverAdapter for HighsAdapter {
    fn apply_changes(&mut self, changes: &[Change]) -> Result<(), SolverError> {
        let mut i = 0;
        while i < changes.len() {
            // Batch: ConstraintAdded immediately followed by CoefficientAdded(Constraint) events
            // for the same constraint → single Highs_addRow with all coefficients instead of
            // N individual Highs_changeCoeff calls.
            if let Change::ConstraintAdded { con, bounds } = &changes[i] {
                let lb = if bounds.lower == f64::NEG_INFINITY { -self.inf } else { bounds.lower };
                let ub = if bounds.upper == f64::INFINITY { self.inf } else { bounds.upper };

                let mut cols: Vec<HighsInt> = Vec::new();
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

                let row_idx = unsafe { ffi::Highs_getNumRow(self.ptr) };
                let ret = if cols.is_empty() {
                    unsafe {
                        ffi::Highs_addRow(self.ptr, lb, ub, 0, std::ptr::null(), std::ptr::null())
                    }
                } else {
                    unsafe {
                        ffi::Highs_addRow(
                            self.ptr,
                            lb,
                            ub,
                            cols.len() as HighsInt,
                            cols.as_ptr(),
                            vals.as_ptr(),
                        )
                    }
                };
                if ret < 0 {
                    return Err(highs_err("Highs_addRow failed"));
                }
                self.row_map.insert(*con, row_idx);
                self.con_bounds.insert(*con, (bounds.lower, bounds.upper));

                i = j;
                continue;
            }

            self.apply_one(&changes[i])?;
            i += 1;
        }
        // Invalidate any cached solution — model has changed.
        self.solution = None;
        self.objective_value = None;
        self.duals = None;
        self.reduced_costs = None;
        self.status = SolverStatus::NotSolved;
        Ok(())
    }

    fn solve(&mut self) -> Result<SolverStatus, SolverError> {
        info!("Starting solve with {} variables and {} constraints", self.col_map.len(), self.row_map.len());
        // check if objective is empty; if so log a warning
        if let Some(obj) = self.active_obj {
            if let Some(costs) = self.obj_costs.get(&obj) {
                if costs.is_empty() {
                    warn!("Solving with active objective that has no costs. Essentially no objective.");
                }
            } else {
                warn!("Active objective not found in obj_costs. This should not happen, indicates a bug in change handling.");
            }
        } else {
            warn!("Warning: Solving with no active objective.");
        }
        let ret = unsafe { ffi::Highs_run(self.ptr) };
        check_status(ret, "Highs_run")?;

        let raw_status = unsafe { ffi::Highs_getModelStatus(self.ptr) };
        let solver_status = Self::map_status(raw_status);
        self.status = solver_status;

        if matches!(solver_status, SolverStatus::Optimal) {
            let num_cols = unsafe { ffi::Highs_getNumCol(self.ptr) } as usize;
            let num_rows = unsafe { ffi::Highs_getNumRow(self.ptr) } as usize;
            let mut col_values = vec![0.0f64; num_cols];
            let mut col_dual = vec![0.0f64; num_cols];
            let mut row_dual = vec![0.0f64; num_rows];
            let ret = unsafe {
                ffi::Highs_getSolution(
                    self.ptr,
                    col_values.as_mut_ptr(),
                    col_dual.as_mut_ptr(),
                    std::ptr::null_mut(), // row_value not needed
                    row_dual.as_mut_ptr(),
                )
            };
            check_status(ret, "Highs_getSolution")?;

            let obj_val = unsafe { ffi::Highs_getObjectiveValue(self.ptr) };
            self.objective_value = Some(obj_val);

            let mut sol: HashMap<VarId, f64> = HashMap::new();
            let mut rc: HashMap<VarId, f64> = HashMap::new();
            for (var, col) in self.col_map.iter() {
                if let Some(v) = col_values.get(col as usize) {
                    sol.insert(var, *v);
                }
                if let Some(v) = col_dual.get(col as usize) {
                    rc.insert(var, *v);
                }
            }
            self.solution = Some(sol);
            self.reduced_costs = Some(rc);

            // Row duals (constraint shadow prices). For MIP these will be
            // all-zero; callers should check the model type before using them.
            let mut duals: HashMap<ConId, f64> = HashMap::new();
            for (con, row) in self.row_map.iter() {
                if let Some(v) = row_dual.get(row as usize) {
                    duals.insert(con, *v);
                }
            }
            self.duals = Some(duals);
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
        unsafe { ffi::Highs_clearModel(self.ptr) };
        // Re-silence output after clear (clearModel resets options).
        let output_flag = c"output_flag";
        unsafe {
            ffi::Highs_setBoolOptionValue(self.ptr, output_flag.as_ptr(), 0);
        }
        self.col_map = IndexMap::new();
        self.row_map = IndexMap::new();
        self.var_bounds.clear();
        self.con_bounds.clear();
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
        // HiGHS supports all incremental change types we emit.
        true
    }
}
