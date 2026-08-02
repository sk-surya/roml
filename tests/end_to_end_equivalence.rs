//! End-to-end equivalence tests for the Model -> DeltaBatch pipeline.
//!
//! These tests exercise the real production path:
//!   Model::commit() -> compile_change() -> ModelOp -> DeltaBatch -> Journal
//!
//! # Verification strategy
//!
//! The tests use a **commuting square** to validate that the Deltas produced
//! by `commit()` are semantically correct. For any mutation M:
//!
//!   rebuild(snapshot_after_commit) == apply(rebuild(snapshot_before_commit), expected_deltas)
//!
//! where `expected_deltas` are ModelOps we construct manually based on the
//! changes we know the Model API should produce. If the equality holds, the
//! production path produced a DeltaBatch equivalent to our expectation.
//!
//! This is NOT unit-testing `compile_change` in isolation — it exercises the
//! full pipeline from Model API through Change compilation to DeltaBatch storage
//! in the Journal, then validates the result through the ReferenceBackend.

// P23: exercises deprecated raw constructors during the pre-1.0 window.
#![allow(deprecated)]

use roml::delta::{DeltaBatch, ModelOp};
use roml::model::coefficient::CoefficientTarget;
use roml::model::ConstraintBounds;
use roml::prelude::*;
use roml::snapshot::ModelSnapshot;
use roml::solver::reference::ReferenceBackend;
use roml::sync::{AdapterCursor, ApplyOutcome};
use roml::ValueExpr;

// =========================================================================
// Helpers
// =========================================================================

/// Verify the commuting square: applying `expected_ops` to the model state
/// captured by `before_snapshot` produces the same state as `after_snapshot`.
fn assert_commuting_square(
    before_snapshot: &ModelSnapshot,
    expected_ops: &[ModelOp],
    after_snapshot: &ModelSnapshot,
) {
    let mut backend = ReferenceBackend::new();
    let mut cursor = AdapterCursor::new();
    backend.rebuild(before_snapshot, &mut cursor);

    let batch = DeltaBatch::new(
        before_snapshot.revision,
        after_snapshot.revision,
        expected_ops.to_vec(),
    )
    .expect("valid delta batch: from < to");

    let outcome = backend.apply_batch(&batch, &mut cursor).unwrap();
    assert!(
        matches!(outcome, ApplyOutcome::Applied { .. }),
        "expected ApplyOutcome::Applied, got {:?}",
        outcome
    );

    let mut expected_backend = ReferenceBackend::new();
    let mut expected_cursor = AdapterCursor::new();
    expected_backend.rebuild(after_snapshot, &mut expected_cursor);

    assert_eq!(
        backend.normalized_view(),
        expected_backend.normalized_view(),
        "commuting square violation: expected ModelOps do not produce the final state"
    );
}

/// Run a single mutation, commit, and verify the commuting square holds.
fn assert_commit_produces<F>(model: &mut Model, setup: F, expected_ops: &[ModelOp])
where
    F: FnOnce(&mut Model) -> Result<(), ModelError>,
{
    let rev_before = model.current_revision();
    let _snap_before = model.take_snapshot().expect("snapshot before commit");

    setup(model).expect("setup should succeed");

    let rev_after = model.commit().expect("commit should succeed");
    assert!(rev_after > rev_before, "revision must advance");

    let snap_after = model.take_snapshot().expect("snapshot after commit");

    assert_commuting_square(&_snap_before, expected_ops, &snap_after);
}

// =========================================================================
// Test 1: One canonical coefficient per cell
// =========================================================================

