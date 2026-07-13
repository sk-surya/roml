//! Backend contract tests — shared test suite that every backend adapter must pass.
//!
//! These tests use the `ReferenceBackend` as the reference implementation and
//! establish the contract that native backends (HiGHS, MOSEK, Xpress) must also
//! satisfy. The contract covers:
//!
//! - Snapshot load and revision identity
//! - Every `ModelOp` variant applied individually
//! - Add/remove/reindex sequences with cascading cleanup
//! - Activity toggles for variables and constraints
//! - Objective switching (activation/deactivation)
//! - Parameter-driven cell value updates
//! - Unsupported operation / corrupt state recovery via rebuild
//! - Empty model invariants
//! - Normalized view equivalence (commuting square)

use roml::delta::{DeltaBatch, ModelOp};
use roml::id::{ConId, Generation, ObjId, ParamId, VarId};
use roml::model::coefficient::CoefficientTarget;
use roml::model::{Bounds, ConstraintBounds, Sense, VarType};
use roml::revision::ModelRevision;
use roml::snapshot::{take_snapshot, ModelSnapshot};
use roml::solver::reference::ReferenceBackend;
use roml::sync::{AdapterCursor, ApplyOutcome};
use roml::value_expr::ValueExpr;
use std::collections::HashMap;

// ── ID helpers ───────────────────────────────────────────────────────────────

fn var_id(index: u32) -> VarId {
    VarId::new(index, Generation::new())
}

fn con_id(index: u32) -> ConId {
    ConId::new(index, Generation::new())
}

fn obj_id(index: u32) -> ObjId {
    ObjId::new(index, Generation::new())
}

fn param_id(index: u32) -> ParamId {
    ParamId::new(index, Generation::new())
}

// ── Setup helpers ────────────────────────────────────────────────────────────

/// Create a sample snapshot at a given revision with one variable, one
/// constraint, one cell, and one parameter.
fn sample_snapshot(rev: ModelRevision) -> ModelSnapshot {
    let v = var_id(0);
    let c = con_id(0);
    let p = param_id(0);

    let mut variables = HashMap::new();
    variables.insert(v, (Bounds::NON_NEGATIVE, VarType::Continuous, true, None));

    let mut constraints = HashMap::new();
    constraints.insert(c, (ConstraintBounds::le(100.0), true));

    let objectives = HashMap::new();
    let mut parameters = HashMap::new();
    parameters.insert(p, 42.0);

    let cells: Vec<((CoefficientTarget, VarId), ValueExpr, f64, Vec<ParamId>)> = vec![(
        (CoefficientTarget::Constraint(c), v),
        ValueExpr::constant(2.5),
        2.5,
        vec![],
    )];

    take_snapshot(
        rev,
        &variables,
        &constraints,
        &objectives,
        &parameters,
        &cells,
    )
}

/// Build a backend from a snapshot and return it with its cursor at the
/// snapshot's revision.
fn rebuild_from_snapshot(
    backend: &mut ReferenceBackend,
    cursor: &mut AdapterCursor,
    snapshot: &ModelSnapshot,
) {
    backend.rebuild(snapshot, cursor);
}

/// Apply a single batch of operations, advancing the cursor.
fn apply_batch(
    backend: &mut ReferenceBackend,
    cursor: &mut AdapterCursor,
    batch: &DeltaBatch,
) -> ApplyOutcome {
    backend
        .apply_batch(batch, cursor)
        .expect("apply_batch should not error")
}

// Helpers: create a backend + cursor at a fresh start.
fn empty_backend() -> (ReferenceBackend, AdapterCursor) {
    (ReferenceBackend::new(), AdapterCursor::new())
}

// ── Test: Empty model ────────────────────────────────────────────────────────

#[test]
fn empty_model() {
    let (backend, _cursor) = empty_backend();
    let view = backend.normalized_view();

    assert_eq!(view.revision, ModelRevision::ZERO);
    assert!(view.variables.is_empty());
    assert!(view.constraints.is_empty());
    assert!(view.objectives.is_empty());
    assert!(view.parameters.is_empty());
    assert!(view.cells.is_empty());
    assert!(view.objective_cells.is_empty());
    assert!(view.active_objective.is_none());
}

// ── Test: Snapshot load and revision identity ────────────────────────────────

#[test]
fn snapshot_load_identity() {
    let rev = ModelRevision::ZERO.next().unwrap();
    let snapshot = sample_snapshot(rev);

    let (mut backend, mut cursor) = empty_backend();
    rebuild_from_snapshot(&mut backend, &mut cursor, &snapshot);

    assert_eq!(cursor.applied_revision, rev);
    let view = backend.normalized_view();
    assert_eq!(view.revision, rev);
}

#[test]
fn snapshot_load_entity_counts() {
    let rev = ModelRevision::ZERO.next().unwrap();
    let snapshot = sample_snapshot(rev);

    let (mut backend, mut cursor) = empty_backend();
    rebuild_from_snapshot(&mut backend, &mut cursor, &snapshot);

    let view = backend.normalized_view();
    assert_eq!(view.variables.len(), 1);
    assert_eq!(view.constraints.len(), 1);
    assert_eq!(view.parameters.len(), 1);
    assert_eq!(view.cells.len(), 1);
    assert_eq!(view.objectives.len(), 0);
}

#[test]
fn empty_snapshot_load() {
    let rev = ModelRevision::ZERO;
    let snapshot = ModelSnapshot::empty(rev);

    let (mut backend, mut cursor) = empty_backend();
    rebuild_from_snapshot(&mut backend, &mut cursor, &snapshot);

    assert_eq!(cursor.applied_revision, rev);
    let view = backend.normalized_view();
    assert!(view.variables.is_empty());
    assert!(view.constraints.is_empty());
}

// ── Tests: Every ModelOp applied individually ────────────────────────────────

#[test]
fn apply_add_variable() {
    let v = var_id(0);
    let (mut backend, mut cursor) = empty_backend();
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();

    let ops = vec![ModelOp::AddVariable {
        var: v,
        bounds: Bounds::new(1.0, 10.0),
        var_type: VarType::Integer,
    }];
    let batch = DeltaBatch::new(r0, r1, ops).unwrap();
    let outcome = apply_batch(&mut backend, &mut cursor, &batch);
    assert!(matches!(outcome, ApplyOutcome::Applied { .. }));

    let view = backend.normalized_view();
    assert_eq!(view.variables.len(), 1);
    let (vid, vbounds, vtype, vactive, vsemi) = view.variables[0];
    assert_eq!(vid, v);
    assert_eq!(vbounds, Bounds::new(1.0, 10.0));
    assert_eq!(vtype, VarType::Integer);
    assert!(vactive);
    assert!(vsemi.is_none());
}

#[test]
fn apply_remove_variable() {
    let v = var_id(0);
    let (mut backend, mut cursor) = empty_backend();
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();
    let r2 = r1.next().unwrap();

    let ops1 = vec![ModelOp::AddVariable {
        var: v,
        bounds: Bounds::NON_NEGATIVE,
        var_type: VarType::Continuous,
    }];
    let batch1 = DeltaBatch::new(r0, r1, ops1).unwrap();
    apply_batch(&mut backend, &mut cursor, &batch1);
    assert_eq!(backend.normalized_view().variables.len(), 1);

    let batch2 = DeltaBatch::new(r1, r2, vec![ModelOp::RemoveVariable { var: v }]).unwrap();
    let outcome = apply_batch(&mut backend, &mut cursor, &batch2);
    assert!(matches!(outcome, ApplyOutcome::Applied { .. }));

    assert!(backend.normalized_view().variables.is_empty());
}

