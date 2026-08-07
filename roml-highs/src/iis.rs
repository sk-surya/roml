//! Audited HiGHS 1.15.0 IIS provider and isolated LP oracle.
//!
//! This module is compiled only for the bundled pinned provider. The system
//! discovery feature deliberately uses the default typed-unsupported methods
//! until a per-version header/library qualification matrix exists.

use std::collections::{HashMap, HashSet};
use std::ffi::CString;

use roml::advanced::{
    BackendSnapshot, CompilationId, CompiledConstraintId, CompiledRestrictionRef,
    CompiledVariableId, SemanticConflictUniverse, SemanticRestrictionAtom,
};
use roml::solver::backend::{BackendError, ErrorCategory, HealthEffect, TerminationStatus};
use roml::solver::infeasibility::{
    BoundSide, FeasibilityEvidence, FeasibilityOracle, FeasibilityOutcome, InfeasibilityEvidence,
    NativeBoundStatus, NativeConflict, NativeConflictMember, NativeConflictRequest,
    NativeMembership, OracleBudget, RestrictionSelection, UnknownReason,
};
use roml::solver::session::{BackendSession, Synchronization};

use crate::bindings;
use crate::error::from_native_status;
use crate::lifecycle::HighsSession;

const QUALIFIED_MAJOR: i32 = 1;
const QUALIFIED_MINOR: i32 = 15;
const QUALIFIED_PATCH: i32 = 0;

/// Return whether a session was built and loaded against the audited version.
pub(crate) fn is_qualified(session: &HighsSession) -> bool {
    session.version_major == QUALIFIED_MAJOR
        && session.version_minor == QUALIFIED_MINOR
        && session.version_patch == QUALIFIED_PATCH
}

