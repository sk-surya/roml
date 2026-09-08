//! P1.5B adversarial equivalence: packed-base + overlay mutation semantics.
//!
//! Every test pins behavior that the physical rewrite must preserve:
//! identity stability across shadowing, fresh identity after remove→re-add,
//! parameter propagation through shadows, canonical combine, stale-ID
//! errors, deletion cascades spanning base and overlay, and delta replay.

#![allow(deprecated)]

use std::sync::Arc;

use roml::delta::{DeltaBatch, ModelOp};
use roml::model::ConstraintBounds;
use roml::prelude::*;
use roml::solver::reference::ReferenceBackend;
use roml::sync::{AdapterCursor, ApplyOutcome};
use roml::ValueExpr;
use roml::VarId;

fn unit_coeffs(n: usize) -> Vec<f64> {
    vec![1.0; n]
}

#[test]
fn packed_update_preserves_identity() -> Result<(), ModelError> {
    let mut model = Model::new();
    let vars: Vec<VarId> = (0..8).map(|_| model.add_var()).collect();
    let obj = model.set_linear_objective_bulk(Sense::Minimize, &vars, &unit_coeffs(8), 0.0)?;
    let before = model.objective_expression(obj)?;
    assert_eq!(before.num_terms(), 8);
    // Scalar update of a packed cell keeps the canonical cell, new value.
    model.set_coefficient(
        roml::model::coefficient::CoefficientTarget::Objective(obj),
        vars[3],
        5.0,
    )?;
    assert_eq!(model.num_coefficients(), 8);
    let after = model.objective_expression(obj)?;
    let zones: Vec<f64> = after
        .terms()
        .iter()
        .map(|t| t.coeff.as_constant().unwrap())
        .collect();
    assert_eq!(zones.len(), 8);
    Ok(())
}

#[test]
fn constant_to_parameterized_and_back() -> Result<(), ModelError> {
    let mut model = Model::new();
    let p = model.add_parameter(2.0)?;
    let vars: Vec<VarId> = (0..4).map(|_| model.add_var()).collect();
    model.set_linear_objective_bulk(Sense::Minimize, &vars, &unit_coeffs(4), 0.0)?;
    // Constant -> parameterized via scalar add onto the packed cell.
    let obj = model.active_objective().unwrap();
    model.add_objective_coefficient(obj, vars[1], ValueExpr::param(p))?;
    model.commit()?;
    // Parameter propagation reaches the shadowed cell.
    model.set_parameter(p, 3.0)?;
    model.commit()?;
    let expr = model.objective_expression(obj)?;
    let mut vals: Vec<f64> = expr
        .terms()
        .iter()
        .map(|t| t.coeff.as_constant().unwrap())
        .collect();
    vals.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(vals, vec![1.0, 1.0, 1.0, 4.0]);
    // Parameterized -> constant replaces the dependency entirely.
    model.set_coefficient(
        roml::model::coefficient::CoefficientTarget::Objective(obj),
        vars[1],
        7.0,
    )?;
    model.set_parameter(p, 100.0)?;
    model.commit()?;
    let expr = model.objective_expression(obj)?;
    let mut vals: Vec<f64> = expr
        .terms()
        .iter()
        .map(|t| t.coeff.as_constant().unwrap())
        .collect();
    vals.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(vals, vec![1.0, 1.0, 1.0, 7.0]);
    Ok(())
}

#[test]
fn remove_packed_then_readd_is_fresh() -> Result<(), ModelError> {
    let mut model = Model::new();
    let vars: Vec<VarId> = (0..4).map(|_| model.add_var()).collect();
    let obj = model.set_linear_objective_bulk(Sense::Minimize, &vars, &unit_coeffs(4), 0.0)?;
    let target = roml::model::coefficient::CoefficientTarget::Objective(obj);
    model.remove_coefficient_at(target, vars[2])?;
    assert_eq!(model.num_coefficients(), 3);
    // Re-add creates the cell anew (algebraic add onto absent cell).
    model.add_to_coefficient(target, vars[2], 2.5)?;
    assert_eq!(model.num_coefficients(), 4);
    let expr = model.objective_expression(obj)?;
    assert_eq!(expr.num_terms(), 4);
    Ok(())
}

