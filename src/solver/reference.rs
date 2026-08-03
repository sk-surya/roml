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

use crate::compiler::backend_ir::{
    BackendDeltaBatch, BackendOp, BackendSnapshot, CompilationId, CompiledConstraintId,
    CompiledEntityRef, CompiledEntityRegistry, CompiledObjectiveId, CompiledObjectivePolicy,
    CompiledVariableId,
};
use crate::compiler::origin::OverlayId;
use crate::compiler::CompileError;
use crate::delta::{DeltaBatch, ModelOp};
use crate::id::{ConId, ObjId, ParamId, VarId};
use crate::model::coefficient::{CellKey, CoefficientTarget};
use crate::model::{Bounds, ConstraintBounds, Sense, VarType};
use crate::revision::ModelRevision;
use crate::snapshot::ModelSnapshot;
use crate::solver::backend::{BackendError, ErrorCategory, HealthEffect};
use crate::solver::overlay::{
    CompiledOverlay, OverlayApplyReceipt, OverlayOp, OverlayRollbackOutcome,
};
use crate::solver::session::OverlaySession;
use crate::sync::{AdapterCursor, ApplyOutcome};
use crate::value_expr::ValueExpr;

/// A reference projection of model state (solver-neutral).
///
/// Stores all entity state in HashMaps exactly as the model would.
/// Used for correctness comparisons, not optimization.
#[derive(Clone, Debug)]
pub struct ReferenceBackend {
    /// The model revision this backend is synchronized to.
    pub revision: ModelRevision,

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

    /// Objective constants: ObjId → constant (tracked for SetCell routing to objective_cells)
    pub objective_constants: HashMap<ObjId, f64>,

    // ── Compiled-IR projection (P26 Task 7) ────────────────────────────────
    //
    // The compiled path consumes `BackendSnapshot`/`BackendDeltaBatch`
    // directly (SM-03.2: backends consume backend IR). The reference backend
    // proves the compiled commuting square:
    //   compiled_rebuild(snapshot rN)
    //       == apply(compiled_rebuild(snapshot r0), compiled_deltas r0→rN)
    // Compiled ids are dense indices; the canonical `variables`/`constraints`/
    // `objectives` maps above remain for the M2 characterization path.
    /// Compiled variables: id → (bounds, var_type).
    pub compiled_variables: HashMap<CompiledVariableId, (Bounds, VarType)>,
    /// Compiled rows: id → (bounds, coefficients).
    #[allow(clippy::type_complexity)]
    pub compiled_rows:
        HashMap<CompiledConstraintId, (ConstraintBounds, Vec<(CompiledVariableId, f64)>)>,
    /// Compiled objectives: id → (sense, coefficients, constant).
    #[allow(clippy::type_complexity)]
    pub compiled_objectives:
        HashMap<CompiledObjectiveId, (Sense, Vec<(CompiledVariableId, f64)>, f64)>,
    /// Active compiled objective policy.
    pub compiled_objective_policy: CompiledObjectivePolicy,
    /// Exact compiled id of the current compiled state (None when never set).
    pub current_compilation: Option<CompilationId>,
    /// Canonical revision of the current compiled state.
    pub compiled_revision: ModelRevision,
    /// Transactional overlay state captured at apply (P27 Task 10). `None`
    /// when no overlay is applied. Set by `apply_overlay`, consumed by
    /// `rollback_overlay`, and used by `verify_overlay_clean` to prove the
    /// base compiled state is restored exactly.
    overlay_state: Option<OverlayApplyState>,
}

/// Prior compiled state captured by an overlay apply, enabling the
/// transactional rollback and the post-rollback clean verification (SM-07.4).
#[derive(Clone, Debug)]
struct OverlayApplyState {
    /// The originating overlay (F3: part of the full receipt tuple validation).
    overlay_id: OverlayId,
    /// The exact base compiled state the overlay was applied on top of.
    base_compilation: CompilationId,
    /// The overlay-compounded compiled state.
    applied_compilation: CompilationId,
    /// Prior bounds of every compiled variable the overlay temporarily set.
    prior_bounds: HashMap<CompiledVariableId, Bounds>,
    /// The temporary compiled rows the overlay added.
    added_rows: Vec<CompiledConstraintId>,
    /// The base compiled objective policy (restored on rollback).
    prior_policy: CompiledObjectivePolicy,
    /// A deterministic view of the base compiled state (rollback's clean
    /// target).
    base_view: CompiledNormalizedView,
}