/// Spawn a fresh HiGHS instance and establish the exact analysis snapshot.
pub(crate) fn spawn_oracle(
    _persistent: &HighsSession,
    snapshot: &BackendSnapshot,
    universe: &SemanticConflictUniverse,
) -> Result<Box<dyn FeasibilityOracle>, BackendError> {
    let mut session = HighsSession::try_new()?;
    if !is_qualified(&session) {
        return Err(unsupported("bundled HiGHS runtime is not exactly 1.15.0"));
    }
    session.synchronize(Synchronization::CompiledRebuild(snapshot.clone()))?;
    Ok(Box::new(HighsAnalysisOracle::new(
        session, snapshot, universe,
    )))
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

    // This is the audited C API option name and generated strategy constant.
    // It is applied only to the isolated native instance.
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

struct HighsAnalysisOracle {
    session: HighsSession,
    snapshot: BackendSnapshot,
    atom_ids: HashSet<roml::ConflictAtomId>,
    selected: HashSet<roml::ConflictAtomId>,
    atoms: Vec<SemanticRestrictionAtom>,
    current_rows: HashMap<CompiledConstraintId, (f64, f64)>,
    current_columns: HashMap<CompiledVariableId, (f64, f64)>,
}

impl HighsAnalysisOracle {
    fn new(
        session: HighsSession,
        snapshot: &BackendSnapshot,
        universe: &SemanticConflictUniverse,
    ) -> Self {
        Self {
            session,
            snapshot: snapshot.clone(),
            atom_ids: universe.atoms.iter().map(|atom| atom.id).collect(),
            selected: universe.atoms.iter().map(|atom| atom.id).collect(),
            atoms: universe.atoms.clone(),
            current_rows: snapshot
                .linear_rows
                .iter()
                .map(|row| (row.id, (row.bounds.lower, row.bounds.upper)))
                .collect(),
            current_columns: snapshot
                .variables
                .iter()
                .map(|variable| (variable.id, (variable.bounds.lower, variable.bounds.upper)))
                .collect(),
        }
    }

    fn apply_selection(&mut self, selection: &RestrictionSelection) -> Result<(), BackendError> {
        let requested: HashSet<_> = selection.atom_ids.iter().copied().collect();
        if requested.iter().any(|id| !self.atom_ids.contains(id)) {
            return Err(BackendError::new(
                "analysis selection contains an atom outside the exact universe",
                ErrorCategory::InvalidInput,
                HealthEffect::Recoverable,
            ));
        }

        // Compute side activity from the immutable exact snapshot, then make
        // only the necessary bound changes. The native session is never
        // rebuilt for candidate checks; restrictions are toggled incrementally.
        let mut row_bounds: HashMap<CompiledConstraintId, (f64, f64)> = self
            .snapshot
            .linear_rows
            .iter()
            .map(|row| (row.id, (f64::NEG_INFINITY, f64::INFINITY)))
            .collect();
        let mut column_bounds: HashMap<CompiledVariableId, (f64, f64)> = self
            .snapshot
            .variables
            .iter()
            .map(|variable| (variable.id, (f64::NEG_INFINITY, f64::INFINITY)))
            .collect();

        for atom in &self.atoms {
            let active = requested.contains(&atom.id);
            if !active {
                continue;
            }
            for restriction in &atom.compiled_restrictions {
                let value = atom.snapshot.value;
                match restriction {
                    CompiledRestrictionRef::ConstraintLower(id) => {
                        if let Some(bounds) = row_bounds.get_mut(id) {
                            bounds.0 = value.unwrap_or_else(|| {
                                self.snapshot.linear_rows[id.0 as usize].bounds.lower
                            });
                        }
                    }
                    CompiledRestrictionRef::ConstraintUpper(id) => {
                        if let Some(bounds) = row_bounds.get_mut(id) {
                            bounds.1 = value.unwrap_or_else(|| {
                                self.snapshot.linear_rows[id.0 as usize].bounds.upper
                            });
                        }
                    }
                    CompiledRestrictionRef::VariableLower(id) => {
                        if let Some(bounds) = column_bounds.get_mut(id) {
                            bounds.0 = value.unwrap_or_else(|| {
                                self.snapshot.variables[id.0 as usize].bounds.lower
                            });
                        }
                    }
                    CompiledRestrictionRef::VariableUpper(id) => {
                        if let Some(bounds) = column_bounds.get_mut(id) {
                            bounds.1 = value.unwrap_or_else(|| {
                                self.snapshot.variables[id.0 as usize].bounds.upper
                            });
                        }
                    }
                    CompiledRestrictionRef::Entity(_) => {}
                }
            }
        }

        for row in &self.snapshot.linear_rows {
            let desired = row_bounds[&row.id];
            if self.current_rows[&row.id] != desired {
                let status = unsafe {
                    bindings::Highs_changeRowBounds(
                        self.session.raw,
                        row.id.0 as bindings::HighsInt,
                        desired.0,
                        desired.1,
                    )
                };
                if status != bindings::STATUS_OK {
                    return Err(from_native_status(
                        status,
                        "Highs_changeRowBounds(IIS toggle)",
                    ));
                }
                self.current_rows.insert(row.id, desired);
            }
        }
        for variable in &self.snapshot.variables {
            let desired = column_bounds[&variable.id];
            if self.current_columns[&variable.id] != desired {
                let status = unsafe {
                    bindings::Highs_changeColBounds(
                        self.session.raw,
                        variable.id.0 as bindings::HighsInt,
                        desired.0,
                        desired.1,
                    )
                };
                if status != bindings::STATUS_OK {
                    return Err(from_native_status(
                        status,
                        "Highs_changeColBounds(IIS toggle)",
                    ));
                }
                self.current_columns.insert(variable.id, desired);
            }
        }
        self.selected = requested;
        Ok(())
    }
}

impl FeasibilityOracle for HighsAnalysisOracle {
    fn compilation_id(&self) -> CompilationId {
        self.snapshot.compilation_id
    }

    fn check(
        &mut self,
        selection: &RestrictionSelection,
        budget: &OracleBudget,
    ) -> Result<FeasibilityOutcome, BackendError> {
        if selection.compilation_id != self.snapshot.compilation_id {
            return Err(BackendError::new(
                "analysis selection has a stale CompilationId",
                ErrorCategory::InvalidInput,
                HealthEffect::Recoverable,
            ));
        }
        self.apply_selection(selection)?;
        if let Some(milliseconds) = budget.time_limit_ms {
            let option = CString::new("time_limit").expect("static option name has no NUL");
            let status = unsafe {
                bindings::Highs_setDoubleOptionValue(
                    self.session.raw,
                    option.as_ptr(),
                    milliseconds as f64 / 1000.0,
                )
            };
            if status != bindings::STATUS_OK {
                return Err(from_native_status(
                    status,
                    "Highs_setDoubleOptionValue(time_limit)",
                ));
            }
        }
        let run_status = unsafe { bindings::Highs_run(self.session.raw) };
        if run_status < 0 {
            return Err(from_native_status(run_status, "Highs_run(IIS oracle)"));
        }
        let model_status = unsafe { bindings::Highs_getModelStatus(self.session.raw) };
        Ok(match model_status {
            bindings::kHighsModelStatusOptimal => {
                FeasibilityOutcome::ProvenFeasible(FeasibilityEvidence {
                    termination: TerminationStatus::Optimal,
                })
            }
            bindings::kHighsModelStatusInfeasible => {
                FeasibilityOutcome::ProvenInfeasible(InfeasibilityEvidence {
                    termination: TerminationStatus::Infeasible,
                })
            }
            bindings::kHighsModelStatusUnboundedOrInfeasible => {
                FeasibilityOutcome::Unknown(UnknownReason::Ambiguous)
            }
            bindings::kHighsModelStatusUnbounded => {
                FeasibilityOutcome::Unknown(UnknownReason::Unbounded)
            }
            bindings::kHighsModelStatusTimeLimit | bindings::kHighsModelStatusIterationLimit => {
                FeasibilityOutcome::Unknown(UnknownReason::Limit)
            }
            _ => FeasibilityOutcome::Unknown(UnknownReason::Unclassified),
        })
    }
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
