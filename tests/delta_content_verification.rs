//! Delta-content verification tests for PR A.
//!
//! These tests verify the exact content of DeltaBatches produced by the
//! real model API through the pipeline:
//!
//!   Model::commit() -> compile_change() -> ModelOp -> DeltaBatch -> Journal
//!
//! Each test exercises the public Model API and then inspects the recorded
//! DeltaBatch operations through `model.deltas_since(revision)`.

#![allow(unused_variables)]

use roml::model::CoefficientTarget;
use roml::{
    continuous, integer, Bounds, ConstraintBounds, DeltaBatch, Model, ModelOp, ModelRevision,
    Sense, ValueExpr, VarType,
};

// =========================================================================
// Helpers
// =========================================================================

/// Return all delta batches recorded since the initial revision.
fn all_batches(model: &Model) -> Vec<&DeltaBatch> {
    model.deltas_since(ModelRevision::ZERO).unwrap()
}

/// Return the single batch at the given index.
fn nth_batch(model: &Model, index: usize) -> &DeltaBatch {
    all_batches(model)
        .into_iter()
        .nth(index)
        .expect("batch index out of range")
}

/// Assert that the delta batch from `from` to `to` has exactly the given
/// sequence of operations, in order.
fn assert_ops(
    batch: &DeltaBatch,
    from: ModelRevision,
    to: ModelRevision,
    expected_ops: &[ModelOp],
) {
    assert_eq!(batch.from, from, "batch.from mismatch");
    assert_eq!(batch.to, to, "batch.to mismatch");
    assert_eq!(
        batch.operations.len(),
        expected_ops.len(),
        "operation count mismatch:\n  got:  {:#?}\n  want: {:#?}",
        batch.operations,
        expected_ops,
    );
    for (i, (got, want)) in batch.operations.iter().zip(expected_ops.iter()).enumerate() {
        assert_eq!(got, want, "operation[{i}] mismatch");
    }
}

// =========================================================================
// 1. Changelog produces correct ModelOps
//
// For each public Model mutation, verify the DeltaBatch produced by
// commit() contains the expected ModelOp sequence.
// =========================================================================

#[test]
fn add_var_single() {
    let mut model = Model::new();
    let x = model.add_var();
    let r = model.commit().unwrap();

    assert_ops(
        nth_batch(&model, 0),
        ModelRevision::ZERO,
        r,
        &[ModelOp::AddVariable {
            var: x,
            bounds: Bounds::NON_NEGATIVE,
            var_type: VarType::Continuous,
        }],
    );
}

#[test]
fn add_multiple_variables() {
    let mut model = Model::new();
    let x = model.add_var();
    let y = model.add_binary();
    let z = model.add_integer(Bounds::new(-5.0, 50.0));
    let r0 = model.commit().unwrap();

    let ops = &nth_batch(&model, 0).operations;
    assert_eq!(ops.len(), 3);
    assert_eq!(
        ops[0],
        ModelOp::AddVariable {
            var: x,
            bounds: Bounds::NON_NEGATIVE,
            var_type: VarType::Continuous,
        }
    );
    assert_eq!(
        ops[1],
        ModelOp::AddVariable {
            var: y,
            bounds: Bounds::BINARY,
            var_type: VarType::Binary,
        }
    );
    assert_eq!(
        ops[2],
        ModelOp::AddVariable {
            var: z,
            bounds: Bounds::new(-5.0, 50.0),
            var_type: VarType::Integer,
        }
    );
}

#[test]
fn add_variable_custom_bounds_and_type() {
    let mut model = Model::new();
    let x = model.add_variable(integer().bounds(-1.5, 20.0)).unwrap();
    let r = model.commit().unwrap();

    assert_ops(
        nth_batch(&model, 0),
        ModelRevision::ZERO,
        r,
        &[ModelOp::AddVariable {
            var: x,
            bounds: Bounds::new(-1.5, 20.0),
            var_type: VarType::Integer,
        }],
    );
}