#[test]
fn apply_set_variable_bounds() {
    let v = var_id(0);
    let (mut backend, mut cursor) = empty_backend();
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();
    let r2 = r1.next().unwrap();

    let batch1 = DeltaBatch::new(
        r0,
        r1,
        vec![ModelOp::AddVariable {
            var: v,
            bounds: Bounds::NON_NEGATIVE,
            var_type: VarType::Continuous,
        }],
    )
    .unwrap();
    apply_batch(&mut backend, &mut cursor, &batch1);

    let new_bounds = Bounds::new(5.0, 20.0);
    let batch2 = DeltaBatch::new(
        r1,
        r2,
        vec![ModelOp::SetVariableBounds {
            var: v,
            bounds: new_bounds,
        }],
    )
    .unwrap();
    let outcome = apply_batch(&mut backend, &mut cursor, &batch2);
    assert!(matches!(outcome, ApplyOutcome::Applied { .. }));

    assert_eq!(backend.normalized_view().variables[0].1, new_bounds);
}

#[test]
fn apply_set_variable_active() {
    let v = var_id(0);
    let (mut backend, mut cursor) = empty_backend();
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();
    let r2 = r1.next().unwrap();

    let batch1 = DeltaBatch::new(
        r0,
        r1,
        vec![ModelOp::AddVariable {
            var: v,
            bounds: Bounds::NON_NEGATIVE,
            var_type: VarType::Continuous,
        }],
    )
    .unwrap();
    apply_batch(&mut backend, &mut cursor, &batch1);
    assert!(backend.normalized_view().variables[0].3);

    let batch2 = DeltaBatch::new(
        r1,
        r2,
        vec![ModelOp::SetVariableActive {
            var: v,
            active: false,
        }],
    )
    .unwrap();
    let outcome = apply_batch(&mut backend, &mut cursor, &batch2);
    assert!(matches!(outcome, ApplyOutcome::Applied { .. }));

    assert!(!backend.normalized_view().variables[0].3);
}

#[test]
fn apply_set_variable_type() {
    let v = var_id(0);
    let (mut backend, mut cursor) = empty_backend();
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();
    let r2 = r1.next().unwrap();

    let batch1 = DeltaBatch::new(
        r0,
        r1,
        vec![ModelOp::AddVariable {
            var: v,
            bounds: Bounds::NON_NEGATIVE,
            var_type: VarType::Continuous,
        }],
    )
    .unwrap();
    apply_batch(&mut backend, &mut cursor, &batch1);

    let batch2 = DeltaBatch::new(
        r1,
        r2,
        vec![ModelOp::SetVariableType {
            var: v,
            var_type: VarType::Binary,
        }],
    )
    .unwrap();
    let outcome = apply_batch(&mut backend, &mut cursor, &batch2);
    assert!(matches!(outcome, ApplyOutcome::Applied { .. }));

    assert_eq!(backend.normalized_view().variables[0].2, VarType::Binary);
}

#[test]
fn apply_add_constraint() {
    let c = con_id(0);
    let (mut backend, mut cursor) = empty_backend();
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();

    let ops = vec![ModelOp::AddConstraint {
        con: c,
        bounds: ConstraintBounds::range(10.0, 50.0),
    }];
    let batch = DeltaBatch::new(r0, r1, ops).unwrap();
    let outcome = apply_batch(&mut backend, &mut cursor, &batch);
    assert!(matches!(outcome, ApplyOutcome::Applied { .. }));

    let view = backend.normalized_view();
    assert_eq!(view.constraints.len(), 1);
    assert_eq!(view.constraints[0].0, c);
    assert_eq!(view.constraints[0].1, ConstraintBounds::range(10.0, 50.0));
    assert!(view.constraints[0].2);
}

#[test]
fn apply_remove_constraint() {
    let v = var_id(0);
    let c = con_id(0);
    let (mut backend, mut cursor) = empty_backend();
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();
    let r2 = r1.next().unwrap();

    let batch1 = DeltaBatch::new(
        r0,
        r1,
        vec![
            ModelOp::AddVariable {
                var: v,
                bounds: Bounds::NON_NEGATIVE,
                var_type: VarType::Continuous,
            },
            ModelOp::AddConstraint {
                con: c,
                bounds: ConstraintBounds::le(100.0),
            },
            ModelOp::SetCell {
                cell_key: (CoefficientTarget::Constraint(c), v),
                value_expr: ValueExpr::constant(3.0),
                evaluated_value: 3.0,
            },
        ],
    )
    .unwrap();
    apply_batch(&mut backend, &mut cursor, &batch1);
    assert_eq!(backend.normalized_view().constraints.len(), 1);
    assert_eq!(backend.normalized_view().cells.len(), 1);

    let batch2 = DeltaBatch::new(r1, r2, vec![ModelOp::RemoveConstraint { con: c }]).unwrap();
    let outcome = apply_batch(&mut backend, &mut cursor, &batch2);
    assert!(matches!(outcome, ApplyOutcome::Applied { .. }));

    let view = backend.normalized_view();
    assert!(view.constraints.is_empty());
    assert!(view.cells.is_empty());
}

#[test]
fn apply_set_constraint_bounds() {
    let c = con_id(0);
    let (mut backend, mut cursor) = empty_backend();
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();
    let r2 = r1.next().unwrap();

    let batch1 = DeltaBatch::new(
        r0,
        r1,
        vec![ModelOp::AddConstraint {
            con: c,
            bounds: ConstraintBounds::le(100.0),
        }],
    )
    .unwrap();
    apply_batch(&mut backend, &mut cursor, &batch1);

    let batch2 = DeltaBatch::new(
        r1,
        r2,
        vec![ModelOp::SetConstraintBounds {
            con: c,
            bounds: ConstraintBounds::ge(50.0),
        }],
    )
    .unwrap();
    let outcome = apply_batch(&mut backend, &mut cursor, &batch2);
    assert!(matches!(outcome, ApplyOutcome::Applied { .. }));

    assert_eq!(
        backend.normalized_view().constraints[0].1,
        ConstraintBounds::ge(50.0)
    );
}

#[test]
fn apply_set_constraint_active() {
    let c = con_id(0);
    let (mut backend, mut cursor) = empty_backend();
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();
    let r2 = r1.next().unwrap();

    let batch1 = DeltaBatch::new(
        r0,
        r1,
        vec![ModelOp::AddConstraint {
            con: c,
            bounds: ConstraintBounds::le(100.0),
        }],
    )
    .unwrap();
    apply_batch(&mut backend, &mut cursor, &batch1);
    assert!(backend.normalized_view().constraints[0].2);

    let batch2 = DeltaBatch::new(
        r1,
        r2,
        vec![ModelOp::SetConstraintActive {
            con: c,
            active: false,
        }],
    )
    .unwrap();
    let outcome = apply_batch(&mut backend, &mut cursor, &batch2);
    assert!(matches!(outcome, ApplyOutcome::Applied { .. }));

    assert!(!backend.normalized_view().constraints[0].2);
}

