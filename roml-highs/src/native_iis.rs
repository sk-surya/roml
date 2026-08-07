//! Audited bundled HiGHS 1.15.0 native IIS provider.
//!
//! This module is deliberately separate from the portable feasibility oracle:
//! system-discovered HiGHS may run ROML analysis, while this native provider
//! remains compile- and runtime-gated to the audited bundled artifact.

use std::ffi::CString;

use roml::advanced::{
    BackendSnapshot, CompiledConstraintId, CompiledRestrictionRef, CompiledVariableId,
};
use roml::solver::backend::{BackendError, ErrorCategory, HealthEffect};
use roml::solver::infeasibility::{
    BoundSide, NativeBoundStatus, NativeConflict, NativeConflictMember, NativeConflictRequest,
    NativeMembership,
};
use roml::solver::session::{BackendSession, Synchronization};

use crate::bindings;
use crate::error::from_native_status;
use crate::lifecycle::HighsSession;

const QUALIFIED_MAJOR: i32 = 1;
const QUALIFIED_MINOR: i32 = 15;
const QUALIFIED_PATCH: i32 = 0;

fn is_qualified(session: &HighsSession) -> bool {
    session.version_major == QUALIFIED_MAJOR
        && session.version_minor == QUALIFIED_MINOR
        && session.version_patch == QUALIFIED_PATCH
}

/// Obtain native compiled membership from a fresh HiGHS analysis instance.
pub(crate) fn native_conflict(
    _persistent: &HighsSession,
    request: &NativeConflictRequest,
) -> Result<NativeConflict, BackendError> {
    let mut session = HighsSession::try_new()?;
    if !is_qualified(&session) {
        return Err(unsupported("bundled HiGHS runtime is not exactly 1.15.0"));
    }
    if request.compilation_id != request.snapshot.compilation_id {
        return Err(BackendError::new(
            "native IIS request compilation identity does not match its snapshot",
            ErrorCategory::InvalidInput,
            HealthEffect::Recoverable,
        ));
    }
    session.synchronize(Synchronization::CompiledRebuild(request.snapshot.clone()))?;

    let option = CString::new("iis_strategy").expect("static option name has no NUL");
    let status = unsafe {
        bindings::Highs_setIntOptionValue(
            session.raw,
            option.as_ptr(),
            bindings::kHighsIisStrategyFromLpRowPriority,
        )
    };
    if status != bindings::STATUS_OK {
        return Err(from_native_status(
            status,
            "Highs_setIntOptionValue(iis_strategy)",
        ));
    }

    let run_status = unsafe { bindings::Highs_run(session.raw) };
    if run_status < 0 {
        return Err(from_native_status(run_status, "Highs_run(native IIS seed)"));
    }

    extract_native_conflict(session.raw, &request.snapshot)
}