#[test]
fn remove_variable_produces_remove_op() {
    let mut model = Model::new();
    let x = model.add_var();
    let r1 = model.commit().unwrap();

    model.remove_variable(x).unwrap();
    let r2 = model.commit().unwrap();

    assert_ops(
        nth_batch(&model, 1),
        r1,
        r2,
        &[ModelOp::RemoveVariable { var: x }],
    );
}

#[test]
fn set_variable_bounds_produces_set_bounds() {
    let mut model = Model::new();
    let x = model.add_var();
    let r1 = model.commit().unwrap();

    model
        .set_variable_bounds(x, Bounds::new(1.0, 10.0))
        .unwrap();
    let r2 = model.commit().unwrap();

    assert_ops(
        nth_batch(&model, 1),
        r1,
        r2,
        &[ModelOp::SetVariableBounds {
            var: x,
            bounds: Bounds::new(1.0, 10.0),
        }],
    );
}

#[test]
fn set_variable_active_produces_active_op() {
    let mut model = Model::new();
    let x = model.add_var();
    let r1 = model.commit().unwrap();

    model.set_variable_active(x, false).unwrap();
    let r2 = model.commit().unwrap();

    assert_ops(
        nth_batch(&model, 1),
        r1,
        r2,
        &[ModelOp::SetVariableActive {
            var: x,
            active: false,
        }],
    );
}

#[test]
fn set_variable_type_produces_type_op() {
    let mut model = Model::new();
    let x = model.add_var();
    let r1 = model.commit().unwrap();

    model.set_variable_type(x, VarType::Integer).unwrap();
    let r2 = model.commit().unwrap();

    assert_ops(
        nth_batch(&model, 1),
        r1,
        r2,
        &[ModelOp::SetVariableType {
            var: x,
            var_type: VarType::Integer,
        }],
    );
}

#[test]
fn set_binary_convenience() {
    let mut model = Model::new();
    let x = model.add_var();
    let r1 = model.commit().unwrap();

    model.set_binary(x).unwrap();
    let r2 = model.commit().unwrap();

    let ops = &nth_batch(&model, 1).operations;
    assert_eq!(ops.len(), 2);
    assert_eq!(
        ops[0],
        ModelOp::SetVariableType {
            var: x,
            var_type: VarType::Binary,
        }
    );
    assert_eq!(
        ops[1],
        ModelOp::SetVariableBounds {
            var: x,
            bounds: Bounds::BINARY,
        }
    );
}

#[test]
fn set_semicontinuous_produces_semi_op() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 100.0)).unwrap();
    let r1 = model.commit().unwrap();

    model.set_semicontinuous(x, 10.0).unwrap();
    let r2 = model.commit().unwrap();

    let ops = &nth_batch(&model, 1).operations;
    // Both SetVariableBounds (lower raised to 10) and SetSemiContinuousBound
    assert!(ops.contains(&ModelOp::SetVariableBounds {
        var: x,
        bounds: Bounds::new(10.0, 100.0),
    }));
    assert!(ops.contains(&ModelOp::SetSemiContinuousBound {
        var: x,
        lower: 10.0,
    }));
    assert_eq!(ops.len(), 2);
}

#[test]
fn add_constraint_produces_add_op() {
    let mut model = Model::new();
    let c = model.add_constraint(ConstraintBounds::le(100.0)).unwrap();
    let r = model.commit().unwrap();

    assert_ops(
        nth_batch(&model, 0),
        ModelRevision::ZERO,
        r,
        &[ModelOp::AddConstraint {
            con: c,
            bounds: ConstraintBounds::le(100.0),
        }],
    );
}

