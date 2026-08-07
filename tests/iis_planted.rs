//! Deterministic planted-conflict scaling evidence.

use std::cell::Cell;

use roml::advanced::{
    AnalysisNumericalPolicy, AnalysisSession, BackendError, BackendSnapshotBuilder,
    CompiledConstraintId, CompiledLinearRow, CompiledVariable, CompiledVariableId,
    ConflictGrouping, EntityOrigin, FeasibilityEvidence, FeasibilityOracle, FeasibilityOutcome,
    InfeasibilityEvidence, InfeasibilityScope, OracleBudget, OriginMap, RestrictionSelection,
    SemanticConflictUniverse, TerminationStatus,
};
use roml::{Bounds, ConstraintBounds, Model, VarId, VarType};

struct PlantedOracle {
    id: roml::advanced::CompilationId,
    calls: Cell<u64>,
}

impl FeasibilityOracle for PlantedOracle {
    fn compilation_id(&self) -> roml::advanced::CompilationId {
        self.id
    }
    fn check(
        &mut self,
        selection: &RestrictionSelection,
        _budget: &OracleBudget,
    ) -> Result<FeasibilityOutcome, BackendError> {
        self.calls.set(self.calls.get() + 1);
        let planted = selection.atom_ids.contains(&roml::ConflictAtomId(0))
            && selection.atom_ids.contains(&roml::ConflictAtomId(1));
        Ok(if planted {
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

#[test]
fn adaptive_reduction_removes_irrelevant_planted_atoms() {
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
    let universe = SemanticConflictUniverse::from_snapshot(
        &snapshot,
        InfeasibilityScope::OriginalLp,
        ConflictGrouping::Individual,
    )
    .unwrap();
    let mut session = AnalysisSession::new(
        PlantedOracle {
            id: universe.compilation_id,
            calls: Cell::new(0),
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
    let roml::advanced::ReductionOutcome::Conflict(conflict) = result else {
        panic!("planted conflict must be found");
    };
    assert_eq!(
        conflict.members,
        vec![roml::ConflictAtomId(0), roml::ConflictAtomId(1)]
    );
    assert!(conflict.statistics.oracle_calls < 20);
}
