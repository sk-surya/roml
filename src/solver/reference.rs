//! Reference projection backend.
//!
//! A solver-neutral backend that stores variables, rows, cells,
//! objectives, activity flags, and revision state. It supports:
//! - applying `DeltaBatch` operations incrementally
//! - rebuilding from a `ModelSnapshot`
//! - normalized state view for correctness comparison
//!
//! This backend proves the commuting square:
//! ```text
//! project(snapshot r1) == apply(project(snapshot r0), deltas r0→r1)
//! ```
//!
//! It is NOT optimized for performance — its purpose is correctness
//! verification, not production solving.

use std::collections::HashMap;

use crate::delta::{DeltaBatch, ModelOp};
use crate::id::{ConId, ObjId, ParamId, VarId};
use crate::model::coefficient::{CellKey, CoefficientTarget};
use crate::model::{Bounds, ConstraintBounds, Sense, VarType};
use crate::revision::ModelRevision;
use crate::snapshot::ModelSnapshot;
use crate::solver::backend::{BackendCapabilities, BackendError, ErrorCategory, HealthEffect};
use crate::solver::request::{SolveRequest, SolveResult};
use crate::solver::session::{BackendMetadata, BackendSession, SessionHealth, SyncReceipt, Synchronization};
use crate::sync::{AdapterCursor, AdapterHealth, ApplyOutcome};
use crate::value_expr::ValueExpr;

/// A reference projection of model state (solver-neutral).
///
/// Stores all entity state in HashMaps exactly as the model would.
/// Used for correctness comparisons, not optimization.
#[derive(Clone, Debug, Default)]
pub struct ReferenceBackend {
    pub revision: ModelRevision,

    /// Synchronization cursor tracking applied revision and health.
    pub cursor: AdapterCursor,

    /// Variables: id → (bounds, var_type, active)
    pub variables: HashMap<VarId, (Bounds, VarType, bool)>,

    /// Semi-continuous lower bounds
    pub semicontinuous: HashMap<VarId, f64>,

    /// Constraints: id → (bounds, active)
    pub constraints: HashMap<ConId, (ConstraintBounds, bool)>,

    /// Objectives: id → (sense, active)
    pub objectives: HashMap<ObjId, (Sense, bool)>,

    /// Active objective (at most one)
    pub active_objective: Option<ObjId>,

    /// Parameters: id → value
    pub parameters: HashMap<ParamId, f64>,

    /// Constraint cells: CellKey → (value_expr, evaluated_value)
    pub constraint_cells: HashMap<CellKey, (ValueExpr, f64)>,

    /// Objective cells: CellKey → (value_expr, evaluated_value, constant)
    pub objective_cells: HashMap<CellKey, (ValueExpr, f64, f64)>,
}

