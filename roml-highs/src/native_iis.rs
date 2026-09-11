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
    native_conflict_with_session(&mut session, request)
}

/// The session-taking half of [`native_conflict`], split out so the
/// qualification and identity guards are testable without a second native
/// instance.
fn native_conflict_with_session(
    session: &mut HighsSession,
    request: &NativeConflictRequest,
) -> Result<NativeConflict, BackendError> {
    if !is_qualified(session) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use roml::advanced::{
        BackendCapabilitySet, BackendFeature, CompilationSession, FeatureSupport, SupportLevel,
    };
    use roml::compiler::capability::CompilationPolicy;
    use roml::Model;

    fn empty_snapshot() -> BackendSnapshot {
        let model = Model::new();
        let model_snapshot = model.take_snapshot().expect("snapshot");
        let mut caps = BackendCapabilitySet::new();
        caps.set(
            BackendFeature::Lp,
            FeatureSupport {
                level: SupportLevel::Native,
                limitations: Default::default(),
            },
        );
        CompilationSession::new()
            .compile_snapshot(
                model.instance(),
                &model_snapshot,
                &CompilationPolicy::Auto,
                &caps,
            )
            .expect("empty base")
    }

    #[test]
    fn pure_native_mappings_cover_all_constants_and_unknowns() {
        assert_eq!(
            bound_sides(bindings::kHighsIisBoundLower),
            vec![BoundSide::Lower]
        );
        assert_eq!(
            bound_sides(bindings::kHighsIisBoundUpper),
            vec![BoundSide::Upper]
        );
        assert_eq!(
            bound_sides(bindings::kHighsIisBoundBoxed),
            vec![BoundSide::Lower, BoundSide::Upper]
        );
        assert!(bound_sides(-999).is_empty());

        assert_eq!(
            native_bound(bindings::kHighsIisBoundFree),
            NativeBoundStatus::Free
        );
        assert_eq!(
            native_bound(bindings::kHighsIisBoundLower),
            NativeBoundStatus::Lower
        );
        assert_eq!(
            native_bound(bindings::kHighsIisBoundUpper),
            NativeBoundStatus::Upper
        );
        assert_eq!(
            native_bound(bindings::kHighsIisBoundBoxed),
            NativeBoundStatus::Boxed
        );
        assert_eq!(native_bound(424242), NativeBoundStatus::Unknown(424242));

        assert_eq!(
            native_membership(bindings::kHighsIisStatusNotInConflict),
            NativeMembership::Excluded
        );
        assert_eq!(
            native_membership(bindings::kHighsIisStatusMaybeInConflict),
            NativeMembership::Possible
        );
        assert_eq!(
            native_membership(bindings::kHighsIisStatusInConflict),
            NativeMembership::Member
        );
        assert_eq!(native_membership(424242), NativeMembership::Unknown(424242));
    }

    #[test]
    fn checked_count_and_index_reject_negative_and_out_of_range() {
        assert_eq!(checked_count(3, "n").expect("positive"), 3);
        assert!(checked_count(-1, "n").is_err());
        assert_eq!(checked_index(1, 3, "i").expect("in range"), 1);
        assert!(checked_index(3, 3, "i").is_err());
        assert!(checked_index(-1, 3, "i").is_err());
    }

    #[test]
    fn session_qualification_and_request_identity_guards() {
        let snapshot = empty_snapshot();
        let request = NativeConflictRequest {
            compilation_id: snapshot.compilation_id,
            snapshot: snapshot.clone(),
        };

        // Unqualified version rejects before touching the request snapshot.
        let mut unqualified = HighsSession::try_new().expect("bundled highs");
        unqualified.version_major = QUALIFIED_MAJOR + 1;
        assert!(!is_qualified(&unqualified));
        let error = native_conflict_with_session(&mut unqualified, &request)
            .expect_err("unqualified runtime rejects");
        assert!(format!("{error}").contains("not exactly"));

        // Qualified version + mismatched compilation identity rejects.
        let mut qualified = HighsSession::try_new().expect("bundled highs");
        assert!(is_qualified(&qualified));
        let mut mismatched = request.clone();
        // A second compile allocates a distinct exact compilation id.
        mismatched.compilation_id = empty_snapshot().compilation_id;
        let error = native_conflict_with_session(&mut qualified, &mismatched)
            .expect_err("mismatched identity rejects");
        assert!(format!("{error}").contains("identity"));
    }

    #[test]
    fn unsupported_helper_is_typed_unsupported() {
        let error = unsupported("nope");
        assert_eq!(error.category, ErrorCategory::Unsupported);
    }
}
