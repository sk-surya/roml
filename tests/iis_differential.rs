//! Portable/native-seed differential invariants.

use roml::advanced::{
    AnalysisNumericalPolicy, AnalysisSession, BackendError, BackendSnapshotBuilder,
    CompiledConstraintId, CompiledLinearRow, CompiledVariable, CompiledVariableId,
    ConflictGrouping, EntityOrigin, FeasibilityEvidence, FeasibilityOracle, FeasibilityOutcome,
    InfeasibilityEvidence, InfeasibilityScope, OracleBudget, OriginMap, RestrictionSelection,
    SemanticConflictUniverse, TerminationStatus,
};
use roml::{Bounds, ConstraintBounds, Model, VarId, VarType};

struct PairOracle {
    id: roml::advanced::CompilationId,
}

impl FeasibilityOracle for PairOracle {
    fn compilation_id(&self) -> roml::advanced::CompilationId {
        self.id
    }
    fn check(
        &mut self,
        selection: &RestrictionSelection,
        _budget: &OracleBudget,
    ) -> Result<FeasibilityOutcome, BackendError> {
        let pair = selection.atom_ids.contains(&roml::ConflictAtomId(0))
            && selection.atom_ids.contains(&roml::ConflictAtomId(1));
        Ok(if pair {
            FeasibilityOutcome::ProvenInfeasible(InfeasibilityEvidence {
                termination: TerminationStatus::Infeasible,
            })
        } else {
            FeasibilityOutcome::ProvenFeasible(FeasibilityEvidence {
                termination: TerminationStatus::Optimal,
            })
        })
    }
}

fn universe() -> SemanticConflictUniverse {
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
    let snapshot = BackendSnapshotBuilder::new(model.instance(), model.current_revision())
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
        .unwrap();
    SemanticConflictUniverse::from_snapshot(
        &snapshot,
        InfeasibilityScope::OriginalLp,
        ConflictGrouping::Individual,
    )
    .unwrap()
}

#[test]
fn different_valid_seeds_are_compared_by_semantic_validity() {
    let universe = universe();
    let seed = RestrictionSelection {
        compilation_id: universe.compilation_id,
        atom_ids: vec![roml::ConflictAtomId(0), roml::ConflictAtomId(1)],
    };
    let mut session = AnalysisSession::new(
        PairOracle {
            id: universe.compilation_id,
        },
        &universe,
        AnalysisNumericalPolicy::default(),
    )
    .unwrap();
    let result = roml::solver::reducer::reduce(&mut session, &universe, seed).unwrap();
    let roml::advanced::ReductionOutcome::Conflict(conflict) = result else {
        panic!("pair seed must be a conflict");
    };
    assert_eq!(conflict.members.len(), 2);
    assert_eq!(conflict.guarantee, roml::ConflictGuarantee::Irreducible);
}
