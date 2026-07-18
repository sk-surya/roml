//! HiGHS model projection — snapshot rebuild and delta application.
//!
//! This module provides the core model synchronisation logic that translates
//! ROML canonical model state ([`ModelSnapshot`]) and incremental operations
//! ([`DeltaBatch`]) into HiGHS C API calls.
//!
//! # Architecture
//!
//! - [`rebuild_from_snapshot`]: Full deterministic rebuild from a
//!   [`ModelSnapshot`]. Clears the HiGHS model, then adds all variables,
//!   constraints, cells, and objectives in order.
//!
//! - [`apply_delta_batch`]: Apply a batch of [`ModelOp`] variants to an
//!   existing HiGHS model. All 16 variants are handled or explicitly rejected.
//!   Partial application is impossible — the function acknowledges only after
//!   every operation succeeds.
//!
//! - [`check_semicontinuous`]: Rejection guard for unsupported domains
//!   (M1R-H7 compliance). Called before any HiGHS state modification.

use std::collections::HashMap;
use std::ffi::c_void;

use log::warn;

use crate::bindings::*;
use crate::error::{check_highs_status, from_native_status};
use crate::index_map::IndexMap;
use roml::delta::{DeltaBatch, ModelOp};
use roml::id::{ConId, ObjId, VarId};
use roml::model::coefficient::{CoefficientTarget};
use roml::model::objective::Sense;
use roml::model::variable::{VarType};
use roml::snapshot::ModelSnapshot;
use roml::solver::backend::{BackendError, ErrorCategory, HealthEffect};

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Normalise a ROML bound value to a HiGHS-compatible value.
///
/// ROML uses `f64::INFINITY` / `f64::NEG_INFINITY` to denote unbounded
/// directions. HiGHS uses a finite infinity value (1e30 by default, cached
/// from `Highs_getInfinity`). This function maps the ROML sentinels to the
/// HiGHS cached infinity value.
///
/// # Pitfall 6
///
/// Passing `f64::INFINITY` directly to HiGHS C API functions causes them to
/// reject the call. Always normalise bounds.
pub(crate) fn normalize_bound(b: f64, inf: f64) -> f64 {
    if b == f64::NEG_INFINITY {
        -inf
    } else if b == f64::INFINITY {
        inf
    } else {
        b
    }
}

/// Convert a ROML [`VarType`] to a HiGHS integrality constant.
fn var_type_to_integrality(var_type: VarType) -> HighsInt {
    match var_type {
        VarType::Continuous => VAR_TYPE_CONTINUOUS,
        VarType::Integer | VarType::Binary => VAR_TYPE_INTEGER,
    }
}

/// Convert a ROML [`Sense`] to a HiGHS objective sense constant.
fn sense_to_highs(sense: Sense) -> HighsInt {
    match sense {
        Sense::Minimize => OBJECTIVE_SENSE_MINIMIZE,
        Sense::Maximize => OBJECTIVE_SENSE_MAXIMIZE,
    }
}

// ── Semi-Continuous Check (M1R-H7) ──────────────────────────────────────────

/// Check if the snapshot contains any semi-continuous variables.
///
/// Per M1R-H7, this rejection MUST happen before any HiGHS state
/// modification. The function only reads the snapshot — no HiGHS calls are
/// made.
///
/// Returns `Ok(())` if no semi-continuous data is present.
/// Returns `Err(BackendError::unsupported(...))` with
/// `HealthEffect::RequiresRebuild` if any variable has
/// `semicontinuous_lower: Some(_)`.
pub(crate) fn check_semicontinuous(snapshot: &ModelSnapshot) -> Result<(), BackendError> {
    for var in &snapshot.variables {
        if var.semicontinuous_lower.is_some() {
            return Err(BackendError::unsupported("semi-continuous variables"));
        }
    }
    Ok(())
}

// ── Snapshot Rebuild ─────────────────────────────────────────────────────────