impl Default for ReferenceBackend {
    fn default() -> Self {
        Self {
            revision: ModelRevision::ZERO,
            variables: HashMap::new(),
            semicontinuous: HashMap::new(),
            constraints: HashMap::new(),
            objectives: HashMap::new(),
            active_objective: None,
            parameters: HashMap::new(),
            constraint_cells: HashMap::new(),
            objective_cells: HashMap::new(),
            objective_constants: HashMap::new(),
            compiled_variables: HashMap::new(),
            compiled_rows: HashMap::new(),
            compiled_objectives: HashMap::new(),
            compiled_objective_policy: CompiledObjectivePolicy::None,
            current_compilation: None,
            compiled_revision: ModelRevision::ZERO,
            overlay_state: None,
        }
    }
}

/// Methods used by backend contract tests and adapters.
#[allow(dead_code)]
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
            ModelOp::SetVariableFixing {
                var,
                fixing: _,
                effective_bounds,
            } => {
                // P27 Task 8: a persistent fixing change is a self-contained
                // bound update (SM-05.3/05.7) — the op carries the effective
                // bounds to apply (equal `[value, value]` for a fix; the
                // current declared bounds for an unfix).
                if let Some(entry) = self.variables.get_mut(var) {
                    entry.0 = *effective_bounds;
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
            } => match cell_key.0 {
                CoefficientTarget::Constraint(_) => {
                    self.constraint_cells
                        .insert(*cell_key, (value_expr.clone(), *evaluated_value));
                }
                CoefficientTarget::Objective(obj_id) => {
                    let constant = self
                        .objective_constants
                        .get(&obj_id)
                        .copied()
                        .unwrap_or(0.0);
                    self.objective_cells
                        .insert(*cell_key, (value_expr.clone(), *evaluated_value, constant));
                }
            },
            ModelOp::RemoveCell { cell_key } => {
                self.constraint_cells.remove(cell_key);
                self.objective_cells.remove(cell_key);
            }
            ModelOp::AddObjective { obj, sense } => {
                self.objectives.insert(*obj, (*sense, false));
                self.objective_constants.insert(*obj, 0.0);
            }
            ModelOp::RemoveObjective { obj } => {
                self.objectives.remove(obj);
                self.objective_constants.remove(obj);
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
                // Keep the per-objective constant authority in sync (API-03.5):
                // `SetObjectiveCell` reports the constant exactly once, so the
                // objective's constant must not be lost when its last cell is
                // later removed.
                if let CoefficientTarget::Objective(obj_id) = cell_key.0 {
                    self.objective_constants.insert(obj_id, *constant);
                }
            }
            ModelOp::SetParameter { param, value } => {
                self.parameters.insert(*param, *value);
            }
            ModelOp::SetObjectiveSense { obj, sense } => {
                if let Some(entry) = self.objectives.get_mut(obj) {
                    entry.0 = *sense;
                }
            }
            ModelOp::SetObjectiveConstant { obj, constant } => {
                self.objective_constants.insert(*obj, *constant);
                // Keep the per-cell cached constant in sync so a subsequent
                // normalized-view comparison matches the rebuild path.
                let keys: Vec<CellKey> = self
                    .objective_cells
                    .keys()
                    .filter(|k| matches!(k.0, CoefficientTarget::Objective(o) if o == *obj))
                    .cloned()
                    .collect();
                for key in keys {
                    if let Some(entry) = self.objective_cells.get_mut(&key) {
                        entry.2 = *constant;
                    }
                }
            }
            ModelOp::SetSemiContinuousBound { var, lower } => {
                self.semicontinuous.insert(*var, *lower);
            }
            // P25 Task 4: constructs are canonical entities, not backend rows
            // (SM-01.6). The reference backend does not compile them yet.
            ModelOp::AddConstruct { .. }
            | ModelOp::RemoveConstruct { .. }
            | ModelOp::SetConstructActive { .. } => {}
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
        self.objective_constants.clear();

        for v in &snapshot.variables {
            // IN-05: fold the persistent fixing into the effective bounds
            // (SM-05.3: a fixing compiles as equal lower/upper bounds), matching
            // the incremental `apply_op(SetVariableFixing)` path. `VariableEntry`
            // carries the fixing so a rebuild can reconstruct it — the canonical
            // rebuild-vs-delta commuting square must hold for fixed models. The
            // active flag stays a separate field (the canonical backend does not
            // fold activity into bounds, unlike the compiled snapshot fold).
            let bounds = match &v.fixing {
                Some(fixing) => Bounds::new(fixing.value, fixing.value),
                None => v.bounds,
            };
            self.variables.insert(v.id, (bounds, v.var_type, v.active));
            if let Some(lower) = v.semicontinuous_lower {
                self.semicontinuous.insert(v.id, lower);
            }
        }

        for c in &snapshot.constraints {
            self.constraints.insert(c.id, (c.bounds, c.active));
        }

        for o in &snapshot.objectives {
            self.objectives.insert(o.id, (o.sense, o.active));
            self.objective_constants.insert(o.id, o.constant);
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

        // Reconcile each objective cell's cached constant with the
        // authoritative per-objective constant (`objective_constants`). The
        // per-cell cache can go stale when a variable's cells are removed
        // (`RemoveVariable` prunes cells but leaves surviving cells' cached
        // constants unchanged), so the view reports the authoritative value —
        // this is what keeps the canonical commuting square (delta vs rebuild)
        // closed for objective constants.
        let mut obj_cells: Vec<_> = self
            .objective_cells
            .iter()
            .map(|(k, (_, value, constant))| {
                let constant = match k.0 {
                    CoefficientTarget::Objective(obj_id) => self
                        .objective_constants
                        .get(&obj_id)
                        .copied()
                        .unwrap_or(*constant),
                    CoefficientTarget::Constraint(_) => *constant,
                };
                (*k, *value, constant)
            })
            .collect();
        obj_cells.sort_by_key(|(k, ..)| *k);

        let mut params: Vec<_> = self
            .parameters
            .iter()
            .map(|(id, value)| (*id, *value))
            .collect();
        params.sort_by_key(|(id, _)| *id);

        let mut objective_constants: Vec<_> = self
            .objective_constants
            .iter()
            .map(|(id, constant)| (*id, *constant))
            .collect();
        objective_constants.sort_by_key(|(id, _)| *id);

        NormalizedView {
            revision: self.revision,
            active_objective: self.active_objective,
            variables: vars,
            constraints: cons,
            objectives: objs,
            parameters: params,
            cells,
            objective_cells: obj_cells,
            objective_constants,
        }
    }

    // ── Compiled-IR projection (P26 Task 7) ────────────────────────────────

    /// Rebuild the compiled state from a [`BackendSnapshot`] (deterministic
    /// full compiled projection).
    ///
    /// # Errors
    ///
    /// Returns a typed [`CompileError::InvalidReference`] when the snapshot
    /// contains a dangling reference (F5) — before any native state is
    /// mutated.
    pub fn rebuild_compiled(&mut self, snapshot: &BackendSnapshot) -> Result<(), CompileError> {
        // F5: reject a malformed snapshot before clearing/reconstructing state.
        snapshot.validate()?;
        self.compiled_variables.clear();
        self.compiled_rows.clear();
        self.compiled_objectives.clear();

        for v in &snapshot.variables {
            self.compiled_variables.insert(v.id, (v.bounds, v.var_type));
        }
        for r in &snapshot.linear_rows {
            self.compiled_rows
                .insert(r.id, (r.bounds, r.coefficients.clone()));
        }
        for o in &snapshot.objectives {
            self.compiled_objectives
                .insert(o.id, (o.sense, o.coefficients.clone(), o.constant));
        }

        self.compiled_objective_policy = snapshot.objective_policy.clone();
        self.current_compilation = Some(snapshot.compilation_id);
        self.compiled_revision = snapshot.source_revision;
        Ok(())
    }

    /// Apply a compiled delta batch to the compiled state.
    ///
    /// The batch's `from_compilation` must equal the current compiled state's
    /// id (D28): a stale batch is rejected with
    /// [`CompileError::StaleCompilation`] before any op is applied.
    ///
    /// F5: every op's referenced IDs are preflight-validated against the
    /// current compiled state BEFORE any op is applied, so a malformed batch
    /// never partially mutates the compiled state.
    pub fn apply_compiled_delta(&mut self, batch: &BackendDeltaBatch) -> Result<(), CompileError> {
        let actual = self.current_compilation.ok_or_else(|| {
            CompileError::RebuildRequired("reference backend has no compiled base".into())
        })?;
        if actual != batch.from_compilation {
            return Err(CompileError::StaleCompilation {
                expected: batch.from_compilation,
                actual,
            });
        }

        let registry = CompiledEntityRegistry {
            variables: self.compiled_variables.keys().copied().collect(),
            rows: self.compiled_rows.keys().copied().collect(),
            objectives: self.compiled_objectives.keys().copied().collect(),
        };
        batch.validate(&registry)?;

        for op in &batch.operations {
            self.apply_compiled_op(op)?;
        }

        self.current_compilation = Some(batch.to_compilation);
        self.compiled_revision = batch.to_revision;
        Ok(())
    }

    /// Apply a single compiled backend op to the compiled state.
    ///
    /// F5: an op referencing a compiled entity that does not exist is a typed
    /// [`CompileError::InvalidReference`], never a silent skip.
    pub fn apply_compiled_op(&mut self, op: &BackendOp) -> Result<(), CompileError> {
        match op {
            BackendOp::AddVariable(v) => {
                self.compiled_variables.insert(v.id, (v.bounds, v.var_type));
            }
            BackendOp::RemoveVariable(id) => {
                if !self.compiled_variables.contains_key(id) {
                    return Err(invalid_variable(*id));
                }
                self.compiled_variables.remove(id);
                // Mirror the canonical `apply_op(ModelOp::RemoveVariable)`
                // cleanup (CR-01): purge every `(CompiledVariableId, f64)`
                // entry referencing the removed variable from all compiled row
                // and objective coefficient vectors, so the compiled state
                // never contains coefficients for a variable that no longer
                // exists (the compiled commuting square holds for removals).
                for (_, coeffs) in self.compiled_rows.values_mut() {
                    coeffs.retain(|(v, _)| v != id);
                }
                for (_, coeffs, _) in self.compiled_objectives.values_mut() {
                    coeffs.retain(|(v, _)| v != id);
                }
            }
            BackendOp::SetVariableBounds { variable, bounds } => {
                let entry = self
                    .compiled_variables
                    .get_mut(variable)
                    .ok_or_else(|| invalid_variable(*variable))?;
                entry.0 = *bounds;
            }
            BackendOp::AddLinearRow(r) => {
                for (cid, _) in &r.coefficients {
                    if !self.compiled_variables.contains_key(cid) {
                        return Err(invalid_variable(*cid));
                    }
                }
                self.compiled_rows
                    .insert(r.id, (r.bounds, r.coefficients.clone()));
            }
            BackendOp::RemoveLinearRow(id) => {
                if !self.compiled_rows.contains_key(id) {
                    return Err(invalid_row(*id));
                }
                self.compiled_rows.remove(id);
            }
            BackendOp::SetLinearRowBounds { constraint, bounds } => {
                let entry = self
                    .compiled_rows
                    .get_mut(constraint)
                    .ok_or_else(|| invalid_row(*constraint))?;
                entry.0 = *bounds;
            }
            BackendOp::SetLinearCoefficient {
                constraint,
                variable,
                value,
            } => {
                if !self.compiled_variables.contains_key(variable) {
                    return Err(invalid_variable(*variable));
                }
                let entry = self
                    .compiled_rows
                    .get_mut(constraint)
                    .ok_or_else(|| invalid_row(*constraint))?;
                upsert_compiled_coefficient(&mut entry.1, *variable, *value);
            }
            BackendOp::RemoveLinearCoefficient {
                constraint,
                variable,
            } => {
                if !self.compiled_variables.contains_key(variable) {
                    return Err(invalid_variable(*variable));
                }
                let entry = self
                    .compiled_rows
                    .get_mut(constraint)
                    .ok_or_else(|| invalid_row(*constraint))?;
                entry.1.retain(|(v, _)| v != variable);
            }
            BackendOp::AddObjective(o) => {
                for (cid, _) in &o.coefficients {
                    if !self.compiled_variables.contains_key(cid) {
                        return Err(invalid_variable(*cid));
                    }
                }
                self.compiled_objectives
                    .insert(o.id, (o.sense, o.coefficients.clone(), o.constant));
            }
            BackendOp::RemoveObjective(id) => {
                if !self.compiled_objectives.contains_key(id) {
                    return Err(invalid_objective(*id));
                }
                self.compiled_objectives.remove(id);
                // CR-02: defensively clear the compiled objective policy when it
                // references the removed id — matching the canonical
                // `apply_op(ModelOp::RemoveObjective)` path which sets
                // `active_objective = None`. The compiler also emits an explicit
                // `SetObjectivePolicy(None)` for an active-objective removal, so
                // this covers a bare `RemoveObjective` op.
                if self.compiled_objective_policy == CompiledObjectivePolicy::Single(*id) {
                    self.compiled_objective_policy = CompiledObjectivePolicy::None;
                }
            }
            BackendOp::SetObjectiveCoefficient {
                objective,
                variable,
                value,
            } => {
                if !self.compiled_variables.contains_key(variable) {
                    return Err(invalid_variable(*variable));
                }
                let entry = self
                    .compiled_objectives
                    .get_mut(objective)
                    .ok_or_else(|| invalid_objective(*objective))?;
                upsert_compiled_coefficient(&mut entry.1, *variable, *value);
            }
            BackendOp::RemoveObjectiveCoefficient {
                objective,
                variable,
            } => {
                if !self.compiled_variables.contains_key(variable) {
                    return Err(invalid_variable(*variable));
                }
                let entry = self
                    .compiled_objectives
                    .get_mut(objective)
                    .ok_or_else(|| invalid_objective(*objective))?;
                entry.1.retain(|(v, _)| v != variable);
            }
            BackendOp::SetObjectiveConstant { objective, value } => {
                let entry = self
                    .compiled_objectives
                    .get_mut(objective)
                    .ok_or_else(|| invalid_objective(*objective))?;
                entry.2 = *value;
            }
            BackendOp::SetObjectiveSense { objective, sense } => {
                let entry = self
                    .compiled_objectives
                    .get_mut(objective)
                    .ok_or_else(|| invalid_objective(*objective))?;
                entry.0 = *sense;
            }
            BackendOp::SetObjectivePolicy(policy) => {
                validate_policy_references(policy, &self.compiled_objectives)?;
                self.compiled_objective_policy = policy.clone();
            }
        }
        Ok(())
    }

    /// Produce a deterministic normalized view of the compiled state.
    ///
    /// Sorted vectors keyed by compiled ids, used for the compiled-delta vs
    /// compiled-rebuild equality check (the compiled commuting square).
    pub fn compiled_normalized_view(&self) -> CompiledNormalizedView {
        let mut variables: Vec<_> = self
            .compiled_variables
            .iter()
            .map(|(id, (bounds, var_type))| (*id, *bounds, *var_type))
            .collect();
        variables.sort_by_key(|(id, _, _)| *id);

        let mut rows: Vec<_> = self
            .compiled_rows
            .iter()
            .map(|(id, (bounds, coefficients))| (*id, *bounds, coefficients.clone()))
            .collect();
        rows.sort_by_key(|(id, _, _)| *id);

        let mut objectives: Vec<_> = self
            .compiled_objectives
            .iter()
            .map(|(id, (sense, coefficients, constant))| {
                (*id, *sense, coefficients.clone(), *constant)
            })
            .collect();
        objectives.sort_by_key(|(id, ..)| *id);

        CompiledNormalizedView {
            revision: self.compiled_revision,
            compilation_id: self.current_compilation,
            variables,
            rows,
            objectives,
            objective_policy: self.compiled_objective_policy.clone(),
        }
    }
}

