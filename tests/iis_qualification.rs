//! Deterministic core qualification corpus for Phase 29.

use std::cell::Cell;

use roml::advanced::{
    AnalysisNumericalPolicy, AnalysisSession, BackendError, BackendSnapshotBuilder,
    CompiledConstraintId, CompiledLinearRow, CompiledVariable, CompiledVariableId,
    ConflictGrouping, EntityOrigin, FeasibilityEvidence, FeasibilityOracle, FeasibilityOutcome,
    InfeasibilityEvidence, InfeasibilityScope, OracleBudget, OriginMap, RestrictionSelection,
    SemanticConflictUniverse, TerminationStatus,
};
use roml::{Bounds, ConstraintBounds, Model, VarId, VarType};

fn fixture() -> roml::advanced::BackendSnapshot {
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

struct Oracle {
    compilation_id: roml::advanced::CompilationId,
    calls: Cell<u64>,
}

impl FeasibilityOracle for Oracle {
    fn compilation_id(&self) -> roml::advanced::CompilationId {
        self.compilation_id
    }

    fn check(
        &mut self,
        selection: &RestrictionSelection,
        _budget: &OracleBudget,
    ) -> Result<FeasibilityOutcome, BackendError> {
        self.calls.set(self.calls.get() + 1);
        if selection.atom_ids.len() >= 2 {
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

#[test]
fn bounded_reduction_is_explicitly_limited_and_not_irreducible() {
    let snapshot = fixture();
    let universe = SemanticConflictUniverse::from_snapshot(
        &snapshot,
        InfeasibilityScope::OriginalLp,
        ConflictGrouping::Individual,
    )
    .unwrap();
    let mut session = AnalysisSession::new(
        Oracle {
            compilation_id: snapshot.compilation_id,
            calls: Cell::new(0),
        },
        &universe,
        AnalysisNumericalPolicy::default(),
    )
    .unwrap();
    let result = roml::solver::reducer::reduce_with_limits(
        &mut session,
        &universe,
        RestrictionSelection::all(&universe),
        OracleBudget::default(),
        Some(2),
        Some(1),
    )
    .unwrap();
    let roml::advanced::ReductionOutcome::Conflict(conflict) = result else {
        panic!("the initial selection is proven infeasible");
    };
    assert!(conflict.statistics.budget_exhausted);
    assert_ne!(conflict.guarantee, roml::ConflictGuarantee::Irreducible);
}