/// Methods used by backend contract tests and adapters.
impl ReferenceBackend {
    /// Create an empty backend.
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply a single ModelOp to this backend.
    pub fn apply_op(&mut self, op: &ModelOp) -> Result<(), String> {
        match op {
            ModelOp::AddVariable {
                var,
                bounds,
                var_type,
            } => {
                self.variables.insert(*var, (*bounds, *var_type, true));
            }
            ModelOp::RemoveVariable { var } => {
                self.variables.remove(var);
                self.semicontinuous.remove(var);
                // Remove all cells involving this variable
                self.constraint_cells.retain(|k, _| k.1 != *var);
                self.objective_cells.retain(|k, _| k.1 != *var);
            }
            ModelOp::SetVariableBounds { var, bounds } => {
                if let Some(entry) = self.variables.get_mut(var) {
                    entry.0 = *bounds;
                }
            }
            ModelOp::SetVariableActive { var, active } => {
                if let Some(entry) = self.variables.get_mut(var) {
                    entry.2 = *active;
                }
            }
            ModelOp::SetVariableType { var, var_type } => {
                if let Some(entry) = self.variables.get_mut(var) {
                    entry.1 = *var_type;
                }
            }
            ModelOp::AddConstraint { con, bounds } => {
                self.constraints.insert(*con, (*bounds, true));
            }
            ModelOp::RemoveConstraint { con } => {
                self.constraints.remove(con);
                // Remove cells for this constraint
                self.constraint_cells.retain(|k, _| match k.0 {
                    CoefficientTarget::Constraint(c) => c != *con,
                    _ => true,
                });
            }
            ModelOp::SetConstraintBounds { con, bounds } => {
                if let Some(entry) = self.constraints.get_mut(con) {
                    entry.0 = *bounds;
                }
            }
            ModelOp::SetConstraintActive { con, active } => {
                if let Some(entry) = self.constraints.get_mut(con) {
                    entry.1 = *active;
                }
            }
            ModelOp::SetCell {
                cell_key,
                value_expr,
                evaluated_value,
            } => {
                self.constraint_cells
                    .insert(*cell_key, (value_expr.clone(), *evaluated_value));
            }
            ModelOp::RemoveCell { cell_key } => {
                self.constraint_cells.remove(cell_key);
            }
            ModelOp::AddObjective { obj, sense } => {
                self.objectives.insert(*obj, (*sense, false));
            }
            ModelOp::RemoveObjective { obj } => {
                self.objectives.remove(obj);
                if self.active_objective == Some(*obj) {
                    self.active_objective = None;
                }
                self.objective_cells.retain(|k, _| match k.0 {
                    CoefficientTarget::Objective(o) => o != *obj,
                    _ => true,
                });
            }
            ModelOp::SetActiveObjective { obj } => {
                // Deactivate previous
                if let Some(prev) = self.active_objective {
                    if let Some(entry) = self.objectives.get_mut(&prev) {
                        entry.1 = false;
                    }
                }
                self.active_objective = *obj;
                if let Some(new) = obj {
                    if let Some(entry) = self.objectives.get_mut(new) {
                        entry.1 = true;
                    }
                }
            }
            ModelOp::SetObjectiveCell {
                cell_key,
                value_expr,
                evaluated_value,
                constant,
            } => {
                self.objective_cells
                    .insert(*cell_key, (value_expr.clone(), *evaluated_value, *constant));
            }
            ModelOp::SetParameter { param, value } => {
                self.parameters.insert(*param, *value);
            }
        }
        Ok(())
    }

    /// Apply an entire delta batch.
    pub fn apply_batch(
        &mut self,
        batch: &DeltaBatch,
        cursor: &mut AdapterCursor,
    ) -> Result<ApplyOutcome, String> {
        if batch.from != cursor.applied_revision {
            return Ok(ApplyOutcome::RecoverableFailure {
                reason: format!(
                    "batch from {} != cursor at {}",
                    batch.from, cursor.applied_revision
                ),
            });
        }

        for (i, op) in batch.operations.iter().enumerate() {
            if let Err(reason) = self.apply_op(op) {
                return Ok(ApplyOutcome::RequiresRebuild {
                    failed_at_op: i,
                    reason,
                });
            }
        }

        cursor.advance(batch).map_err(|e| e.to_string())?;
        self.revision = cursor.applied_revision;
        Ok(ApplyOutcome::Applied {
            new_revision: cursor.applied_revision,
        })
    }

    /// Rebuild this backend from a snapshot (deterministic full projection).
    pub fn rebuild(&mut self, snapshot: &ModelSnapshot, cursor: &mut AdapterCursor) {
        self.variables.clear();
        self.constraints.clear();
        self.objectives.clear();
        self.constraint_cells.clear();
        self.objective_cells.clear();
        self.parameters.clear();
        self.semicontinuous.clear();

        for v in &snapshot.variables {
            self.variables
                .insert(v.id, (v.bounds, v.var_type, v.active));
            if let Some(lower) = v.semicontinuous_lower {
                self.semicontinuous.insert(v.id, lower);
            }
        }

        for c in &snapshot.constraints {
            self.constraints.insert(c.id, (c.bounds, c.active));
        }

        for o in &snapshot.objectives {
            self.objectives.insert(o.id, (o.sense, o.active));
            if o.active {
                self.active_objective = Some(o.id);
            }
        }

        for p in &snapshot.parameters {
            self.parameters.insert(p.id, p.value);
        }

        for cell in &snapshot.cells {
            match cell.cell_key.0 {
                CoefficientTarget::Constraint(_) => {
                    self.constraint_cells.insert(
                        cell.cell_key,
                        (cell.value_expr.clone(), cell.evaluated_value),
                    );
                }
                CoefficientTarget::Objective(_) => {
                    // Find the objective to get its constant
                    let constant = snapshot
                        .objectives
                        .iter()
                        .find(|o| {
                            matches!(
                                cell.cell_key.0,
                                CoefficientTarget::Objective(oid) if oid == o.id
                            )
                        })
                        .map(|o| o.constant)
                        .unwrap_or(0.0);
                    self.objective_cells.insert(
                        cell.cell_key,
                        (cell.value_expr.clone(), cell.evaluated_value, constant),
                    );
                }
            }
        }

        cursor.mark_ready(snapshot.revision);
        self.revision = snapshot.revision;
    }