// ── OverlaySession (P27 Task 10) ─────────────────────────────────────────────

impl OverlaySession for ReferenceBackend {
    /// Apply a compiled overlay against the exact base compiled state,
    /// transitioning `C_base -> C_overlay`.
    ///
    /// The overlay's `base_compilation` must equal the backend's current
    /// compiled state (D28 exact-id authority); a stale overlay is rejected
    /// BEFORE any mutation. On success the prior bounds of every touched
    /// variable, the added temporary rows, the prior objective policy, and a
    /// deterministic base view are captured for the transactional rollback and
    /// the post-rollback clean verification.
    fn apply_overlay(
        &mut self,
        overlay: &CompiledOverlay,
    ) -> Result<OverlayApplyReceipt, BackendError> {
        // Exact-id stale rejection before mutation (D28 / SM-03.9).
        let actual = self.current_compilation.ok_or_else(|| {
            BackendError::new(
                "reference backend has no compiled base to apply the overlay on top of",
                ErrorCategory::InvalidInput,
                HealthEffect::RequiresRebuild,
            )
        })?;
        if actual != overlay.base_compilation {
            return Err(BackendError::new(
                format!(
                    "stale overlay base: the overlay requires compilation {:?}, but the backend \
                     holds {:?} (D28 exact compilation authority)",
                    overlay.base_compilation, actual
                ),
                ErrorCategory::InvalidInput,
                HealthEffect::RequiresRebuild,
            ));
        }

        // F2: run the COMPLETE overlay preflight against the exact live
        // compiled registry BEFORE any mutation — malformed envelopes, missing
        // origins, row-id collisions, dangling objective policies, and
        // rollback-only ops are all rejected here, before the staged copy-on-
        // write even starts.
        let registry = CompiledEntityRegistry {
            variables: self.compiled_variables.keys().copied().collect(),
            rows: self.compiled_rows.keys().copied().collect(),
            objectives: self.compiled_objectives.keys().copied().collect(),
        };
        if let Err(e) = overlay.validate(&registry) {
            return Err(BackendError::new(
                format!("overlay failed preflight validation: {e}"),
                ErrorCategory::InvalidInput,
                HealthEffect::RequiresRebuild,
            ));
        }

        let mut state = OverlayApplyState {
            overlay_id: overlay.overlay_id,
            base_compilation: overlay.base_compilation,
            applied_compilation: overlay.compilation_id,
            prior_bounds: HashMap::new(),
            added_rows: Vec::new(),
            prior_policy: self.compiled_objective_policy.clone(),
            base_view: self.compiled_normalized_view(),
        };

        // WR-04: the apply is TRANSACTIONAL — stage every mutation on clones of
        // the compiled maps and commit only on full success. A mid-apply
        // failure (e.g. a second op referencing an unknown compiled variable)
        // then leaves the backend exactly at the base: no half-overlaid state,
        // no way for a later rollback to fail ("no applied overlay to roll
        // back"). Compare `compile_delta`'s copy-on-write on a scratch
        // `CurrentCompilation`.
        let mut staged_variables = self.compiled_variables.clone();
        let mut staged_rows = self.compiled_rows.clone();
        let mut staged_policy = self.compiled_objective_policy.clone();

        for op in &overlay.operations {
            match op {
                OverlayOp::SetTemporaryVariableBounds { variable, bounds } => {
                    let entry = staged_variables.get_mut(variable).ok_or_else(|| {
                        BackendError::new(
                            format!("overlay references unknown compiled variable {variable:?}"),
                            ErrorCategory::InvalidInput,
                            HealthEffect::RequiresRebuild,
                        )
                    })?;
                    // Record the FIRST prior value only — later overlay ops on
                    // the same variable must not overwrite the base capture.
                    state.prior_bounds.entry(*variable).or_insert(entry.0);
                    entry.0 = *bounds;
                }
                OverlayOp::AddTemporaryRow { row } => {
                    if staged_rows.contains_key(&row.id) {
                        return Err(BackendError::new(
                            format!(
                                "overlay row id {:?} already exists in the compiled state",
                                row.id
                            ),
                            ErrorCategory::InvalidInput,
                            HealthEffect::RequiresRebuild,
                        ));
                    }
                    for (cid, _) in &row.coefficients {
                        if !staged_variables.contains_key(cid) {
                            return Err(BackendError::new(
                                format!(
                                    "overlay row {:?} references unknown compiled variable {cid:?}",
                                    row.id
                                ),
                                ErrorCategory::InvalidInput,
                                HealthEffect::RequiresRebuild,
                            ));
                        }
                    }
                    staged_rows.insert(row.id, (row.bounds, row.coefficients.clone()));
                    state.added_rows.push(row.id);
                }
                // RemoveTemporaryRow is a rollback-only op; apply does not
                // produce it.
                OverlayOp::RemoveTemporaryRow { row } => {
                    return Err(BackendError::new(
                        format!("apply_overlay does not accept RemoveTemporaryRow ({row:?})"),
                        ErrorCategory::InvalidInput,
                        HealthEffect::RequiresRebuild,
                    ));
                }
                OverlayOp::SetObjectivePolicy(policy) => {
                    // The policy is applied to the overlay state; the base
                    // policy was captured in `prior_policy`.
                    staged_policy = policy.clone();
                }
            }
        }

        // Commit: every op succeeded, so the staged state becomes the held state.
        self.compiled_variables = staged_variables;
        self.compiled_rows = staged_rows;
        self.compiled_objective_policy = staged_policy;
        self.current_compilation = Some(overlay.compilation_id);
        self.overlay_state = Some(state);
        Ok(OverlayApplyReceipt {
            overlay_id: overlay.overlay_id,
            base_compilation: overlay.base_compilation,
            applied_compilation: overlay.compilation_id,
        })
    }