#[test]
fn apply_set_cell() {
    let v = var_id(0);
    let c = con_id(0);
    let (mut backend, mut cursor) = empty_backend();
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();

    let ops = vec![
        ModelOp::AddVariable {
            var: v,
            bounds: Bounds::NON_NEGATIVE,
            var_type: VarType::Continuous,
        },
        ModelOp::AddConstraint {
            con: c,
            bounds: ConstraintBounds::le(100.0),
        },
        ModelOp::SetCell {
            cell_key: (CoefficientTarget::Constraint(c), v),
            value_expr: ValueExpr::constant(7.5),
            evaluated_value: 7.5,
        },
    ];
    let batch = DeltaBatch::new(r0, r1, ops).unwrap();
    let outcome = apply_batch(&mut backend, &mut cursor, &batch);
    assert!(matches!(outcome, ApplyOutcome::Applied { .. }));

    let view = backend.normalized_view();
    assert_eq!(view.cells.len(), 1);
    let (ckey, cval) = view.cells[0];
    assert_eq!(ckey, (CoefficientTarget::Constraint(c), v));
    assert!((cval - 7.5).abs() < 1e-12);
}

#[test]
fn apply_remove_cell() {
    let v = var_id(0);
    let c = con_id(0);
    let (mut backend, mut cursor) = empty_backend();
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();
    let r2 = r1.next().unwrap();

    let batch1 = DeltaBatch::new(
        r0,
        r1,
        vec![
            ModelOp::AddVariable {
                var: v,
                bounds: Bounds::NON_NEGATIVE,
                var_type: VarType::Continuous,
            },
            ModelOp::AddConstraint {
                con: c,
                bounds: ConstraintBounds::le(100.0),
            },
            ModelOp::SetCell {
                cell_key: (CoefficientTarget::Constraint(c), v),
                value_expr: ValueExpr::constant(3.0),
                evaluated_value: 3.0,
            },
        ],
    )
    .unwrap();
    apply_batch(&mut backend, &mut cursor, &batch1);
    assert_eq!(backend.normalized_view().cells.len(), 1);

    let batch2 = DeltaBatch::new(
        r1,
        r2,
        vec![ModelOp::RemoveCell {
            cell_key: (CoefficientTarget::Constraint(c), v),
        }],
    )
    .unwrap();
    let outcome = apply_batch(&mut backend, &mut cursor, &batch2);
    assert!(matches!(outcome, ApplyOutcome::Applied { .. }));

    assert!(backend.normalized_view().cells.is_empty());
}

#[test]
fn apply_add_objective() {
    let o = obj_id(0);
    let (mut backend, mut cursor) = empty_backend();
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();

    let ops = vec![ModelOp::AddObjective {
        obj: o,
        sense: Sense::Maximize,
    }];
    let batch = DeltaBatch::new(r0, r1, ops).unwrap();
    let outcome = apply_batch(&mut backend, &mut cursor, &batch);
    assert!(matches!(outcome, ApplyOutcome::Applied { .. }));

    let view = backend.normalized_view();
    assert_eq!(view.objectives.len(), 1);
    assert_eq!(view.objectives[0].0, o);
    assert_eq!(view.objectives[0].1, Sense::Maximize);
    assert!(!view.objectives[0].2);
    assert!(view.active_objective.is_none());
}

#[test]
fn apply_remove_objective() {
    let v = var_id(0);
    let o = obj_id(0);
    let (mut backend, mut cursor) = empty_backend();
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();
    let r2 = r1.next().unwrap();

    let batch1 = DeltaBatch::new(
        r0,
        r1,
        vec![
            ModelOp::AddVariable {
                var: v,
                bounds: Bounds::NON_NEGATIVE,
                var_type: VarType::Continuous,
            },
            ModelOp::AddObjective {
                obj: o,
                sense: Sense::Minimize,
            },
            ModelOp::SetObjectiveCell {
                cell_key: (CoefficientTarget::Objective(o), v),
                value_expr: ValueExpr::constant(2.0),
                evaluated_value: 2.0,
                constant: 0.0,
            },
        ],
    )
    .unwrap();
    apply_batch(&mut backend, &mut cursor, &batch1);
    assert_eq!(backend.normalized_view().objectives.len(), 1);
    assert_eq!(backend.normalized_view().objective_cells.len(), 1);

    let batch2 = DeltaBatch::new(r1, r2, vec![ModelOp::RemoveObjective { obj: o }]).unwrap();
    let outcome = apply_batch(&mut backend, &mut cursor, &batch2);
    assert!(matches!(outcome, ApplyOutcome::Applied { .. }));

    let view = backend.normalized_view();
    assert!(view.objectives.is_empty());
    assert!(view.objective_cells.is_empty());
    assert!(view.active_objective.is_none());
}

#[test]
fn apply_set_active_objective() {
    let o1 = obj_id(0);
    let o2 = obj_id(1);
    let (mut backend, mut cursor) = empty_backend();
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();
    let r2 = r1.next().unwrap();
    let r3 = r2.next().unwrap();

    let batch1 = DeltaBatch::new(
        r0,
        r1,
        vec![
            ModelOp::AddObjective {
                obj: o1,
                sense: Sense::Minimize,
            },
            ModelOp::AddObjective {
                obj: o2,
                sense: Sense::Maximize,
            },
        ],
    )
    .unwrap();
    apply_batch(&mut backend, &mut cursor, &batch1);

    // Activate o1
    let batch2 =
        DeltaBatch::new(r1, r2, vec![ModelOp::SetActiveObjective { obj: Some(o1) }]).unwrap();
    apply_batch(&mut backend, &mut cursor, &batch2);

    let view = backend.normalized_view();
    assert_eq!(view.active_objective, Some(o1));
    let o1_active = view.objectives.iter().any(|(id, _, a)| *id == o1 && *a);
    let o2_inactive = view.objectives.iter().any(|(id, _, a)| *id == o2 && !*a);
    assert!(o1_active);
    assert!(o2_inactive);

    // Switch to o2
    let batch3 =
        DeltaBatch::new(r2, r3, vec![ModelOp::SetActiveObjective { obj: Some(o2) }]).unwrap();
    let outcome = apply_batch(&mut backend, &mut cursor, &batch3);
    assert!(matches!(outcome, ApplyOutcome::Applied { .. }));

    let view = backend.normalized_view();
    assert_eq!(view.active_objective, Some(o2));
    let o1_now_inactive = view.objectives.iter().any(|(id, _, a)| *id == o1 && !*a);
    let o2_now_active = view.objectives.iter().any(|(id, _, a)| *id == o2 && *a);
    assert!(o1_now_inactive);
    assert!(o2_now_active);
}

#[test]
fn apply_clear_active_objective() {
    let o = obj_id(0);
    let (mut backend, mut cursor) = empty_backend();
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();
    let r2 = r1.next().unwrap();
    let r3 = r2.next().unwrap();

    let batch1 = DeltaBatch::new(
        r0,
        r1,
        vec![ModelOp::AddObjective {
            obj: o,
            sense: Sense::Minimize,
        }],
    )
    .unwrap();
    apply_batch(&mut backend, &mut cursor, &batch1);

    // Activate
    let batch2 =
        DeltaBatch::new(r1, r2, vec![ModelOp::SetActiveObjective { obj: Some(o) }]).unwrap();
    apply_batch(&mut backend, &mut cursor, &batch2);
    assert_eq!(backend.normalized_view().active_objective, Some(o));

    // Clear with None
    let batch3 = DeltaBatch::new(r2, r3, vec![ModelOp::SetActiveObjective { obj: None }]).unwrap();
    let outcome = apply_batch(&mut backend, &mut cursor, &batch3);
    assert!(matches!(outcome, ApplyOutcome::Applied { .. }));

    let view = backend.normalized_view();
    assert!(view.active_objective.is_none());
    assert!(!view.objectives[0].2);
}