#[test]
fn canonical_coefficient_per_cell() -> Result<(), ModelError> {
    let mut model = Model::new();
    let x = model.add_var();
    let c = model.add_constraint(ConstraintBounds::le(100.0)).unwrap();
    let p = model.add_parameter(3.0).unwrap();

    // Add constant coefficient, then param-dependent coefficient to same cell
    let k1 = model.add_constraint_coefficient(c, x, ValueExpr::from(3.0))?;
    let k2 = model.add_constraint_coefficient(c, x, ValueExpr::param(p))?;

    // Should combine into a single canonical cell
    assert_eq!(model.num_coefficients(), 1);
    assert_eq!(k1, k2, "same cell returns same CoeffId");

    // Initial combined value: 3.0 + 3.0 = 6.0
    assert!(
        (model.coefficient(k1).unwrap().cached_value - 6.0).abs() < 1e-9,
        "expected combined value 6.0, got {}",
        model.coefficient(k1).unwrap().cached_value
    );

    // Update parameter: 3.0 + 5.0 = 8.0
    model.set_parameter(p, 5.0).unwrap();
    model.commit()?;

    assert_eq!(model.num_coefficients(), 1);
    assert!(
        (model.coefficient(k1).unwrap().cached_value - 8.0).abs() < 1e-9,
        "expected combined value 8.0 after param update, got {}",
        model.coefficient(k1).unwrap().cached_value
    );

    Ok(())
}

// =========================================================================
// Test 2: End-to-end commuting square through Model API
// =========================================================================

#[test]
fn end_to_end_commuting_square() -> Result<(), ModelError> {
    let mut model = Model::new();

    let rev_before = model.current_revision();
    let _snap_before = model.take_snapshot()?;

    // Build a model with all entity types using fine-grained primitives
    let x = model.add_var();
    let y = model.add_var();
    let con = model.add_constraint(ConstraintBounds::le(100.0)).unwrap();
    model.add_coeff(con, x, 2.0)?;
    model.add_coeff(con, y, 3.0)?;
    let obj = model.add_objective(Sense::Minimize);
    model.set_active_objective(obj)?;
    model.add_objective_coeff(obj, x, 1.0)?;

    let rev_after = model.commit()?;
    assert!(rev_after > rev_before);

    let snap_after = model.take_snapshot()?;

    let expected_ops = vec![
        ModelOp::AddVariable {
            var: x,
            bounds: Bounds::NON_NEGATIVE,
            var_type: VarType::Continuous,
        },
        ModelOp::AddVariable {
            var: y,
            bounds: Bounds::NON_NEGATIVE,
            var_type: VarType::Continuous,
        },
        ModelOp::AddConstraint {
            con,
            bounds: ConstraintBounds::le(100.0),
        },
        ModelOp::SetCell {
            cell_key: (CoefficientTarget::Constraint(con), x),
            value_expr: ValueExpr::constant(2.0),
            evaluated_value: 2.0,
        },
        ModelOp::SetCell {
            cell_key: (CoefficientTarget::Constraint(con), y),
            value_expr: ValueExpr::constant(3.0),
            evaluated_value: 3.0,
        },
        ModelOp::AddObjective {
            obj,
            sense: Sense::Minimize,
        },
        ModelOp::SetActiveObjective { obj: Some(obj) },
        ModelOp::SetCell {
            cell_key: (CoefficientTarget::Objective(obj), x),
            value_expr: ValueExpr::constant(1.0),
            evaluated_value: 1.0,
        },
    ];

    assert_commuting_square(&_snap_before, &expected_ops, &snap_after);

    // Verify model state directly
    assert_eq!(model.num_variables(), 2);
    assert_eq!(model.num_constraints(), 1);
    assert_eq!(model.num_coefficients(), 3);
    assert_eq!(model.num_objectives(), 1);
    assert_eq!(model.active_objective(), Some(obj));

    Ok(())
}

// =========================================================================
// Test 3: Every public mutation verified
// =========================================================================

// ── Variable mutations ──────────────────────────────────────────────────

#[test]
fn add_var_produces_addvariable() {
    let mut model = Model::new();

    // Ensure we start clean so the snapshot captures the expected state
    assert!(model.current_revision().is_zero());

    let x = model.add_var();
    let rev_before = model.current_revision();
    let rev_after = model.commit().unwrap();
    assert!(rev_after > rev_before);

    assert_eq!(model.num_variables(), 1);
    assert_eq!(model.variable_bounds(x), Some(Bounds::NON_NEGATIVE));

    let expected_ops = vec![ModelOp::AddVariable {
        var: x,
        bounds: Bounds::NON_NEGATIVE,
        var_type: VarType::Continuous,
    }];
    let _snap_before = ModelSnapshot::empty(roml::revision::ModelRevision::ZERO);
    let snap_after = model.take_snapshot().unwrap();
    assert_commuting_square(&_snap_before, &expected_ops, &snap_after);
}

