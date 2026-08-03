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

use roml::expr::TermCoeff;
use roml::model::CoefficientTarget;
use roml::{
    continuous, ConstraintBounds, ConstraintExprExt, FunctionConstraint, IntoScalarFunction, Model,
    ModelOp, ModelRevision, ScalarFunction, ScalarSet, ValueExpr, VarId,
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
// 3a. Symbolic parameter expressions are preserved in the semantic IR (F1)
// =========================================================================

/// F1: a parameterized coefficient `p * x` must NOT become a bare constant in
/// the semantic IR. The symbolic expression belongs INSIDE the scalar function
/// (design §6): `constraint_function`, the snapshot `FunctionEntry`, and the
/// delta `FunctionEntry` all carry `ScalarFunction::Linear(LinExpr)` whose
/// terms hold `TermCoeff::Expr(ValueExpr)` referencing `p` — not a parallel
/// `terms`/`dependencies` view. Dependencies are DERIVED, never stored
/// ([`ScalarFunction::parameter_dependencies`]). After updating `p`, the
/// semantic entries still carry the symbolic form.
#[test]
fn semantic_ir_preserves_symbolic_parameter_terms() {
    fn coefficient_expr(function: &ScalarFunction, var: VarId) -> &ValueExpr {
        match function {
            ScalarFunction::Linear(expr) => {
                let term = expr
                    .terms()
                    .iter()
                    .find(|t| t.var == var)
                    .expect("term for variable must be present");
                match &term.coeff {
                    TermCoeff::Expr(e) => e,
                    TermCoeff::Constant(v) => {
                        panic!("expected TermCoeff::Expr coefficient, got Constant({v})")
                    }
                }
            }
            // ScalarFunction is #[non_exhaustive]; M3 implements only Linear.
            _ => panic!("expected a linear scalar function"),
        }
    }

    let mut model = Model::new();
    let p = model.add_parameter(2.0).unwrap();
    let x = model.add_variable(continuous()).unwrap();
    let con = model.add_constraint((p * x).le(10.0)).unwrap();
    let r1 = model.commit().unwrap();

    // 1. Canonical `constraint_function` carries the symbolic coefficient
    //    INSIDE its linear function (no parallel terms/dependencies view).
    let fc = model.constraint_function(con).unwrap();
    let coeff = coefficient_expr(&fc.function, x);
    assert!(
        coeff.has_dependencies(),
        "symbolic ValueExpr must reference the parameter, not just the evaluated number"
    );
    assert_eq!(
        coeff.dependencies(),
        std::collections::HashSet::from([p]),
        "the coefficient's ValueExpr depends on exactly p"
    );
    assert_eq!(
        fc.parameter_dependencies(),
        vec![p],
        "derived function dependencies == [p]"
    );

    // 2. Snapshot FunctionEntry carries the symbolic coefficient inside its
    //    reconstructed function.
    let snap = model.take_snapshot().unwrap();
    let snap_entry = snap
        .functions
        .iter()
        .find(|e| e.constraint == con)
        .expect("snapshot carries the function entry");
    let snap_coeff = coefficient_expr(&snap_entry.function, x);
    assert!(snap_coeff.has_dependencies());
    assert_eq!(
        snap_coeff.dependencies(),
        std::collections::HashSet::from([p]),
        "snapshot coefficient ValueExpr depends on exactly p"
    );
    assert_eq!(
        snap_entry.function.parameter_dependencies(),
        vec![p],
        "snapshot derived dependencies == [p]"
    );

    // 3. Delta FunctionEntry carries the symbolic coefficient inside its
    //    reconstructed function.
    let batches = model.deltas_since(ModelRevision::ZERO).unwrap();
    let batch = batches
        .iter()
        .find(|b| b.to == r1)
        .expect("constraint-add batch present");
    let delta_entry = batch
        .functions
        .iter()
        .find(|e| e.constraint == con)
        .expect("delta carries the function entry");
    let delta_coeff = coefficient_expr(&delta_entry.function, x);
    assert!(
        delta_coeff.has_dependencies(),
        "delta symbolic ValueExpr must reference the parameter"
    );
    assert_eq!(
        delta_coeff.dependencies(),
        std::collections::HashSet::from([p]),
        "delta coefficient ValueExpr depends on exactly p"
    );
    assert_eq!(
        delta_entry.function.parameter_dependencies(),
        vec![p],
        "delta derived dependencies == [p]"
    );

    // 4. After updating `p`, the semantic entries STILL carry the symbolic
    //    form.
    model.set_parameter(p, 5.0).unwrap();
    model.commit().unwrap();
    let fc2 = model.constraint_function(con).unwrap();
    assert_eq!(
        fc2.parameter_dependencies(),
        vec![p],
        "derived dependency survives parameter update"
    );
    assert!(coefficient_expr(&fc2.function, x).has_dependencies());
    let snap2 = model.take_snapshot().unwrap();
    let snap_entry2 = snap2
        .functions
        .iter()
        .find(|e| e.constraint == con)
        .expect("snapshot carries the function entry after update");
    assert_eq!(
        snap_entry2.function.parameter_dependencies(),
        vec![p],
        "snapshot derived dependency survives parameter update"
    );
    assert!(coefficient_expr(&snap_entry2.function, x).has_dependencies());
}

// =========================================================================
// 3a2. Delta `functions` contract (F2)
// =========================================================================

/// F2 (a): a constraint added and removed in the SAME commit must not leave a
/// stale `FunctionEntry` — the delta `functions` view is "constraints ADDED by
/// this batch ... minus constraints removed by this batch".
#[test]
fn delta_functions_exclude_constraints_added_and_removed_in_same_batch() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).unwrap();
    let con = model.add_constraint((x).le(10.0)).unwrap();
    model.remove_constraint(con).unwrap();
    let r1 = model.commit().unwrap();

    let batches = model.deltas_since(ModelRevision::ZERO).unwrap();
    let batch = batches
        .iter()
        .find(|b| b.to == r1)
        .expect("add-then-remove batch present");
    assert!(
        !batch.functions.iter().any(|e| e.constraint == con),
        "add-then-remove in one batch must not leave a stale FunctionEntry"
    );
}

