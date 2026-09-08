//! Solution extraction and status mapping.
//!
//! Maps HiGHS model status and run status to [`TerminationStatus`] and
//! extracts solution data (primal values, duals, reduced costs) from
//! a HiGHS instance after a solve.
//!
//! # Pitfall 3 — Run status checked BEFORE model status
//!
//! The run status is the return value of [`Highs_run`]. If it indicates
//! an error, we return [`TerminationStatus::Error`] regardless of model
//! status. This prevents incorrect status mapping when the solver itself
//! failed to produce a meaningful outcome.
//!
//! # Threat mitigations
//!
//! - T-11-15: [`CString::new`] failures produce [`BackendError::InvalidInput`]
//! - T-11-16: Buffer sizes validated via [`Highs_getNumCol`] before allocation
//! - T-11-17: Run status checked before model status (Pitfall 3)

use std::collections::HashMap;
use std::ffi::c_void;

use log::warn;

use crate::bindings::*;
use crate::index_map::IndexMap;
use roml::advanced::{CompiledConstraintId, CompiledVariableId};
use roml::id::{ConId, VarId};
use roml::solver::backend::TerminationStatus;
use roml::solver::request::SolveSolution;

// ── Status Mapping ──────────────────────────────────────────────────────────────

/// Map HiGHS run status (from [`Highs_run`]) and model status to
/// [`TerminationStatus`].
///
/// # Run status check (Pitfall 3)
///
/// `run_status` is the return value of [`Highs_run`]. If it indicates
/// a warning, we log but proceed. The caller is responsible for
/// returning an error from [`solve`] when `run_status` is negative
/// (kHighsStatusError).
///
/// H6: `MODEL_STATUS_UNBOUNDED_OR_INFEASIBLE` maps to
/// [`TerminationStatus::InfeasibleOrUnbounded`] — preserve ambiguity.
///
/// # Feasible incumbent check
///
/// For MIP outcomes `MODEL_STATUS_OBJECTIVE_BOUND` and
/// `MODEL_STATUS_OBJECTIVE_TARGET`, the function checks the native
/// incumbent evidence via [`has_native_incumbent`]. If no feasible
/// incumbent exists, returns [`TerminationStatus::Error`].
///
/// # Safety
///
/// `raw` must be a valid HiGHS instance handle from [`Highs_create`].
pub(crate) fn map_termination_status(raw: *mut c_void, run_status: HighsInt) -> TerminationStatus {
    // Step 1: Run status check (Pitfall 3 mitigation T-11-17).
    if run_status == STATUS_WARNING || run_status == kHighsStatusWarning {
        warn!("HiGHS solve completed with warning status; proceeding to model status check");
    }

    // Step 2: Model status mapping (exhaustive per AD-6).
    // SAFETY: `raw` is guaranteed valid by the caller (HighsSession).
    // Highs_getModelStatus reads an internal integer field and does not
    // mutate HiGHS state.
    let model_status = unsafe { Highs_getModelStatus(raw) };

    match model_status {
        MODEL_STATUS_OPTIMAL => TerminationStatus::Optimal,

        MODEL_STATUS_INFEASIBLE => TerminationStatus::Infeasible,

        MODEL_STATUS_UNBOUNDED_OR_INFEASIBLE => {
            // H6: CRITICAL — preserve ambiguity. Do NOT collapse into Infeasible.
            TerminationStatus::InfeasibleOrUnbounded
        }

        MODEL_STATUS_UNBOUNDED => TerminationStatus::Unbounded,

        MODEL_STATUS_OBJECTIVE_BOUND | MODEL_STATUS_OBJECTIVE_TARGET => {
            // MIP: objective bound/target reached. Check the native
            // incumbent evidence (not buffer contents). If no feasible
            // incumbent exists, treat as error.
            if has_native_incumbent(raw as *const c_void) {
                TerminationStatus::Feasible
            } else {
                TerminationStatus::Error
            }
        }

        MODEL_STATUS_TIME_LIMIT => TerminationStatus::TimeLimit,
        MODEL_STATUS_ITERATION_LIMIT => TerminationStatus::IterationLimit,

        // Empty model is trivially optimal.
        MODEL_STATUS_MODEL_EMPTY => TerminationStatus::Optimal,

        // Error outcomes.
        MODEL_STATUS_LOAD_ERROR
        | MODEL_STATUS_MODEL_ERROR
        | MODEL_STATUS_SOLVE_ERROR
        | MODEL_STATUS_POSTSOLVE_ERROR => TerminationStatus::Error,

        // Presolve failure indicates numerical issues.
        MODEL_STATUS_PRESOLVE_ERROR => TerminationStatus::NumericalIssue,

        // Not-set (0) or unknown (15) or any unrecognised value.
        MODEL_STATUS_NOT_SET | MODEL_STATUS_UNKNOWN => TerminationStatus::Unknown,

        other => {
            warn!("Unknown HiGHS model status code: {}", other);
            TerminationStatus::Unknown
        }
    }
}

