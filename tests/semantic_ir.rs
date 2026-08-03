//! P25 Task 3 — function-in-set canonical constraints (SM-01.1, SM-01.2,
//! SM-01.4, SM-01.5).
//!
//! The ordinary M2 `LinExpr` / `.le` / `.ge` / `.eq` / `.between` builders
//! remain the canonical linear user path (SM-01.5). This test verifies that
//! their specs convert to the canonical `FunctionConstraint` representation
//! (design §6), that the coefficient index stays the single authority, and
//! that snapshots and deltas carry the reconstructed semantic function/set
//! entries with the transitional legacy fields invariant-checked.

use roml::{
    continuous, ConstraintExprExt, FunctionConstraint, IntoScalarFunction, Model, ModelRevision,
    ScalarFunction, ScalarSet, ValueExpr,
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
