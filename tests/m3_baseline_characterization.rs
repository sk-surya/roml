//! P25 Task 1 — untouched-tree characterization of M2 ordinary behavior.
//!
//! These tests capture the observable semantics of the current `roml` core
//! *before* P25 adds lineage/instance identity, metadata, function-in-set
//! constraints, and constructs. They are characterization, not red/green
//! feature tests: they must pass on the untouched tree and must keep passing
//! as P25 extends canonical state (SM-01.5 / SM-15.1: ordinary M2 `LinExpr`
//! and builder APIs remain the canonical linear user path).
//!
//! Covered behaviors:
//!   1. Fluent linear modeling (`Model::new` + `add_variable` + `add_constraint` + `maximize`).
//!   2. Deterministic snapshot round-trip (`take_snapshot` is stable/equal).
//!   3. Parameter update (`set_parameter` + `commit` propagates to coefficients).
//!   4. Objective constant propagation.
//!   5. Solution metadata.
//!   6. One-rebuild-retry behavior (incremental delta state equals a fresh
//!      post-update snapshot).

#![allow(deprecated)] // exercises the pre-1.0 compatibility surface

use roml::{
    continuous, Bounds, ConstraintBounds, ConstraintExprExt, Model, ModelError, ModelRevision,
    ModelSnapshot, ObjectiveExprExt, Sense, SolutionBuilder, SolveMetadata, SolveStatus,
    SynchronizationMode, ValueExpr,
};

// =========================================================================
// 1. Fluent linear modeling
// =========================================================================

/// `Model::new` + `add_variable` + `add_constraint` + `maximize` composes a
/// canonical MILP through the ordinary M2 surface, and the model reads back
/// the expected entities.
#[test]
fn fluent_linear_modeling() {
    let mut model = Model::new();

    let x = model.add_variable(continuous().bounds(0.0, 40.0)).unwrap();
    let y = model.add_variable(continuous().bounds(0.0, 30.0)).unwrap();

    // 2x + y <= 60  (upper-bounded row)
    let c1 = model.add_constraint((2.0 * x + y).le(60.0)).unwrap();
    // x - y >= 0  (lower-bounded row)
    let c2 = model.add_constraint((x - y).ge(0.0)).unwrap();

    // maximize 3x + 4y + 5
    let obj = model.maximize(3.0 * x + 4.0 * y + 5.0).unwrap();

    assert_eq!(model.num_variables(), 2);
    assert_eq!(model.num_constraints(), 2);
    assert_eq!(model.num_objectives(), 1);
    assert_eq!(model.active_objective(), Some(obj));

    // The constant is folded into the objective, not into the rows.
    assert_eq!(model.objective_constant(obj), Some(5.0));

    // Constraint bounds reflect the requested rows (no constant shift here).
    assert_eq!(
        model.constraint_bounds(c1),
        Some(ConstraintBounds::le(60.0))
    );
    assert_eq!(model.constraint_bounds(c2), Some(ConstraintBounds::ge(0.0)));

    // Reconstructed expressions agree with what was added.
    let expr_c1 = model.constraint_expression(c1).unwrap();
    assert_eq!(expr_c1.num_terms(), 2);
    assert_eq!(expr_c1.get_constant(), 0.0);
    let expr_obj = model.objective_expression(obj).unwrap();
    assert_eq!(expr_obj.num_terms(), 2);
    assert_eq!(expr_obj.get_constant(), 5.0);

    // Invariants hold on the composed model.
    assert!(model.validate_invariants().is_ok(), "invariants must hold");
}

// =========================================================================
// 2. Deterministic snapshot round-trip
// =========================================================================

/// `take_snapshot` is deterministic: two snapshots of the same model at the
/// same revision are equal, and a model that evolves is captured with the
/// expected entity and cell content.
#[test]
fn deterministic_snapshot_round_trip() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    let c = model.add_constraint((x).le(5.0)).unwrap();
    let obj = model.maximize(x).unwrap();
    let r1 = model.commit().unwrap();

    // Two snapshots from the same state are byte-equal.
    let snap_a = model.take_snapshot().unwrap();
    let snap_b = model.take_snapshot().unwrap();
    assert_eq!(snap_a, snap_b, "snapshots must be deterministic");

    assert_eq!(snap_a.revision, r1);
    assert_eq!(snap_a.variables.len(), 1);
    assert_eq!(snap_a.constraints.len(), 1);
    assert_eq!(snap_a.objectives.len(), 1);
    // One cell for the constraint `x <= 5` and one for the objective `x`.
    assert_eq!(snap_a.cells.len(), 2);

    for cell in &snap_a.cells {
        assert_eq!(cell.evaluated_value, 1.0);
        assert!(cell.dependencies.is_empty());
    }

    // The snapshot is a self-contained, revisioned record.
    let empty = ModelSnapshot::empty(ModelRevision::ZERO);
    assert!(empty.is_empty());
    assert_ne!(snap_a.revision, ModelRevision::ZERO);
}

// =========================================================================
// 3. Parameter update
// =========================================================================

