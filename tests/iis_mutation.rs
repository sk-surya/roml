//! Mutation-resistance checks for Phase 29 safety claims.

use roml::advanced::{
    AnalysisNumericalPolicy, AnalysisSession, BackendError, FeasibilityEvidence, FeasibilityOracle,
    FeasibilityOutcome, InfeasibilityEvidence, OracleBudget, RestrictionSelection,
    SemanticConflictUniverse, UnknownReason,
};

struct UnknownOracle {
    id: roml::advanced::CompilationId,
}

impl FeasibilityOracle for UnknownOracle {
    fn compilation_id(&self) -> roml::advanced::CompilationId {
        self.id
    }
    fn check(
        &mut self,
        _selection: &RestrictionSelection,
        _budget: &OracleBudget,
    ) -> Result<FeasibilityOutcome, BackendError> {
        Ok(FeasibilityOutcome::Unknown(UnknownReason::Numerical))
    }
}

#[test]
fn unknown_never_promotes_to_infeasible_or_irreducible() {
    let model = roml::Model::new();
    let snapshot =
        roml::advanced::BackendSnapshotBuilder::new(model.instance(), model.current_revision())
            .finalize()
            .unwrap();
    let universe = SemanticConflictUniverse::from_snapshot(
        &snapshot,
        roml::InfeasibilityScope::OriginalLp,
        roml::advanced::ConflictGrouping::Individual,
    )
    .unwrap();
    let mut session = AnalysisSession::new(
        UnknownOracle {
            id: universe.compilation_id,
        },
        &universe,
        AnalysisNumericalPolicy::default(),
    )
    .unwrap();
    let result = roml::solver::reducer::reduce(
        &mut session,
        &universe,
        RestrictionSelection::all(&universe),
    )
    .unwrap();
    assert_eq!(result, roml::advanced::ReductionOutcome::NoConflictProof);
    let _ = (
        FeasibilityEvidence {
            termination: roml::TerminationStatus::Optimal,
        },
        InfeasibilityEvidence {
            termination: roml::TerminationStatus::Infeasible,
        },
    );
}
