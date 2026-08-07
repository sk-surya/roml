//! RED characterization for portable semantic conflict reduction.

use std::cell::Cell;

use roml::advanced::{
    AnalysisNumericalPolicy, AnalysisSession, BackendError, BackendSnapshotBuilder,
    CompiledConstraintId, CompiledLinearRow, CompiledVariable, CompiledVariableId,
    ConflictGrouping, EntityOrigin, FeasibilityEvidence, FeasibilityOracle, FeasibilityOutcome,
    InfeasibilityEvidence, InfeasibilityScope, OracleBudget, OriginMap, ReductionOutcome,
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
            bounds: ConstraintBounds::range(2.0, 4.0),
            coefficients: vec![(CompiledVariableId(0), 1.0)],
            name: None,
        })
        .finalize()
        .unwrap()
}

struct PairOracle {
    compilation_id: roml::advanced::CompilationId,
    calls: Cell<u64>,
    unknown: bool,
}

impl FeasibilityOracle for PairOracle {
    fn compilation_id(&self) -> roml::advanced::CompilationId {
        self.compilation_id
    }

    fn check(
        &mut self,
        selection: &RestrictionSelection,
        _budget: &OracleBudget,
    ) -> Result<FeasibilityOutcome, BackendError> {
        self.calls.set(self.calls.get() + 1);
        if self.unknown {
            return Ok(FeasibilityOutcome::Unknown(
                roml::advanced::UnknownReason::Numerical,
            ));
        }
        if selection.atom_ids.contains(&roml::ConflictAtomId(0))
            && selection.atom_ids.contains(&roml::ConflictAtomId(1))
        {
            Ok(FeasibilityOutcome::ProvenInfeasible(
                InfeasibilityEvidence {
                    termination: TerminationStatus::Infeasible,
                },
            ))
        } else {
            Ok(FeasibilityOutcome::ProvenFeasible(FeasibilityEvidence {
                termination: TerminationStatus::Optimal,
            }))
        }
    }
}

fn make_session(unknown: bool) -> (AnalysisSession<PairOracle>, SemanticConflictUniverse) {
    let source = snapshot();
    let universe = SemanticConflictUniverse::from_snapshot(
        &source,
        InfeasibilityScope::OriginalLp,
        ConflictGrouping::Individual,
    )
    .unwrap();
    let session = AnalysisSession::new(
        PairOracle {
            compilation_id: source.compilation_id,
            calls: Cell::new(0),
            unknown,
        },
        &universe,
        AnalysisNumericalPolicy::default(),
    )
    .unwrap();
    (session, universe)
}

#[test]
fn reducer_polishes_irrelevant_atoms_and_freshly_verifies_every_deletion() {
    let (mut session, universe) = make_session(false);
    let result = roml::solver::reducer::reduce(
        &mut session,
        &universe,
        RestrictionSelection::all(&universe),
    )
    .unwrap();
    let ReductionOutcome::Conflict(conflict) = result else {
        panic!("pair conflict should be reduced");
    };
    assert_eq!(
        conflict.members,
        vec![roml::ConflictAtomId(0), roml::ConflictAtomId(1)]
    );
    assert_eq!(
        conflict.guarantee,
        roml::advanced::ConflictGuarantee::Irreducible
    );
    assert!(conflict.statistics.fresh_verification_checks >= 3);
}

#[test]
fn unknown_initial_check_never_becomes_a_conflict_claim() {
    let (mut session, universe) = make_session(true);
    let result = roml::solver::reducer::reduce(
        &mut session,
        &universe,
        RestrictionSelection::all(&universe),
    )
    .unwrap();
    assert_eq!(result, ReductionOutcome::NoConflictProof);
}