/// `set_parameter` + `commit` propagates the new value to dependent
/// coefficients and advances the revision.
#[test]
fn parameter_update_propagates_to_coefficients() {
    let mut model = Model::new();
    let p = model.add_parameter(2.0).unwrap();
    let x = model.add_variable(continuous()).unwrap();
    let c = model.add_constraint((p * x).le(100.0)).unwrap();
    let r1 = model.commit().unwrap();

    // Read the current x coefficient from the reconstructed row.
    let x_coefficient = |m: &Model| -> f64 {
        let expr = m.constraint_expression(c).unwrap();
        let term = expr
            .terms()
            .iter()
            .find(|t| t.var == x)
            .expect("x term must be present");
        term.coeff.as_constant().expect("constant coefficient")
    };

    // Initial evaluated coefficient is 2.0.
    assert!((x_coefficient(&model) - 2.0).abs() < 1e-12);

    // Update the parameter and commit.
    model.set_parameter(p, 5.0).unwrap();
    let r2 = model.commit().unwrap();
    assert!(r2 > r1, "revision must advance on a real change");

    // The coefficient now evaluates to 5.0.
    assert!((x_coefficient(&model) - 5.0).abs() < 1e-12);

    // Non-finite parameter values are rejected (typed error, no mutation).
    let err = model.set_parameter(p, f64::NAN);
    assert!(matches!(err, Err(ModelError::NonFiniteValue(_))));
}

// =========================================================================
// 4. Objective constant propagation
// =========================================================================

/// Objective constants are stored on the objective, reconstructed by
/// `objective_expression`, and carried into the snapshot.
#[test]
fn objective_constant_propagation() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).unwrap();
    let obj = model.maximize(3.0 * x + 7.0).unwrap();

    assert_eq!(model.objective_constant(obj), Some(7.0));
    let expr = model.objective_expression(obj).unwrap();
    assert_eq!(expr.get_constant(), 7.0);

    let snap = model.take_snapshot().unwrap();
    let o = snap
        .objectives
        .iter()
        .find(|o| o.id == obj)
        .expect("objective in snapshot");
    assert_eq!(o.constant, 7.0);
    assert_eq!(o.sense, Sense::Maximize);
    assert!(o.active);
}

// =========================================================================
// 5. Solution metadata
// =========================================================================

/// `Solution` carries `SolveMetadata` (backend identity, revision, effective
/// configuration, synchronization mode); metadata round-trips through the
/// builder and default is `NoChange` at revision zero.
#[test]
fn solution_metadata_round_trip() {
    let meta = SolveMetadata {
        backend_name: "ReferenceBackend".to_string(),
        model_revision: ModelRevision::from_u64(4),
        synchronization: SynchronizationMode::Rebuild,
        ..SolveMetadata::default()
    };

    let solution = SolutionBuilder::new()
        .status(SolveStatus::Optimal)
        .objective_value(42.0)
        .metadata(meta.clone())
        .build();

    assert!(solution.is_optimal());
    assert_eq!(solution.objective_value(), Some(42.0));
    assert_eq!(solution.metadata(), &meta);

    // Default metadata: no change, zero revision, empty backend name.
    let d = SolveMetadata::default();
    assert_eq!(d.model_revision, ModelRevision::ZERO);
    assert_eq!(d.synchronization, SynchronizationMode::NoChange);
    assert!(d.backend_name.is_empty());
}

// =========================================================================
// 6. One-rebuild-retry behavior
// =========================================================================

/// After the model advances past a session's base revision, a single fresh
/// snapshot reflects the post-update canonical state — i.e. the rebuild path
/// reproduces exactly what the incremental delta path produced. The delta
/// batch from `r1 -> r2` records the parameter change and the re-evaluated
/// cell; the post-update snapshot carries the same evaluated value.
#[test]
fn one_rebuild_retry_recovers_post_update_state() {
    let mut model = Model::new();
    let p = model.add_parameter(2.0).unwrap();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    let c = model.add_constraint((p * x).le(20.0)).unwrap();
    let r1 = model.commit().unwrap();

    // Session is current at r1; the pre-update snapshot is the "clean" state.
    let clean = model.take_snapshot().unwrap();
    assert_eq!(clean.revision, r1);

    // The model advances to r2 (a parameter delta).
    model.set_parameter(p, 10.0).unwrap();
    let r2 = model.commit().unwrap();
    assert!(r2 > r1);

    // The delta batch from r1 -> r2 contains the parameter change and the
    // re-evaluated cell (incremental path).
    let batch = model
        .deltas_since(r1)
        .unwrap()
        .into_iter()
        .find(|b| b.to == r2)
        .expect("r1 -> r2 batch");
    assert!(batch
        .operations
        .iter()
        .any(|op| matches!(op, roml::ModelOp::SetParameter { param, .. } if *param == p)));
    assert!(batch.operations.iter().any(|op| matches!(op, roml::ModelOp::SetCell { evaluated_value, .. } if (*evaluated_value - 10.0).abs() < 1e-12)));

    // A single fresh snapshot (the rebuild path) reproduces the same
    // post-update canonical state as the incremental path: coefficient 10.0.
    let rebuilt = model.take_snapshot().unwrap();
    assert_eq!(rebuilt.revision, r2);
    let cell = rebuilt
        .cells
        .iter()
        .find(|cell| cell.evaluated_value != 0.0 || cell.value_expr.has_dependencies())
        .expect("parameterized cell present");
    assert!((cell.evaluated_value - 10.0).abs() < 1e-12);
    assert!(cell.dependencies.contains(&p));

    // The pre-update "clean" snapshot is distinct from the rebuilt one — the
    // rebuild recovered the *new* state, never a stale one.
    assert_ne!(clean, rebuilt);
}