/// F2 (b): the delta `functions` view is narrowed — updates to PRE-EXISTING
/// constraints ride the underlying ops (`SetConstraintBounds`/`SetCell`/
/// `RemoveConstraint`), not the `functions` view. A bounds-only update to an
/// existing constraint produces no `FunctionEntry`.
#[test]
fn delta_functions_contract_updates_ride_ops_not_functions_view() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).unwrap();
    let con = model.add_constraint((x).le(10.0)).unwrap();
    let r1 = model.commit().unwrap();

    // A pre-existing constraint's bounds are updated in the next commit.
    model
        .set_constraint_bounds(con, ConstraintBounds::le(5.0))
        .unwrap();
    let r2 = model.commit().unwrap();
    assert_ne!(r1, r2);

    let batches = model.deltas_since(ModelRevision::ZERO).unwrap();
    let batch = batches
        .iter()
        .find(|b| b.to == r2)
        .expect("bounds-update batch present");
    assert!(
        batch.functions.is_empty(),
        "updates to pre-existing constraints ride the ops, not the functions view"
    );
    assert!(
        batch
            .operations
            .iter()
            .any(|op| matches!(op, ModelOp::SetConstraintBounds { con: c, .. } if *c == con)),
        "the update is carried by the underlying SetConstraintBounds op"
    );
}

/// F2 (A31): updating a coefficient of a PRE-EXISTING constraint (here by
/// updating the parameter it depends on) produces NO entry in `delta.functions`
/// for that constraint — the update rides the underlying `SetCell` op, not the
/// functions view.
#[test]
fn delta_functions_contract_coefficient_update_rides_ops_not_functions_view() {
    let mut model = Model::new();
    let p = model.add_parameter(2.0).unwrap();
    let x = model.add_variable(continuous()).unwrap();
    let con = model.add_constraint((p * x).le(10.0)).unwrap();
    let r1 = model.commit().unwrap();

    // Update the parameter; the dependent coefficient's cached value changes,
    // producing a `SetCell` op in the next batch. The constraint itself is
    // pre-existing — it was added in the FIRST commit.
    model.set_parameter(p, 5.0).unwrap();
    let r2 = model.commit().unwrap();
    assert_ne!(r1, r2);

    let batches = model.deltas_since(ModelRevision::ZERO).unwrap();
    let batch = batches
        .iter()
        .find(|b| b.to == r2)
        .expect("parameter-update batch present");
    assert!(
        !batch.functions.iter().any(|e| e.constraint == con),
        "updates to a pre-existing constraint must not appear in delta.functions"
    );
    assert!(
        batch.operations.iter().any(|op| matches!(
            op,
            ModelOp::SetCell { cell_key, .. }
                if cell_key.0 == CoefficientTarget::Constraint(con)
        )),
        "the coefficient update rides the underlying SetCell op"
    );
}

/// F2 (A31): removing a PRE-EXISTING constraint produces NO entry in
/// `delta.functions` for it (the functions view is constraints ADDED minus
/// constraints REMOVED by this batch), and the removal rides the underlying
/// `RemoveConstraint` op.
#[test]
fn delta_functions_contract_existing_row_removal_rides_ops_not_functions_view() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).unwrap();
    let con = model.add_constraint((x).le(10.0)).unwrap();
    let r1 = model.commit().unwrap();

    // Remove the pre-existing constraint in the next commit.
    model.remove_constraint(con).unwrap();
    let r2 = model.commit().unwrap();
    assert_ne!(r1, r2);

    let batches = model.deltas_since(ModelRevision::ZERO).unwrap();
    let batch = batches
        .iter()
        .find(|b| b.to == r2)
        .expect("constraint-removal batch present");
    assert!(
        !batch.functions.iter().any(|e| e.constraint == con),
        "removing a pre-existing constraint must not leave a stale FunctionEntry"
    );
    assert!(
        batch
            .operations
            .iter()
            .any(|op| matches!(op, ModelOp::RemoveConstraint { con: c } if *c == con)),
        "the removal rides the underlying RemoveConstraint op"
    );
}