#[test]
fn remove_variable_produces_removevariable() {
    let mut model = Model::new();
    let x = model.add_var();
    model.commit().unwrap();

    let expected_ops = vec![ModelOp::RemoveVariable { var: x }];
    assert_commit_produces(
        &mut model,
        |m| {
            m.remove_variable(x)?;
            Ok(())
        },
        &expected_ops,
    );

    assert_eq!(model.num_variables(), 0);
}

#[test]
fn set_variable_bounds_produces_setvariablebounds() {
    let mut model = Model::new();
    let x = model.add_var();
    model.commit().unwrap();

    let new_bounds = Bounds::new(5.0, 20.0);
    let expected_ops = vec![ModelOp::SetVariableBounds {
        var: x,
        bounds: new_bounds,
    }];
    assert_commit_produces(
        &mut model,
        |m| {
            m.set_variable_bounds(x, new_bounds)?;
            Ok(())
        },
        &expected_ops,
    );

    assert_eq!(model.variable_bounds(x), Some(new_bounds));
}

#[test]
fn set_variable_type_produces_setvariabletype() {
    let mut model = Model::new();
    let x = model.add_var();
    model.commit().unwrap();

    let expected_ops = vec![ModelOp::SetVariableType {
        var: x,
        var_type: VarType::Integer,
    }];
    assert_commit_produces(
        &mut model,
        |m| {
            m.set_variable_type(x, VarType::Integer)?;
            Ok(())
        },
        &expected_ops,
    );
}

#[test]
fn set_variable_active_produces_setvariableactive() {
    let mut model = Model::new();
    let x = model.add_var();
    model.commit().unwrap();

    let expected_ops = vec![ModelOp::SetVariableActive {
        var: x,
        active: false,
    }];
    assert_commit_produces(
        &mut model,
        |m| {
            m.set_variable_active(x, false)?;
            Ok(())
        },
        &expected_ops,
    );
}

// ── Constraint mutations ────────────────────────────────────────────────

#[test]
fn add_constraint_produces_addconstraint() {
    let mut model = Model::new();

    let _snap_before = {
        // Start with a variable so the snapshot context is non-trivial
        let _x = model.add_var();
        model.commit().unwrap();
        model.take_snapshot().unwrap()
    };

    // New constraint on a clean model (no coefficients attached)
    let con = model.add_constraint(ConstraintBounds::le(100.0)).unwrap();
    let snap_after = {
        let rev_before = model.current_revision();
        let rev_after = model.commit().unwrap();
        assert!(rev_after > rev_before);
        model.take_snapshot().unwrap()
    };

    assert_eq!(model.num_constraints(), 1);

    let expected_ops = vec![ModelOp::AddConstraint {
        con,
        bounds: ConstraintBounds::le(100.0),
    }];
    assert_commuting_square(&_snap_before, &expected_ops, &snap_after);
}

#[test]
fn remove_constraint_produces_removeconstraint() {
    let mut model = Model::new();
    let con = model.add_constraint(ConstraintBounds::le(50.0)).unwrap();
    model.commit().unwrap();

    let expected_ops = vec![ModelOp::RemoveConstraint { con }];
    assert_commit_produces(
        &mut model,
        |m| {
            m.remove_constraint(con)?;
            Ok(())
        },
        &expected_ops,
    );

    assert_eq!(model.num_constraints(), 0);
}

#[test]
fn set_constraint_bounds_produces_setconstraintbounds() {
    let mut model = Model::new();
    let con = model.add_constraint(ConstraintBounds::le(100.0)).unwrap();
    model.commit().unwrap();

    let new_bounds = ConstraintBounds::range(10.0, 50.0);
    let expected_ops = vec![ModelOp::SetConstraintBounds {
        con,
        bounds: new_bounds,
    }];
    assert_commit_produces(
        &mut model,
        |m| {
            m.set_constraint_bounds(con, new_bounds)?;
            Ok(())
        },
        &expected_ops,
    );
}