#[test]
fn apply_set_objective_cell() {
    let v = var_id(0);
    let o = obj_id(0);
    let (mut backend, mut cursor) = empty_backend();
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();

    let ops = vec![
        ModelOp::AddVariable {
            var: v,
            bounds: Bounds::NON_NEGATIVE,
            var_type: VarType::Continuous,
        },
        ModelOp::AddObjective {
            obj: o,
            sense: Sense::Minimize,
        },
        ModelOp::SetObjectiveCell {
            cell_key: (CoefficientTarget::Objective(o), v),
            value_expr: ValueExpr::constant(3.5),
            evaluated_value: 3.5,
            constant: 10.0,
        },
    ];
    let batch = DeltaBatch::new(r0, r1, ops).unwrap();
    let outcome = apply_batch(&mut backend, &mut cursor, &batch);
    assert!(matches!(outcome, ApplyOutcome::Applied { .. }));

    let view = backend.normalized_view();
    assert_eq!(view.objective_cells.len(), 1);
    let (ckey, cval, cconst) = view.objective_cells[0];
    assert_eq!(ckey, (CoefficientTarget::Objective(o), v));
    assert!((cval - 3.5).abs() < 1e-12);
    assert!((cconst - 10.0).abs() < 1e-12);
}

#[test]
fn apply_set_parameter() {
    let p = param_id(0);
    let (mut backend, mut cursor) = empty_backend();
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();

    let ops = vec![ModelOp::SetParameter {
        param: p,
        value: 99.5,
    }];
    let batch = DeltaBatch::new(r0, r1, ops).unwrap();
    let outcome = apply_batch(&mut backend, &mut cursor, &batch);
    assert!(matches!(outcome, ApplyOutcome::Applied { .. }));

    let view = backend.normalized_view();
    assert_eq!(view.parameters.len(), 1);
    assert_eq!(view.parameters[0], (p, 99.5));
}

#[test]
fn apply_update_parameter() {
    let p = param_id(0);
    let (mut backend, mut cursor) = empty_backend();
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();
    let r2 = r1.next().unwrap();

    let batch1 = DeltaBatch::new(
        r0,
        r1,
        vec![ModelOp::SetParameter {
            param: p,
            value: 10.0,
        }],
    )
    .unwrap();
    apply_batch(&mut backend, &mut cursor, &batch1);
    assert!((backend.normalized_view().parameters[0].1 - 10.0).abs() < 1e-12);

    let batch2 = DeltaBatch::new(
        r1,
        r2,
        vec![ModelOp::SetParameter {
            param: p,
            value: 25.0,
        }],
    )
    .unwrap();
    let outcome = apply_batch(&mut backend, &mut cursor, &batch2);
    assert!(matches!(outcome, ApplyOutcome::Applied { .. }));

    assert!((backend.normalized_view().parameters[0].1 - 25.0).abs() < 1e-12);
}

// ── Tests: Add/remove/reindex sequences ──────────────────────────────────────

#[test]
fn add_remove_var_cascading_cell_removal() {
    let v = var_id(0);
    let c = con_id(0);
    let (mut backend, mut cursor) = empty_backend();
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();
    let r2 = r1.next().unwrap();

    let batch1 = DeltaBatch::new(
        r0,
        r1,
        vec![
            ModelOp::AddVariable {
                var: v,
                bounds: Bounds::NON_NEGATIVE,
                var_type: VarType::Continuous,
            },
            ModelOp::AddConstraint {
                con: c,
                bounds: ConstraintBounds::le(100.0),
            },
            ModelOp::SetCell {
                cell_key: (CoefficientTarget::Constraint(c), v),
                value_expr: ValueExpr::constant(5.0),
                evaluated_value: 5.0,
            },
        ],
    )
    .unwrap();
    apply_batch(&mut backend, &mut cursor, &batch1);

    let view1 = backend.normalized_view();
    assert_eq!(view1.variables.len(), 1);
    assert_eq!(view1.constraints.len(), 1);
    assert_eq!(view1.cells.len(), 1);

    // Remove var — cells involving this var should cascade
    let batch2 = DeltaBatch::new(r1, r2, vec![ModelOp::RemoveVariable { var: v }]).unwrap();
    let outcome = apply_batch(&mut backend, &mut cursor, &batch2);
    assert!(matches!(outcome, ApplyOutcome::Applied { .. }));

    let view2 = backend.normalized_view();
    assert!(view2.variables.is_empty());
    assert_eq!(view2.constraints.len(), 1);
    assert!(view2.cells.is_empty());
}

#[test]
fn remove_var_with_multiple_cells_only_cascades_matching() {
    let v1 = var_id(0);
    let v2 = var_id(1);
    let c = con_id(0);
    let (mut backend, mut cursor) = empty_backend();
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();
    let r2 = r1.next().unwrap();

    let batch1 = DeltaBatch::new(
        r0,
        r1,
        vec![
            ModelOp::AddVariable {
                var: v1,
                bounds: Bounds::NON_NEGATIVE,
                var_type: VarType::Continuous,
            },
            ModelOp::AddVariable {
                var: v2,
                bounds: Bounds::NON_NEGATIVE,
                var_type: VarType::Continuous,
            },
            ModelOp::AddConstraint {
                con: c,
                bounds: ConstraintBounds::le(100.0),
            },
            ModelOp::SetCell {
                cell_key: (CoefficientTarget::Constraint(c), v1),
                value_expr: ValueExpr::constant(1.0),
                evaluated_value: 1.0,
            },
            ModelOp::SetCell {
                cell_key: (CoefficientTarget::Constraint(c), v2),
                value_expr: ValueExpr::constant(2.0),
                evaluated_value: 2.0,
            },
        ],
    )
    .unwrap();
    apply_batch(&mut backend, &mut cursor, &batch1);
    assert_eq!(backend.normalized_view().cells.len(), 2);

    let batch2 = DeltaBatch::new(r1, r2, vec![ModelOp::RemoveVariable { var: v1 }]).unwrap();
    let outcome = apply_batch(&mut backend, &mut cursor, &batch2);
    assert!(matches!(outcome, ApplyOutcome::Applied { .. }));

    let view = backend.normalized_view();
    assert_eq!(view.variables.len(), 1);
    assert_eq!(view.cells.len(), 1);
    assert_eq!(view.cells[0].0 .1, v2);
}

#[test]
fn add_remove_constraint_cascade() {
    let v = var_id(0);
    let c = con_id(0);
    let (mut backend, mut cursor) = empty_backend();
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();
    let r2 = r1.next().unwrap();

    let batch1 = DeltaBatch::new(
        r0,
        r1,
        vec![
            ModelOp::AddVariable {
                var: v,
                bounds: Bounds::NON_NEGATIVE,
                var_type: VarType::Continuous,
            },
            ModelOp::AddConstraint {
                con: c,
                bounds: ConstraintBounds::le(100.0),
            },
            ModelOp::SetCell {
                cell_key: (CoefficientTarget::Constraint(c), v),
                value_expr: ValueExpr::constant(5.0),
                evaluated_value: 5.0,
            },
        ],
    )
    .unwrap();
    apply_batch(&mut backend, &mut cursor, &batch1);
    assert_eq!(backend.normalized_view().cells.len(), 1);

    let batch2 = DeltaBatch::new(r1, r2, vec![ModelOp::RemoveConstraint { con: c }]).unwrap();
    let outcome = apply_batch(&mut backend, &mut cursor, &batch2);
    assert!(matches!(outcome, ApplyOutcome::Applied { .. }));

    let view = backend.normalized_view();
    assert_eq!(view.variables.len(), 1);
    assert!(view.constraints.is_empty());
    assert!(view.cells.is_empty());
}