#[test]
fn remove_constraint_produces_remove_op() {
    let mut model = Model::new();
    let c = model
        .add_constraint(ConstraintBounds::range(0.0, 50.0))
        .unwrap();
    let r1 = model.commit().unwrap();

    model.remove_constraint(c).unwrap();
    let r2 = model.commit().unwrap();

    assert_ops(
        nth_batch(&model, 1),
        r1,
        r2,
        &[ModelOp::RemoveConstraint { con: c }],
    );
}

#[test]
fn set_constraint_bounds_produces_bounds_op() {
    let mut model = Model::new();
    let c = model.add_constraint(ConstraintBounds::le(10.0)).unwrap();
    let r1 = model.commit().unwrap();

    model
        .set_constraint_bounds(c, ConstraintBounds::range(5.0, 20.0))
        .unwrap();
    let r2 = model.commit().unwrap();

    assert_ops(
        nth_batch(&model, 1),
        r1,
        r2,
        &[ModelOp::SetConstraintBounds {
            con: c,
            bounds: ConstraintBounds::range(5.0, 20.0),
        }],
    );
}

#[test]
fn set_constraint_active_produces_active_op() {
    let mut model = Model::new();
    let c = model.add_constraint(ConstraintBounds::le(10.0)).unwrap();
    let r1 = model.commit().unwrap();

    model.set_constraint_active(c, false).unwrap();
    let r2 = model.commit().unwrap();

    assert_ops(
        nth_batch(&model, 1),
        r1,
        r2,
        &[ModelOp::SetConstraintActive {
            con: c,
            active: false,
        }],
    );
}

#[test]
fn add_objective_produces_add_op() {
    let mut model = Model::new();
    let obj = model.add_objective(Sense::Minimize);
    let r = model.commit().unwrap();

    assert_ops(
        nth_batch(&model, 0),
        ModelRevision::ZERO,
        r,
        &[ModelOp::AddObjective {
            obj,
            sense: Sense::Minimize,
        }],
    );
}

#[test]
fn remove_objective_produces_remove_op() {
    let mut model = Model::new();
    let obj = model.add_objective(Sense::Maximize);
    let r1 = model.commit().unwrap();

    model.remove_objective(obj).unwrap();
    let r2 = model.commit().unwrap();

    assert_ops(
        nth_batch(&model, 1),
        r1,
        r2,
        &[ModelOp::RemoveObjective { obj }],
    );
}

#[test]
fn set_active_objective_produces_active_op() {
    let mut model = Model::new();
    let obj = model.add_objective(Sense::Minimize);
    let r1 = model.commit().unwrap();

    model.set_active_objective(obj).unwrap();
    let r2 = model.commit().unwrap();

    assert_ops(
        nth_batch(&model, 1),
        r1,
        r2,
        &[ModelOp::SetActiveObjective { obj: Some(obj) }],
    );
}

#[test]
fn clear_active_objective_produces_none_op() {
    let mut model = Model::new();
    let x = model.add_var();
    let obj = model.minimize(x).unwrap(); // adds + activates
    let r1 = model.commit().unwrap();

    model.clear_active_objective();
    let r2 = model.commit().unwrap();

    // The batch should have SetActiveObjective(None)
    let ops = &nth_batch(&model, 1).operations;
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0], ModelOp::SetActiveObjective { obj: None });
}

#[test]
fn add_constraint_coefficient_new_cell() {
    let mut model = Model::new();
    let x = model.add_var();
    let c = model.add_constraint(ConstraintBounds::le(100.0)).unwrap();
    let r1 = model.commit().unwrap();

    model.add_coeff(c, x, 2.0).unwrap();
    let r2 = model.commit().unwrap();

    assert_ops(
        nth_batch(&model, 1),
        r1,
        r2,
        &[ModelOp::SetCell {
            cell_key: (CoefficientTarget::Constraint(c), x),
            value_expr: ValueExpr::constant(2.0),
            evaluated_value: 2.0,
        }],
    );
}