// ── Objective mutations ─────────────────────────────────────────────────

#[test]
fn set_objective_produces_addobjective_and_cell_and_active() {
    let mut model = Model::new();
    let x = model.add_var();
    model.commit().unwrap();

    let _snap_before = model.take_snapshot().unwrap();

    let obj = model.minimize(x).unwrap();
    let snap_after = {
        let rev_before = model.current_revision();
        let rev_after = model.commit().unwrap();
        assert!(rev_after > rev_before);
        model.take_snapshot().unwrap()
    };

    assert_eq!(model.num_objectives(), 1);
    assert_eq!(model.active_objective(), Some(obj));

    let expected_ops = vec![
        ModelOp::AddObjective {
            obj,
            sense: Sense::Minimize,
        },
        ModelOp::SetCell {
            cell_key: (CoefficientTarget::Objective(obj), x),
            value_expr: ValueExpr::constant(1.0),
            evaluated_value: 1.0,
        },
        ModelOp::SetActiveObjective { obj: Some(obj) },
    ];
    assert_commuting_square(&_snap_before, &expected_ops, &snap_after);
}

#[test]
fn set_active_objective_produces_setobjectiveactive() {
    let mut model = Model::new();
    let x = model.add_var();
    let obj1 = model.minimize(x).unwrap();
    model.commit().unwrap();

    // Add a second objective but don't activate it within the commit
    let obj2 = model.add_objective(Sense::Maximize);
    model.commit().unwrap();

    let expected_ops = vec![ModelOp::SetActiveObjective { obj: Some(obj2) }];
    assert_commit_produces(
        &mut model,
        |m| {
            m.set_active_objective(obj2)?;
            Ok(())
        },
        &expected_ops,
    );

    assert_eq!(model.active_objective(), Some(obj2));
    // obj1 should be deactivated
    assert_ne!(obj1, obj2);
}

// ── Coefficient mutations ───────────────────────────────────────────────

#[test]
fn coefficient_mutation_produces_setcoefficient() {
    let mut model = Model::new();
    let x = model.add_var();
    let con = model.add_constraint(ConstraintBounds::le(100.0)).unwrap();
    model.commit().unwrap();

    let _snap_before = model.take_snapshot().unwrap();

    model.add_coeff(con, x, 7.5).unwrap();
    let snap_after = {
        let rev_before = model.current_revision();
        let rev_after = model.commit().unwrap();
        assert!(rev_after > rev_before);
        model.take_snapshot().unwrap()
    };

    assert_eq!(model.num_coefficients(), 1);

    let expected_ops = vec![ModelOp::SetCell {
        cell_key: (CoefficientTarget::Constraint(con), x),
        value_expr: ValueExpr::constant(7.5),
        evaluated_value: 7.5,
    }];
    assert_commuting_square(&_snap_before, &expected_ops, &snap_after);
}

#[test]
fn remove_coefficient_produces_removecell() {
    let mut model = Model::new();
    let x = model.add_var();
    let con = model.add_constraint(ConstraintBounds::le(100.0)).unwrap();
    let coeff = model.add_coeff(con, x, 2.0).unwrap();
    model.commit().unwrap();

    let expected_ops = vec![ModelOp::RemoveCell {
        cell_key: (CoefficientTarget::Constraint(con), x),
    }];
    assert_commit_produces(
        &mut model,
        |m| {
            m.remove_coefficient(coeff)?;
            Ok(())
        },
        &expected_ops,
    );

    assert_eq!(model.num_coefficients(), 0);
}

#[test]
fn add_objective_coefficient_produces_setcoefficient() {
    let mut model = Model::new();
    let x = model.add_var();
    let obj = model.add_objective(Sense::Maximize);
    model.set_active_objective(obj).unwrap();
    model.commit().unwrap();

    let _snap_before = model.take_snapshot().unwrap();

    model.add_objective_coeff(obj, x, 4.0).unwrap();
    let snap_after = {
        let rev_before = model.current_revision();
        let rev_after = model.commit().unwrap();
        assert!(rev_after > rev_before);
        model.take_snapshot().unwrap()
    };

    assert_eq!(model.num_coefficients(), 1);

    let expected_ops = vec![ModelOp::SetCell {
        cell_key: (CoefficientTarget::Objective(obj), x),
        value_expr: ValueExpr::constant(4.0),
        evaluated_value: 4.0,
    }];
    assert_commuting_square(&_snap_before, &expected_ops, &snap_after);
}