// ── Tests: Activity toggles ──────────────────────────────────────────────────

#[test]
fn activity_toggle_variable_full_cycle() {
    let v = var_id(0);
    let (mut backend, mut cursor) = empty_backend();
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();
    let r2 = r1.next().unwrap();
    let r3 = r2.next().unwrap();

    let batch1 = DeltaBatch::new(
        r0,
        r1,
        vec![ModelOp::AddVariable {
            var: v,
            bounds: Bounds::NON_NEGATIVE,
            var_type: VarType::Continuous,
        }],
    )
    .unwrap();
    apply_batch(&mut backend, &mut cursor, &batch1);
    assert!(backend.normalized_view().variables[0].3);

    // Deactivate
    let batch2 = DeltaBatch::new(
        r1,
        r2,
        vec![ModelOp::SetVariableActive {
            var: v,
            active: false,
        }],
    )
    .unwrap();
    apply_batch(&mut backend, &mut cursor, &batch2);
    assert!(!backend.normalized_view().variables[0].3);

    // Reactivate
    let batch3 = DeltaBatch::new(
        r2,
        r3,
        vec![ModelOp::SetVariableActive {
            var: v,
            active: true,
        }],
    )
    .unwrap();
    let outcome = apply_batch(&mut backend, &mut cursor, &batch3);
    assert!(matches!(outcome, ApplyOutcome::Applied { .. }));
    assert!(backend.normalized_view().variables[0].3);
}

#[test]
fn activity_toggle_constraint_full_cycle() {
    let c = con_id(0);
    let (mut backend, mut cursor) = empty_backend();
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();
    let r2 = r1.next().unwrap();
    let r3 = r2.next().unwrap();

    let batch1 = DeltaBatch::new(
        r0,
        r1,
        vec![ModelOp::AddConstraint {
            con: c,
            bounds: ConstraintBounds::le(100.0),
        }],
    )
    .unwrap();
    apply_batch(&mut backend, &mut cursor, &batch1);
    assert!(backend.normalized_view().constraints[0].2);

    let batch2 = DeltaBatch::new(
        r1,
        r2,
        vec![ModelOp::SetConstraintActive {
            con: c,
            active: false,
        }],
    )
    .unwrap();
    apply_batch(&mut backend, &mut cursor, &batch2);
    assert!(!backend.normalized_view().constraints[0].2);

    let batch3 = DeltaBatch::new(
        r2,
        r3,
        vec![ModelOp::SetConstraintActive {
            con: c,
            active: true,
        }],
    )
    .unwrap();
    let outcome = apply_batch(&mut backend, &mut cursor, &batch3);
    assert!(matches!(outcome, ApplyOutcome::Applied { .. }));
    assert!(backend.normalized_view().constraints[0].2);
}

// ── Tests: Objective switch ──────────────────────────────────────────────────

#[test]
fn objective_switch_deactivates_previous() {
    let o1 = obj_id(0);
    let o2 = obj_id(1);
    let (mut backend, mut cursor) = empty_backend();
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();
    let r2 = r1.next().unwrap();
    let r3 = r2.next().unwrap();
    let r4 = r3.next().unwrap();

    let batch1 = DeltaBatch::new(
        r0,
        r1,
        vec![
            ModelOp::AddObjective {
                obj: o1,
                sense: Sense::Minimize,
            },
            ModelOp::AddObjective {
                obj: o2,
                sense: Sense::Maximize,
            },
        ],
    )
    .unwrap();
    apply_batch(&mut backend, &mut cursor, &batch1);

    // Activate o1
    let batch2 =
        DeltaBatch::new(r1, r2, vec![ModelOp::SetActiveObjective { obj: Some(o1) }]).unwrap();
    apply_batch(&mut backend, &mut cursor, &batch2);
    assert_eq!(backend.normalized_view().active_objective, Some(o1));

    // Switch to o2 — o1 deactivates, o2 activates
    let batch3 =
        DeltaBatch::new(r2, r3, vec![ModelOp::SetActiveObjective { obj: Some(o2) }]).unwrap();
    apply_batch(&mut backend, &mut cursor, &batch3);

    let view = backend.normalized_view();
    assert_eq!(view.active_objective, Some(o2));
    assert!(view.objectives.iter().any(|(id, _, a)| *id == o1 && !*a));
    assert!(view.objectives.iter().any(|(id, _, a)| *id == o2 && *a));

    // Switch back to o1
    let batch4 =
        DeltaBatch::new(r3, r4, vec![ModelOp::SetActiveObjective { obj: Some(o1) }]).unwrap();
    apply_batch(&mut backend, &mut cursor, &batch4);

    let view = backend.normalized_view();
    assert_eq!(view.active_objective, Some(o1));
}

// ── Tests: Parameter-driven cell changes ─────────────────────────────────────

#[test]
fn parameter_driven_cell_update() {
    let v = var_id(0);
    let c = con_id(0);
    let p = param_id(0);
    let (mut backend, mut cursor) = empty_backend();
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();
    let r2 = r1.next().unwrap();
    let r3 = r2.next().unwrap();

    // Set param to 10, create cell with value = param * 2 = 20
    let batch1 = DeltaBatch::new(
        r0,
        r1,
        vec![
            ModelOp::SetParameter {
                param: p,
                value: 10.0,
            },
            ModelOp::AddVariable {
                var: v,
                bounds: Bounds::NON_NEGATIVE,
                var_type: VarType::Continuous,
            },
            ModelOp::AddConstraint {
                con: c,
                bounds: ConstraintBounds::le(100.0),
            },
            ModelOp::SetCell {
                cell_key: (CoefficientTarget::Constraint(c), v),
                value_expr: ValueExpr::mul(ValueExpr::constant(2.0), ValueExpr::param(p)),
                evaluated_value: 20.0,
            },
        ],
    )
    .unwrap();
    apply_batch(&mut backend, &mut cursor, &batch1);
    assert!((backend.normalized_view().cells[0].1 - 20.0).abs() < 1e-12);
    assert!((backend.normalized_view().parameters[0].1 - 10.0).abs() < 1e-12);

    // Update param to 15
    let batch2 = DeltaBatch::new(
        r1,
        r2,
        vec![ModelOp::SetParameter {
            param: p,
            value: 15.0,
        }],
    )
    .unwrap();
    apply_batch(&mut backend, &mut cursor, &batch2);
    assert!((backend.normalized_view().parameters[0].1 - 15.0).abs() < 1e-12);

    // Now update cell to reflect new param value (2 * 15 = 30)
    let batch3 = DeltaBatch::new(
        r2,
        r3,
        vec![ModelOp::SetCell {
            cell_key: (CoefficientTarget::Constraint(c), v),
            value_expr: ValueExpr::mul(ValueExpr::constant(2.0), ValueExpr::param(p)),
            evaluated_value: 30.0,
        }],
    )
    .unwrap();
    let outcome = apply_batch(&mut backend, &mut cursor, &batch3);
    assert!(matches!(outcome, ApplyOutcome::Applied { .. }));

    let view = backend.normalized_view();
    assert!((view.cells[0].1 - 30.0).abs() < 1e-12);
    assert!((view.parameters[0].1 - 15.0).abs() < 1e-12);
}