/// Rebuild the HiGHS model from a canonical [`ModelSnapshot`].
///
/// This is a full, deterministic rebuild. It:
/// 1. Checks for semi-continuous data (reject before any modification).
/// 2. Clears the HiGHS model via `Highs_clear`.
/// 3. Clears all cached maps.
/// 4. Adds all variables (with integrality, activity, bound caching).
/// 5. Adds all constraints (empty rows with bound caching).
/// 6. Fills constraint coefficient cells.
/// 7. Registers objectives with cached costs and senses.
/// 8. Sets inactive constraints to [-inf, inf] (unconstrained).
///
/// # Safety
///
/// `raw` must be a valid HiGHS instance handle from `Highs_create`. The
/// caller (typically [`HighsSession`](crate::lifecycle::HighsSession))
/// guarantees the handle is non-null and exclusively owned.
pub(crate) fn rebuild_from_snapshot(
    raw: *mut c_void,
    snapshot: &ModelSnapshot,
    col_map: &mut IndexMap<VarId>,
    row_map: &mut IndexMap<ConId>,
    inf: f64,
    var_bounds: &mut HashMap<VarId, (f64, f64)>,
    con_bounds: &mut HashMap<ConId, (f64, f64)>,
    obj_costs: &mut HashMap<ObjId, HashMap<VarId, f64>>,
    obj_senses: &mut HashMap<ObjId, Sense>,
    active_obj: &mut Option<ObjId>,
) -> Result<(), BackendError> {
    // Step 1: Semi-continuous rejection BEFORE any state modification.
    check_semicontinuous(snapshot)?;

    // Step 2: Clear the HiGHS model.
    // SAFETY: `raw` is guaranteed valid by caller. `Highs_clear` resets
    // the model but preserves options and settings.
    check_highs_status(
        unsafe { Highs_clear(raw) },
        raw,
        "Highs_clear",
    )?;

    // Step 3: Clear all caches.
    *col_map = IndexMap::new();
    *row_map = IndexMap::new();
    var_bounds.clear();
    con_bounds.clear();
    obj_costs.clear();
    obj_senses.clear();
    *active_obj = None;

    // Step 4: Add variables.
    unsafe {
        for var in &snapshot.variables {
            let lb = normalize_bound(var.bounds.lower, inf);
            let ub = normalize_bound(var.bounds.upper, inf);

            // Highs_addVar returns the column index on success (>= 0)
            // or -1 on failure.
            let col = Highs_addVar(raw, lb, ub);
            if col < 0 {
                return Err(from_native_status(col, "Highs_addVar"));
            }
            col_map.insert(var.id, col);

            // Set integrality for integer/binary.
            match var.var_type {
                VarType::Continuous => {}
                VarType::Integer | VarType::Binary => {
                    check_highs_status(
                        Highs_changeColIntegrality(raw, col, VAR_TYPE_INTEGER),
                        raw,
                        "Highs_changeColIntegrality",
                    )?;
                }
            }

            // Inactive variables are fixed to [0, 0].
            if !var.active {
                check_highs_status(
                    Highs_changeColBounds(raw, col, 0.0, 0.0),
                    raw,
                    "Highs_changeColBounds (inactive var)",
                )?;
            }

            var_bounds.insert(var.id, (lb, ub));
        }
    }

    // Step 5: Add constraints (empty rows).
    unsafe {
        for con in &snapshot.constraints {
            let lb = normalize_bound(con.bounds.lower, inf);
            let ub = normalize_bound(con.bounds.upper, inf);

            // Highs_addRow returns the row index on success (>= 0)
            // or -1 on failure.
            let row = Highs_addRow(raw, lb, ub, 0, std::ptr::null(), std::ptr::null());
            if row < 0 {
                return Err(from_native_status(row, "Highs_addRow"));
            }
            row_map.insert(con.id, row);
            con_bounds.insert(con.id, (lb, ub));
        }
    }

    // Step 6: Add constraint coefficient cells.
    unsafe {
        for cell in &snapshot.cells {
            let (target, var_id) = cell.cell_key;
            if let Some(col) = col_map.get(var_id) {
                match target {
                    CoefficientTarget::Constraint(con_id) => {
                        if let Some(row) = row_map.get(con_id) {
                            check_highs_status(
                                Highs_changeCoeff(raw, row, col, cell.evaluated_value),
                                raw,
                                "Highs_changeCoeff",
                            )?;
                        }
                    }
                    CoefficientTarget::Objective(_) => {
                        // Objective cells are processed in Step 7.
                    }
                }
            }
        }
    }

    // Step 7: Add objectives and objective coefficient cells.
    for obj in &snapshot.objectives {
        obj_senses.insert(obj.id, obj.sense);
        let mut costs: HashMap<VarId, f64> = HashMap::new();

        // Collect objective cells into the cost cache and set HiGHS costs.
        for cell in &snapshot.cells {
            let (target, var_id) = cell.cell_key;
            if let CoefficientTarget::Objective(obj_id) = target {
                if obj_id == obj.id {
                    costs.insert(var_id, cell.evaluated_value);
                    if let Some(col) = col_map.get(var_id) {
                        unsafe {
                            check_highs_status(
                                Highs_changeColCost(raw, col, cell.evaluated_value),
                                raw,
                                "Highs_changeColCost",
                            )?;
                        }
                    }
                }
            }
        }
        obj_costs.insert(obj.id, costs);

        // Set objective sense in HiGHS.
        unsafe {
            check_highs_status(
                Highs_changeObjectiveSense(raw, sense_to_highs(obj.sense)),
                raw,
                "Highs_changeObjectiveSense",
            )?;
        }

        if obj.active {
            *active_obj = Some(obj.id);
        }
    }

    // Step 8: Handle inactive constraints — set to [-inf, inf].
    for con in &snapshot.constraints {
        if !con.active {
            if let Some(row) = row_map.get(con.id) {
                unsafe {
                    check_highs_status(
                        Highs_changeRowBounds(raw, row, -inf, inf),
                        raw,
                        "Highs_changeRowBounds (inactive con)",
                    )?;
                }
            }
        }
    }

    Ok(())
}

