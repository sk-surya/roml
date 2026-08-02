//! P21 — Objective constant propagation on the delta path (API-03.5).
//!
//! Objective constants are stored on the model and captured by snapshots, so
//! a rebuild propagates them. The incremental (delta) path must ALSO carry
//! them: the projection keeps a per-objective offset cache, and if a constant
//! is never journaled the backend reports objective values without it on every
//! incremental solve. These tests pin that the constant reaches the backend
//! exactly once through `SetObjectiveConstant`.

use roml::delta::ModelOp;
use roml::prelude::*;
use roml::revision::ModelRevision;

/// A non-zero objective constant is journaled and appears in the committed
/// delta batch exactly once.
#[test]
fn objective_constant_is_propagated_in_delta_batch() {
    let mut model = Model::new();
    let x = model.add_var();
    let y = model.add_var();
    model.constrain((x + y).le(4.0)).unwrap();
    let obj = model.maximize(3.0 * x + y + 5.0).unwrap();
    assert_eq!(model.objective_constant(obj), Some(5.0));

    let _r1 = model.commit().unwrap();
    let batches = model.deltas_since(ModelRevision::ZERO).unwrap();
    let ops = &batches.last().expect("one committed batch").operations;
    assert!(
        ops.iter().any(|op| matches!(
            op,
            ModelOp::SetObjectiveConstant { obj: o, constant }
                if *o == obj && (*constant - 5.0).abs() < 1e-9
        )),
        "delta must carry the objective constant exactly once, ops: {ops:#?}"
    );
}

/// Changing an objective constant via `set_objective_expr` journals the new
/// constant for the incremental path.
#[test]
fn objective_constant_change_is_journaled() {
    let mut model = Model::new();
    let x = model.add_var();
    let obj = model.maximize(x + 5.0).unwrap();
    let r1 = model.commit().unwrap();

    model.set_objective_expr(obj, x + 9.0).unwrap();
    let r2 = model.commit().unwrap();
    assert_ne!(r2, r1);

    let batches = model.deltas_since(r1).unwrap();
    let ops = &batches.last().expect("second batch").operations;
    assert!(
        ops.iter().any(|op| matches!(
            op,
            ModelOp::SetObjectiveConstant { obj: o, constant }
                if *o == obj && (*constant - 9.0).abs() < 1e-9
        )),
        "constant change must be journaled, ops: {ops:#?}"
    );
}

/// A zero objective constant is the default and need not be journaled.
#[test]
fn zero_constant_is_not_emitted() {
    let mut model = Model::new();
    let x = model.add_var();
    model.maximize(x).unwrap();
    let _r1 = model.commit().unwrap();
    let batches = model.deltas_since(ModelRevision::ZERO).unwrap();
    let ops = &batches.last().expect("one committed batch").operations;
    assert!(
        !ops.iter()
            .any(|op| matches!(op, ModelOp::SetObjectiveConstant { .. })),
        "zero constant must not be journaled, ops: {ops:#?}"
    );
}
