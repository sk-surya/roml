//! RED characterization for the isolated tri-state feasibility oracle seam.

use std::cell::Cell;

use roml::advanced::{
    classify_feasibility, AnalysisNumericalPolicy, AnalysisSession, BackendError,
    BackendSnapshotBuilder, CompiledConstraintId, CompiledLinearRow, CompiledVariable,
    CompiledVariableId, ConflictGrouping, EntityOrigin, FeasibilityEvidence, FeasibilityOracle,
    FeasibilityOutcome, InfeasibilityEvidence, InfeasibilityScope, OracleBudget, OriginMap,
    RestrictionSelection, SemanticConflictUniverse, TerminationStatus,
};
use roml::{Bounds, ConstraintBounds, Model, VarId, VarType};

fn snapshot() -> roml::advanced::BackendSnapshot {
    let model = Model::new();
    let mut origins = OriginMap::new();
    origins.insert_variable(
        CompiledVariableId(0),
        EntityOrigin::UserVariable(VarId::new(0, roml::id::Generation::new())),
    );
    origins.insert_constraint(
        CompiledConstraintId(0),
        EntityOrigin::UserConstraint(roml::ConId::new(0, roml::id::Generation::new())),
    );
    BackendSnapshotBuilder::new(model.instance(), model.current_revision())
        .origin_map(origins)
        .add_variable(CompiledVariable {
            id: CompiledVariableId(0),
            bounds: Bounds::new(0.0, 5.0),
            var_type: VarType::Continuous,
            name: None,
        })
        .add_linear_row(CompiledLinearRow {
            id: CompiledConstraintId(0),
            bounds: ConstraintBounds::le(4.0),
            coefficients: vec![(CompiledVariableId(0), 1.0)],
            name: None,
        })
        .finalize()
        .unwrap()
}

struct ScriptedOracle {
    compilation_id: roml::advanced::CompilationId,
    calls: Cell<u64>,
    outcome: FeasibilityOutcome,
}

struct FailingOracle {
    compilation_id: roml::advanced::CompilationId,
}

impl FeasibilityOracle for FailingOracle {
    fn compilation_id(&self) -> roml::advanced::CompilationId {
        self.compilation_id
    }

    fn check(
        &mut self,
        _selection: &RestrictionSelection,
        _budget: &OracleBudget,
    ) -> Result<FeasibilityOutcome, BackendError> {
        Err(BackendError::unsupported("injected oracle failure"))
    }
}

impl FeasibilityOracle for ScriptedOracle {
    fn compilation_id(&self) -> roml::advanced::CompilationId {
        self.compilation_id
    }

    fn check(
        &mut self,
        _selection: &RestrictionSelection,
        _budget: &OracleBudget,
    ) -> Result<FeasibilityOutcome, BackendError> {
        self.calls.set(self.calls.get() + 1);
        Ok(self.outcome.clone())
    }
}

#[test]
fn isolated_oracle_caches_only_exact_tri_state_results() {
    let source_snapshot = snapshot();
    let universe = SemanticConflictUniverse::from_snapshot(
        &source_snapshot,
        InfeasibilityScope::OriginalLp,
        ConflictGrouping::Individual,
    )
    .unwrap();
    let oracle = ScriptedOracle {
        compilation_id: source_snapshot.compilation_id,
        calls: Cell::new(0),
        outcome: FeasibilityOutcome::ProvenInfeasible(InfeasibilityEvidence {
            termination: TerminationStatus::Infeasible,
        }),
    };
    let mut session =
        AnalysisSession::new(oracle, &universe, AnalysisNumericalPolicy::default()).unwrap();
    let selection = RestrictionSelection::all(&universe);
    let first = session.check(&selection, &OracleBudget::default()).unwrap();
    let second = session.check(&selection, &OracleBudget::default()).unwrap();
    assert!(matches!(first, FeasibilityOutcome::ProvenInfeasible(_)));
    assert_eq!(first, second);
    assert_eq!(session.oracle_calls(), 1);
}

#[test]
fn analysis_session_rejects_stale_selection_before_oracle_call() {
    let source_snapshot = snapshot();
    let universe = SemanticConflictUniverse::from_snapshot(
        &source_snapshot,
        InfeasibilityScope::OriginalLp,
        ConflictGrouping::Individual,
    )
    .unwrap();
    let oracle = ScriptedOracle {
        compilation_id: source_snapshot.compilation_id,
        calls: Cell::new(0),
        outcome: FeasibilityOutcome::ProvenFeasible(FeasibilityEvidence {
            termination: TerminationStatus::Optimal,
        }),
    };
    let mut session =
        AnalysisSession::new(oracle, &universe, AnalysisNumericalPolicy::default()).unwrap();
    let stale_snapshot = snapshot();
    let stale = RestrictionSelection {
        compilation_id: stale_snapshot.compilation_id,
        atom_ids: vec![],
    };
    assert!(session.check(&stale, &OracleBudget::default()).is_err());
    assert_eq!(session.oracle_calls(), 0);
}

#[test]
fn failed_oracle_check_requires_isolated_session_rebuild() {
    let source_snapshot = snapshot();
    let universe = SemanticConflictUniverse::from_snapshot(
        &source_snapshot,
        InfeasibilityScope::OriginalLp,
        ConflictGrouping::Individual,
    )
    .unwrap();
    let mut session = AnalysisSession::new(
        FailingOracle {
            compilation_id: source_snapshot.compilation_id,
        },
        &universe,
        AnalysisNumericalPolicy::default(),
    )
    .unwrap();
    let selection = RestrictionSelection::all(&universe);
    assert!(session.check(&selection, &OracleBudget::default()).is_err());
    assert_eq!(
        session.health(),
        roml::advanced::AnalysisSessionHealth::RequiresRebuild
    );
    assert!(session.check(&selection, &OracleBudget::default()).is_err());
}

#[test]
fn limit_and_uncertainty_statuses_remain_distinct_unknown_reasons() {
    assert!(matches!(
        classify_feasibility(TerminationStatus::TimeLimit),
        FeasibilityOutcome::Unknown(roml::advanced::UnknownReason::TimeLimit)
    ));
    assert!(matches!(
        classify_feasibility(TerminationStatus::IterationLimit),
        FeasibilityOutcome::Unknown(roml::advanced::UnknownReason::IterationLimit)
    ));
    assert!(matches!(
        classify_feasibility(TerminationStatus::NumericalIssue),
        FeasibilityOutcome::Unknown(roml::advanced::UnknownReason::Numerical)
    ));
    assert!(!matches!(
        classify_feasibility(TerminationStatus::TimeLimit),
        FeasibilityOutcome::ProvenInfeasible(_)
    ));
}
