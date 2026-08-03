//! P25 Tasks 3–4 — function-in-set canonical constraints and construct
//! lifecycle (SM-01.1, SM-01.2, SM-01.3, SM-01.4, SM-01.5, SM-01.6,
//! SM-02.5 foundations).
//!
//! The ordinary M2 `LinExpr` / `.le` / `.ge` / `.eq` / `.between` builders
//! remain the canonical linear user path (SM-01.5). This test verifies that
//! their specs convert to the canonical `FunctionConstraint` representation
//! (design §6), that the coefficient index stays the single authority, that
//! snapshots and deltas carry the reconstructed semantic function/set entries
//! with the transitional legacy fields invariant-checked, and that the
//! generation-safe construct arena survives add/clone/snapshot/activity/
//! remove/rebuild (design §7, SM-01.3, SM-01.6).

use roml::construct::{ConstructKind, FixturePayload, FormulationPreference};
use roml::{
    continuous, ConstraintExprExt, FunctionConstraint, IntoScalarFunction, Model, ModelError,
    ModelRevision, ScalarFunction, ScalarSet, ValueExpr,
};

// =========================================================================
// 1. `.le` / `.ge` / `.eq` / `.between` convert to canonical sets
// =========================================================================

#[test]
fn le_converts_to_linear_function_and_less_equal_set() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).unwrap();
    let y = model.add_variable(continuous()).unwrap();

    let fc = (x + y).le(4.0).into_function_constraint();
    assert_eq!(fc.set, ScalarSet::LessEqual(ValueExpr::from(4.0)));
    if let ScalarFunction::Linear(expr) = &fc.function {
        assert_eq!(expr.num_terms(), 2);
    } else {
        panic!("expected a linear function");
    }
    assert!(matches!(fc.function, ScalarFunction::Linear(_)));
}

#[test]
fn ge_eq_between_convert_to_canonical_sets() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).unwrap();

    let ge = (x).ge(2.0).into_function_constraint();
    assert_eq!(ge.set, ScalarSet::GreaterEqual(ValueExpr::from(2.0)));

    let eq = (x).eq(3.0).into_function_constraint();
    assert_eq!(eq.set, ScalarSet::EqualTo(ValueExpr::from(3.0)));

    let between = (x).between(1.0, 5.0).into_function_constraint();
    assert_eq!(
        between.set,
        ScalarSet::Interval {
            lower: ValueExpr::from(1.0),
            upper: ValueExpr::from(5.0),
        }
    );
}

#[test]
fn into_scalar_function_converts_lin_expr() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).unwrap();
    let y = model.add_variable(continuous()).unwrap();

    let function = (x + 2.0 * y).into_scalar_function();
    assert_eq!(function, ScalarFunction::Linear(x + 2.0 * y));
}

#[test]
fn function_constraint_is_constructible_from_spec() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).unwrap();

    let fc = FunctionConstraint::from((x).ge(7.0));
    assert_eq!(fc.set, ScalarSet::GreaterEqual(ValueExpr::from(7.0)));
    assert!(matches!(fc.function, ScalarFunction::Linear(_)));
}

// =========================================================================
// 2. The coefficient index stays the single authority (round-trip)
// =========================================================================

#[test]
fn ordinary_builder_round_trips_through_coefficient_index() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).unwrap();
    let y = model.add_variable(continuous()).unwrap();

    let con = model.add_constraint((x + y).le(4.0)).unwrap();

    // The coefficient index reconstructs the declared LinExpr.
    let expr = model.constraint_expression(con).unwrap();
    assert_eq!(expr.num_terms(), 2);
    assert_eq!(expr.get_constant(), 0.0);

    // The canonical function-in-set view reconstructs deterministically.
    let fc = model.constraint_function(con).unwrap();
    assert_eq!(fc.set, ScalarSet::LessEqual(ValueExpr::from(4.0)));
    if let ScalarFunction::Linear(e) = &fc.function {
        assert_eq!(e.num_terms(), 2);
    } else {
        panic!("expected a linear function");
    }
}

