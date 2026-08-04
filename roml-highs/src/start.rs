//! Qualified HiGHS MIP start application (P28 Task 3; SM-08.1, SM-08.7).
//!
//! The pinned official header audit (`docs/knowledge/highs_mip_start_api.md`)
//! qualifies `Highs_setSparseSolution` as the native partial-MIP-start
//! primitive in the bundled `highs-sys 1.15.0` (and the CI system floor
//! 1.9.0). This module maps each start's user-`Variable` values through the
//! compiled-keyed origin maps to native column indices and applies them with
//! checked return codes (D19: implement through existing official/generated
//! binding boundaries; never infer signatures).
//!
//! Variable hints have NO native API in this version and are never simulated:
//! the `SolvePlan` executor rejects them by default (SM-08.4) and the
//! `apply_variable_hints` override on [`crate::session::HighsSession`] stays a
//! typed `Unsupported` error.

use std::collections::HashMap;
use std::ffi::c_void;

use crate::bindings::{HighsInt, Highs_setSparseSolution};
use crate::error::check_highs_status;
use crate::index_map::IndexMap;
use roml::advanced::CompiledVariableId;
use roml::id::VarId;
use roml::solver::backend::{BackendError, ErrorCategory, HealthEffect};
use roml::MipStart;

/// Apply qualified MIP starts to the HiGHS instance via
/// [`Highs_setSparseSolution`].
///
/// Each start's user-`Variable` values are mapped to native column indices via
/// `col_map` (compiled id -> native column index) and
/// `compiled_to_user_variable` (compiled id -> user variable). A variable with
/// no compiled column (stale, or from another model) is a typed error — never
/// a silent skip (T-28-01). Every native return code is checked through the
/// existing `check_highs_status` pattern; an index/value the backend rejects
/// maps to a typed [`BackendError`], never a panic or unchecked return.
///
/// # Safety
///
/// `raw` must be a valid HiGHS instance handle (owned by the calling session).
/// The `indices`/`values` arrays are populated for `num_entries` before the
/// call; the return code is checked immediately.
pub fn apply_mip_starts(
    raw: *mut c_void,
    starts: &[MipStart],
    col_map: &IndexMap<CompiledVariableId>,
    compiled_to_user_variable: &HashMap<CompiledVariableId, VarId>,
) -> Result<(), BackendError> {
    // Map user variable -> compiled id once (a start's values are keyed by
    // user variable).
    let compiled_by_user: HashMap<VarId, CompiledVariableId> = compiled_to_user_variable
        .iter()
        .map(|(cid, var)| (*var, *cid))
        .collect();

    for start in starts {
        let mut indices: Vec<HighsInt> = Vec::with_capacity(start.assignment.values.len());
        let mut values: Vec<f64> = Vec::with_capacity(start.assignment.values.len());
        for (variable, value) in &start.assignment.values {
            let cid = *compiled_by_user.get(variable).ok_or_else(|| {
                BackendError::new(
                    format!(
                        "MIP start references user variable {variable:?} with no compiled column"
                    ),
                    ErrorCategory::InvalidInput,
                    HealthEffect::Recoverable,
                )
            })?;
            let idx = col_map.get(cid).ok_or_else(|| {
                BackendError::new(
                    format!("MIP start references compiled variable {cid:?} with no native column"),
                    ErrorCategory::InvalidInput,
                    HealthEffect::Recoverable,
                )
            })?;
            indices.push(idx);
            values.push(*value);
        }
        // SAFETY: raw is a valid HiGHS instance handle; indices/values are
        // populated for indices.len() entries; the return code is checked
        // immediately through check_highs_status.
        unsafe {
            check_highs_status(
                Highs_setSparseSolution(
                    raw,
                    indices.len() as HighsInt,
                    indices.as_ptr(),
                    values.as_ptr(),
                ),
                raw,
                "Highs_setSparseSolution (MIP start)",
            )?;
        }
    }
    Ok(())
}

/// The typed `Unsupported` error returned by a request that has no qualified
/// native API in the pinned bundled version (SM-08.4, D19 — never simulate).
pub fn unsupported_hint_error() -> BackendError {
    BackendError::new(
        "HiGHS has no variable-hint API in the pinned bundled version \
         (see docs/knowledge/highs_mip_start_api.md); absent hints reject by default",
        ErrorCategory::Unsupported,
        HealthEffect::Recoverable,
    )
}