    /// Roll back an applied overlay, transitioning `C_overlay -> C_base`.
    ///
    /// Restores the prior bounds, removes the added temporary rows, restores
    /// the base objective policy, and sets `current_compilation` back to the
    /// base. A `Clean` outcome restores the exact base compiled state; an
    /// unknown/foreign receipt is a [`RequiresRebuild`](OverlayRollbackOutcome::RequiresRebuild)
    /// (D7 invariant).
    fn rollback_overlay(
        &mut self,
        receipt: &OverlayApplyReceipt,
    ) -> Result<OverlayRollbackOutcome, BackendError> {
        let state = self.overlay_state.as_ref().ok_or_else(|| {
            BackendError::new(
                "no applied overlay to roll back; the receipt does not match this backend",
                ErrorCategory::Internal,
                HealthEffect::RequiresRebuild,
            )
        })?;
        // F3: validate the FULL receipt tuple (overlay_id, base_compilation,
        // applied_compilation) against the recorded overlay state. A forged
        // receipt — ANY field mismatch — is rollback UNCERTAINTY, never a
        // `Clean` rollback.
        if state.overlay_id != receipt.overlay_id
            || state.base_compilation != receipt.base_compilation
            || state.applied_compilation != receipt.applied_compilation
        {
            return Ok(OverlayRollbackOutcome::RequiresRebuild {
                reason: format!(
                    "receipt (overlay {:?}, base {:?}, applied {:?}) does not match the \
                     backend's applied overlay (overlay {:?}, base {:?}, applied {:?})",
                    receipt.overlay_id,
                    receipt.base_compilation,
                    receipt.applied_compilation,
                    state.overlay_id,
                    state.base_compilation,
                    state.applied_compilation,
                ),
            });
        }

        // Restore prior bounds.
        for (variable, bounds) in &state.prior_bounds {
            if let Some(entry) = self.compiled_variables.get_mut(variable) {
                entry.0 = *bounds;
            }
        }
        // Remove added temporary rows.
        for row in &state.added_rows {
            self.compiled_rows.remove(row);
        }
        // Restore the base objective policy.
        self.compiled_objective_policy = state.prior_policy.clone();

        let restored = state.base_compilation;
        self.current_compilation = Some(restored);
        Ok(OverlayRollbackOutcome::Clean {
            restored_compilation: restored,
        })
    }

