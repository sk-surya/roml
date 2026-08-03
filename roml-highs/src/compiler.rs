//! HiGHS native projection from backend IR (P26 Task 7, design §8).
//!
//! This module is the HiGHS-side translation of backend IR
//! ([`BackendSnapshot`]/[`BackendDeltaBatch`]) into HiGHS C API calls. After
//! the P26 migration the HiGHS session receives NO canonical
//! [`ModelSnapshot`]/[`DeltaBatch`] — the identity compiler lowers canonical
//! state to backend IR first, and this module (plus the session's compiled
//! `synchronize`) is the only path that mutates the native HiGHS model
//! (SM-03.2, must-have truth 6).
//!
//! # Architecture
//!
//! - [`rebuild_from_backend_snapshot`]: Full deterministic rebuild from a
//!   [`BackendSnapshot`]. Clears the HiGHS model, then adds all compiled
//!   variables, rows (with their coefficients), and objectives in order, and
//!   projects the active compiled objective policy.
//!
//! - [`apply_backend_delta`]: Apply a batch of [`BackendOp`] variants to an
//!   existing HiGHS model. All pinned variants are handled.
//!
//! # Safety
//!
//! `raw` must be a valid HiGHS instance handle from `Highs_create`. Every
//! native return code is checked through `check_highs_status` /
//! `from_native_status` (unchanged from the M2 binding policy).

#![allow(clippy::too_many_arguments)]

use std::collections::HashMap;
use std::ffi::c_void;

use crate::bindings::*;
use crate::error::{check_highs_status, from_native_status};
use crate::index_map::IndexMap;
use roml::advanced::{
    BackendDeltaBatch, BackendOp, BackendSnapshot, CompiledConstraintId, CompiledObjectiveId,
    CompiledObjectivePolicy, CompiledVariableId,
};
use roml::id::{ConId, ObjId, VarId};
use roml::model::objective::Sense;
use roml::model::variable::VarType;
use roml::solver::backend::{BackendError, ErrorCategory, HealthEffect};

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Normalise a ROML bound value to a HiGHS-compatible value.
///
/// ROML uses `f64::INFINITY` / `f64::NEG_INFINITY` to denote unbounded
/// directions. HiGHS uses a finite infinity value (1e30 by default, cached
/// from `Highs_getInfinity`).
pub(crate) fn normalize_bound(b: f64, inf: f64) -> f64 {
    if b == f64::NEG_INFINITY {
        -inf
    } else if b == f64::INFINITY {
        inf
    } else {
        b
    }
}

/// Convert a ROML [`Sense`] to a HiGHS objective sense constant.
fn sense_to_highs(sense: Sense) -> HighsInt {
    match sense {
        Sense::Minimize => OBJECTIVE_SENSE_MINIMIZE,
        Sense::Maximize => OBJECTIVE_SENSE_MAXIMIZE,
    }
}

/// Clear all column costs to zero (used before projecting an active
/// objective, so stale costs from a previous objective never blend in).
unsafe fn clear_all_costs(raw: *mut c_void) -> Result<(), BackendError> {
    let num_cols = Highs_getNumCol(raw);
    if num_cols > 0 {
        let zeros = vec![0.0_f64; num_cols as usize];
        check_highs_status(
            Highs_changeColsCostByRange(raw, 0, num_cols - 1, zeros.as_ptr()),
            raw,
            "Highs_changeColsCostByRange (clear all costs)",
        )?;
    }
    Ok(())
}

// ── Snapshot rebuild ─────────────────────────────────────────────────────────

