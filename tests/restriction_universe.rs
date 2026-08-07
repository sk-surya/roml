//! RED characterization for the semantic restriction universe.

use roml::advanced::{
    BackendSnapshotBuilder, BoundContributionStack, BoundLayerSource, CompiledConstraintId,
    CompiledLinearRow, CompiledVariable, CompiledVariableId, ConflictGrouping, EntityOrigin,
    OriginMap, RestrictionOriginMap, SemanticConflictUniverse,
};
use roml::{Bounds, ConstraintBounds, InfeasibilityScope, Model, VarId, VarType};

fn ranged_snapshot() -> roml::advanced::BackendSnapshot {
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
            bounds: Bounds::new(1.0, 5.0),
            var_type: VarType::Continuous,
            name: Some("x".to_string()),
        })
        .add_linear_row(CompiledLinearRow {
            id: CompiledConstraintId(0),
            bounds: ConstraintBounds::range(2.0, 4.0),
            coefficients: vec![(CompiledVariableId(0), 1.0)],
            name: Some("row".to_string()),
        })
        .finalize()
        .expect("origin-complete fixture")
}

#[test]
fn ranged_rows_and_variable_bounds_become_side_atoms_in_order() {
    let snapshot = ranged_snapshot();
    let universe = SemanticConflictUniverse::from_snapshot(
        &snapshot,
        InfeasibilityScope::OriginalLp,
        ConflictGrouping::Individual,
    )
    .expect("universe construction");

    assert_eq!(universe.compilation_id, snapshot.compilation_id);
    assert_eq!(universe.atoms.len(), 4);
}

#[test]
fn stale_compilation_identity_is_rejected_before_mapping() {
    let snapshot = ranged_snapshot();
    let universe = SemanticConflictUniverse::from_snapshot(
        &snapshot,
        InfeasibilityScope::OriginalLp,
        ConflictGrouping::Individual,
    )
    .unwrap();
    let other_snapshot = ranged_snapshot();
    let other = SemanticConflictUniverse::from_snapshot(
        &other_snapshot,
        InfeasibilityScope::OriginalLp,
        ConflictGrouping::Individual,
    )
    .unwrap();
    assert_ne!(universe.compilation_id, other.compilation_id);

    let map = RestrictionOriginMap::new(&universe).unwrap();
    let member = universe.compiled_restrictions[0];
    assert!(map.map_compiled(other.compilation_id, member).is_err());
}

#[test]
fn disabling_a_bound_layer_restores_its_predecessor() {
    let mut stack = BoundContributionStack::new(Bounds::new(0.0, 10.0));
    stack
        .push(
            roml::ConflictAtomId(1),
            BoundLayerSource::PersistentFixing,
            Bounds::new(2.0, 8.0),
        )
        .unwrap();
    stack
        .push(
            roml::ConflictAtomId(2),
            BoundLayerSource::SolveLock,
            Bounds::fixed(5.0, None),
        )
        .unwrap();
    assert_eq!(stack.current(), Bounds::fixed(5.0, None));
    stack.disable(roml::ConflictAtomId(2)).unwrap();
    assert_eq!(stack.current(), Bounds::new(2.0, 8.0));
    stack.disable(roml::ConflictAtomId(1)).unwrap();
    assert_eq!(stack.current(), Bounds::new(0.0, 10.0));
}