// ── Parameter mutations ─────────────────────────────────────────────────

#[test]
fn parameter_set_produces_setparameter() {
    let mut model = Model::new();
    let p = model.add_parameter(3.0).unwrap();
    model.commit().unwrap();

    let expected_ops = vec![ModelOp::SetParameter {
        param: p,
        value: 5.0,
    }];
    assert_commit_produces(
        &mut model,
        |m| {
            m.set_parameter(p, 5.0).unwrap();
            Ok(())
        },
        &expected_ops,
    );

    assert!((model.parameter_value(p).unwrap() - 5.0).abs() < 1e-9);
}

// =========================================================================
// Test 4: Multiple coefficients per cell combine to one ModelOp
// =========================================================================

#[test]
fn multiple_coefficients_per_cell_combine_to_one_modelop() {
    let mut model = Model::new();
    let x = model.add_var();
    let c = model.add_constraint(ConstraintBounds::le(100.0)).unwrap();
    model.commit().unwrap();

    let _snap_before = model.take_snapshot().unwrap();

    // Two coefficients to same (c, x) cell combine algebraically
    model.add_constraint_coefficient(c, x, 3.0).unwrap();
    model.add_constraint_coefficient(c, x, 5.0).unwrap();

    let snap_after = {
        let rev_before = model.current_revision();
        let rev_after = model.commit().unwrap();
        assert!(rev_after > rev_before);
        model.take_snapshot().unwrap()
    };

    // Exactly one coefficient in the model
    assert_eq!(model.num_coefficients(), 1);

    // verify the combined value through constraint_expression
    let expr = model.constraint_expression(c).unwrap();
    assert_eq!(expr.num_terms(), 1);
    // The coefficient value should be 8.0 (3.0 + 5.0)
    assert!(
        (expr.terms()[0].coeff.as_constant().unwrap_or(f64::NAN) - 8.0).abs() < 1e-9,
        "expected combined coefficient 8.0"
    );

    // The DeltaBatch has TWO SetCell ops because each add produces a Change:
    //   1st: CoefficientAdded { value: 3.0 }  -> SetCell { value: 3.0 }
    //   2nd: CoefficientValueChanged { old: 3.0, new: 8.0 } -> SetCell { value: 8.0 }
    // Both are drained by commit() and compiled into separate ModelOps.
    //
    // This is correct: the second SetCell overwrites the first when applied.
    let expected_ops = vec![
        ModelOp::SetCell {
            cell_key: (CoefficientTarget::Constraint(c), x),
            value_expr: ValueExpr::constant(3.0),
            evaluated_value: 3.0,
        },
        ModelOp::SetCell {
            cell_key: (CoefficientTarget::Constraint(c), x),
            value_expr: ValueExpr::constant(8.0),
            evaluated_value: 8.0,
        },
    ];
    assert_commuting_square(&_snap_before, &expected_ops, &snap_after);
}

// =========================================================================
// Test 5: Empty commit no-ops
// =========================================================================

#[test]
fn empty_commit_noop() {
    let mut model = Model::new();
    let r0 = model.current_revision();
    assert!(r0.is_zero());

    // Commit an empty model with no changes
    let r1 = model.commit().unwrap();
    assert_eq!(
        r1, r0,
        "empty commit should return the same revision (no batch created)"
    );

    // Also works after a prior commit
    let r2 = model.commit().unwrap();
    assert_eq!(r2, r0, "second empty commit should also be a no-op");

    // Snapshot should be empty
    let snap = model.take_snapshot().unwrap();
    assert!(snap.is_empty());
}

// =========================================================================
// Test 6: Sequential revision advancement
// =========================================================================