// =========================================================================
// 3b. Constant-folding constraints reconstruct folded bounds in deltas
// =========================================================================

/// CR-01: `add_constraint((x + 3.0).le(5.0))` folds the expression constant
/// into the bounds via a same-batch `SetConstraintBounds` op. The delta's
/// reconstructed `FunctionEntry.set` must equal the model's canonical folded
/// set (`LessEqual(2.0)`), never the pre-adjustment declared bounds.
#[test]
fn delta_set_reflects_bounds_folded_from_expression_constant() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).unwrap();
    let con = model.add_constraint((x + 3.0).le(5.0)).unwrap();
    let r1 = model.commit().unwrap();

    let batches = model.deltas_since(ModelRevision::ZERO).unwrap();
    let batch = batches
        .iter()
        .find(|b| b.to == r1)
        .expect("constraint-add batch present");
    let entry = batch
        .functions
        .iter()
        .find(|e| e.constraint == con)
        .expect("delta carries the constraint's function entry");

    // (x + 3) <= 5  =>  x <= 2.
    let fc = model.constraint_function(con).unwrap();
    assert_eq!(
        entry.set, fc.set,
        "delta set must equal the canonical folded set"
    );
    assert_eq!(entry.set, ScalarSet::LessEqual(ValueExpr::from(2.0)));
}

/// CR-01 variants: `.ge` folds the constant into the lower bound and
/// `.between` folds it into both interval ends.
#[test]
fn delta_set_reflects_folded_bounds_for_ge_and_between() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).unwrap();
    let ge = model.add_constraint((x + 3.0).ge(5.0)).unwrap();
    let between = model.add_constraint((x + 3.0).between(1.0, 5.0)).unwrap();
    let r1 = model.commit().unwrap();

    let batches = model.deltas_since(ModelRevision::ZERO).unwrap();
    let batch = batches
        .iter()
        .find(|b| b.to == r1)
        .expect("constraint-add batch present");

    // (x + 3) >= 5  =>  x >= 2.
    let ge_entry = batch
        .functions
        .iter()
        .find(|e| e.constraint == ge)
        .expect("ge function entry");
    assert_eq!(
        ge_entry.set,
        model.constraint_function(ge).unwrap().set,
        "ge delta set must equal the canonical folded set"
    );
    assert_eq!(ge_entry.set, ScalarSet::GreaterEqual(ValueExpr::from(2.0)));

    // 1 <= (x + 3) <= 5  =>  -2 <= x <= 2.
    let between_entry = batch
        .functions
        .iter()
        .find(|e| e.constraint == between)
        .expect("between function entry");
    assert_eq!(
        between_entry.set,
        model.constraint_function(between).unwrap().set,
        "between delta set must equal the canonical folded set"
    );
    assert_eq!(
        between_entry.set,
        ScalarSet::Interval {
            lower: ValueExpr::from(-2.0),
            upper: ValueExpr::from(2.0),
        }
    );
}

/// WR-01: the canonical `constraint_function` expression and the snapshot's
/// reconstructed function entry must agree in term order (both sorted by var).
#[test]
fn constraint_expression_term_order_matches_snapshot() {
    let mut model = Model::new();
    let a = model.add_variable(continuous()).unwrap();
    let b = model.add_variable(continuous()).unwrap();
    let con = model.add_constraint((a + 2.0 * b).le(10.0)).unwrap();

    let canonical = model.constraint_function(con).unwrap();
    let snap = model.take_snapshot().unwrap();
    let snap_entry = snap
        .functions
        .iter()
        .find(|e| e.constraint == con)
        .expect("snapshot carries the function entry");

    if let (ScalarFunction::Linear(canonical_expr), ScalarFunction::Linear(snap_expr)) =
        (&canonical.function, &snap_entry.function)
    {
        assert_eq!(
            canonical_expr, snap_expr,
            "canonical and snapshot expressions must agree in term order"
        );
        // Terms are sorted by var (VarId implements Ord) so the order is
        // deterministic across runs.
        let vars: Vec<_> = canonical_expr.terms().iter().map(|t| t.var).collect();
        let mut sorted = vars.clone();
        sorted.sort();
        assert_eq!(vars, sorted, "terms sorted by var");
    } else {
        panic!("expected linear functions");
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
//
// The construct-lifecycle tests moved IN-CRATE (F3): they exercise the
// crate-private fixture scaffolding (`FixturePayload`, `ConstructKind::Fixture`,
// `add_construct_fixture`, `Model::construct`, snapshot/delta `.constructs`)
// which is not part of the public surface until P32. They live in
// `src/model/mod.rs` (`#[cfg(test)] mod construct_tests`) so they can construct
// fixture payloads and read crate-private snapshot/delta construct entries.