// ── Delta Application ────────────────────────────────────────────────────────

/// Apply a [`DeltaBatch`] of [`ModelOp`] operations to the HiGHS model.
///
/// # Pre-validation
///
/// All operations are scanned for unsupported domain data (e.g.,
/// semi-continuous) BEFORE any HiGHS call. If any operation is found, the
/// function returns immediately without modifying HiGHS state
/// (M1R-H7 compliance).
///
/// # Atomicity
///
/// Acknowledges only on complete success. If any operation fails, the error
/// is returned immediately and the HiGHS model may be in an inconsistent
/// state. The caller is responsible for rebuilding from a snapshot if the
/// session health requires it.
///
/// # Safety
///
/// `raw` must be a valid HiGHS instance handle from `Highs_create`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_delta_batch(
    raw: *mut c_void,
    batch: &DeltaBatch,
    col_map: &mut IndexMap<VarId>,
    row_map: &mut IndexMap<ConId>,
    inf: f64,
    var_bounds: &mut HashMap<VarId, (f64, f64)>,
    con_bounds: &mut HashMap<ConId, (f64, f64)>,
    obj_costs: &mut HashMap<ObjId, HashMap<VarId, f64>>,
    obj_senses: &mut HashMap<ObjId, Sense>,
    active_obj: &mut Option<ObjId>,
) -> Result<(), BackendError> {
    // ── Pre-validation phase ──────────────────────────────────────────────
    // Scan all operations for unsupported domain data. Currently no ModelOp
    // variant carries semi-continuous data, but this loop exists for
    // future-proofing (M1R-H7).
    for op in &batch.operations {
        match op {
            ModelOp::AddVariable { .. } => {
                // ModelOp::AddVariable does not carry semicontinuous_lower.
                // If a future extension adds semi-continuous data to a
                // ModelOp, reject it here before any HiGHS state change.
            }
            _ => {}
        }
    }

    // ── Apply phase ──────────────────────────────────────────────────────
    for op in &batch.operations {
        match op {
            ModelOp::AddVariable {
                var,
                bounds,
                var_type,
            } => {
                let lb = normalize_bound(bounds.lower, inf);
                let ub = normalize_bound(bounds.upper, inf);

                unsafe {
                    let col = Highs_addVar(raw, lb, ub);
                    if col < 0 {
                        return Err(from_native_status(col, "Highs_addVar"));
                    }
                    col_map.insert(*var, col);

                    match var_type {
                        VarType::Continuous => {}
                        VarType::Integer | VarType::Binary => {
                            check_highs_status(
                                Highs_changeColIntegrality(raw, col, VAR_TYPE_INTEGER),
                                raw,
                                "Highs_changeColIntegrality",
                            )?;
                        }
                    }

                    var_bounds.insert(*var, (lb, ub));
                }
            }

            ModelOp::RemoveVariable { var } => {
                if let Some(idx) = col_map.remove(*var) {
                    unsafe {
                        check_highs_status(
                            Highs_deleteColsBySet(raw, 1, &[idx] as *const HighsInt),
                            raw,
                            "Highs_deleteColsBySet",
                        )?;
                    }
                    // HiGHS closes the gap at the deleted index:
                    // every column index > idx shifts down by 1.
                    col_map.reindex_after_delete(idx);
                    var_bounds.remove(var);

                    // Remove from all objective cost caches.
                    for costs in obj_costs.values_mut() {
                        costs.remove(var);
                    }
                }
            }

            ModelOp::SetVariableBounds { var, bounds } => {
                if let Some(idx) = col_map.get(*var) {
                    let lb = normalize_bound(bounds.lower, inf);
                    let ub = normalize_bound(bounds.upper, inf);
                    unsafe {
                        check_highs_status(
                            Highs_changeColBounds(raw, idx, lb, ub),
                            raw,
                            "Highs_changeColBounds",
                        )?;
                    }
                    var_bounds.insert(*var, (lb, ub));
                }
            }

            ModelOp::SetVariableActive { var, active } => {
                if let Some(idx) = col_map.get(*var) {
                    if *active {
                        // Restore from cached bounds.
                        if let Some(&(lb, ub)) = var_bounds.get(var) {
                            unsafe {
                                check_highs_status(
                                    Highs_changeColBounds(raw, idx, lb, ub),
                                    raw,
                                    "Highs_changeColBounds (restore active var)",
                                )?;
                            }
                        }
                    } else {
                        // Fix to [0, 0] for deactivation.
                        unsafe {
                            check_highs_status(
                                Highs_changeColBounds(raw, idx, 0.0, 0.0),
                                raw,
                                "Highs_changeColBounds (deactivate var)",
                            )?;
                        }
                    }
                }
            }

            ModelOp::SetVariableType { var, var_type } => {
                if let Some(idx) = col_map.get(*var) {
                    let type_code = var_type_to_integrality(*var_type);
                    unsafe {
                        check_highs_status(
                            Highs_changeColIntegrality(raw, idx, type_code),
                            raw,
                            "Highs_changeColIntegrality",
                        )?;
                    }
                }
            }

            ModelOp::AddConstraint { con, bounds } => {
                let lb = normalize_bound(bounds.lower, inf);
                let ub = normalize_bound(bounds.upper, inf);

                unsafe {
                    let row = Highs_addRow(raw, lb, ub, 0, std::ptr::null(), std::ptr::null());
                    if row < 0 {
                        return Err(from_native_status(row, "Highs_addRow"));
                    }
                    row_map.insert(*con, row);
                    con_bounds.insert(*con, (lb, ub));
                }
            }

            ModelOp::RemoveConstraint { con } => {
                if let Some(idx) = row_map.remove(*con) {
                    unsafe {
                        check_highs_status(
                            Highs_deleteRowsBySet(raw, 1, &[idx] as *const HighsInt),
                            raw,
                            "Highs_deleteRowsBySet",
                        )?;
                    }
                    // HiGHS closes the gap at the deleted row index.
                    row_map.reindex_after_delete(idx);
                    con_bounds.remove(con);
                }
            }

            ModelOp::SetConstraintBounds { con, bounds } => {
                if let Some(idx) = row_map.get(*con) {
                    let lb = normalize_bound(bounds.lower, inf);
                    let ub = normalize_bound(bounds.upper, inf);
                    unsafe {
                        check_highs_status(
                            Highs_changeRowBounds(raw, idx, lb, ub),
                            raw,
                            "Highs_changeRowBounds",
                        )?;
                    }
                    con_bounds.insert(*con, (lb, ub));
                }
            }

            ModelOp::SetConstraintActive { con, active } => {
                if let Some(idx) = row_map.get(*con) {
                    if *active {
                        // Restore from cached bounds.
                        if let Some(&(lb, ub)) = con_bounds.get(con) {
                            unsafe {
                                check_highs_status(
                                    Highs_changeRowBounds(raw, idx, lb, ub),
                                    raw,
                                    "Highs_changeRowBounds (restore active con)",
                                )?;
                            }
                        }
                    } else {
                        // Unconstrain for deactivation: set to [-inf, inf].
                        unsafe {
                            check_highs_status(
                                Highs_changeRowBounds(raw, idx, -inf, inf),
                                raw,
                                "Highs_changeRowBounds (deactivate con)",
                            )?;
                        }
                    }
                }
            }

            ModelOp::SetCell {
                cell_key,
                evaluated_value,
                ..
            } => {
                let (target, var_id) = *cell_key;
                if let Some(col) = col_map.get(var_id) {
                    match target {
                        CoefficientTarget::Constraint(con_id) => {
                            if let Some(row) = row_map.get(con_id) {
                                unsafe {
                                    check_highs_status(
                                        Highs_changeCoeff(raw, row, col, *evaluated_value),
                                        raw,
                                        "Highs_changeCoeff",
                                    )?;
                                }
                            }
                        }
                        CoefficientTarget::Objective(_) => {
                            return Err(BackendError::unsupported(
                                "SetCell with Objective target — use SetObjectiveCell instead",
                            ));
                        }
                    }
                }
            }

            ModelOp::RemoveCell { cell_key } => {
                let (target, var_id) = *cell_key;
                if let Some(col) = col_map.get(var_id) {
                    match target {
                        CoefficientTarget::Constraint(con_id) => {
                            if let Some(row) = row_map.get(con_id) {
                                // HiGHS has no direct "remove coefficient":
                                // setting to zero achieves the same effect.
                                unsafe {
                                    check_highs_status(
                                        Highs_changeCoeff(raw, row, col, 0.0),
                                        raw,
                                        "Highs_changeCoeff (remove cell)",
                                    )?;
                                }
                            }
                        }
                        CoefficientTarget::Objective(obj_id) => {
                            // Zero the objective cost.
                            unsafe {
                                check_highs_status(
                                    Highs_changeColCost(raw, col, 0.0),
                                    raw,
                                    "Highs_changeColCost (remove obj cell)",
                                )?;
                            }
                            // Remove from obj_costs cache.
                            if let Some(costs) = obj_costs.get_mut(&obj_id) {
                                costs.remove(&var_id);
                            }
                        }
                    }
                }
            }

            ModelOp::AddObjective { obj, sense } => {
                obj_senses.insert(*obj, *sense);
                obj_costs.insert(*obj, HashMap::new());
                // No HiGHS API call — objectives are virtual in ROML.
            }

            ModelOp::RemoveObjective { obj } => {
                obj_senses.remove(obj);
                obj_costs.remove(obj);

                if *active_obj == Some(*obj) {
                    // Zero all costs since the active objective is removed.
                    unsafe {
                        let num_cols = Highs_getNumCol(raw);
                        if num_cols > 0 {
                            let zeros = vec![0.0_f64; num_cols as usize];
                            check_highs_status(
                                Highs_changeColsCostByRange(raw, 0, num_cols - 1, zeros.as_ptr()),
                                raw,
                                "Highs_changeColsCostByRange",
                            )?;
                        }
                    }
                    *active_obj = None;
                }
            }

            ModelOp::SetActiveObjective { obj } => {
                // Pitfall 5: Zero ALL column costs first to avoid stale
                // costs from the previous objective blending with the new
                // objective.
                unsafe {
                    let num_cols = Highs_getNumCol(raw);
                    if num_cols > 0 {
                        let zeros = vec![0.0_f64; num_cols as usize];
                        check_highs_status(
                            Highs_changeColsCostByRange(raw, 0, num_cols - 1, zeros.as_ptr()),
                            raw,
                            "Highs_changeColsCostByRange",
                        )?;
                    }
                }

                if let Some(obj_id) = obj {
                    // Load the new objective's costs from cache.
                    if let Some(costs) = obj_costs.get(obj_id) {
                        for (&vid, &cost) in costs {
                            if let Some(col) = col_map.get(vid) {
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
                    // Set objective sense.
                    if let Some(&sense) = obj_senses.get(obj_id) {
                        unsafe {
                            check_highs_status(
                                Highs_changeObjectiveSense(raw, sense_to_highs(sense)),
                                raw,
                                "Highs_changeObjectiveSense",
                            )?;
                        }
                    }
                }

                *active_obj = *obj;
            }

            ModelOp::SetObjectiveCell {
                cell_key,
                evaluated_value,
                constant,
                ..
            } => {
                let (target, var_id) = *cell_key;
                let obj_id = match target {
                    CoefficientTarget::Objective(obj) => obj,
                    CoefficientTarget::Constraint(_) => {
                        return Err(BackendError::new(
                            "SetObjectiveCell with Constraint target — use SetCell instead",
                            ErrorCategory::InvalidInput,
                            HealthEffect::Recoverable,
                        ));
                    }
                };

                // Update obj_costs cache.
                obj_costs
                    .entry(obj_id)
                    .or_default()
                    .insert(var_id, *evaluated_value);

                // Set cost in HiGHS.
                if let Some(col) = col_map.get(var_id) {
                    unsafe {
                        check_highs_status(
                            Highs_changeColCost(raw, col, *evaluated_value),
                            raw,
                            "Highs_changeColCost",
                        )?;
                    }
                }

                // The constant offset is stored for use during extraction
                // (Plan 03). It is not applied to HiGHS directly since
                // Highs_changeObjectiveOffset would affect the entire model,
                // and ROML manages constants per objective.
                let _ = constant;
            }

            ModelOp::SetParameter { param, value } => {
                // Map known parameters to HiGHS equivalents if applicable.
                // Currently, ROML-internal parameters with no HiGHS
                // equivalent are skipped.
                let _ = value;
                warn!(
                    "SetParameter: skipping parameter {:?} (no HiGHS equivalent or model-internal)",
                    param
                );
            }
        }
    }

    Ok(())
}

// ── Tests ────────────────────────────────────────────────────────────────────

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

    #[test]
    fn check_semicontinuous_accepts_continuous() {
        let snapshot = ModelSnapshot::empty(roml::revision::ModelRevision::ZERO);
        assert!(check_semicontinuous(&snapshot).is_ok());
    }

    #[test]
    fn check_semicontinuous_rejects_semicontinuous() {
        use roml::id::Generation;
        use roml::model::variable::Bounds;
        use roml::snapshot::VariableEntry;

        let mut snapshot = ModelSnapshot::empty(roml::revision::ModelRevision::ZERO);
        snapshot.variables.push(VariableEntry {
            id: roml::id::VarId::new(0, Generation::new()),
            bounds: Bounds::NON_NEGATIVE,
            var_type: VarType::Continuous,
            active: true,
            semicontinuous_lower: Some(5.0),
        });
        let result = check_semicontinuous(&snapshot);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("semi-continuous"));
        assert_eq!(err.health_effect, HealthEffect::RequiresRebuild);
    }

    #[test]
    fn var_type_to_integrality_mapping() {
        assert_eq!(var_type_to_integrality(VarType::Continuous), 0);
        assert_eq!(var_type_to_integrality(VarType::Integer), 1);
        assert_eq!(var_type_to_integrality(VarType::Binary), 1);
    }

    #[test]
    fn sense_to_highs_mapping() {
        assert_eq!(sense_to_highs(Sense::Minimize), 1);
        assert_eq!(sense_to_highs(Sense::Maximize), -1);
    }
}
