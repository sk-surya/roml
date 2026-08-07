//! Isolated LP feasibility oracle for HiGHS.

use roml::advanced::{
    BackendSnapshot, CompilationId, CompiledConstraintId, CompiledRestrictionRef,
    CompiledVariableId, SemanticConflictUniverse, SemanticRestrictionAtom,
};
use roml::solver::backend::{BackendError, ErrorCategory, HealthEffect, TerminationStatus};
use roml::solver::infeasibility::{
    FeasibilityEvidence, FeasibilityOracle, FeasibilityOutcome, InfeasibilityEvidence,
    OracleBudget, RestrictionSelection, UnknownReason,
};
use roml::solver::session::{BackendSession, Synchronization};
use std::collections::{HashMap, HashSet};
use std::ffi::CString;

use crate::bindings;
use crate::error::from_native_status;
use crate::lifecycle::HighsSession;

/// Spawn a fresh HiGHS instance and establish the exact analysis snapshot.
pub(crate) fn spawn_oracle(
    _persistent: &HighsSession,
    snapshot: &BackendSnapshot,
    universe: &SemanticConflictUniverse,
) -> Result<Box<dyn FeasibilityOracle>, BackendError> {
    let mut session = HighsSession::try_new()?;
    session.synchronize(Synchronization::CompiledRebuild(snapshot.clone()))?;
    neutralize_objective(&mut session)?;
    Ok(Box::new(HighsAnalysisOracle::new(
        session, snapshot, universe,
    )))
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
                let value = atom
                    .restriction_values
                    .iter()
                    .find(|(member, _)| member == restriction)
                    .and_then(|(_, value)| *value)
                    .or(atom.snapshot.value);
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
        if let Some(tolerance) = budget.feasibility_tolerance {
            let option = CString::new("primal_feasibility_tolerance")
                .expect("static option name has no NUL");
            let status = unsafe {
                bindings::Highs_setDoubleOptionValue(self.session.raw, option.as_ptr(), tolerance)
            };
            if status != bindings::STATUS_OK {
                return Err(from_native_status(
                    status,
                    "Highs_setDoubleOptionValue(primal_feasibility_tolerance)",
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
            bindings::kHighsModelStatusTimeLimit => {
                FeasibilityOutcome::Unknown(UnknownReason::TimeLimit)
            }
            bindings::kHighsModelStatusIterationLimit => {
                FeasibilityOutcome::Unknown(UnknownReason::IterationLimit)
            }
            _ => FeasibilityOutcome::Unknown(UnknownReason::Unclassified),
        })
    }
}

fn neutralize_objective(session: &mut HighsSession) -> Result<(), BackendError> {
    for (_, index) in session.col_map.iter() {
        let status =
            unsafe { bindings::Highs_changeColCost(session.raw, index as bindings::HighsInt, 0.0) };
        if status != bindings::STATUS_OK {
            return Err(from_native_status(
                status,
                "Highs_changeColCost(feasibility)",
            ));
        }
    }
    Ok(())
}