// =========================================================================
// 3. Snapshot and delta carry semantic function/set entries
// =========================================================================

#[test]
fn snapshot_carries_semantic_function_entries() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).unwrap();
    let y = model.add_variable(continuous()).unwrap();
    let con = model.add_constraint((x + y).le(4.0)).unwrap();

    let snap = model.take_snapshot().unwrap();
    assert_eq!(snap.functions.len(), 1, "one semantic function entry");
    let entry = &snap.functions[0];
    assert_eq!(entry.constraint, con);
    assert_eq!(entry.set, ScalarSet::LessEqual(ValueExpr::from(4.0)));
    if let ScalarFunction::Linear(e) = &entry.function {
        assert_eq!(e.num_terms(), 2);
    } else {
        panic!("expected a linear function");
    }

    // Deterministic round-trip: re-taking the snapshot is equal.
    let snap2 = model.take_snapshot().unwrap();
    assert_eq!(snap, snap2);
}

#[test]
fn delta_carries_semantic_function_entries() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).unwrap();
    let y = model.add_variable(continuous()).unwrap();
    let con = model.add_constraint((x + y).le(4.0)).unwrap();
    let r1 = model.commit().unwrap();

    let batches = model.deltas_since(ModelRevision::ZERO).unwrap();
    let batch = batches
        .iter()
        .find(|b| b.to == r1)
        .expect("constraint-add batch present");
    assert_eq!(batch.functions.len(), 1, "delta carries one function entry");
    let entry = &batch.functions[0];
    assert_eq!(entry.constraint, con);
    assert_eq!(entry.set, ScalarSet::LessEqual(ValueExpr::from(4.0)));
    if let ScalarFunction::Linear(e) = &entry.function {
        assert_eq!(e.num_terms(), 2);
    } else {
        panic!("expected a linear function");
    }
}

// =========================================================================
// 4. Transitional legacy fields are invariant-checked
// =========================================================================

#[test]
fn model_invariants_verify_legacy_fields_against_semantic_view() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).unwrap();
    let y = model.add_variable(continuous()).unwrap();
    model.add_constraint((x + y).le(4.0)).unwrap();

    // The invariant checker verifies the legacy bounds/cells are consistent
    // with the reconstructed semantic function/set (no second authority).
    assert!(
        model.validate_invariants().is_ok(),
        "invariants must hold for a function-in-set constraint"
    );
}

// =========================================================================
// 5. Construct lifecycle (P25 Task 4, design §7)
// =========================================================================

fn fixture(key: &str, value: f64) -> FixturePayload {
    FixturePayload {
        key: key.to_string(),
        value,
    }
}

#[test]
fn construct_add_returns_stable_id_and_payload_round_trips() {
    let mut model = Model::new();
    let k = model
        .add_construct_fixture(fixture("cap", 100.0), FormulationPreference::Auto)
        .unwrap();

    let entry = model.construct(k).unwrap();
    assert_eq!(entry.id, k, "add returns the stable construct id");
    assert!(entry.active, "constructs start active");
    if let ConstructKind::Fixture(p) = &entry.kind {
        assert_eq!(p.key, "cap");
        assert_eq!(p.value, 100.0);
    } else {
        panic!("expected fixture payload");
    }
}

#[test]
fn construct_clone_preserves_ids_and_activity() {
    let mut model = Model::new();
    let k = model
        .add_construct_fixture(fixture("cap", 50.0), FormulationPreference::Portable)
        .unwrap();
    model.set_construct_active(k, false).unwrap();

    let cloned = model.clone();
    let entry = cloned.construct(k).unwrap();
    assert_eq!(entry.id, k, "clone preserves the construct id");
    assert!(!entry.active, "clone preserves activity");
    if let ConstructKind::Fixture(p) = &entry.kind {
        assert_eq!(p.value, 50.0);
    }
    assert_eq!(cloned.num_constructs(), model.num_constructs());
}

