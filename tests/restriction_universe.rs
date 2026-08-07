//! RED characterization for the semantic restriction universe.

use roml::advanced::{
    BackendCapabilitySet, BackendFeature, BackendSnapshotBuilder, BoundContributionStack,
    BoundLayerSource, CompilationPolicy, CompilationSession, CompiledConstraintId,
    CompiledLinearRow, CompiledVariable, CompiledVariableId, ConflictAtomKind, ConflictGrouping,
    ConflictOrigin, EntityOrigin, FeatureSupport, OriginMap, RestrictionOriginMap,
    SemanticConflictUniverse,
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
fn one_compiled_bound_can_map_to_multiple_layer_contributions() {
    let snapshot = ranged_snapshot();
    let mut universe = SemanticConflictUniverse::from_snapshot(
        &snapshot,
        InfeasibilityScope::OriginalLp,
        ConflictGrouping::Individual,
    )
    .unwrap();

    // A persistent fixing is a later contribution to an existing compiled
    // lower/upper bound. The map must retain both semantic owners instead of
    // overwriting the declared bound atom with the fixing atom.
    let mut fixing = universe.atoms[0].clone();
    fixing.id = roml::ConflictAtomId(99);
    universe.atoms.push(fixing);

    let map = RestrictionOriginMap::new(&universe).unwrap();
    let mapped = map
        .map_compiled(snapshot.compilation_id, universe.compiled_restrictions[0])
        .unwrap();
    assert_eq!(mapped, vec![universe.atoms[0].id, roml::ConflictAtomId(99)]);
}

#[test]
fn construct_grouping_toggles_all_generated_restrictions_together() {
    let mut model = Model::new();
    let x = model
        .add_variable(roml::continuous().bounds(0.0, 1.0))
        .unwrap();
    let y = model
        .add_variable(roml::continuous().bounds(0.0, 1.0))
        .unwrap();
    model
        .add_minmax(
            vec![x.into(), y.into()],
            roml::MinMaxSense::Max,
            roml::MinMaxRelation::Exact,
            None,
        )
        .unwrap();

    let mut capabilities = BackendCapabilitySet::new();
    for feature in [
        BackendFeature::Lp,
        BackendFeature::Mip,
        BackendFeature::IncrementalBounds,
        BackendFeature::IncrementalRows,
        BackendFeature::IncrementalCoefficients,
    ] {
        capabilities.set(feature, FeatureSupport::native(Default::default()));
    }
    capabilities.set(
        BackendFeature::MinMax,
        FeatureSupport::bridge(Default::default()),
    );
    let canonical = model.take_snapshot().unwrap();
    let mut compiler = CompilationSession::new();
    let snapshot = compiler
        .compile_snapshot(
            model.instance(),
            &canonical,
            &CompilationPolicy::Portable,
            &capabilities,
        )
        .unwrap();
    let universe = SemanticConflictUniverse::from_snapshot(
        &snapshot,
        InfeasibilityScope::LpRelaxation,
        ConflictGrouping::ByConstruct,
    )
    .unwrap();

    let grouped: Vec<_> = universe
        .atoms
        .iter()
        .filter(|atom| atom.kind == ConflictAtomKind::GroupedConstruct)
        .collect();
    assert_eq!(grouped.len(), 1);
    assert!(grouped[0].compiled_restrictions.len() > 1);
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

#[test]
fn semantic_default_keeps_declared_bounds_below_persistent_fixing() {
    let mut model = Model::new();
    let x = model
        .add_variable(roml::continuous().bounds(0.0, 10.0))
        .unwrap();
    model.fix(x, 1.0).unwrap();
    let canonical = model.take_snapshot().unwrap();

    let mut capabilities = BackendCapabilitySet::new();
    for feature in [
        BackendFeature::Lp,
        BackendFeature::Mip,
        BackendFeature::IncrementalBounds,
        BackendFeature::IncrementalRows,
        BackendFeature::IncrementalCoefficients,
    ] {
        capabilities.set(feature, FeatureSupport::native(Default::default()));
    }
    let mut compiler = CompilationSession::new();
    let snapshot = compiler
        .compile_snapshot(
            model.instance(),
            &canonical,
            &CompilationPolicy::Auto,
            &capabilities,
        )
        .unwrap();
    let universe = SemanticConflictUniverse::from_model_snapshot(
        &snapshot,
        &canonical,
        InfeasibilityScope::OriginalLp,
        ConflictGrouping::Semantic,
    )
    .unwrap();

    let base_bounds: Vec<_> = universe
        .atoms
        .iter()
        .filter_map(|atom| match atom.origin {
            ConflictOrigin::VariableBound { variable, .. } if variable == x => {
                Some(atom.snapshot.value)
            }
            _ => None,
        })
        .collect();
    assert_eq!(base_bounds, vec![Some(0.0), Some(10.0)]);
    assert!(!universe.atoms.iter().any(|atom| {
        matches!(
            atom.origin,
            ConflictOrigin::VariableEquality { variable } if variable == x
        )
    }));
    let fixing = universe
        .atoms
        .iter()
        .find(|atom| {
            matches!(
                atom.origin,
                ConflictOrigin::PersistentFixing { variable } if variable == x
            )
        })
        .expect("persistent fixing atom");
    let selected_without_fixing: Vec<_> = universe
        .atoms
        .iter()
        .filter(|atom| atom.id != fixing.id)
        .map(|atom| atom.id)
        .collect();
    let selected_values: Vec<_> = universe
        .atoms
        .iter()
        .filter(|atom| selected_without_fixing.contains(&atom.id))
        .flat_map(|atom| atom.restriction_values.iter().copied())
        .collect();
    assert!(selected_values.iter().any(|(_, value)| *value == Some(0.0)));
    assert!(selected_values
        .iter()
        .any(|(_, value)| *value == Some(10.0)));
}

fn persistent_fixing_universe(declared: Bounds, fixing: f64) -> (SemanticConflictUniverse, VarId) {
    let mut model = Model::new();
    let x = model
        .add_variable(roml::continuous().bounds(declared.lower, declared.upper))
        .unwrap();
    model.fix(x, fixing).unwrap();
    let canonical = model.take_snapshot().unwrap();

    let mut capabilities = BackendCapabilitySet::new();
    for feature in [
        BackendFeature::Lp,
        BackendFeature::Mip,
        BackendFeature::IncrementalBounds,
        BackendFeature::IncrementalRows,
        BackendFeature::IncrementalCoefficients,
    ] {
        capabilities.set(feature, FeatureSupport::native(Default::default()));
    }
    let mut compiler = CompilationSession::new();
    let snapshot = compiler
        .compile_snapshot(
            model.instance(),
            &canonical,
            &CompilationPolicy::Auto,
            &capabilities,
        )
        .unwrap();
    (
        SemanticConflictUniverse::from_model_snapshot(
            &snapshot,
            &canonical,
            InfeasibilityScope::OriginalLp,
            ConflictGrouping::Semantic,
        )
        .unwrap(),
        x,
    )
}

#[test]
fn declared_equality_plus_fixing_keeps_one_equality_base_atom() {
    let (universe, x) = persistent_fixing_universe(Bounds::new(3.0, 3.0), 3.0);
    let variable_atoms: Vec<_> = universe
        .atoms
        .iter()
        .filter(|atom| {
            matches!(
                atom.origin,
                ConflictOrigin::VariableEquality { variable }
                    | ConflictOrigin::VariableBound { variable, .. }
                    | ConflictOrigin::PersistentFixing { variable }
                    if variable == x
            )
        })
        .collect();
    assert_eq!(variable_atoms.len(), 2);
    assert!(matches!(
        variable_atoms[0].origin,
        ConflictOrigin::VariableEquality { variable } if variable == x
    ));
    assert!(matches!(
        variable_atoms[1].origin,
        ConflictOrigin::PersistentFixing { variable } if variable == x
    ));
}

#[test]
fn unbounded_declaration_plus_fixing_has_no_phantom_bound_atoms() {
    let (universe, x) =
        persistent_fixing_universe(Bounds::new(f64::NEG_INFINITY, f64::INFINITY), 1.0);
    let variable_atoms: Vec<_> = universe
        .atoms
        .iter()
        .filter(|atom| {
            matches!(
                atom.origin,
                ConflictOrigin::VariableEquality { variable }
                    | ConflictOrigin::VariableBound { variable, .. }
                    | ConflictOrigin::PersistentFixing { variable }
                    if variable == x
            )
        })
        .collect();
    assert_eq!(variable_atoms.len(), 1);
    assert!(matches!(
        variable_atoms[0].origin,
        ConflictOrigin::PersistentFixing { variable } if variable == x
    ));
}
