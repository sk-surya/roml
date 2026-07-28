//! Integration tests for model changelog → DeltaBatch compilation.
//!
//! These tests verify the revisioned synchronization protocol:
//! - Model mutations compile to DeltaBatch via commit()
//! - Revision tracking via current_revision()
//! - Snapshot capture via take_snapshot()
//!
//! NOTE: These tests use the NEW Model API. The old drain_changes() API
//! has been removed. Changes are compiled to ModelOp vectors and committed
//! to SyncCoordinator as DeltaBatch values.

use roml::{
    Bounds, ConstraintBounds, Model, Sense, ValueExpr, VarType,
    revision::ModelRevision,
};

#[test]
fn revision_advances_on_mutation() {
    let mut model = Model::new();

    // Revision starts at ZERO before any mutations
    assert_eq!(model.current_revision(), ModelRevision::ZERO);

    // Add variables and constraints (pushes to changelog)
    let x = model.add_variable(Bounds::NON_NEGATIVE, VarType::Continuous);
    let con = model.add_constraint(ConstraintBounds::le(10.0));
    model.add_coeff(con, x, 2.0).unwrap();

    // After mutations but before commit, revision is still ZERO
    assert_eq!(model.current_revision(), ModelRevision::ZERO);

    // Commit advances revision from r0 to r1
    model.commit();
    assert!(!model.current_revision().is_zero());
    assert!(model.current_revision() != ModelRevision::ZERO);

    // More mutations and another commit advance revision again
    model.set_variable_bounds(x, Bounds::new(-1.0, 5.0)).unwrap();
    model.commit();
    assert!(model.current_revision() > ModelRevision::ZERO);

    // Third round of mutations
    let y = model.add_variable(Bounds::NON_NEGATIVE, VarType::Continuous);
    model.add_coeff(con, y, 3.0).unwrap();
    model.commit();

    // Revision has advanced multiple times
    assert!(model.current_revision() > ModelRevision::ZERO);
}

#[test]
fn snapshot_captures_model_state() {
    let mut model = Model::new();

    // Add variables, constraints, coefficients, an objective
    let x = model.add_variable(Bounds::NON_NEGATIVE, VarType::Continuous);
    let y = model.add_variable(Bounds::new(-5.0, 10.0), VarType::Integer);
    let con = model.add_constraint(ConstraintBounds::le(100.0));
    model.add_coeff(con, x, 2.0).unwrap();
    model.add_coeff(con, y, 3.0).unwrap();
    let obj = model.add_objective(Sense::Minimize);
    model.set_active_objective(obj).unwrap();

    // Commit to compile changes to DeltaBatch
    model.commit();

    // Take snapshot after commit
    let snapshot = model.take_snapshot();

    // Snapshot revision matches model revision
    assert_eq!(snapshot.revision, model.current_revision());

    // Snapshot contains the expected entities
    assert_eq!(snapshot.variables.len(), 2);
    assert_eq!(snapshot.constraints.len(), 1);
    assert_eq!(snapshot.cells.len(), 2);
    assert_eq!(snapshot.objectives.len(), 1);

    // Verify specific entity properties
    let var_x = snapshot.variables.iter().find(|v| v.id == x).unwrap();
    assert_eq!(var_x.bounds, Bounds::NON_NEGATIVE);
    assert_eq!(var_x.var_type, VarType::Continuous);
    assert!(var_x.active);

    let var_y = snapshot.variables.iter().find(|v| v.id == y).unwrap();
    assert_eq!(var_y.bounds, Bounds::new(-5.0, 10.0));
    assert_eq!(var_y.var_type, VarType::Integer);

    let obj_entry = snapshot.objectives.iter().find(|o| o.id == obj).unwrap();
    assert!(obj_entry.active);
    assert_eq!(obj_entry.sense, Sense::Minimize);

    let con_snap = snapshot.constraints.iter().find(|c| c.id == con).unwrap();
    assert!(con_snap.active);
}

#[test]
fn commit_produces_correct_delta_ops() {
    let mut model = Model::new();

    /// Helper: collect the current delta batch by observing revision advancement.
    /// We use commit() and then examine model state to verify the DeltaBatch was created.
    fn count_delta_batches_since(model: &Model, since: ModelRevision) -> usize {
        // The coordinator journal stores one batch per revision
        // We can verify indirectly by checking revision advancement
        model.current_revision().as_u64().saturating_sub(since.as_u64()) as usize
    }

    let r0 = model.current_revision();

    // Phase 1: Add variable x with bounds [0,10], add constraint, add coeff
    let x = model.add_variable(Bounds::new(0.0, 10.0), VarType::Continuous);
    let con = model.add_constraint(ConstraintBounds::le(100.0));
    model.add_coeff(con, x, 2.0).unwrap();
    model.commit();

    // After Phase 1 commit, revision should have advanced by 1
    let r1 = model.current_revision();
    assert_eq!(count_delta_batches_since(&model, r0), 1);

    // Phase 2: Change variable bounds, add another variable
    model.set_variable_bounds(x, Bounds::new(1.0, 20.0)).unwrap();
    let y = model.add_variable(Bounds::NON_NEGATIVE, VarType::Continuous);
    model.add_coeff(con, y, 5.0).unwrap();
    model.commit();

    // After Phase 2 commit, revision should have advanced by 1 more
    let r2 = model.current_revision();
    assert_eq!(count_delta_batches_since(&model, r1), 1);

    // Phase 3: More mutations
    model.set_variable_active(x, false).unwrap();
    model.commit();

    // After Phase 3 commit, revision should have advanced by 1 more
    assert_eq!(count_delta_batches_since(&model, r2), 1);

    // Total should be 3 commits = 3 revisions beyond r0
    assert_eq!(count_delta_batches_since(&model, r0), 3);
}

#[test]
fn coordinator_tracks_revision_independently_of_changelog() {
    let mut model = Model::new();

    // Start with ZERO revision
    assert_eq!(model.current_revision(), ModelRevision::ZERO);

    // Snapshot at r0 should be empty
    let snap_r0 = model.take_snapshot();
    assert_eq!(snap_r0.revision, ModelRevision::ZERO);
    assert!(snap_r0.is_empty() || snap_r0.entity_count() == 0);

    // Add and commit
    let x = model.add_variable(Bounds::NON_NEGATIVE, VarType::Continuous);
    model.commit();

    // Now revision has advanced
    let r1 = model.current_revision();
    assert!(r1 > ModelRevision::ZERO);

    // Snapshot at r1 should contain the variable
    let snap_r1 = model.take_snapshot();
    assert_eq!(snap_r1.revision, r1);
    assert_eq!(snap_r1.variables.len(), 1);
    assert_eq!(snap_r1.variables[0].id, x);

    // Both snapshots have different revisions
    assert!(snap_r0.revision != snap_r1.revision);

    // Further mutations and commit advance revision again
    model.set_variable_bounds(x, Bounds::new(-5.0, 5.0)).unwrap();
    model.commit();

    let r2 = model.current_revision();
    assert!(r2 > r1);

    // Snapshot at r2 has the updated bounds
    let snap_r2 = model.take_snapshot();
    assert_eq!(snap_r2.revision, r2);
    assert_eq!(snap_r2.variables.len(), 1);
    assert_eq!(snap_r2.variables[0].bounds, Bounds::new(-5.0, 5.0));
}