/// Rebuild the HiGHS model from a compiled [`BackendSnapshot`].
///
/// This is the compiled-path equivalent of the M2 `rebuild_from_snapshot`.
/// Compiled rows already carry their coefficients, so rows are added with
/// their coefficients directly; the active objective policy selects which
/// compiled objective is projected into the HiGHS single-objective surface.
///
/// # Safety
///
/// `raw` must be a valid HiGHS instance handle from `Highs_create`.
pub(crate) fn rebuild_from_backend_snapshot(
    raw: *mut c_void,
    snapshot: &BackendSnapshot,
    col_map: &mut IndexMap<CompiledVariableId>,
    row_map: &mut IndexMap<CompiledConstraintId>,
    compiled_to_user_variable: &mut HashMap<CompiledVariableId, VarId>,
    compiled_to_user_constraint: &mut HashMap<CompiledConstraintId, ConId>,
    compiled_to_user_objective: &mut HashMap<CompiledObjectiveId, ObjId>,
    inf: f64,
    var_bounds: &mut HashMap<CompiledVariableId, (f64, f64)>,
    con_bounds: &mut HashMap<CompiledConstraintId, (f64, f64)>,
    obj_costs: &mut HashMap<CompiledObjectiveId, HashMap<CompiledVariableId, f64>>,
    obj_senses: &mut HashMap<CompiledObjectiveId, Sense>,
    obj_offsets: &mut HashMap<CompiledObjectiveId, f64>,
    active_obj: &mut Option<ObjId>,
) -> Result<(), BackendError> {
    // Step 1: Clear the HiGHS model (preserves options/settings).
    // SAFETY: `raw` is guaranteed valid by the caller.
    check_highs_status(unsafe { Highs_clear(raw) }, raw, "Highs_clear")?;

    // Step 2: Clear all caches.
    *col_map = IndexMap::new();
    *row_map = IndexMap::new();
    compiled_to_user_variable.clear();
    compiled_to_user_constraint.clear();
    compiled_to_user_objective.clear();
    var_bounds.clear();
    con_bounds.clear();
    obj_costs.clear();
    obj_senses.clear();
    obj_offsets.clear();
    *active_obj = None;

    // Step 3: Add compiled variables.
    unsafe {
        for v in &snapshot.variables {
            let lb = normalize_bound(v.bounds.lower, inf);
            let ub = normalize_bound(v.bounds.upper, inf);
            let col = Highs_getNumCol(raw);
            let status = Highs_addVar(raw, lb, ub);
            if status != STATUS_OK {
                return Err(from_native_status(status, "Highs_addVar"));
            }
            col_map.insert(v.id, col);

            match v.var_type {
                VarType::Continuous => {}
                VarType::Integer | VarType::Binary => {
                    check_highs_status(
                        Highs_changeColIntegrality(raw, col, VAR_TYPE_INTEGER),
                        raw,
                        "Highs_changeColIntegrality",
                    )?;
                }
            }
            var_bounds.insert(v.id, (lb, ub));
        }
    }

    // Step 4: Add compiled rows (with their coefficients).
    unsafe {
        for r in &snapshot.linear_rows {
            let lb = normalize_bound(r.bounds.lower, inf);
            let ub = normalize_bound(r.bounds.upper, inf);
            let row = Highs_getNumRow(raw);

            // Resolve compiled variable ids to native column indices.
            let mut indices: Vec<HighsInt> = Vec::with_capacity(r.coefficients.len());
            let mut values: Vec<f64> = Vec::with_capacity(r.coefficients.len());
            for (cid, value) in &r.coefficients {
                if let Some(col) = col_map.get(*cid) {
                    indices.push(col);
                    values.push(*value);
                }
            }
            let status = Highs_addRow(
                raw,
                lb,
                ub,
                indices.len() as HighsInt,
                indices.as_ptr(),
                values.as_ptr(),
            );
            if status != STATUS_OK {
                return Err(from_native_status(status, "Highs_addRow"));
            }
            row_map.insert(r.id, row);
            con_bounds.insert(r.id, (lb, ub));
        }
    }

    // Step 5: Populate per-objective caches and the compiled->user objective
    // map from the snapshot's mandatory origin map (SM-02.5).
    for o in &snapshot.objectives {
        obj_senses.insert(o.id, o.sense);
        obj_offsets.insert(o.id, o.constant);
        let costs: HashMap<CompiledVariableId, f64> = o.coefficients.iter().copied().collect();
        obj_costs.insert(o.id, costs);
        // WR-5: a missing origin is a typed `BackendError`, never a panic —
        // `BackendSnapshot` has all-`pub` fields, so a malformed snapshot can
        // reach the FFI projection directly (bypassing builder finalization).
        match snapshot.origin_map.objective_origin(o.id) {
            Some(roml::advanced::EntityOrigin::UserObjective(obj)) => {
                compiled_to_user_objective.insert(o.id, *obj);
            }
            Some(_) => {} // non-user origin (construct/overlay): no user objective
            None => {
                return Err(BackendError::new(
                    format!(
                        "every compiled objective must have a recorded origin; missing for {o:?}"
                    ),
                    ErrorCategory::InvalidInput,
                    HealthEffect::RequiresRebuild,
                ));
            }
        }
    }

    // Step 6: Rebuild compiled->user variable/constraint maps from the origin
    // map (every compiled entity has a recorded origin — D5).
    for v in &snapshot.variables {
        // WR-5: a missing origin is a typed `BackendError`, never a panic.
        match snapshot.origin_map.variable_origin(v.id) {
            Some(roml::advanced::EntityOrigin::UserVariable(var)) => {
                compiled_to_user_variable.insert(v.id, *var);
            }
            Some(_) => {} // non-user origin (construct/overlay): no user variable
            None => {
                return Err(BackendError::new(
                    format!(
                        "every compiled variable must have a recorded origin; missing for {v:?}"
                    ),
                    ErrorCategory::InvalidInput,
                    HealthEffect::RequiresRebuild,
                ));
            }
        }
    }
    for r in &snapshot.linear_rows {
        // WR-5: a missing origin is a typed `BackendError`, never a panic.
        match snapshot.origin_map.constraint_origin(r.id) {
            Some(roml::advanced::EntityOrigin::UserConstraint(con)) => {
                compiled_to_user_constraint.insert(r.id, *con);
            }
            Some(_) => {} // non-user origin (construct/overlay): no user constraint
            None => {
                return Err(BackendError::new(
                    format!("every compiled row must have a recorded origin; missing for {r:?}"),
                    ErrorCategory::InvalidInput,
                    HealthEffect::RequiresRebuild,
                ));
            }
        }
    }

    // Step 7: Project the active objective policy into HiGHS.
    // SAFETY: raw is valid; all costs are cleared first.
    unsafe {
        clear_all_costs(raw)?;
    }
    match &snapshot.objective_policy {
        CompiledObjectivePolicy::Single(cid) => {
            if let Some(obj) = compiled_to_user_objective.get(cid).copied() {
                *active_obj = Some(obj);
                if let Some(costs) = obj_costs.get(cid) {
                    for (vid, &cost) in costs {
                        if let Some(col) = col_map.get(*vid) {
                            unsafe {
                                check_highs_status(
                                    Highs_changeColCost(raw, col, cost),
                                    raw,
                                    "Highs_changeColCost",
                                )?;
                            }
                        }
                    }
                }
                if let Some(&sense) = obj_senses.get(cid) {
                    unsafe {
                        check_highs_status(
                            Highs_changeObjectiveSense(raw, sense_to_highs(sense)),
                            raw,
                            "Highs_changeObjectiveSense",
                        )?;
                    }
                }
                if let Some(&offset) = obj_offsets.get(cid) {
                    unsafe {
                        check_highs_status(
                            Highs_changeObjectiveOffset(raw, offset),
                            raw,
                            "Highs_changeObjectiveOffset",
                        )?;
                    }
                }
            }
        }
        // A32 / B1: no active objective — costs already cleared, offset zero.
        CompiledObjectivePolicy::None => {
            *active_obj = None;
        }
        // Weighted/Lexicographic are reachable only from the P31 canonical
        // ObjectivePolicy (design §15); P26 does not emit them.
        CompiledObjectivePolicy::Weighted(_) | CompiledObjectivePolicy::Lexicographic(_) => {
            *active_obj = None;
        }
    }

    Ok(())
}