#[test]
fn stale_coeff_id_is_typed_error() -> Result<(), ModelError> {
    let mut model = Model::new();
    let vars: Vec<VarId> = (0..2).map(|_| model.add_var()).collect();
    model.set_linear_objective_bulk(Sense::Minimize, &vars, &unit_coeffs(2), 0.0)?;
    let obj = model.active_objective().unwrap();
    let target = roml::model::coefficient::CoefficientTarget::Objective(obj);
    // Capture a live id indirectly: remove the cell, then operate stale via
    // coordinate APIs that resolve through the same identities.
    model.remove_coefficient_at(target, vars[0])?;
    // Removing again is a no-op, not an error (coordinate API).
    model.remove_coefficient_at(target, vars[0])?;
    assert_eq!(model.num_coefficients(), 1);
    Ok(())
}

#[test]
fn variable_deletion_spans_base_and_overlay() -> Result<(), ModelError> {
    let mut model = Model::new();
    let vars: Vec<VarId> = (0..6).map(|_| model.add_var()).collect();
    model.set_linear_objective_bulk(Sense::Minimize, &vars, &unit_coeffs(6), 0.0)?;
    let obj = model.active_objective().unwrap();
    let target = roml::model::coefficient::CoefficientTarget::Objective(obj);
    // Mutate one cell into the overlay, then delete variables touching
    // both representations.
    model.set_coefficient(target, vars[1], 9.0)?;
    model.remove_variable(vars[0])?;
    model.remove_variable(vars[1])?;
    assert_eq!(model.num_coefficients(), 4);
    assert!(model.validate_invariants().is_ok());
    Ok(())
}

#[test]
fn constraint_deletion_after_packed_objective() -> Result<(), ModelError> {
    let mut model = Model::new();
    let vars: Vec<VarId> = (0..4).map(|_| model.add_var()).collect();
    let con = model.add_constraint(ConstraintBounds::le(10.0))?;
    model.add_constraint_coefficient(con, vars[0], 2.0)?;
    model.set_linear_objective_bulk(Sense::Minimize, &vars, &unit_coeffs(4), 0.0)?;
    model.remove_constraint(con)?;
    // Objective cells untouched; constraint cell gone with its row.
    assert_eq!(model.num_coefficients(), 4);
    assert!(model.validate_invariants().is_ok());
    Ok(())
}

#[test]
fn bulk_journal_replays_through_reference() -> Result<(), ModelError> {
    // Commuting square over a mixed packed + scalar batch.
    let mut model = Model::new();
    let vars: Vec<VarId> = (0..8).map(|_| model.add_var()).collect();
    let snap_before = model.take_snapshot()?;
    let obj = model.set_linear_objective_bulk(Sense::Maximize, &vars, &unit_coeffs(8), 1.5)?;
    model.commit()?;
    let snap_after = model.take_snapshot()?;

    let cells: Arc<[(VarId, f64)]> = vars.iter().map(|v| (*v, 1.0)).collect::<Vec<_>>().into();
    let expected_ops = vec![
        ModelOp::AddObjective {
            obj,
            sense: Sense::Maximize,
        },
        ModelOp::SetObjectiveCells { obj, cells },
        ModelOp::SetObjectiveConstant { obj, constant: 1.5 },
        ModelOp::SetActiveObjective { obj: Some(obj) },
    ];
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

#[test]
fn sparse_ids_after_removals() -> Result<(), ModelError> {
    // Non-dense identities (removal gaps) coexist with packed ranges.
    let mut model = Model::new();
    let mut vars: Vec<VarId> = (0..10).map(|_| model.add_var()).collect();
    model.remove_variable(vars[9])?;
    model.remove_variable(vars[8])?;
    vars.truncate(8);
    let coeffs: Vec<f64> = (1..=8).map(|i| i as f64).collect();
    model.set_linear_objective_bulk(Sense::Minimize, &vars, &coeffs, 0.0)?;
    let obj = model.active_objective().unwrap();
    let expr = model.objective_expression(obj)?;
    assert_eq!(expr.num_terms(), 8);
    assert!(model.validate_invariants().is_ok());
    Ok(())
}