    /// Verify the backend's compiled state is restored to the exact base view
    /// captured at apply (the post-rollback clean check).
    fn verify_overlay_clean(&mut self) -> Result<(), BackendError> {
        let state = self.overlay_state.as_ref().ok_or_else(|| {
            BackendError::new(
                "verify_overlay_clean called with no recorded overlay apply",
                ErrorCategory::Internal,
                HealthEffect::RequiresRebuild,
            )
        })?;
        let current = self.compiled_normalized_view();
        if current != state.base_view {
            return Err(BackendError::new(
                "post-rollback verification failed: the compiled state does not equal the base",
                ErrorCategory::Internal,
                HealthEffect::RequiresRebuild,
            ));
        }
        Ok(())
    }
}

/// F5: a typed `InvalidReference` error for an unknown compiled variable.
fn invalid_variable(id: CompiledVariableId) -> CompileError {
    CompileError::InvalidReference {
        entity: CompiledEntityRef::Variable(id),
    }
}

/// F5: a typed `InvalidReference` error for an unknown compiled row.
fn invalid_row(id: CompiledConstraintId) -> CompileError {
    CompileError::InvalidReference {
        entity: CompiledEntityRef::Constraint(id),
    }
}

/// F5: a typed `InvalidReference` error for an unknown compiled objective.
fn invalid_objective(id: CompiledObjectiveId) -> CompileError {
    CompileError::InvalidReference {
        entity: CompiledEntityRef::Objective(id),
    }
}