// ── Delta application ────────────────────────────────────────────────────────

/// Apply a [`BackendDeltaBatch`] of [`BackendOp`] operations to the HiGHS
/// model.
///
/// Compiled ops carry dense compiled ids; `col_map`/`row_map` translate them
/// to native indices. Partial application is impossible to observe — the
/// function acknowledges only after every operation succeeds; a failure
/// returns the error and the caller recovers via one compiled rebuild.
///
/// # Safety
///
/// `raw` must be a valid HiGHS instance handle from `Highs_create`.
pub(crate) fn apply_backend_delta(
    raw: *mut c_void,
    batch: &BackendDeltaBatch,
    col_map: &mut IndexMap<CompiledVariableId>,
    row_map: &mut IndexMap<CompiledConstraintId>,
    compiled_to_user_variable: &mut HashMap<CompiledVariableId, VarId>,
    compiled_to_user_constraint: &mut HashMap<CompiledConstraintId, ConId>,
    compiled_to_user_objective: &mut HashMap<CompiledObjectiveId, ObjId>,
    inf: f64,
    var_bounds: &mut HashMap<CompiledVariableId, (f64, f64)>,
    con_bounds: &mut HashMap<CompiledConstraintId, (f64, f64)>,
    obj_costs: &mut HashMap<CompiledObjectiveId, HashMap<CompiledVariableId, f64>>,
    obj_senses: &mut HashMap<CompiledObjectiveId, Sense>,
    obj_offsets: &mut HashMap<CompiledObjectiveId, f64>,
    active_obj: &mut Option<ObjId>,
) -> Result<(), BackendError> {
    for op in &batch.operations {
        match op {
            BackendOp::AddVariable(v) => {
                let lb = normalize_bound(v.bounds.lower, inf);
                let ub = normalize_bound(v.bounds.upper, inf);
                unsafe {
                    let col = Highs_getNumCol(raw);
                    let status = Highs_addVar(raw, lb, ub);
                    if status != STATUS_OK {
                        return Err(from_native_status(status, "Highs_addVar"));
                    }
                    col_map.insert(v.id, col);
                    match v.var_type {
                        VarType::Continuous => {}
                        VarType::Integer | VarType::Binary => {
                            check_highs_status(
                                Highs_changeColIntegrality(raw, col, VAR_TYPE_INTEGER),
                                raw,
                                "Highs_changeColIntegrality",
                            )?;
                        }
                    }
                    var_bounds.insert(v.id, (lb, ub));
                }
                // The user mapping for delta-added entities comes from the
                // batch's origin additions (SM-02.5).
                if let Some(roml::advanced::EntityOrigin::UserVariable(var)) =
                    batch.origin_additions.variable_origin(v.id)
                {
                    compiled_to_user_variable.insert(v.id, *var);
                }
            }

            BackendOp::RemoveVariable(id) => {
                if let Some(idx) = col_map.remove(*id) {
                    unsafe {
                        check_highs_status(
                            Highs_deleteColsBySet(raw, 1, &[idx] as *const HighsInt),
                            raw,
                            "Highs_deleteColsBySet",
                        )?;
                    }
                    col_map.reindex_after_delete(idx);
                    var_bounds.remove(id);
                    compiled_to_user_variable.remove(id);
                    for costs in obj_costs.values_mut() {
                        costs.remove(id);
                    }
                }
            }

            BackendOp::SetVariableBounds { variable, bounds } => {
                if let Some(idx) = col_map.get(*variable) {
                    let lb = normalize_bound(bounds.lower, inf);
                    let ub = normalize_bound(bounds.upper, inf);
                    unsafe {
                        check_highs_status(
                            Highs_changeColBounds(raw, idx, lb, ub),
                            raw,
                            "Highs_changeColBounds",
                        )?;
                    }
                    var_bounds.insert(*variable, (lb, ub));
                }
            }

            BackendOp::AddLinearRow(r) => {
                let lb = normalize_bound(r.bounds.lower, inf);
                let ub = normalize_bound(r.bounds.upper, inf);
                unsafe {
                    let row = Highs_getNumRow(raw);
                    let mut indices: Vec<HighsInt> = Vec::with_capacity(r.coefficients.len());
                    let mut values: Vec<f64> = Vec::with_capacity(r.coefficients.len());
                    for (cid, value) in &r.coefficients {
                        if let Some(col) = col_map.get(*cid) {
                            indices.push(col);
                            values.push(*value);
                        }
                    }
                    let status = Highs_addRow(
                        raw,
                        lb,
                        ub,
                        indices.len() as HighsInt,
                        indices.as_ptr(),
                        values.as_ptr(),
                    );
                    if status != STATUS_OK {
                        return Err(from_native_status(status, "Highs_addRow"));
                    }
                    row_map.insert(r.id, row);
                    con_bounds.insert(r.id, (lb, ub));
                }
                if let Some(roml::advanced::EntityOrigin::UserConstraint(con)) =
                    batch.origin_additions.constraint_origin(r.id)
                {
                    compiled_to_user_constraint.insert(r.id, *con);
                }
            }

            BackendOp::RemoveLinearRow(id) => {
                if let Some(idx) = row_map.remove(*id) {
                    unsafe {
                        check_highs_status(
                            Highs_deleteRowsBySet(raw, 1, &[idx] as *const HighsInt),
                            raw,
                            "Highs_deleteRowsBySet",
                        )?;
                    }
                    row_map.reindex_after_delete(idx);
                    con_bounds.remove(id);
                    compiled_to_user_constraint.remove(id);
                }
            }

            BackendOp::SetLinearRowBounds { constraint, bounds } => {
                if let Some(idx) = row_map.get(*constraint) {
                    let lb = normalize_bound(bounds.lower, inf);
                    let ub = normalize_bound(bounds.upper, inf);
                    unsafe {
                        check_highs_status(
                            Highs_changeRowBounds(raw, idx, lb, ub),
                            raw,
                            "Highs_changeRowBounds",
                        )?;
                    }
                    con_bounds.insert(*constraint, (lb, ub));
                }
            }

            BackendOp::SetLinearCoefficient {
                constraint,
                variable,
                value,
            } => {
                if let (Some(row), Some(col)) = (row_map.get(*constraint), col_map.get(*variable)) {
                    unsafe {
                        check_highs_status(
                            Highs_changeCoeff(raw, row, col, *value),
                            raw,
                            "Highs_changeCoeff",
                        )?;
                    }
                }
            }

            BackendOp::RemoveLinearCoefficient {
                constraint,
                variable,
            } => {
                if let (Some(row), Some(col)) = (row_map.get(*constraint), col_map.get(*variable)) {
                    // HiGHS has no direct "remove coefficient": setting to
                    // zero achieves the same effect.
                    unsafe {
                        check_highs_status(
                            Highs_changeCoeff(raw, row, col, 0.0),
                            raw,
                            "Highs_changeCoeff (remove cell)",
                        )?;
                    }
                }
            }

            BackendOp::AddObjective(o) => {
                obj_senses.insert(o.id, o.sense);
                obj_offsets.insert(o.id, o.constant);
                let costs: HashMap<CompiledVariableId, f64> =
                    o.coefficients.iter().copied().collect();
                obj_costs.insert(o.id, costs);
                if let Some(roml::advanced::EntityOrigin::UserObjective(obj)) =
                    batch.origin_additions.objective_origin(o.id)
                {
                    compiled_to_user_objective.insert(o.id, *obj);
                }
            }

            BackendOp::RemoveObjective(id) => {
                let was_active = compiled_to_user_objective.get(id).copied() == *active_obj;
                obj_senses.remove(id);
                obj_costs.remove(id);
                obj_offsets.remove(id);
                compiled_to_user_objective.remove(id);
                if was_active {
                    unsafe {
                        clear_all_costs(raw)?;
                        check_highs_status(
                            Highs_changeObjectiveOffset(raw, 0.0),
                            raw,
                            "Highs_changeObjectiveOffset",
                        )?;
                    }
                    *active_obj = None;
                }
            }

            BackendOp::SetObjectiveCoefficient {
                objective,
                variable,
                value,
            } => {
                obj_costs
                    .entry(*objective)
                    .or_default()
                    .insert(*variable, *value);
                if *active_obj == compiled_to_user_objective.get(objective).copied() {
                    if let Some(col) = col_map.get(*variable) {
                        unsafe {
                            check_highs_status(
                                Highs_changeColCost(raw, col, *value),
                                raw,
                                "Highs_changeColCost",
                            )?;
                        }
                    }
                }
            }

            BackendOp::RemoveObjectiveCoefficient {
                objective,
                variable,
            } => {
                if *active_obj == compiled_to_user_objective.get(objective).copied() {
                    if let Some(col) = col_map.get(*variable) {
                        unsafe {
                            check_highs_status(
                                Highs_changeColCost(raw, col, 0.0),
                                raw,
                                "Highs_changeColCost (remove obj cell)",
                            )?;
                        }
                    }
                }
                if let Some(costs) = obj_costs.get_mut(objective) {
                    costs.remove(variable);
                }
            }

            BackendOp::SetObjectiveConstant { objective, value } => {
                obj_offsets.insert(*objective, *value);
                if *active_obj == compiled_to_user_objective.get(objective).copied() {
                    unsafe {
                        check_highs_status(
                            Highs_changeObjectiveOffset(raw, *value),
                            raw,
                            "Highs_changeObjectiveOffset",
                        )?;
                    }
                }
            }

            BackendOp::SetObjectiveSense { objective, sense } => {
                obj_senses.insert(*objective, *sense);
                if *active_obj == compiled_to_user_objective.get(objective).copied() {
                    unsafe {
                        check_highs_status(
                            Highs_changeObjectiveSense(raw, sense_to_highs(*sense)),
                            raw,
                            "Highs_changeObjectiveSense",
                        )?;
                    }
                }
            }

            BackendOp::SetObjectivePolicy(policy) => {
                unsafe {
                    clear_all_costs(raw)?;
                }
                match policy {
                    CompiledObjectivePolicy::Single(cid) => {
                        if let Some(obj) = compiled_to_user_objective.get(cid).copied() {
                            if let Some(costs) = obj_costs.get(cid) {
                                for (vid, &cost) in costs {
                                    if let Some(col) = col_map.get(*vid) {
                                        unsafe {
                                            check_highs_status(
                                                Highs_changeColCost(raw, col, cost),
                                                raw,
                                                "Highs_changeColCost",
                                            )?;
                                        }
                                    }
                                }
                            }
                            if let Some(&sense) = obj_senses.get(cid) {
                                unsafe {
                                    check_highs_status(
                                        Highs_changeObjectiveSense(raw, sense_to_highs(sense)),
                                        raw,
                                        "Highs_changeObjectiveSense",
                                    )?;
                                }
                            }
                            if let Some(&offset) = obj_offsets.get(cid) {
                                unsafe {
                                    check_highs_status(
                                        Highs_changeObjectiveOffset(raw, offset),
                                        raw,
                                        "Highs_changeObjectiveOffset",
                                    )?;
                                }
                            }
                            *active_obj = Some(obj);
                        }
                    }
                    CompiledObjectivePolicy::None => {
                        unsafe {
                            check_highs_status(
                                Highs_changeObjectiveOffset(raw, 0.0),
                                raw,
                                "Highs_changeObjectiveOffset",
                            )?;
                        }
                        *active_obj = None;
                    }
                    CompiledObjectivePolicy::Weighted(_)
                    | CompiledObjectivePolicy::Lexicographic(_) => {
                        *active_obj = None;
                    }
                }
            }

            // `BackendOp` is #[non_exhaustive]: future ops are safely ignored
            // by the HiGHS projection until the bridge tasks define them.
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_bound_maps_infinity() {
        let inf = 1e30;
        assert_eq!(normalize_bound(f64::INFINITY, inf), inf);
        assert_eq!(normalize_bound(f64::NEG_INFINITY, inf), -inf);
        assert_eq!(normalize_bound(0.0, inf), 0.0);
        assert_eq!(normalize_bound(5.5, inf), 5.5);
        assert_eq!(normalize_bound(-3.0, inf), -3.0);
    }

    /// WR-5: a `BackendSnapshot` with all-`pub` fields can be constructed
    /// directly, bypassing `BackendSnapshotBuilder::finalize`'s origin
    /// completeness check. The FFI projection must return a typed
    /// `BackendError` for an origin-less entry — never panic via `.expect()`.
    #[test]
    fn rebuild_from_snapshot_rejects_origin_less_entry_with_error_not_panic() {
        use crate::index_map::IndexMap;
        use crate::lifecycle::HighsSession;
        use roml::advanced::{BackendSnapshotBuilder, CompiledVariable, EntityOrigin, OriginMap};
        use roml::id::Generation;
        use roml::model::Bounds;
        use roml::revision::ModelRevision;
        use roml::solver::backend::ErrorCategory;
        use roml::Model;

        let session = HighsSession::try_new().expect("HiGHS should be available");
        let instance = Model::new().instance();
        let compiled_var = CompiledVariable {
            id: CompiledVariableId(0),
            bounds: Bounds::new(0.0, 1.0),
            var_type: VarType::Continuous,
            name: None,
        };
        let mut origin_map = OriginMap::new();
        origin_map.insert_variable(
            compiled_var.id,
            EntityOrigin::UserVariable(VarId::new(0, Generation::new())),
        );
        let builder = BackendSnapshotBuilder::new(instance, ModelRevision::ZERO)
            .origin_map(origin_map)
            .objective_policy(CompiledObjectivePolicy::None)
            .add_variable(compiled_var);
        let mut snapshot = builder.finalize().expect("valid snapshot must build");

        // Strip the origin map to simulate a malformed snapshot reaching the
        // FFI projection directly (bypassing the builder's completeness check).
        snapshot.origin_map = OriginMap::new();

        let mut col_map = IndexMap::new();
        let mut row_map = IndexMap::new();
        let mut c2u_var = HashMap::new();
        let mut c2u_con = HashMap::new();
        let mut c2u_obj = HashMap::new();
        let mut var_bounds = HashMap::new();
        let mut con_bounds = HashMap::new();
        let mut obj_costs = HashMap::new();
        let mut obj_senses = HashMap::new();
        let mut obj_offsets = HashMap::new();
        let mut active_obj = None;

        let result = rebuild_from_backend_snapshot(
            session.raw,
            &snapshot,
            &mut col_map,
            &mut row_map,
            &mut c2u_var,
            &mut c2u_con,
            &mut c2u_obj,
            session.inf,
            &mut var_bounds,
            &mut con_bounds,
            &mut obj_costs,
            &mut obj_senses,
            &mut obj_offsets,
            &mut active_obj,
        );
        let err =
            result.expect_err("origin-less snapshot entry must produce a typed error, not a panic");
        assert_eq!(
            err.category,
            ErrorCategory::InvalidInput,
            "an origin-less entry is invalid input"
        );
        assert!(
            err.message.contains("origin"),
            "the error must name the missing origin, got: {}",
            err.message
        );
    }
}