fn extract_native_conflict(
    raw: *mut std::ffi::c_void,
    snapshot: &BackendSnapshot,
) -> Result<NativeConflict, BackendError> {
    let mut num_col = 0;
    let mut num_row = 0;
    let first = unsafe {
        bindings::Highs_getIis(
            raw,
            &mut num_col,
            &mut num_row,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if first != bindings::STATUS_OK {
        return Err(from_native_status(first, "Highs_getIis(counts)"));
    }
    let nc = checked_count(num_col, "IIS column count")?;
    let nr = checked_count(num_row, "IIS row count")?;
    let mut col_index = vec![0; nc];
    let mut row_index = vec![0; nr];
    let mut col_bound = vec![0; nc];
    let mut row_bound = vec![0; nr];
    let mut col_status = vec![bindings::kHighsIisStatusNotInConflict; snapshot.variables.len()];
    let mut row_status = vec![bindings::kHighsIisStatusNotInConflict; snapshot.linear_rows.len()];
    let second = unsafe {
        bindings::Highs_getIis(
            raw,
            &mut num_col,
            &mut num_row,
            col_index.as_mut_ptr(),
            row_index.as_mut_ptr(),
            col_bound.as_mut_ptr(),
            row_bound.as_mut_ptr(),
            col_status.as_mut_ptr(),
            row_status.as_mut_ptr(),
        )
    };
    if second != bindings::STATUS_OK {
        return Err(from_native_status(second, "Highs_getIis(data)"));
    }

    let mut members = Vec::new();
    let mut evidence = Vec::new();
    for (index, bound) in col_index.into_iter().zip(col_bound) {
        let id = checked_index(index, snapshot.variables.len(), "IIS column index")?;
        let status = native_membership(col_status[id]);
        for side in bound_sides(bound) {
            let restriction = match side {
                BoundSide::Lower => {
                    CompiledRestrictionRef::VariableLower(CompiledVariableId(id as u32))
                }
                BoundSide::Upper => {
                    CompiledRestrictionRef::VariableUpper(CompiledVariableId(id as u32))
                }
            };
            let record = NativeConflictMember {
                restriction,
                membership: status,
                bound: Some(native_bound(bound)),
            };
            if status == NativeMembership::Member {
                members.push(restriction);
            }
            evidence.push(record);
        }
    }
    for (index, bound) in row_index.into_iter().zip(row_bound) {
        let id = checked_index(index, snapshot.linear_rows.len(), "IIS row index")?;
        let status = native_membership(row_status[id]);
        for side in bound_sides(bound) {
            let restriction = match side {
                BoundSide::Lower => {
                    CompiledRestrictionRef::ConstraintLower(CompiledConstraintId(id as u32))
                }
                BoundSide::Upper => {
                    CompiledRestrictionRef::ConstraintUpper(CompiledConstraintId(id as u32))
                }
            };
            let record = NativeConflictMember {
                restriction,
                membership: status,
                bound: Some(native_bound(bound)),
            };
            if status == NativeMembership::Member {
                members.push(restriction);
            }
            evidence.push(record);
        }
    }

    Ok(NativeConflict {
        compilation_id: snapshot.compilation_id,
        members,
        evidence,
    })
}

fn checked_count(value: bindings::HighsInt, label: &str) -> Result<usize, BackendError> {
    usize::try_from(value).map_err(|_| {
        BackendError::new(
            format!("{label} was negative: {value}"),
            ErrorCategory::Internal,
            HealthEffect::Recoverable,
        )
    })
}

fn checked_index(
    value: bindings::HighsInt,
    len: usize,
    label: &str,
) -> Result<usize, BackendError> {
    let index = checked_count(value, label)?;
    if index >= len {
        return Err(BackendError::new(
            format!("{label} {index} is outside length {len}"),
            ErrorCategory::Internal,
            HealthEffect::Recoverable,
        ));
    }
    Ok(index)
}

fn bound_sides(value: bindings::HighsInt) -> Vec<BoundSide> {
    match value {
        bindings::kHighsIisBoundLower => vec![BoundSide::Lower],
        bindings::kHighsIisBoundUpper => vec![BoundSide::Upper],
        bindings::kHighsIisBoundBoxed => vec![BoundSide::Lower, BoundSide::Upper],
        _ => Vec::new(),
    }
}

fn native_bound(value: bindings::HighsInt) -> NativeBoundStatus {
    match value {
        bindings::kHighsIisBoundFree => NativeBoundStatus::Free,
        bindings::kHighsIisBoundLower => NativeBoundStatus::Lower,
        bindings::kHighsIisBoundUpper => NativeBoundStatus::Upper,
        bindings::kHighsIisBoundBoxed => NativeBoundStatus::Boxed,
        other => NativeBoundStatus::Unknown(other),
    }
}

fn native_membership(value: bindings::HighsInt) -> NativeMembership {
    match value {
        bindings::kHighsIisStatusNotInConflict => NativeMembership::Excluded,
        bindings::kHighsIisStatusMaybeInConflict => NativeMembership::Possible,
        bindings::kHighsIisStatusInConflict => NativeMembership::Member,
        other => NativeMembership::Unknown(other),
    }
}

fn unsupported(message: impl Into<String>) -> BackendError {
    BackendError::new(
        message,
        ErrorCategory::Unsupported,
        HealthEffect::Recoverable,
    )
}
