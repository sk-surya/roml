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

/// F5: a typed error for an op referencing a compiled variable not present in
/// the held native state (no silent skip). Shared with the overlay path.
pub(crate) fn missing_variable(id: CompiledVariableId) -> BackendError {
    BackendError::new(
        format!("compiled variable {id:?} is not present in the held native state"),
        ErrorCategory::InvalidInput,
        HealthEffect::RequiresRebuild,
    )
}

/// F5: a typed error for an op referencing a compiled row not present in the
/// held native state (no silent skip).
pub(crate) fn missing_row(id: CompiledConstraintId) -> BackendError {
    BackendError::new(
        format!("compiled row {id:?} is not present in the held native state"),
        ErrorCategory::InvalidInput,
        HealthEffect::RequiresRebuild,
    )
}

/// F5: a typed error for an op referencing a compiled objective not present in
/// the held native state (no silent skip).
pub(crate) fn missing_objective(id: CompiledObjectiveId) -> BackendError {
    BackendError::new(
        format!("compiled objective {id:?} is not present in the held native state"),
        ErrorCategory::InvalidInput,
        HealthEffect::RequiresRebuild,
    )
}

/// Clear all column costs to zero (used before projecting an active
/// objective, so stale costs from a previous objective never blend in).
pub(crate) unsafe fn clear_all_costs(raw: *mut c_void) -> Result<(), BackendError> {
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

/// Project a compiled objective policy onto the single-objective HiGHS
/// surface (P27 Task 10).
///
/// Shared by the `BackendOp::SetObjectivePolicy` delta arm and the overlay
/// apply/rollback path. Clears all column costs, then projects the policy:
/// `Single` applies the objective's costs/sense/offset; `None` clears the
/// offset and deactivates the objective. Weighted/Lexicographic (P31
/// constructs) are rejected with a typed `Unsupported` error — never silently
/// treated as no-active-objective (F4).
pub(crate) fn project_objective_policy(
    raw: *mut c_void,
    policy: &CompiledObjectivePolicy,
    col_map: &IndexMap<CompiledVariableId>,
    compiled_to_user_objective: &HashMap<CompiledObjectiveId, ObjId>,
    obj_costs: &HashMap<CompiledObjectiveId, HashMap<CompiledVariableId, f64>>,
    obj_senses: &HashMap<CompiledObjectiveId, Sense>,
    obj_offsets: &HashMap<CompiledObjectiveId, f64>,
    active_obj: &mut Option<ObjId>,
) -> Result<(), BackendError> {
    // SAFETY: raw is a valid HiGHS instance handle.
    unsafe {
        clear_all_costs(raw)?;
    }
    match policy {
        CompiledObjectivePolicy::Single(cid) => {
            if let Some(obj) = compiled_to_user_objective.get(cid).copied() {
                if let Some(costs) = obj_costs.get(cid) {
                    for (vid, &cost) in costs {
                        if let Some(col) = col_map.get(*vid) {
                            // SAFETY: raw is valid; col is a live native index.
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
                    // SAFETY: raw is valid.
                    unsafe {
                        check_highs_status(
                            Highs_changeObjectiveSense(raw, sense_to_highs(sense)),
                            raw,
                            "Highs_changeObjectiveSense",
                        )?;
                    }
                }
                if let Some(&offset) = obj_offsets.get(cid) {
                    // SAFETY: raw is valid.
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
            // SAFETY: raw is valid.
            unsafe {
                check_highs_status(
                    Highs_changeObjectiveOffset(raw, 0.0),
                    raw,
                    "Highs_changeObjectiveOffset",
                )?;
            }
            *active_obj = None;
        }
        // F4: Weighted/Lexicographic cannot be represented on the
        // single-objective HiGHS surface in P26 — reject with a typed error,
        // never silently treat as no-active-objective.
        CompiledObjectivePolicy::Weighted(_) => {
            return Err(BackendError::unsupported(
                "weighted objective policy is not supported by the P26 HiGHS \
                 projection (P31 compiles it)",
            ));
        }
        CompiledObjectivePolicy::Lexicographic(_) => {
            return Err(BackendError::unsupported(
                "lexicographic objective policy is not supported by the P26 HiGHS \
                 projection (P31 compiles it)",
            ));
        }
    }
    Ok(())
}

/// Project a solve-scoped objective that has no user objective identity.
///
/// Portable feasibility repair creates an objective whose coefficients refer
/// to temporary violation columns. Those columns are deliberately absent from
/// the persistent user-objective maps, so the ordinary policy projector cannot
/// resolve the objective by `ObjId`. This helper projects the complete
/// temporary objective directly and leaves ownership/rollback to the overlay
/// session.
pub(crate) fn project_temporary_objective(
    raw: *mut c_void,
    objective: &roml::advanced::CompiledObjective,
    col_map: &IndexMap<CompiledVariableId>,
) -> Result<(), BackendError> {
    // Resolve every coefficient before changing native costs. A malformed
    // solve-scoped objective must not clear a persistent objective and then
    // fail halfway through projection.
    let mapped_coefficients: Vec<(HighsInt, f64)> = objective
        .coefficients
        .iter()
        .map(|(variable, cost)| {
            col_map
                .get(*variable)
                .map(|col| (col, *cost))
                .ok_or_else(|| missing_variable(*variable))
        })
        .collect::<Result<_, _>>()?;

    // SAFETY: raw is a valid HiGHS instance handle owned by the session.
    unsafe {
        clear_all_costs(raw)?;
    }
    for (col, cost) in mapped_coefficients {
        // SAFETY: raw is valid and col was resolved from the live column map.
        unsafe {
            check_highs_status(
                Highs_changeColCost(raw, col, cost),
                raw,
                "Highs_changeColCost (temporary objective)",
            )?;
        }
    }
    // SAFETY: raw is valid; the objective fields were validated by the
    // portable overlay compiler before reaching this projection.
    unsafe {
        check_highs_status(
            Highs_changeObjectiveSense(raw, sense_to_highs(objective.sense)),
            raw,
            "Highs_changeObjectiveSense (temporary objective)",
        )?;
        check_highs_status(
            Highs_changeObjectiveOffset(raw, objective.constant),
            raw,
            "Highs_changeObjectiveOffset (temporary objective)",
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
    // F3: reject an unsupported objective policy BEFORE any native mutation —
    // Weighted/Lexicographic are P31 constructs the single-objective HiGHS
    // surface cannot represent. The check runs before `Highs_clear`, so a
    // rejected rebuild leaves the native model (columns/rows/costs) untouched.
    match &snapshot.objective_policy {
        CompiledObjectivePolicy::Weighted(_) => {
            return Err(BackendError::unsupported(
                "weighted objective policy is not supported by the P26 HiGHS projection \
                 (P31 compiles it)",
            ));
        }
        CompiledObjectivePolicy::Lexicographic(_) => {
            return Err(BackendError::unsupported(
                "lexicographic objective policy is not supported by the P26 HiGHS projection \
                 (P31 compiles it)",
            ));
        }
        _ => {}
    }

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

            // Resolve compiled variable ids to native column indices. F5: a
            // row coefficient referencing a compiled variable absent from this
            // snapshot is a typed error, never a silent skip (the preflight
            // snapshot.validate() catches it first; this is defense-in-depth).
            let mut indices: Vec<HighsInt> = Vec::with_capacity(r.coefficients.len());
            let mut values: Vec<f64> = Vec::with_capacity(r.coefficients.len());
            for (cid, value) in &r.coefficients {
                let col = col_map.get(*cid).ok_or_else(|| {
                    BackendError::new(
                        format!(
                            "row {:?} references compiled variable {cid:?} that is not present \
                             in this snapshot",
                            r.id
                        ),
                        ErrorCategory::InvalidInput,
                        HealthEffect::RequiresRebuild,
                    )
                })?;
                indices.push(col);
                values.push(*value);
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
        // F4: Weighted/Lexicographic are reachable only from the P31 canonical
        // ObjectivePolicy (design §15); P26 does not emit them and the
        // single-objective HiGHS surface cannot represent them. Reject with a
        // typed error instead of silently treating them as no-active-objective.
        CompiledObjectivePolicy::Weighted(_) => {
            return Err(BackendError::unsupported(
                "weighted objective policy is not supported by the P26 HiGHS projection \
                 (P31 compiles it)",
            ));
        }
        CompiledObjectivePolicy::Lexicographic(_) => {
            return Err(BackendError::unsupported(
                "lexicographic objective policy is not supported by the P26 HiGHS projection \
                 (P31 compiles it)",
            ));
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
    // F3: reject an unsupported objective policy op BEFORE any native
    // mutation — including the cost clearing the `SetObjectivePolicy` arm
    // would otherwise perform first. A rejected batch leaves the native
    // model (including objective costs) untouched.
    for op in &batch.operations {
        if let BackendOp::SetObjectivePolicy(policy) = op {
            match policy {
                CompiledObjectivePolicy::Weighted(_) => {
                    return Err(BackendError::unsupported(
                        "weighted objective policy is not supported by the P26 HiGHS \
                         projection (P31 compiles it)",
                    ));
                }
                CompiledObjectivePolicy::Lexicographic(_) => {
                    return Err(BackendError::unsupported(
                        "lexicographic objective policy is not supported by the P26 HiGHS \
                         projection (P31 compiles it)",
                    ));
                }
                _ => {}
            }
        }
    }

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
                let idx = col_map.remove(*id).ok_or_else(|| missing_variable(*id))?;
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

            BackendOp::SetVariableBounds { variable, bounds } => {
                let idx = col_map
                    .get(*variable)
                    .ok_or_else(|| missing_variable(*variable))?;
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

            BackendOp::AddLinearRow(r) => {
                let lb = normalize_bound(r.bounds.lower, inf);
                let ub = normalize_bound(r.bounds.upper, inf);
                unsafe {
                    let row = Highs_getNumRow(raw);
                    let mut indices: Vec<HighsInt> = Vec::with_capacity(r.coefficients.len());
                    let mut values: Vec<f64> = Vec::with_capacity(r.coefficients.len());
                    for (cid, value) in &r.coefficients {
                        // F5: a coefficient referencing a compiled variable not
                        // present in the held state is a typed error, not a
                        // silent skip.
                        let col = col_map.get(*cid).ok_or_else(|| missing_variable(*cid))?;
                        indices.push(col);
                        values.push(*value);
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
                let idx = row_map.remove(*id).ok_or_else(|| missing_row(*id))?;
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

            BackendOp::SetLinearRowBounds { constraint, bounds } => {
                let idx = row_map
                    .get(*constraint)
                    .ok_or_else(|| missing_row(*constraint))?;
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

            BackendOp::SetLinearCoefficient {
                constraint,
                variable,
                value,
            } => {
                let row = row_map
                    .get(*constraint)
                    .ok_or_else(|| missing_row(*constraint))?;
                let col = col_map
                    .get(*variable)
                    .ok_or_else(|| missing_variable(*variable))?;
                unsafe {
                    check_highs_status(
                        Highs_changeCoeff(raw, row, col, *value),
                        raw,
                        "Highs_changeCoeff",
                    )?;
                }
            }

            BackendOp::RemoveLinearCoefficient {
                constraint,
                variable,
            } => {
                let row = row_map
                    .get(*constraint)
                    .ok_or_else(|| missing_row(*constraint))?;
                let col = col_map
                    .get(*variable)
                    .ok_or_else(|| missing_variable(*variable))?;
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
                if !compiled_to_user_objective.contains_key(id) {
                    return Err(missing_objective(*id));
                }
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
                if !compiled_to_user_objective.contains_key(objective) {
                    return Err(missing_objective(*objective));
                }
                if col_map.get(*variable).is_none() {
                    return Err(missing_variable(*variable));
                }
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
                if !compiled_to_user_objective.contains_key(objective) {
                    return Err(missing_objective(*objective));
                }
                if col_map.get(*variable).is_none() {
                    return Err(missing_variable(*variable));
                }
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
                if !compiled_to_user_objective.contains_key(objective) {
                    return Err(missing_objective(*objective));
                }
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
                if !compiled_to_user_objective.contains_key(objective) {
                    return Err(missing_objective(*objective));
                }
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
                // Shared projection (P27 Task 10): the overlay apply/rollback
                // path uses the exact same compiled-policy projection.
                project_objective_policy(
                    raw,
                    policy,
                    col_map,
                    compiled_to_user_objective,
                    obj_costs,
                    obj_senses,
                    obj_offsets,
                    active_obj,
                )?;
            }

            // `BackendOp` is #[non_exhaustive] (the pinned 15-variant
            // enumeration, backend-contract point B3). F4: a FUTURE op variant
            // this projection does not understand must be a hard typed error —
            // never a silent no-op that would let a batch "succeed" without
            // applying one of its operations. The wildcard is unreachable in
            // P26 (all pinned variants are handled above) but is enforced by
            // construction for the bridge tasks that add variants.
            _ => {
                return Err(BackendError::unsupported(
                    "backend op variant not supported by this projection",
                ));
            }
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