#[test]
fn sequential_revision_advancement() {
    let mut model = Model::new();

    let r0 = model.commit().unwrap();
    assert!(r0.is_zero(), "empty commit returns initial revision");

    // First mutation
    model.add_var();
    let r1 = model.commit().unwrap();
    assert!(r1 > r0, "first commit advances");
    assert_eq!(r1.as_u64(), 1);

    // Second mutation
    model.add_var();
    let r2 = model.commit().unwrap();
    assert!(r2 > r1, "second commit advances");
    assert_eq!(r2.as_u64(), 2);

    // Third mutation
    model.add_constraint(ConstraintBounds::le(100.0)).unwrap();
    let r3 = model.commit().unwrap();
    assert!(r3 > r2, "third commit advances");
    assert_eq!(r3.as_u64(), 3);

    // current_revision matches
    assert_eq!(model.current_revision(), r3);
}

// =========================================================================
// Test 7: Semi-continuous bounds
// =========================================================================

#[test]
fn semicontinuous_bounds_produces_setsemicontinuousbound() {
    let mut model = Model::new();
    // Start with a variable that already has a lower bound >= the semicontinuous lower,
    // so that set_semicontinuous produces exactly one Change (no extra bounds update).
    let x = model
        .add_variable(continuous().bounds(10.0, 100.0))
        .unwrap();
    model.commit().unwrap();

    // Set semicontinuous lower to 5.0, which is <= the current lower bound of 10.0
    // This avoids an additional VariableBoundsChanged event.
    let expected_ops = vec![ModelOp::SetSemiContinuousBound { var: x, lower: 5.0 }];
    assert_commit_produces(
        &mut model,
        |m| {
            m.set_semicontinuous(x, 5.0)?;
            Ok(())
        },
        &expected_ops,
    );
}

#[test]
fn semicontinuous_with_bounds_update_produces_two_ops() {
    let mut model = Model::new();
    // Variable with lower bound 0.0
    let x = model.add_variable(continuous().bounds(0.0, 100.0)).unwrap();
    model.commit().unwrap();

    let _snap_before = model.take_snapshot().unwrap();

    // Set semicontinuous lower to 15.0, which exceeds current lower bound of 0.0.
    // This produces TWO Changes: VariableBoundsChanged + SemiContinuousBoundChanged.
    model.set_semicontinuous(x, 15.0).unwrap();

    let snap_after = {
        let rev_before = model.current_revision();
        let rev_after = model.commit().unwrap();
        assert!(rev_after > rev_before);
        model.take_snapshot().unwrap()
    };

    // Final lower bound should be 15.0 (raised by set_semicontinuous)
    assert_eq!(model.variable_bounds(x), Some(Bounds::new(15.0, 100.0)));

    // Two ModelOps: SetVariableBounds + SetSemiContinuousBound
    let expected_ops = vec![
        ModelOp::SetVariableBounds {
            var: x,
            bounds: Bounds::new(15.0, 100.0),
        },
        ModelOp::SetSemiContinuousBound {
            var: x,
            lower: 15.0,
        },
    ];
    assert_commuting_square(&_snap_before, &expected_ops, &snap_after);
}

// =========================================================================
// Test 8: Objective sense changes
// =========================================================================

// Note: There is currently no `Model::set_objective_sense()` public method.
// The `Change::ObjectiveSenseChanged` variant is defined and compiles to
// `ModelOp::SetObjectiveSense { obj, sense }`, but no Model API produces
// this change. This test verifies that objective sense is correctly captured
// through the `AddObjective` path.