#[test]
fn parameter_driven_objective_cell_update() {
    let v = var_id(0);
    let o = obj_id(0);
    let p = param_id(0);
    let (mut backend, mut cursor) = empty_backend();
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();
    let r2 = r1.next().unwrap();

    let batch1 = DeltaBatch::new(
        r0,
        r1,
        vec![
            ModelOp::SetParameter {
                param: p,
                value: 5.0,
            },
            ModelOp::AddVariable {
                var: v,
                bounds: Bounds::NON_NEGATIVE,
                var_type: VarType::Continuous,
            },
            ModelOp::AddObjective {
                obj: o,
                sense: Sense::Minimize,
            },
            ModelOp::SetObjectiveCell {
                cell_key: (CoefficientTarget::Objective(o), v),
                value_expr: ValueExpr::param(p),
                evaluated_value: 5.0,
                constant: 0.0,
            },
        ],
    )
    .unwrap();
    apply_batch(&mut backend, &mut cursor, &batch1);

    let batch2 = DeltaBatch::new(
        r1,
        r2,
        vec![
            ModelOp::SetParameter {
                param: p,
                value: 8.0,
            },
            ModelOp::SetObjectiveCell {
                cell_key: (CoefficientTarget::Objective(o), v),
                value_expr: ValueExpr::param(p),
                evaluated_value: 8.0,
                constant: 0.0,
            },
        ],
    )
    .unwrap();
    let outcome = apply_batch(&mut backend, &mut cursor, &batch2);
    assert!(matches!(outcome, ApplyOutcome::Applied { .. }));

    assert!((backend.normalized_view().objective_cells[0].1 - 8.0).abs() < 1e-12);
}

// ── Tests: Unsupported operation → rebuild ───────────────────────────────────

#[test]
fn recoverable_failure_triggers_rebuild_cycle() {
    let v = var_id(0);
    let (mut backend, mut cursor) = empty_backend();
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();
    let r2 = r1.next().unwrap();

    // Advance cursor to r1
    let batch1 = DeltaBatch::new(
        r0,
        r1,
        vec![ModelOp::AddVariable {
            var: v,
            bounds: Bounds::NON_NEGATIVE,
            var_type: VarType::Continuous,
        }],
    )
    .unwrap();
    apply_batch(&mut backend, &mut cursor, &batch1);
    assert!(cursor.is_ready());
    assert_eq!(cursor.applied_revision, r1);

    // Try batch with wrong from revision (from=r0 instead of r1)
    let bad_batch = DeltaBatch::new(
        r0,
        r2,
        vec![ModelOp::SetVariableBounds {
            var: v,
            bounds: Bounds::new(0.0, 50.0),
        }],
    )
    .unwrap();
    let outcome = apply_batch(&mut backend, &mut cursor, &bad_batch);
    match &outcome {
        ApplyOutcome::RecoverableFailure { reason } => {
            assert!(reason.contains("!="), "reason: {reason}");
        }
        other => panic!("expected RecoverableFailure, got {other:?}"),
    }

    // Cursor unchanged
    assert_eq!(cursor.applied_revision, r1);

    // Rebuild from snapshot at r1 to recover
    let snapshot = sample_snapshot(r1);
    rebuild_from_snapshot(&mut backend, &mut cursor, &snapshot);
    assert!(cursor.is_ready());
    assert_eq!(cursor.applied_revision, r1);
}

#[test]
fn corrupt_state_rebuild_recovers() {
    let c = con_id(0);
    let (mut backend, mut cursor) = empty_backend();
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();

    let snapshot = sample_snapshot(r1);
    rebuild_from_snapshot(&mut backend, &mut cursor, &snapshot);

    let clean_view = backend.normalized_view();
    assert_eq!(clean_view.variables.len(), 1);
    assert_eq!(clean_view.cells.len(), 1);

    // Manually corrupt: insert a dangling cell (var not in variables map)
    backend.constraint_cells.insert(
        (CoefficientTarget::Constraint(c), var_id(99)),
        (ValueExpr::constant(999.0), 999.0),
    );
    assert_eq!(backend.normalized_view().cells.len(), 2);

    // Rebuild from same snapshot — should clear corruption
    rebuild_from_snapshot(&mut backend, &mut cursor, &snapshot);

    let recovered_view = backend.normalized_view();
    assert_eq!(recovered_view, clean_view);
    assert!(cursor.is_ready());
    assert_eq!(cursor.applied_revision, r1);
}

// ── Tests: Normalized view equivalence ───────────────────────────────────────

#[test]
fn normalized_view_equivalence_same_operations() {
    let v = var_id(0);
    let c = con_id(0);
    let p = param_id(0);
    let o = obj_id(0);

    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();
    let r2 = r1.next().unwrap();

    let snap_r0 = ModelSnapshot::empty(r0);

    // Backend A: rebuild from r0, then apply r0→r1 batch
    let (mut backend_a, mut cursor_a) = empty_backend();
    rebuild_from_snapshot(&mut backend_a, &mut cursor_a, &snap_r0);

    let batch_a = DeltaBatch::new(
        r0,
        r1,
        vec![
            ModelOp::AddVariable {
                var: v,
                bounds: Bounds::NON_NEGATIVE,
                var_type: VarType::Continuous,
            },
            ModelOp::AddConstraint {
                con: c,
                bounds: ConstraintBounds::le(100.0),
            },
            ModelOp::SetParameter {
                param: p,
                value: 42.0,
            },
            ModelOp::SetCell {
                cell_key: (CoefficientTarget::Constraint(c), v),
                value_expr: ValueExpr::param(p),
                evaluated_value: 42.0,
            },
        ],
    )
    .unwrap();
    apply_batch(&mut backend_a, &mut cursor_a, &batch_a);
    let view_a = backend_a.normalized_view();

    // Backend B: same sequence
    let (mut backend_b, mut cursor_b) = empty_backend();
    rebuild_from_snapshot(&mut backend_b, &mut cursor_b, &snap_r0);

    let batch_b = DeltaBatch::new(
        r0,
        r1,
        vec![
            ModelOp::AddVariable {
                var: v,
                bounds: Bounds::NON_NEGATIVE,
                var_type: VarType::Continuous,
            },
            ModelOp::AddConstraint {
                con: c,
                bounds: ConstraintBounds::le(100.0),
            },
            ModelOp::SetParameter {
                param: p,
                value: 42.0,
            },
            ModelOp::SetCell {
                cell_key: (CoefficientTarget::Constraint(c), v),
                value_expr: ValueExpr::param(p),
                evaluated_value: 42.0,
            },
        ],
    )
    .unwrap();
    apply_batch(&mut backend_b, &mut cursor_b, &batch_b);
    let view_b = backend_b.normalized_view();

    assert_eq!(view_a, view_b);

    // Apply more ops to both
    let batch_adv = DeltaBatch::new(
        r1,
        r2,
        vec![
            ModelOp::AddObjective {
                obj: o,
                sense: Sense::Maximize,
            },
            ModelOp::SetActiveObjective { obj: Some(o) },
            ModelOp::SetObjectiveCell {
                cell_key: (CoefficientTarget::Objective(o), v),
                value_expr: ValueExpr::constant(3.0),
                evaluated_value: 3.0,
                constant: 5.0,
            },
        ],
    )
    .unwrap();
    apply_batch(&mut backend_a, &mut cursor_a, &batch_adv);

    // Reset B to r0 and replay everything
    rebuild_from_snapshot(&mut backend_b, &mut cursor_b, &snap_r0);
    apply_batch(&mut backend_b, &mut cursor_b, &batch_b);
    apply_batch(&mut backend_b, &mut cursor_b, &batch_adv);

    assert_eq!(backend_a.normalized_view(), backend_b.normalized_view());
}