    /// Produce a normalized state view for comparison.
    ///
    /// Returns sorted vectors of key state for deterministic comparison.
    pub fn normalized_view(&self) -> NormalizedView {
        let mut vars: Vec<_> = self
            .variables
            .iter()
            .map(|(id, (bounds, var_type, active))| {
                (
                    *id,
                    *bounds,
                    *var_type,
                    *active,
                    self.semicontinuous.get(id).copied(),
                )
            })
            .collect();
        vars.sort_by_key(|(id, ..)| *id);

        let mut cons: Vec<_> = self
            .constraints
            .iter()
            .map(|(id, (bounds, active))| (*id, *bounds, *active))
            .collect();
        cons.sort_by_key(|(id, ..)| *id);

        let mut objs: Vec<_> = self
            .objectives
            .iter()
            .map(|(id, (sense, active))| (*id, *sense, *active))
            .collect();
        objs.sort_by_key(|(id, ..)| *id);

        let mut cells: Vec<_> = self
            .constraint_cells
            .iter()
            .map(|(k, (_, value))| (*k, *value))
            .collect();
        cells.sort_by_key(|(k, _)| *k);

        let mut obj_cells: Vec<_> = self
            .objective_cells
            .iter()
            .map(|(k, (_, value, constant))| (*k, *value, *constant))
            .collect();
        obj_cells.sort_by_key(|(k, ..)| *k);

        let mut params: Vec<_> = self
            .parameters
            .iter()
            .map(|(id, value)| (*id, *value))
            .collect();
        params.sort_by_key(|(id, _)| *id);

        NormalizedView {
            revision: self.revision,
            active_objective: self.active_objective,
            variables: vars,
            constraints: cons,
            objectives: objs,
            parameters: params,
            cells,
            objective_cells: obj_cells,
        }
    }
}

// ── BackendSession ──────────────────────────────────────────────────────────────

impl BackendSession for ReferenceBackend {
    /// Apply a [`Synchronization`] — either a full rebuild from snapshot or
    /// an incremental delta batch.
    fn synchronize(&mut self, sync: Synchronization) -> Result<SyncReceipt, BackendError> {
        match sync {
            Synchronization::DeltaBatch(batch) => {
                // Take cursor out temporarily to avoid borrow conflict with self
                let mut tmp_cursor = std::mem::take(&mut self.cursor);
                let result = match self.apply_batch(&batch, &mut tmp_cursor) {
                    Ok(ApplyOutcome::Applied { .. }) => {
                        self.cursor = tmp_cursor;
                        Ok(SyncReceipt {
                            cursor: self.cursor.clone(),
                            health: AdapterHealth::Ready,
                        })
                    }
                    Ok(ApplyOutcome::RecoverableFailure { reason }) => {
                        self.cursor = tmp_cursor;
                        Err(BackendError::new(reason, ErrorCategory::InvalidInput, HealthEffect::Recoverable))
                    }
                    Ok(ApplyOutcome::RequiresRebuild { reason, .. }) => {
                        self.cursor = tmp_cursor;
                        Err(BackendError::new(reason, ErrorCategory::Unsupported, HealthEffect::RequiresRebuild))
                    }
                    Ok(ApplyOutcome::DirtyFailure { reason }) => {
                        self.cursor = tmp_cursor;
                        Err(BackendError::new(
                            reason,
                            ErrorCategory::Internal,
                            HealthEffect::RequiresRebuild,
                        ))
                    }
                    Err(e) => {
                        self.cursor = tmp_cursor;
                        Err(BackendError::new(
                            e.to_string(),
                            ErrorCategory::Internal,
                            HealthEffect::Terminal,
                        ))
                    }
                };
                result
            }
            Synchronization::Rebuild(snapshot) => {
                let mut tmp_cursor = std::mem::take(&mut self.cursor);
                self.rebuild(&snapshot, &mut tmp_cursor);
                self.cursor = tmp_cursor;
                Ok(SyncReceipt {
                    cursor: self.cursor.clone(),
                    health: AdapterHealth::Ready,
                })
            }
        }
    }

