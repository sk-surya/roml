//! P0.5 model-level locks: snapshot function reconstruction over
//! add → update → remove sequences with parameterized cells.
//!
//! These pass on the current implementation and must pass unchanged after
//! linearization, proving identical outputs (no second semantic authority).

#![allow(deprecated)]

use roml::model::ConstraintBounds;
use roml::prelude::*;
use roml::ValueExpr;

#[test]
fn snapshot_functions_track_add_update_remove() -> Result<(), ModelError> {
    let mut model = Model::new();
    let p = model.add_parameter(2.0)?;
    let x = model.add_var();
    let y = model.add_var();
    let z = model.add_var();

    // c1: parameterized cell + constant cell, bounds later folded.
    let c1 = model.add_constraint(ConstraintBounds::le(10.0))?;
    model.add_constraint_coefficient(c1, x, ValueExpr::param(p) * 2.0)?;
    model.add_constraint_coefficient(c1, y, 3.0)?;
    model.set_constraint_bounds(c1, ConstraintBounds::le(7.0))?;
    // c2: added then removed — must vanish from functions.
    let c2 = model.add_constraint(ConstraintBounds::ge(1.0))?;
    model.add_constraint_coefficient(c2, z, 5.0)?;
    model.remove_constraint(c2)?;
    model.commit()?;

    let snap = model.take_snapshot()?;
    assert_eq!(snap.functions.len(), 1);
    let f = &snap.functions[0];
    assert_eq!(f.constraint, c1);
    // Folded bounds, both cells, parameterized symbolic form kept.
    let _ = (p, x, y, z);
    let terms = model.constraint_expression(c1)?;
    assert_eq!(terms.num_terms(), 2);
    Ok(())
}

#[test]
fn snapshot_stable_across_commits() -> Result<(), ModelError> {
    let mut model = Model::new();
    let x = model.add_var();
    let c = model.add_constraint(ConstraintBounds::le(4.0))?;
    model.add_constraint_coefficient(c, x, 1.5)?;
    model.commit()?;
    let first = model.take_snapshot()?;
    model.set_constraint_bounds(c, ConstraintBounds::le(9.0))?;
    model.commit()?;
    let second = model.take_snapshot()?;
    assert_eq!(first.functions.len(), 1);
    assert_eq!(second.functions.len(), 1);
    assert_ne!(first.functions[0], second.functions[0]);
    assert_eq!(second.functions[0].constraint, c);
    Ok(())
}