#[test]
fn normalized_view_commuting_square() {
    // project(snapshot r1) == apply(project(snapshot r0), deltas r0→r1)
    let v = var_id(0);
    let c = con_id(0);
    let p = param_id(0);

    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();

    let mut vars = HashMap::new();
    vars.insert(v, (Bounds::NON_NEGATIVE, VarType::Continuous, true, None));
    let mut cons = HashMap::new();
    cons.insert(c, (ConstraintBounds::le(100.0), true));
    let objs = HashMap::new();
    let mut params = HashMap::new();
    params.insert(p, 42.0);
    let cells: Vec<((CoefficientTarget, VarId), ValueExpr, f64, Vec<ParamId>)> = vec![(
        (CoefficientTarget::Constraint(c), v),
        ValueExpr::param(p),
        42.0,
        vec![p],
    )];
    let snap_r1 = take_snapshot(r1, &vars, &cons, &objs, &params, &cells);

    // Backend A: rebuild from r1
    let (mut backend_a, mut cursor_a) = empty_backend();
    rebuild_from_snapshot(&mut backend_a, &mut cursor_a, &snap_r1);
    let view_a = backend_a.normalized_view();

    // Backend B: rebuild from r0, apply deltas
    let snap_r0 = ModelSnapshot::empty(r0);
    let (mut backend_b, mut cursor_b) = empty_backend();
    rebuild_from_snapshot(&mut backend_b, &mut cursor_b, &snap_r0);

    let batch = DeltaBatch::new(
        r0,
        r1,
        vec![
            ModelOp::AddVariable {
                var: v,
                bounds: Bounds::NON_NEGATIVE,
                var_type: VarType::Continuous,
            },
            ModelOp::AddConstraint {
                con: c,
                bounds: ConstraintBounds::le(100.0),
            },
            ModelOp::SetParameter {
                param: p,
                value: 42.0,
            },
            ModelOp::SetCell {
                cell_key: (CoefficientTarget::Constraint(c), v),
                value_expr: ValueExpr::param(p),
                evaluated_value: 42.0,
            },
        ],
    )
    .unwrap();
    apply_batch(&mut backend_b, &mut cursor_b, &batch);
    let view_b = backend_b.normalized_view();

    assert_eq!(view_a, view_b);
}

// ── Tests: Batch application invariants ──────────────────────────────────────

#[test]
fn apply_batch_rejects_backwards_revision() {
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();

    assert!(DeltaBatch::new(r1, r0, vec![]).is_none());
}

#[test]
fn apply_empty_batch() {
    let (mut backend, mut cursor) = empty_backend();
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();

    let batch = DeltaBatch::new(r0, r1, vec![]).unwrap();
    let outcome = apply_batch(&mut backend, &mut cursor, &batch);
    assert!(matches!(outcome, ApplyOutcome::Applied { .. }));

    let view = backend.normalized_view();
    assert!(view.variables.is_empty());
    assert!(view.constraints.is_empty());
}

#[test]
fn multiple_batches_advance_cursor() {
    let v = var_id(0);
    let c = con_id(0);
    let (mut backend, mut cursor) = empty_backend();
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();
    let r2 = r1.next().unwrap();

    let batch1 = DeltaBatch::new(
        r0,
        r1,
        vec![ModelOp::AddVariable {
            var: v,
            bounds: Bounds::NON_NEGATIVE,
            var_type: VarType::Continuous,
        }],
    )
    .unwrap();
    apply_batch(&mut backend, &mut cursor, &batch1);
    assert_eq!(cursor.applied_revision, r1);

    let batch2 = DeltaBatch::new(
        r1,
        r2,
        vec![ModelOp::AddConstraint {
            con: c,
            bounds: ConstraintBounds::le(50.0),
        }],
    )
    .unwrap();
    apply_batch(&mut backend, &mut cursor, &batch2);
    assert_eq!(cursor.applied_revision, r2);

    let view = backend.normalized_view();
    assert_eq!(view.variables.len(), 1);
    assert_eq!(view.constraints.len(), 1);
    assert_eq!(view.revision, r2);
}

#[test]
fn rebuild_resets_revision_and_state() {
    let v = var_id(0);
    let (mut backend, mut cursor) = empty_backend();
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();
    let r2 = r1.next().unwrap();

    let batch = DeltaBatch::new(
        r0,
        r1,
        vec![ModelOp::AddVariable {
            var: v,
            bounds: Bounds::NON_NEGATIVE,
            var_type: VarType::Continuous,
        }],
    )
    .unwrap();
    apply_batch(&mut backend, &mut cursor, &batch);
    assert_eq!(backend.normalized_view().variables.len(), 1);

    let snapshot = ModelSnapshot::empty(r2);
    rebuild_from_snapshot(&mut backend, &mut cursor, &snapshot);

    let view = backend.normalized_view();
    assert!(view.variables.is_empty());
    assert_eq!(view.revision, r2);
    assert_eq!(cursor.applied_revision, r2);
}

#[test]
fn rebuild_with_full_snapshot() {
    let rev = ModelRevision::ZERO.next().unwrap();
    let snapshot = sample_snapshot(rev);

    let (mut backend, mut cursor) = empty_backend();
    rebuild_from_snapshot(&mut backend, &mut cursor, &snapshot);

    let view = backend.normalized_view();
    assert_eq!(view.variables.len(), 1);
    assert_eq!(view.constraints.len(), 1);
    assert_eq!(view.parameters.len(), 1);
    assert_eq!(view.cells.len(), 1);

    assert_eq!(view.variables[0].1, Bounds::NON_NEGATIVE);
    assert_eq!(view.variables[0].2, VarType::Continuous);
    assert_eq!(view.constraints[0].1, ConstraintBounds::le(100.0));
    assert!((view.parameters[0].1 - 42.0).abs() < 1e-12);
    assert!((view.cells[0].1 - 2.5).abs() < 1e-12);
}

// ── Tests: Edge cases ────────────────────────────────────────────────────────

#[test]
fn remove_nonexistent_variable_is_noop() {
    let v = var_id(0);
    let v2 = var_id(1);
    let (mut backend, mut cursor) = empty_backend();
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();
    let r2 = r1.next().unwrap();

    let batch1 = DeltaBatch::new(
        r0,
        r1,
        vec![ModelOp::AddVariable {
            var: v,
            bounds: Bounds::NON_NEGATIVE,
            var_type: VarType::Continuous,
        }],
    )
    .unwrap();
    apply_batch(&mut backend, &mut cursor, &batch1);

    let batch2 = DeltaBatch::new(r1, r2, vec![ModelOp::RemoveVariable { var: v2 }]).unwrap();
    let outcome = apply_batch(&mut backend, &mut cursor, &batch2);
    assert!(matches!(outcome, ApplyOutcome::Applied { .. }));

    assert_eq!(backend.normalized_view().variables.len(), 1);
    assert_eq!(backend.normalized_view().variables[0].0, v);
}