    /// ReferenceBackend does not support solve operations.
    fn solve(&mut self, _request: &SolveRequest) -> Result<SolveResult, BackendError> {
        Err(BackendError::new(
            "ReferenceBackend does not support solve operations",
            ErrorCategory::Unsupported,
            HealthEffect::None,
        ))
    }

    /// Close the session — no-op for the reference backend.
    fn close(self) -> Result<(), BackendError> {
        Ok(())
    }
}

// ── SessionHealth ───────────────────────────────────────────────────────────────

impl SessionHealth for ReferenceBackend {
    fn health(&self) -> AdapterHealth {
        self.cursor.health
    }

    fn revision(&self) -> ModelRevision {
        self.cursor.applied_revision
    }
}

// ── BackendMetadata ─────────────────────────────────────────────────────────────

impl BackendMetadata for ReferenceBackend {
    fn name(&self) -> &str {
        "ReferenceBackend"
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            add_variable: true,
            add_constraint: true,
            set_coefficient: true,
            set_bounds: true,
            set_objective: true,
            delete: true,
            semicontinuous: true,
            semiinteger: true,
            parameter_update: true,
            // All solve flags are false — ReferenceBackend does not solve.
            lp: false,
            mip: false,
            callbacks: false,
            solution: false,
            duals: false,
            reduced_costs: false,
        }
    }
}

/// A normalized, deterministic view of backend state for comparison.
#[derive(Clone, Debug, PartialEq)]
pub struct NormalizedView {
    pub revision: ModelRevision,
    pub active_objective: Option<ObjId>,
    pub variables: Vec<(VarId, Bounds, VarType, bool, Option<f64>)>,
    pub constraints: Vec<(ConId, ConstraintBounds, bool)>,
    pub objectives: Vec<(ObjId, Sense, bool)>,
    pub parameters: Vec<(ParamId, f64)>,
    pub cells: Vec<(CellKey, f64)>,
    pub objective_cells: Vec<(CellKey, f64, f64)>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::delta::DeltaBatch;
    use crate::id::Generation;
    use crate::snapshot::take_snapshot;

    fn make_var(index: u32) -> VarId {
        VarId::new(index, Generation::new())
    }
    fn make_con(index: u32) -> ConId {
        ConId::new(index, Generation::new())
    }
    fn _make_obj(index: u32) -> ObjId {
        ObjId::new(index, Generation::new())
    }
    fn make_param(index: u32) -> ParamId {
        ParamId::new(index, Generation::new())
    }

    #[test]
    fn empty_backend() {
        let backend = ReferenceBackend::new();
        assert_eq!(backend.revision, ModelRevision::ZERO);
        assert!(backend.variables.is_empty());
    }