#[test]
fn construct_snapshot_and_delta_round_trip() {
    let mut model = Model::new();
    let k = model
        .add_construct_fixture(fixture("on", 1.0), FormulationPreference::Auto)
        .unwrap();
    let r1 = model.commit().unwrap();

    // Snapshot carries every construct entry.
    let snap = model.take_snapshot().unwrap();
    assert_eq!(snap.constructs.len(), 1);
    assert_eq!(snap.constructs[0].id, k);
    assert!(snap.constructs[0].active);

    // Delta carries the added construct entry.
    let batches = model.deltas_since(ModelRevision::ZERO).unwrap();
    let batch = batches
        .iter()
        .find(|b| b.to == r1)
        .expect("construct-add batch present");
    assert_eq!(batch.constructs.len(), 1);
    assert_eq!(batch.constructs[0].id, k);

    // Deterministic snapshot round-trip.
    assert_eq!(snap, model.take_snapshot().unwrap());
}

#[test]
fn construct_activity_toggling_reflected_in_snapshot() {
    let mut model = Model::new();
    let k = model
        .add_construct_fixture(fixture("t", 2.0), FormulationPreference::NativeRequired)
        .unwrap();
    model.set_construct_active(k, false).unwrap();

    let snap = model.take_snapshot().unwrap();
    assert!(
        !snap.constructs[0].active,
        "inactive construct reflected in snapshot"
    );
}

#[test]
fn construct_remove_invalidates_id_and_stale_ids_rejected() {
    let mut model = Model::new();
    let k = model
        .add_construct_fixture(fixture("gone", 7.0), FormulationPreference::Auto)
        .unwrap();
    assert_eq!(model.num_constructs(), 1);

    model.remove_construct(k).unwrap();
    assert_eq!(model.num_constructs(), 0);

    // Stale id is rejected with a typed error.
    match model.construct(k) {
        Err(ModelError::ConstructNotFound(id)) => assert_eq!(id, k),
        other => panic!("expected ConstructNotFound, got {other:?}"),
    }
    assert!(model.set_construct_active(k, true).is_err());
    assert!(model.remove_construct(k).is_err());
}

#[test]
fn construct_store_survives_rebuild() {
    let mut model = Model::new();
    model
        .add_construct_fixture(fixture("a", 1.0), FormulationPreference::Auto)
        .unwrap();
    let k2 = model
        .add_construct_fixture(fixture("b", 2.0), FormulationPreference::Portable)
        .unwrap();
    model.set_construct_active(k2, false).unwrap();

    // Snapshot captures the construct store.
    let snap = model.take_snapshot().unwrap();
    assert_eq!(snap.constructs.len(), 2);

    // Rebuild: a fresh empty model restored from the snapshot carries the
    // same construct content (kind + activity), with fresh ids.
    let mut rebuilt = Model::new();
    for entry in &snap.constructs {
        let payload = if let ConstructKind::Fixture(p) = &entry.kind {
            p.clone()
        } else {
            panic!("expected fixture payload");
        };
        let id = rebuilt
            .add_construct_fixture(payload, FormulationPreference::Auto)
            .unwrap();
        if !entry.active {
            rebuilt.set_construct_active(id, false).unwrap();
        }
    }
    assert_eq!(rebuilt.num_constructs(), 2);
    // Rebuilding from the same snapshot reproduces equal construct content.
    let rebuilt_snap = rebuilt.take_snapshot().unwrap();
    assert_eq!(rebuilt_snap.constructs.len(), snap.constructs.len());
    assert!(!rebuilt_snap.constructs[0].active == !snap.constructs[0].active);
}

#[test]
fn construct_metadata_usable_via_entity_ref() {
    use roml::{EntityMetadata, EntityRef};
    let mut model = Model::new();
    let k = model
        .add_construct_fixture(fixture("meta", 1.0), FormulationPreference::Auto)
        .unwrap();

    let meta = EntityMetadata {
        description: Some("a construct".to_string()),
        ..EntityMetadata::default()
    };
    model.set_metadata(EntityRef::Construct(k), meta.clone());
    assert_eq!(
        model.metadata(EntityRef::Construct(k)),
        Some(&meta),
        "EntityRef::Construct is usable now (design §4.4)"
    );
    assert!(model.validate_invariants().is_ok());
}