#[test]
fn add_objective_coefficient_new_cell() {
    let mut model = Model::new();
    let x = model.add_var();
    let obj = model.add_objective(Sense::Maximize);
    let r1 = model.commit().unwrap();

    let _coeff = model
        .add_objective_coefficient(obj, x, ValueExpr::constant(3.0))
        .unwrap();
    let r2 = model.commit().unwrap();

    assert_ops(
        nth_batch(&model, 1),
        r1,
        r2,
        &[ModelOp::SetCell {
            cell_key: (CoefficientTarget::Objective(obj), x),
            value_expr: ValueExpr::constant(3.0),
            evaluated_value: 3.0,
        }],
    );
}

#[test]
fn remove_coefficient_produces_remove_cell() {
    let mut model = Model::new();
    let x = model.add_var();
    let c = model.add_constraint(ConstraintBounds::le(100.0)).unwrap();
    let coeff = model.add_coeff(c, x, 2.0).unwrap();
    let r1 = model.commit().unwrap();

    model.remove_coefficient(coeff).unwrap();
    let r2 = model.commit().unwrap();

    assert_ops(
        nth_batch(&model, 1),
        r1,
        r2,
        &[ModelOp::RemoveCell {
            cell_key: (CoefficientTarget::Constraint(c), x),
        }],
    );
}

// =========================================================================
// 2. Parameter propagation through commit
// =========================================================================

#[test]
fn parameter_change_produces_set_parameter_and_updated_cell() {
    let mut model = Model::new();
    let p = model.add_parameter(5.0).unwrap();
    let x = model.add_var();
    let c = model.add_constraint(ConstraintBounds::le(100.0)).unwrap();
    model
        .add_constraint_coefficient(c, x, ValueExpr::param(p))
        .unwrap();
    let r1 = model.commit().unwrap();

    // Change the parameter value
    model.set_parameter(p, 10.0).unwrap();
    let r2 = model.commit().unwrap();

    // Verify the batch has:
    // 1. SetParameter with the new value
    // 2. SetCell with the evaluated value (10.0 instead of 5.0)
    let ops = &nth_batch(&model, 1).operations;
    assert_eq!(ops.len(), 2);
    assert!(ops.contains(&ModelOp::SetParameter {
        param: p,
        value: 10.0,
    }));
    // Cell value_expr now preserves the original expression.
    assert!(ops.iter().any(|op| matches!(op, ModelOp::SetCell {
        cell_key, evaluated_value, ..
    } if *cell_key == (CoefficientTarget::Constraint(c), x) && (*evaluated_value - 10.0).abs() < 1e-9)));
}