#[test]
fn remove_nonexistent_constraint_is_noop() {
    let c = con_id(0);
    let c2 = con_id(1);
    let (mut backend, mut cursor) = empty_backend();
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();
    let r2 = r1.next().unwrap();

    let batch1 = DeltaBatch::new(
        r0,
        r1,
        vec![ModelOp::AddConstraint {
            con: c,
            bounds: ConstraintBounds::le(100.0),
        }],
    )
    .unwrap();
    apply_batch(&mut backend, &mut cursor, &batch1);

    let batch2 = DeltaBatch::new(r1, r2, vec![ModelOp::RemoveConstraint { con: c2 }]).unwrap();
    let outcome = apply_batch(&mut backend, &mut cursor, &batch2);
    assert!(matches!(outcome, ApplyOutcome::Applied { .. }));

    assert_eq!(backend.normalized_view().constraints.len(), 1);
}

#[test]
fn set_bounds_on_nonexistent_variable_is_noop() {
    let v = var_id(0);
    let (mut backend, mut cursor) = empty_backend();
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();

    let batch = DeltaBatch::new(
        r0,
        r1,
        vec![ModelOp::SetVariableBounds {
            var: v,
            bounds: Bounds::new(0.0, 10.0),
        }],
    )
    .unwrap();
    let outcome = apply_batch(&mut backend, &mut cursor, &batch);
    assert!(matches!(outcome, ApplyOutcome::Applied { .. }));
    assert!(backend.normalized_view().variables.is_empty());
}

#[test]
fn set_active_on_nonexistent_objective_does_not_panic() {
    let o = obj_id(0);
    let (mut backend, mut cursor) = empty_backend();
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();

    let batch =
        DeltaBatch::new(r0, r1, vec![ModelOp::SetActiveObjective { obj: Some(o) }]).unwrap();
    let outcome = apply_batch(&mut backend, &mut cursor, &batch);
    assert!(matches!(outcome, ApplyOutcome::Applied { .. }));

    // active_objective is set even if the objective doesn't exist yet
    // (the objective may be added in a later batch)
    assert_eq!(backend.normalized_view().active_objective, Some(o));
}

#[test]
fn multiple_ops_single_batch() {
    let v1 = var_id(0);
    let v2 = var_id(1);
    let c = con_id(0);
    let o = obj_id(0);
    let p = param_id(0);
    let (mut backend, mut cursor) = empty_backend();
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();

    let ops = vec![
        ModelOp::AddVariable {
            var: v1,
            bounds: Bounds::NON_NEGATIVE,
            var_type: VarType::Continuous,
        },
        ModelOp::AddVariable {
            var: v2,
            bounds: Bounds::BINARY,
            var_type: VarType::Binary,
        },
        ModelOp::AddConstraint {
            con: c,
            bounds: ConstraintBounds::eq(50.0),
        },
        ModelOp::AddObjective {
            obj: o,
            sense: Sense::Minimize,
        },
        ModelOp::SetParameter {
            param: p,
            value: 7.0,
        },
        ModelOp::SetCell {
            cell_key: (CoefficientTarget::Constraint(c), v1),
            value_expr: ValueExpr::constant(1.0),
            evaluated_value: 1.0,
        },
        ModelOp::SetCell {
            cell_key: (CoefficientTarget::Constraint(c), v2),
            value_expr: ValueExpr::constant(2.0),
            evaluated_value: 2.0,
        },
        ModelOp::SetObjectiveCell {
            cell_key: (CoefficientTarget::Objective(o), v1),
            value_expr: ValueExpr::constant(3.0),
            evaluated_value: 3.0,
            constant: 10.0,
        },
        ModelOp::SetActiveObjective { obj: Some(o) },
    ];
    let batch = DeltaBatch::new(r0, r1, ops).unwrap();
    let outcome = apply_batch(&mut backend, &mut cursor, &batch);
    assert!(matches!(outcome, ApplyOutcome::Applied { .. }));

    let view = backend.normalized_view();
    assert_eq!(view.variables.len(), 2);
    assert_eq!(view.constraints.len(), 1);
    assert_eq!(view.objectives.len(), 1);
    assert_eq!(view.parameters.len(), 1);
    assert_eq!(view.cells.len(), 2);
    assert_eq!(view.objective_cells.len(), 1);
    assert_eq!(view.active_objective, Some(o));
}

// ── Tests: Semi-continuous variable support ──────────────────────────────────

#[test]
fn semicontinuous_in_snapshot() {
    let v = var_id(0);
    let rev = ModelRevision::ZERO.next().unwrap();

    let mut variables = HashMap::new();
    variables.insert(
        v,
        (
            Bounds::new(0.0, 100.0),
            VarType::Continuous,
            true,
            Some(5.0),
        ),
    );
    let constraints = HashMap::new();
    let objectives = HashMap::new();
    let parameters = HashMap::new();
    let cells = Vec::new();
    let snapshot = take_snapshot(
        rev,
        &variables,
        &constraints,
        &objectives,
        &parameters,
        &cells,
    );

    let (mut backend, mut cursor) = empty_backend();
    rebuild_from_snapshot(&mut backend, &mut cursor, &snapshot);

    let view = backend.normalized_view();
    assert_eq!(view.variables.len(), 1);
    let (_, _, _, _, semi_lower) = view.variables[0];
    assert_eq!(semi_lower, Some(5.0));
}

// ── Tests: Objective cell constant preservation ──────────────────────────────

#[test]
fn objective_cell_constant_preserved_across_batches() {
    let v = var_id(0);
    let o = obj_id(0);
    let (mut backend, mut cursor) = empty_backend();
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().unwrap();
    let r2 = r1.next().unwrap();

    let batch1 = DeltaBatch::new(
        r0,
        r1,
        vec![
            ModelOp::AddVariable {
                var: v,
                bounds: Bounds::NON_NEGATIVE,
                var_type: VarType::Continuous,
            },
            ModelOp::AddObjective {
                obj: o,
                sense: Sense::Maximize,
            },
            ModelOp::SetObjectiveCell {
                cell_key: (CoefficientTarget::Objective(o), v),
                value_expr: ValueExpr::constant(1.0),
                evaluated_value: 1.0,
                constant: 100.0,
            },
        ],
    )
    .unwrap();
    apply_batch(&mut backend, &mut cursor, &batch1);
    assert!((backend.normalized_view().objective_cells[0].2 - 100.0).abs() < 1e-12);

    // Update objective cell value but keep the same constant
    let batch2 = DeltaBatch::new(
        r1,
        r2,
        vec![ModelOp::SetObjectiveCell {
            cell_key: (CoefficientTarget::Objective(o), v),
            value_expr: ValueExpr::constant(2.0),
            evaluated_value: 2.0,
            constant: 100.0,
        }],
    )
    .unwrap();
    let outcome = apply_batch(&mut backend, &mut cursor, &batch2);
    assert!(matches!(outcome, ApplyOutcome::Applied { .. }));

    let view = backend.normalized_view();
    assert!((view.objective_cells[0].1 - 2.0).abs() < 1e-12);
    assert!((view.objective_cells[0].2 - 100.0).abs() < 1e-12);
}