    #[test]
    fn build_from_snapshot_and_apply_deltas_are_equivalent() {
        // Build backend A from snapshot at r1
        // Build backend B from snapshot at r0, then apply deltas r0→r1
        // They must produce the same normalized view.

        let var = make_var(0);
        let con = make_con(0);
        let p = make_param(0);

        let r0 = ModelRevision::ZERO;
        let r1 = r0.next().unwrap();

        // --- Snapshot at r0 (empty) ---
        let snap_r0 = ModelSnapshot::empty(r0);

        // --- Snapshot at r1 (has var, con, param, cell) ---
        let mut vars_r1 = HashMap::new();
        vars_r1.insert(var, (Bounds::NON_NEGATIVE, VarType::Continuous, true, None));
        let mut cons_r1 = HashMap::new();
        cons_r1.insert(con, (ConstraintBounds::le(10.0), true));
        let mut params_r1 = HashMap::new();
        params_r1.insert(p, 5.0);
        let objs_r1 = HashMap::new();
        let cells_r1: Vec<(CellKey, ValueExpr, f64, Vec<ParamId>)> = vec![(
            (CoefficientTarget::Constraint(con), var),
            ValueExpr::param(p),
            5.0,
            vec![p],
        )];

        let snap_r1 = take_snapshot(r1, &vars_r1, &cons_r1, &objs_r1, &params_r1, &cells_r1);

        // --- Deltas from r0 to r1 ---
        let ops = vec![
            ModelOp::AddVariable {
                var,
                bounds: Bounds::NON_NEGATIVE,
                var_type: VarType::Continuous,
            },
            ModelOp::AddConstraint {
                con,
                bounds: ConstraintBounds::le(10.0),
            },
            ModelOp::SetParameter {
                param: p,
                value: 5.0,
            },
            ModelOp::SetCell {
                cell_key: (CoefficientTarget::Constraint(con), var),
                value_expr: ValueExpr::param(p),
                evaluated_value: 5.0,
            },
        ];
        let batch = DeltaBatch::new(r0, r1, ops).unwrap();

        // --- Backend A: rebuild from r1 snapshot ---
        let mut backend_a = ReferenceBackend::new();
        let mut cursor_a = AdapterCursor::new();
        backend_a.rebuild(&snap_r1, &mut cursor_a);
        let view_a = backend_a.normalized_view();

        // --- Backend B: rebuild from r0, then apply deltas ---
        let mut backend_b = ReferenceBackend::new();
        let mut cursor_b = AdapterCursor::new();
        backend_b.rebuild(&snap_r0, &mut cursor_b);
        let outcome = backend_b.apply_batch(&batch, &mut cursor_b).unwrap();
        assert!(matches!(outcome, ApplyOutcome::Applied { .. }));
        let view_b = backend_b.normalized_view();

        // --- They must be equivalent ---
        assert_eq!(
            view_a, view_b,
            "snapshot r1 != apply(snapshot r0, deltas r0→r1)"
        );
    }

    #[test]
    fn rebuild_resets_state() {
        let var = make_var(0);
        let con = make_con(0);

        let mut backend = ReferenceBackend::new();
        let mut cursor = AdapterCursor::new();

        // Apply some mutations
        let r0 = ModelRevision::ZERO;
        let r1 = r0.next().unwrap();
        let ops = vec![
            ModelOp::AddVariable {
                var,
                bounds: Bounds::NON_NEGATIVE,
                var_type: VarType::Continuous,
            },
            ModelOp::AddConstraint {
                con,
                bounds: ConstraintBounds::le(10.0),
            },
            ModelOp::SetCell {
                cell_key: (CoefficientTarget::Constraint(con), var),
                value_expr: ValueExpr::constant(3.0),
                evaluated_value: 3.0,
            },
        ];
        let batch = DeltaBatch::new(r0, r1, ops).unwrap();
        backend.apply_batch(&batch, &mut cursor).unwrap();

        assert_eq!(backend.variables.len(), 1);

        // Rebuild from empty snapshot
        let snap = ModelSnapshot::empty(r1);
        backend.rebuild(&snap, &mut cursor);
        assert!(backend.variables.is_empty());
        assert_eq!(backend.revision, r1);
    }

    #[test]
    fn objectiveless_rebuild() {
        let var = make_var(0);
        let con = make_con(0);

        let r1 = ModelRevision::ZERO.next().unwrap();
        let mut vars = HashMap::new();
        vars.insert(var, (Bounds::new(0.0, 1.0), VarType::Binary, true, None));
        let mut cons = HashMap::new();
        cons.insert(con, (ConstraintBounds::le(1.0), true));
        let objs = HashMap::new();
        let params = HashMap::new();
        let cells: Vec<(CellKey, ValueExpr, f64, Vec<ParamId>)> = vec![(
            (CoefficientTarget::Constraint(con), var),
            ValueExpr::constant(1.0),
            1.0,
            vec![],
        )];

        let snap = take_snapshot(r1, &vars, &cons, &objs, &params, &cells);

        let mut backend = ReferenceBackend::new();
        let mut cursor = AdapterCursor::new();
        backend.rebuild(&snap, &mut cursor);

        let view = backend.normalized_view();
        assert_eq!(view.revision, r1);
        assert_eq!(view.variables.len(), 1);
        assert_eq!(view.constraints.len(), 1);
        assert_eq!(view.cells.len(), 1);
        assert!(view.objectives.is_empty());
    }
}