#[test]
fn multiple_parameters_in_one_commit_batch() {
    let mut model = Model::new();
    let p1 = model.add_parameter(1.0).unwrap();
    let p2 = model.add_parameter(2.0).unwrap();
    let x = model.add_var();
    let c = model.add_constraint(ConstraintBounds::le(100.0)).unwrap();
    model
        .add_constraint_coefficient(
            c,
            x,
            ValueExpr::mul(ValueExpr::param(p1), ValueExpr::param(p2)),
        )
        .unwrap();
    let r1 = model.commit().unwrap();

    // Change both parameters before a single commit
    model.set_parameter(p1, 3.0).unwrap();
    model.set_parameter(p2, 4.0).unwrap();
    let r2 = model.commit().unwrap();

    let ops = &nth_batch(&model, 1).operations;
    // Two SetParameter ops, plus each sequential parameter application
    // produces a SetCell for the shared dependent coefficient.
    // The total is 4. The final evaluated value after both changes is 12.0.
    assert_eq!(ops.len(), 4);

    let set_params: Vec<_> = ops
        .iter()
        .filter_map(|op| {
            if let ModelOp::SetParameter { param, value } = op {
                Some((*param, *value))
            } else {
                None
            }
        })
        .collect();
    assert!(set_params.contains(&(p1, 3.0)));
    assert!(set_params.contains(&(p2, 4.0)));

    // There should be two SetCell ops: one intermediate and one final.
    // At least one SetCell must have the final value 12.0.
    let set_cells: Vec<f64> = ops
        .iter()
        .filter_map(|op| {
            if let ModelOp::SetCell {
                evaluated_value, ..
            } = op
            {
                Some(*evaluated_value)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(set_cells.len(), 2);
    assert!(
        set_cells.contains(&12.0),
        "expected a SetCell with final value 12.0, got: {set_cells:?}"
    );
}

// =========================================================================
// 3. Transaction batching
//
// Verify that multiple parameter changes are batched into a single
// DeltaBatch rather than producing a batch per parameter.
// =========================================================================

#[test]
fn all_pending_changes_in_one_batch() {
    let mut model = Model::new();
    let p1 = model.add_parameter(1.0).unwrap();
    let p2 = model.add_parameter(2.0).unwrap();
    let x = model.add_var();
    let c = model.add_constraint(ConstraintBounds::le(100.0)).unwrap();
    model
        .add_constraint_coefficient(c, x, ValueExpr::param(p1) * ValueExpr::param(p2))
        .unwrap();
    let r1 = model.commit().unwrap();

    // Queue multiple changes
    model.set_parameter(p1, 3.0).unwrap();
    model.set_parameter(p2, 5.0).unwrap();

    // One commit should capture all changes
    let r2 = model.commit().unwrap();

    // Verify: exactly one new batch from r1 to r2
    let batches = all_batches(&model);
    assert_eq!(batches.len(), 2);
    assert_eq!(batches[1].from, r1);
    assert_eq!(batches[1].to, r2);

    // The batch should contain both SetParameter ops and a SetCell
    let ops = &batches[1].operations;
    assert!(ops.contains(&ModelOp::SetParameter {
        param: p1,
        value: 3.0,
    }));
    assert!(ops.contains(&ModelOp::SetParameter {
        param: p2,
        value: 5.0,
    }));
    // Cell value_expr now preserves the original expression.
    assert!(ops.iter().any(|op| matches!(op, ModelOp::SetCell {
        cell_key, evaluated_value, ..
    } if *cell_key == (CoefficientTarget::Constraint(c), x) && (*evaluated_value - 15.0).abs() < 1e-9)));
}

// =========================================================================
// 4. Empty changelog produces no revision advance
// =========================================================================

#[test]
fn no_changes_returns_same_revision() {
    let mut model = Model::new();

    let r0 = model.commit().unwrap();
    assert_eq!(r0, ModelRevision::ZERO);

    // No changes since last commit
    let r1 = model.commit().unwrap();
    assert_eq!(
        r1, r0,
        "second commit with no changes should return the same revision"
    );

    // No batches were recorded
    assert!(all_batches(&model).is_empty());
}

#[test]
fn commit_after_mutations_then_commit_noop() {
    let mut model = Model::new();
    let _x = model.add_var();
    let r1 = model.commit().unwrap();
    assert!(r1 > ModelRevision::ZERO);

    // No changes — same revision back
    let r2 = model.commit().unwrap();
    assert_eq!(r2, r1);
}

// =========================================================================
// 5. Mixed mutation sequence
// =========================================================================

#[test]
fn multiple_operations_in_single_batch() {
    let mut model = Model::new();

    // Make several changes before committing
    let x = model.add_var();
    let y = model.add_var();
    let c = model.add_constraint(ConstraintBounds::le(50.0)).unwrap();
    model.add_coeff(c, x, 2.0).unwrap();
    model.add_coeff(c, y, 3.0).unwrap();
    let r = model.commit().unwrap();

    // One batch with 5 operations: AddVar, AddVar, AddCon, SetCell, SetCell
    let ops = &nth_batch(&model, 0).operations;
    assert_eq!(ops.len(), 5);
    assert_eq!(
        ops[0],
        ModelOp::AddVariable {
            var: x,
            bounds: Bounds::NON_NEGATIVE,
            var_type: VarType::Continuous,
        }
    );
    assert_eq!(
        ops[1],
        ModelOp::AddVariable {
            var: y,
            bounds: Bounds::NON_NEGATIVE,
            var_type: VarType::Continuous,
        }
    );
    assert_eq!(
        ops[2],
        ModelOp::AddConstraint {
            con: c,
            bounds: ConstraintBounds::le(50.0),
        }
    );
    assert_eq!(
        ops[3],
        ModelOp::SetCell {
            cell_key: (CoefficientTarget::Constraint(c), x),
            value_expr: ValueExpr::constant(2.0),
            evaluated_value: 2.0,
        }
    );
    assert_eq!(
        ops[4],
        ModelOp::SetCell {
            cell_key: (CoefficientTarget::Constraint(c), y),
            value_expr: ValueExpr::constant(3.0),
            evaluated_value: 3.0,
        }
    );
}

#[test]
fn sequence_of_separate_commits() {
    let mut model = Model::new();

    // Commit 1: add variable
    let x = model.add_var();
    let r1 = model.commit().unwrap();

    // Commit 2: add constraint
    let con = model.add_constraint(ConstraintBounds::ge(10.0)).unwrap();
    let r2 = model.commit().unwrap();

    // Commit 3: add coefficient
    model.add_coeff(con, x, 4.0).unwrap();
    let r3 = model.commit().unwrap();

    // Commit 4: change bounds
    model
        .set_variable_bounds(x, Bounds::new(-1.0, 1.0))
        .unwrap();
    let r4 = model.commit().unwrap();

    let batches = all_batches(&model);
    assert_eq!(batches.len(), 4);

    // Batch 0: AddVariable
    assert_ops(
        batches[0],
        ModelRevision::ZERO,
        r1,
        &[ModelOp::AddVariable {
            var: x,
            bounds: Bounds::NON_NEGATIVE,
            var_type: VarType::Continuous,
        }],
    );

    // Batch 1: AddConstraint
    assert_ops(
        batches[1],
        r1,
        r2,
        &[ModelOp::AddConstraint {
            con,
            bounds: ConstraintBounds::ge(10.0),
        }],
    );

    // Batch 2: SetCell
    assert_ops(
        batches[2],
        r2,
        r3,
        &[ModelOp::SetCell {
            cell_key: (CoefficientTarget::Constraint(con), x),
            value_expr: ValueExpr::constant(4.0),
            evaluated_value: 4.0,
        }],
    );

    // Batch 3: SetVariableBounds
    assert_ops(
        batches[3],
        r3,
        r4,
        &[ModelOp::SetVariableBounds {
            var: x,
            bounds: Bounds::new(-1.0, 1.0),
        }],
    );
}

// =========================================================================
// 6. Deletion cascades
// =========================================================================

#[test]
fn remove_variable_includes_remove_cell_ops() {
    let mut model = Model::new();
    let x = model.add_var();
    let y = model.add_var();
    let c = model.add_constraint(ConstraintBounds::le(50.0)).unwrap();
    model.add_coeff(c, x, 2.0).unwrap();
    model.add_coeff(c, y, 3.0).unwrap();
    let r1 = model.commit().unwrap();

    // Remove variable x — should cascade to remove its coefficient cell
    model.remove_variable(x).unwrap();
    let r2 = model.commit().unwrap();

    let ops = &nth_batch(&model, 1).operations;
    // Should contain RemoveVariable for x AND RemoveCell for the (c, x) cell
    // The order depends on internal impl: first coefficients are removed,
    // then the variable.
    assert!(ops.contains(&ModelOp::RemoveVariable { var: x }));
    assert!(ops.contains(&ModelOp::RemoveCell {
        cell_key: (CoefficientTarget::Constraint(c), x),
    }));
    // y and its coefficient should NOT be touched
    assert!(!ops.contains(&ModelOp::RemoveVariable { var: y }));
    // Total ops: RemoveCell + RemoveVariable = 2
    assert_eq!(ops.len(), 2);
}

#[test]
fn remove_constraint_includes_remove_cell_ops() {
    let mut model = Model::new();
    let x = model.add_var();
    let c1 = model.add_constraint(ConstraintBounds::le(50.0)).unwrap();
    let c2 = model.add_constraint(ConstraintBounds::le(30.0)).unwrap();
    model.add_coeff(c1, x, 2.0).unwrap();
    model.add_coeff(c2, x, 3.0).unwrap();
    let r1 = model.commit().unwrap();

    // Remove constraint c1 — should cascade to remove its coefficient cell
    model.remove_constraint(c1).unwrap();
    let r2 = model.commit().unwrap();

    let ops = &nth_batch(&model, 1).operations;
    assert!(ops.contains(&ModelOp::RemoveConstraint { con: c1 }));
    assert!(ops.contains(&ModelOp::RemoveCell {
        cell_key: (CoefficientTarget::Constraint(c1), x),
    }));
    // c2 and its coefficient should NOT be touched
    assert!(!ops.contains(&ModelOp::RemoveConstraint { con: c2 }));
    assert_eq!(ops.len(), 2);
}

#[test]
fn remove_objective_includes_remove_cell_ops() {
    let mut model = Model::new();
    let x = model.add_var();
    let obj = model.add_objective(Sense::Minimize);
    model
        .add_objective_coefficient(obj, x, ValueExpr::constant(5.0))
        .unwrap();
    let r1 = model.commit().unwrap();

    // Remove objective — should cascade to remove its coefficient cell
    model.remove_objective(obj).unwrap();
    let r2 = model.commit().unwrap();

    let ops = &nth_batch(&model, 1).operations;
    assert!(ops.contains(&ModelOp::RemoveObjective { obj }));
    assert!(ops.contains(&ModelOp::RemoveCell {
        cell_key: (CoefficientTarget::Objective(obj), x),
    }));
    assert_eq!(ops.len(), 2);
}

// =========================================================================
// 7. Stale ID handling
//
// After removing an entity, verify that the operation errors and
// commit() does not produce operations for the stale ID.
// =========================================================================

#[test]
fn stale_variable_id_produces_no_ops() {
    let mut model = Model::new();
    let x = model.add_var();
    let r1 = model.commit().unwrap();

    // Remove the variable
    model.remove_variable(x).unwrap();
    let r2 = model.commit().unwrap();

    // Now try to operate on the stale ID — this should fail
    let err = model.set_variable_bounds(x, Bounds::new(0.0, 5.0));
    assert!(err.is_err(), "should error on stale variable ID");

    // Commit after the errored operation — no changes should be emitted
    let r3 = model.commit().unwrap();
    assert_eq!(
        r3, r2,
        "revision should not advance after failed stale-ID operation"
    );
}

#[test]
fn stale_constraint_id_produces_no_ops() {
    let mut model = Model::new();
    let c = model.add_constraint(ConstraintBounds::le(10.0)).unwrap();
    let r1 = model.commit().unwrap();

    // Remove the constraint
    model.remove_constraint(c).unwrap();
    let r2 = model.commit().unwrap();

    // Try to operate on stale ID — should fail
    let err = model.set_constraint_bounds(c, ConstraintBounds::le(20.0));
    assert!(err.is_err(), "should error on stale constraint ID");

    // No changes, same revision
    let r3 = model.commit().unwrap();
    assert_eq!(r3, r2);
}

#[test]
fn stale_objective_id_produces_no_ops() {
    let mut model = Model::new();
    let obj = model.add_objective(Sense::Maximize);
    let r1 = model.commit().unwrap();

    model.remove_objective(obj).unwrap();
    let r2 = model.commit().unwrap();

    let err = model.set_active_objective(obj);
    assert!(err.is_err(), "should error on stale objective ID");

    let r3 = model.commit().unwrap();
    assert_eq!(r3, r2);
}