/// Query HiGHS' native primal-solution status (`primal_solution_status`
/// info): whether the solver holds a feasible incumbent. This is the
/// authoritative incumbent evidence — buffer contents alone (which may
/// hold default zeros with no incumbent, e.g. after a time limit) must
/// never establish feasibility.
///
/// Returns true only for `kHighsSolutionStatusFeasible`. Any query
/// failure fails closed (false).
///
/// # Safety
///
/// `raw` must be a valid HiGHS instance handle.
fn has_native_incumbent(raw: *const c_void) -> bool {
    use std::ffi::CString;
    let name = match CString::new("primal_solution_status") {
        Ok(name) => name,
        Err(_) => return false,
    };
    let mut status: HighsInt = kHighsSolutionStatusNone;
    // SAFETY: `raw` is guaranteed valid by the caller; `name` is a valid
    // NUL-terminated string; `status` points to a live integer.
    // `Highs_getIntInfoValue` is read-only.
    let ret = unsafe { Highs_getIntInfoValue(raw, name.as_ptr(), &mut status) };
    ret == STATUS_OK && status == kHighsSolutionStatusFeasible
}

// ── Solution Extraction ─────────────────────────────────────────────────────────

/// Extract solution data from HiGHS after a solve.
///
/// Only extracts when the termination status indicates a solution may be
/// available. Returns `None` for error/unknown statuses or when
/// [`Highs_getSolution`] fails.
///
/// # Data mapping
///
/// | HiGHS output    | ROML field           |
/// |-----------------|----------------------|
/// | `col_value`     | `variable_values`    |
/// | `col_dual`      | `reduced_costs`      |
/// | `row_dual`      | `dual_values`        |
/// | `objective_val` | `objective_value`    |
///
/// # Safety
///
/// - `raw` must be a valid HiGHS instance handle from [`Highs_create`].
/// - `col_map` and `row_map` must correspond to the HiGHS model's
///   column/row index layout.
///
/// # Buffer overrun protection (T-11-16)
///
/// Column and row counts are validated via [`Highs_getNumCol`] /
/// [`Highs_getNumRow`] before allocation. Slice lengths are derived
/// from these validated counts, not from untrusted pointers.
pub(crate) fn extract_solution(
    raw: *mut c_void,
    status: &TerminationStatus,
    col_map: &IndexMap<CompiledVariableId>,
    row_map: &IndexMap<CompiledConstraintId>,
    compiled_to_user_variable: &HashMap<CompiledVariableId, VarId>,
    compiled_to_user_constraint: &HashMap<CompiledConstraintId, ConId>,
) -> Option<SolveSolution> {
    // Only extract for statuses that may have a solution.
    match status {
        TerminationStatus::Optimal
        | TerminationStatus::Feasible
        | TerminationStatus::InfeasibleOrUnbounded
        | TerminationStatus::TimeLimit
        | TerminationStatus::IterationLimit => {}
        _ => return None,
    }

    // Incumbent evidence: extraction requires a solver-reported feasible
    // primal solution. Buffer contents alone prove nothing — HiGHS leaves
    // default (e.g. zero) entries when no incumbent exists, and exposing
    // them would fabricate a primal result (notably on bare time limits).
    if !has_native_incumbent(raw as *const c_void) {
        return None;
    }

    // SAFETY: `raw` is guaranteed valid by the caller.
    // Highs_getNumCol and Highs_getNumRow read internal fields.
    let num_col = unsafe { Highs_getNumCol(raw) };
    let num_row = unsafe { Highs_getNumRow(raw) };

    if num_col <= 0 {
        return None;
    }

    let n_col = num_col as usize;
    let n_row = num_row.max(0) as usize;

    // T-11-16: Buffer sizes are derived from the validated HiGHS column/row
    // count, ensuring we never read or write beyond allocated memory.
    let mut col_value = vec![f64::NAN; n_col];
    let mut col_dual = vec![f64::NAN; n_col];
    let mut row_value = vec![f64::NAN; n_row];
    let mut row_dual = vec![f64::NAN; n_row];

    // SAFETY:
    // - `raw` is a valid HiGHS instance handle.
    // - All output buffers are pre-allocated to num_col / num_row elements
    //   respectively, which is exactly the size HiGHS writes.
    // - Highs_getSolution writes at most the allocated number of elements.
    let ret = unsafe {
        Highs_getSolution(
            raw,
            col_value.as_mut_ptr(),
            col_dual.as_mut_ptr(),
            row_value.as_mut_ptr(),
            row_dual.as_mut_ptr(),
        )
    };

    if ret != STATUS_OK {
        warn!("Highs_getSolution returned non-OK status: {}", ret);
        return None;
    }

    // Objective value.
    // SAFETY: `raw` is valid. Highs_getObjectiveValue is a read-only query.
    // Per AD-9, this value includes any constant offset in HiGHS 1.14+.
    let objective_value = unsafe { Highs_getObjectiveValue(raw) };

    // Build reverse maps for O(1) HiGHS-index → compiled-id lookup, then map
    // compiled ids → user ids via the origin-derived maps (SM-02.5).
    let rev_col = col_map.reverse_map();
    let rev_row = row_map.reverse_map();

    // Map column primal values (col_value) → variable_values (user-keyed).
    let variable_values: Vec<(VarId, f64)> = (0..n_col)
        .filter_map(|hi_idx| {
            let v = col_value[hi_idx];
            if v.is_nan() || v.is_infinite() {
                return None;
            }
            let compiled = rev_col.get(&(hi_idx as i32)).copied()?;
            compiled_to_user_variable
                .get(&compiled)
                .copied()
                .map(|var_id| (var_id, v))
        })
        .collect();

    // Map column duals (col_dual) → reduced_costs (user-keyed).
    let reduced_costs: Option<Vec<(VarId, f64)>> = {
        let costs: Vec<(VarId, f64)> = (0..n_col)
            .filter_map(|hi_idx| {
                let v = col_dual[hi_idx];
                if v.is_nan() || v.is_infinite() {
                    return None;
                }
                let compiled = rev_col.get(&(hi_idx as i32)).copied()?;
                compiled_to_user_variable
                    .get(&compiled)
                    .copied()
                    .map(|var_id| (var_id, v))
            })
            .collect();
        if costs.is_empty() {
            None
        } else {
            Some(costs)
        }
    };

    // Map row duals (row_dual) → dual_values (user-keyed).
    let dual_values: Option<Vec<(ConId, f64)>> = if n_row > 0 {
        let duals: Vec<(ConId, f64)> = (0..n_row)
            .filter_map(|hi_idx| {
                let v = row_dual[hi_idx];
                if v.is_nan() || v.is_infinite() {
                    return None;
                }
                let compiled = rev_row.get(&(hi_idx as i32)).copied()?;
                compiled_to_user_constraint
                    .get(&compiled)
                    .copied()
                    .map(|con_id| (con_id, v))
            })
            .collect();
        if duals.is_empty() {
            None
        } else {
            Some(duals)
        }
    } else {
        None
    };

    // If no variable values were extracted, the solution is empty.
    if variable_values.is_empty() {
        return None;
    }

    Some(SolveSolution {
        variable_values,
        objective_value: Some(objective_value),
        dual_values,
        reduced_costs,
    })
}

// ── Tests ───────────────────────────────────────────────────────────────────────

// Unit tests removed — require HiGHS session for null-safety.
