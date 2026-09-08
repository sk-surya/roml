//! P1A contract: bulk linear-row insertion.
//!
//! RED phase: names `Model::add_linear_rows_bulk`, the packed
//! `LinearRowBlock` journal/delta shapes, and equivalence with the scalar
//! row path (canonical/shuffled/duplicated/cancellation/empty inputs,
//! atomic rejection, replay).

#![allow(deprecated)]

use std::sync::Arc;

use roml::delta::{DeltaBatch, LinearRowBlock, ModelOp};
use roml::model::ConstraintBounds;
use roml::prelude::*;
use roml::solver::reference::ReferenceBackend;
use roml::sync::{AdapterCursor, ApplyOutcome};
use roml::ConId;
use roml::VarId;

fn vars(model: &mut Model, n: usize) -> Vec<VarId> {
    (0..n).map(|_| model.add_var()).collect()
}

/// Build the same rows through scalar `add_constraint` for equivalence.
fn scalar_rows(
    model: &mut Model,
    rows: &[Vec<(VarId, f64)>],
    bound: ConstraintBounds,
) -> Vec<ConId> {
    rows.iter()
        .map(|terms| {
            let mut expr = LinExpr::new();
            for (v, c) in terms {
                expr = expr.term(*c, *v);
            }
            model
                .add_constraint(ConstraintSpec::new(expr, bound))
                .unwrap()
        })
        .collect()
}

#[test]
fn bulk_rows_match_scalar_canonical_state() -> Result<(), ModelError> {
    // Canonical + shuffled + duplicated + empty + cancellation rows.
    let rows = vec![
        vec![(0usize, 1.0), (1, 2.0)],
        vec![],
        vec![(3, 1.0), (2, 1.0), (3, 1.0)], // shuffled + duplicate
        vec![(4, 1.0), (4, -1.0)],          // cancels to zero
        vec![(5, 3.0)],
    ];
    let mut bulk = Model::new();
    let bxs = vars(&mut bulk, 6);
    let (ptr, flat_vars, flat_vals) = flatten(&bxs, &rows);
    let bounds = vec![ConstraintBounds::le(10.0); rows.len()];
    let cons = bulk.add_linear_rows_bulk(&ptr, &flat_vars, &flat_vals, &bounds)?;
    assert_eq!(cons.len(), rows.len());

    let mut scalar = Model::new();
    let sxs = vars(&mut scalar, 6);
    let srows: Vec<Vec<(VarId, f64)>> = rows
        .iter()
        .map(|r| r.iter().map(|(i, c)| (sxs[*i], *c)).collect())
        .collect();
    scalar_rows(&mut scalar, &srows, ConstraintBounds::le(10.0));
    let _ = (cons, sxs);

    assert_eq!(bulk.num_coefficients(), scalar.num_coefficients());
    assert_eq!(bulk.num_constraints(), scalar.num_constraints());
    // Identical models allocate identical identities: snapshots compare
    // directly (sorted canonical cells + values).
    assert_eq!(
        bulk.take_snapshot()?,
        scalar.take_snapshot()?,
        "bulk and scalar construction must agree exactly"
    );
    Ok(())
}

fn flatten(xs: &[VarId], rows: &[Vec<(usize, f64)>]) -> (Vec<u32>, Vec<VarId>, Vec<f64>) {
    let mut ptr = vec![0u32];
    let mut flat_vars = Vec::new();
    let mut flat_vals = Vec::new();
    for row in rows {
        for (i, c) in row {
            flat_vars.push(xs[*i]);
            flat_vals.push(*c);
        }
        ptr.push(flat_vars.len() as u32);
    }
    (ptr, flat_vars, flat_vals)
}

#[test]
fn bulk_rows_reject_atomically() {
    let mut model = Model::new();
    let xs = vars(&mut model, 3);
    let rev = model.current_revision();
    // Stale variable in the second row.
    let doomed = model.add_var();
    model.remove_variable(doomed).unwrap();
    let ptr = vec![0u32, 2, 3];
    let stale_vars = vec![xs[0], xs[1], doomed];
    let stale_vals = vec![1.0, 2.0, 3.0];
    let bounds = vec![ConstraintBounds::le(1.0); 2];
    let err = model
        .add_linear_rows_bulk(&ptr, &stale_vars, &stale_vals, &bounds)
        .unwrap_err();
    assert!(matches!(err, ModelError::VariableNotFound(_)));
    assert_eq!(model.num_constraints(), 0);
    assert_eq!(model.num_coefficients(), 0);
    assert_eq!(model.current_revision(), rev);

    let flat_vars = vec![xs[0], xs[1], xs[2]];
    let flat_vals = vec![1.0, f64::NAN, 1.0];
    let bounds = vec![ConstraintBounds::le(1.0); 2];
    let err = model
        .add_linear_rows_bulk(&ptr, &flat_vars, &flat_vals, &bounds)
        .unwrap_err();
    assert!(matches!(err, ModelError::NonFiniteValue(_)));
    assert_eq!(model.num_constraints(), 0);
    assert_eq!(model.num_coefficients(), 0);
    assert_eq!(model.current_revision(), rev);

    // Bad row pointers.
    let err = model
        .add_linear_rows_bulk(&[0u32, 5], &flat_vars[..2], &flat_vals[..2], &bounds[..1])
        .unwrap_err();
    assert!(matches!(
        err,
        ModelError::MismatchedRowBlock { .. } | ModelError::MismatchedBulkLengths { .. }
    ));
    assert_eq!(model.num_constraints(), 0);

    // Inverted bounds.
    let err = model
        .add_linear_rows_bulk(
            &[0u32, 1],
            &flat_vars[..1],
            &flat_vals[..1],
            &[ConstraintBounds::range(5.0, 1.0)],
        )
        .unwrap_err();
    assert!(matches!(err, ModelError::InvalidBounds));
    assert_eq!(model.num_constraints(), 0);
}

#[test]
fn bulk_rows_journal_replays_like_snapshot() -> Result<(), ModelError> {
    let mut model = Model::new();
    let xs = vars(&mut model, 4);
    let snap_before = model.take_snapshot()?;
    let ptr = vec![0u32, 2, 4];
    let flat_vars = vec![xs[0], xs[1], xs[2], xs[3]];
    let flat_vals = vec![1.0, 2.0, 3.0, 4.0];
    let bounds = vec![ConstraintBounds::le(10.0); 2];
    let cons = model.add_linear_rows_bulk(&ptr, &flat_vars, &flat_vals, &bounds)?;
    model.commit()?;
    let snap_after = model.take_snapshot()?;

    let block = Arc::new(LinearRowBlock {
        constraints: cons.clone(),
        bounds: bounds.clone(),
        row_ptr: vec![0, 2, 4],
        vars: flat_vars.clone(),
        values: flat_vals.clone(),
    });
    let expected_ops = vec![ModelOp::AddLinearRows { block }];
    let mut backend = ReferenceBackend::new();
    let mut cursor = AdapterCursor::new();
    backend.rebuild(&snap_before, &mut cursor);
    let batch = DeltaBatch::new(snap_before.revision, snap_after.revision, expected_ops).unwrap();
    let outcome = backend.apply_batch(&batch, &mut cursor).unwrap();
    assert!(matches!(outcome, ApplyOutcome::Applied { .. }));
    let mut expected_backend = ReferenceBackend::new();
    let mut expected_cursor = AdapterCursor::new();
    expected_backend.rebuild(&snap_after, &mut expected_cursor);
    assert_eq!(
        backend.normalized_view(),
        expected_backend.normalized_view()
    );
    Ok(())
}