#[test]
fn objective_sense_in_add_objective() {
    let mut model = Model::new();
    let x = model.add_var();
    model.commit().unwrap();

    let _snap_before = model.take_snapshot().unwrap();

    // Use a zero-constant expression to avoid the constant-propagation gap:
    // the objective constant (e.g. from x + 10.0) is model state not
    // propagated through the Change/ModelOp pipeline. Constants are verified
    // separately in `objective_constants_in_snapshot`.
    let obj = model.maximize(x).unwrap();
    let snap_after = {
        let rev_before = model.current_revision();
        let rev_after = model.commit().unwrap();
        assert!(rev_after > rev_before);
        model.take_snapshot().unwrap()
    };

    // Objective sense is captured in the AddObjective op
    assert_eq!(snap_after.objectives.len(), 1);
    assert_eq!(snap_after.objectives[0].sense, Sense::Maximize);

    // The expected ops include AddObjective with the correct sense
    let expected_ops = vec![
        ModelOp::AddObjective {
            obj,
            sense: Sense::Maximize,
        },
        ModelOp::SetCell {
            cell_key: (CoefficientTarget::Objective(obj), x),
            value_expr: ValueExpr::constant(1.0),
            evaluated_value: 1.0,
        },
        ModelOp::SetActiveObjective { obj: Some(obj) },
    ];
    assert_commuting_square(&_snap_before, &expected_ops, &snap_after);
}

// =========================================================================
// Test 9: Objective constants
// =========================================================================

#[test]
fn objective_constants_in_snapshot() {
    let mut model = Model::new();
    let x = model.add_var();
    model.commit().unwrap();

    let obj = model.maximize(x + 10.0).unwrap();
    model.commit().unwrap();

    // Snapshot should capture objective constant
    let s = model.take_snapshot().unwrap();
    assert_eq!(s.objectives.len(), 1);
    assert!(
        (s.objectives[0].constant - 10.0).abs() < 1e-9,
        "expected objective constant 10.0, got {}",
        s.objectives[0].constant
    );

    // Also verify through Model API
    assert_eq!(model.objective_constant(obj), Some(10.0));

    // Reconstructed expression should include constant
    let expr = model.objective_expression(obj).unwrap();
    assert!(
        (expr.get_constant() - 10.0).abs() < 1e-9,
        "expected expression constant 10.0, got {}",
        expr.get_constant()
    );
}

// =========================================================================
// Test 10: Multi-objective active switching
// =========================================================================

#[test]
fn multi_objective_active_switching() {
    let mut model = Model::new();
    let x = model.add_var();
    let y = model.add_var();
    model.commit().unwrap();

    let _snap_before = model.take_snapshot().unwrap();

    // Create and activate first objective
    let obj1 = model.maximize(x).unwrap();

    // Create and activate second objective (replaces obj1)
    let obj2 = model.minimize(y).unwrap();

    // Explicitly set active to obj2 — note: this is a no-op at this point
    // because set_objective already activates. Included to verify the
    // no-op case in the changelog.
    model.set_active_objective(obj2).unwrap();

    let snap_after = {
        let rev_before = model.current_revision();
        let rev_after = model.commit().unwrap();
        assert!(rev_after > rev_before);
        model.take_snapshot().unwrap()
    };

    // Final state: obj2 is active, both objectives exist
    assert_eq!(model.num_objectives(), 2);
    assert_eq!(model.active_objective(), Some(obj2));

    // Expected ModelOps in order:
    // 1. AddObjective { obj1, Maximize }
    // 2. SetCell { (Objective(obj1), x), 1.0 }
    // 3. SetActiveObjective { Some(obj1) }
    // 4. AddObjective { obj2, Minimize }
    // 5. SetCell { (Objective(obj2), y), 1.0 }
    // 6. SetActiveObjective { Some(obj2) }
    // (Note: the explicit set_active_objective(obj2) is a no-op because
    //  obj2 is already active after the set_objective call above.)
    let expected_ops = vec![
        ModelOp::AddObjective {
            obj: obj1,
            sense: Sense::Maximize,
        },
        ModelOp::SetCell {
            cell_key: (CoefficientTarget::Objective(obj1), x),
            value_expr: ValueExpr::constant(1.0),
            evaluated_value: 1.0,
        },
        ModelOp::SetActiveObjective { obj: Some(obj1) },
        ModelOp::AddObjective {
            obj: obj2,
            sense: Sense::Minimize,
        },
        ModelOp::SetCell {
            cell_key: (CoefficientTarget::Objective(obj2), y),
            value_expr: ValueExpr::constant(1.0),
            evaluated_value: 1.0,
        },
        ModelOp::SetActiveObjective { obj: Some(obj2) },
    ];
    assert_commuting_square(&_snap_before, &expected_ops, &snap_after);
}
