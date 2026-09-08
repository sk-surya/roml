//! Certification regressions (independent review of the P0–P2A stack).
//!
//! - Lazy packed variable-index invalidation across appends: `for_var`
//!   removal cascades must see packed cells appended after the index was
//!   built (orphaned coefficients referencing removed variables).
//! - Overlay parameter propagation must not clobber the cached value when
//!   the re-evaluated delta is below the change threshold.

#![allow(deprecated)]

use roml::model::ConstraintBounds;
use roml::prelude::*;
use roml::ValueExpr;
use roml::VarId;

#[test]
fn remove_variable_sees_packed_cells_appended_after_index_build() -> Result<(), ModelError> {
    let mut model = Model::new();
    let a: Vec<VarId> = (0..4).map(|_| model.add_var()).collect();
    let b: Vec<VarId> = (0..4).map(|_| model.add_var()).collect();
    model.set_linear_objective_bulk(Sense::Minimize, &a, &[1.0; 4], 0.0)?;
    // Builds the lazy packed variable index inside the coefficient store.
    model.remove_variable(a[0])?;
    assert_eq!(model.num_coefficients(), 3);
    // Append new packed cells after the index was built.
    model.add_linear_rows_bulk(&[0, 4], &b, &[1.0; 4], &[ConstraintBounds::le(10.0)])?;
    assert_eq!(model.num_coefficients(), 7);
    // The removal cascade must reach the post-index packed cell.
    model.remove_variable(b[0])?;
    assert_eq!(model.num_coefficients(), 6);
    assert!(model.validate_invariants().is_ok());
    // And again after a further append, for good measure.
    let c: Vec<VarId> = (0..2).map(|_| model.add_var()).collect();
    model.add_linear_rows_bulk(&[0, 2], &c, &[2.0; 2], &[ConstraintBounds::le(5.0)])?;
    model.remove_variable(c[1])?;
    assert_eq!(model.num_coefficients(), 7);
    assert!(model.validate_invariants().is_ok());
    Ok(())
}

#[test]
fn sub_epsilon_param_update_preserves_overlay_cached_value() -> Result<(), ModelError> {
    let mut model = Model::new();
    let x = model.add_var();
    let p = model.add_parameter(1.0)?;
    // Overlay cell with a scaled-parameter expression: value = 0.1 * p.
    model.set_linear_objective_bulk(Sense::Minimize, &[x], &[0.0], 0.0)?;
    let obj = model.active_objective().unwrap();
    model.add_objective_coefficient(obj, x, ValueExpr::constant(0.1) * ValueExpr::param(p))?;
    // A parameter change that passes the outer change gate while moving
    // the coefficient value by less than the journaling threshold.
    let tiny = 5.0 * f64::EPSILON;
    model.set_parameter(p, 1.0 + tiny)?;
    model.commit()?;
    let expr = model.objective_expression(obj)?;
    let v1: f64 = expr.terms()[0].coeff.as_constant().unwrap();
    assert!(
        (v1 - 0.1 * (1.0 + tiny)).abs() < 1e-12,
        "cached value corrupted: {v1}"
    );
    assert!(model.validate_invariants().is_ok());
    Ok(())
}

#[test]
fn mixed_bulk_rows_plus_scalar_add_snapshot_completeness() -> Result<(), ModelError> {
    let mut model = Model::new();
    let v: Vec<VarId> = (0..4).map(|_| model.add_var()).collect();
    model.add_linear_rows_bulk(
        &[0, 2, 4],
        &v,
        &[1.0, 2.0, 3.0, 4.0],
        &[ConstraintBounds::le(10.0), ConstraintBounds::le(20.0)],
    )?;
    // Scalar row alongside packed rows: snapshot must see every cell.
    let x = model.add_var();
    let cons = model.add_linear_rows_bulk(&[0, 1], &[x], &[5.0], &[ConstraintBounds::le(1.0)])?;
    assert_eq!(cons.len(), 1);
    model.commit()?;
    let snap = model.take_snapshot()?;
    assert_eq!(snap.cells.len(), 5);
    assert!(model.validate_invariants().is_ok());
    Ok(())
}