/// F5: reject an objective policy that references a compiled objective absent
/// from the backend's compiled state — a dangling policy is a typed error.
#[allow(clippy::type_complexity)]
fn validate_policy_references(
    policy: &CompiledObjectivePolicy,
    objectives: &HashMap<CompiledObjectiveId, (Sense, Vec<(CompiledVariableId, f64)>, f64)>,
) -> Result<(), CompileError> {
    match policy {
        CompiledObjectivePolicy::None => {}
        CompiledObjectivePolicy::Single(id) => {
            if !objectives.contains_key(id) {
                return Err(invalid_objective(*id));
            }
        }
        CompiledObjectivePolicy::Weighted(items) => {
            for item in items {
                if !objectives.contains_key(&item.objective) {
                    return Err(invalid_objective(item.objective));
                }
            }
        }
        CompiledObjectivePolicy::Lexicographic(items) => {
            for item in items {
                if !objectives.contains_key(&item.objective) {
                    return Err(invalid_objective(item.objective));
                }
            }
        }
    }
    Ok(())
}

/// Upsert one `(CompiledVariableId, f64)` coefficient into a deterministic
/// var-ordered coefficient list.
fn upsert_compiled_coefficient(
    coefficients: &mut Vec<(CompiledVariableId, f64)>,
    variable: CompiledVariableId,
    value: f64,
) {
    if let Some(entry) = coefficients.iter_mut().find(|(v, _)| *v == variable) {
        entry.1 = value;
    } else {
        coefficients.push((variable, value));
        coefficients.sort_by_key(|(v, _)| *v);
    }
}

/// A deterministic normalized view of the compiled state for comparison
/// (P26 Task 7).
#[derive(Clone, Debug, PartialEq)]
pub struct CompiledNormalizedView {
    /// The canonical revision of the compiled state.
    pub revision: ModelRevision,
    /// The exact compiled state id (None when never set).
    pub compilation_id: Option<CompilationId>,
    /// Compiled variables as `(id, bounds, var_type)`.
    pub variables: Vec<(CompiledVariableId, Bounds, VarType)>,
    /// Compiled rows as `(id, bounds, coefficients)`.
    #[allow(clippy::type_complexity)]
    pub rows: Vec<(
        CompiledConstraintId,
        ConstraintBounds,
        Vec<(CompiledVariableId, f64)>,
    )>,
    /// Compiled objectives as `(id, sense, coefficients, constant)`.
    #[allow(clippy::type_complexity)]
    pub objectives: Vec<(
        CompiledObjectiveId,
        Sense,
        Vec<(CompiledVariableId, f64)>,
        f64,
    )>,
    /// Active compiled objective policy.
    pub objective_policy: CompiledObjectivePolicy,
}

/// A normalized, deterministic view of backend state for comparison.
#[derive(Clone, Debug, PartialEq)]
pub struct NormalizedView {
    /// The revision represented by this view.
    pub revision: ModelRevision,
    /// The active objective, if any.
    pub active_objective: Option<ObjId>,
    /// Variables as `(id, bounds, type, active, semicontinuous_lower)`.
    pub variables: Vec<(VarId, Bounds, VarType, bool, Option<f64>)>,
    /// Constraints as `(id, bounds, active)`.
    pub constraints: Vec<(ConId, ConstraintBounds, bool)>,
    /// Objectives as `(id, sense, active)`.
    pub objectives: Vec<(ObjId, Sense, bool)>,
    /// Parameters as `(id, value)`.
    pub parameters: Vec<(ParamId, f64)>,
    /// Constraint cells as `(cell, evaluated_value)`.
    pub cells: Vec<(CellKey, f64)>,
    /// Objective cells as `(cell, evaluated_value, constant)`.
    pub objective_cells: Vec<(CellKey, f64, f64)>,
    /// Per-objective constant offsets as `(obj, constant)` (API-03.5).
    pub objective_constants: Vec<(ObjId, f64)>,
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
        vars_r1.insert(
            var,
            (Bounds::NON_NEGATIVE, VarType::Continuous, true, None, None),
        );
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
        vars.insert(
            var,
            (Bounds::new(0.0, 1.0), VarType::Binary, true, None, None),
        );
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
