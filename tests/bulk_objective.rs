//! P0 contract: bulk constant-objective insertion.
//!
//! RED phase: these tests name the intended public API
//! (`Model::set_linear_objective_bulk`) and packed journal/delta shapes.
//! They must fail to compile until the implementation lands; behavior must
//! then match the scalar path exactly (R2.2 canonical cells, R3.2/R3.5
//! revisioned equivalence) with a packed journal, not 1M scalar events.

// Uses deprecated raw constructors like the other characterization tests.
#![allow(deprecated)]

use std::sync::Arc;

use roml::delta::{DeltaBatch, ModelOp};
use roml::prelude::*;
use roml::solver::reference::ReferenceBackend;
use roml::sync::{AdapterCursor, ApplyOutcome};
use roml::ValueExpr;
use roml::VarId;

fn add_vars(model: &mut Model, n: usize) -> Vec<VarId> {
    (0..n).map(|_| model.add_var()).collect()
}

/// Canonical mathematical state of the active objective as
/// (sense, constant, sorted (var, value) pairs).
fn objective_state(model: &Model) -> (Sense, f64, Vec<(VarId, f64)>) {
    let obj = model.active_objective().expect("active objective");
    let sense = model.objective_sense(obj).expect("sense");
    let constant = model.objective_constant(obj).unwrap_or(f64::NAN);
    let expr = model.objective_expression(obj).unwrap();
    let mut terms: Vec<(VarId, f64)> = expr
        .terms()
        .iter()
        .map(|t| (t.var, t.coeff.as_constant().unwrap()))
        .collect();
    terms.sort_by_key(|(var, _)| *var);
    (sense, constant, terms)
}

#[test]
fn bulk_matches_scalar_canonical_state() -> Result<(), ModelError> {
    let n = 64;
    // Include negatives, fractional values, and exact zeros (scalar
    // `simplify` drops |v| < EPSILON; bulk must match).
    let coeffs: Vec<f64> = (0..n)
        .map(|i| match i % 5 {
            0 => 0.0,
            1 => -(i as f64) * 0.5,
            2 => f64::EPSILON / 2.0,
            _ => i as f64 + 0.25,
        })
        .collect();

    let mut bulk = Model::new();
    let bvars = add_vars(&mut bulk, n);
    let obj = bulk.set_linear_objective_bulk(Sense::Minimize, &bvars, &coeffs, 3.0)?;
    assert_eq!(bulk.active_objective(), Some(obj));

    let mut scalar = Model::new();
    let svars = add_vars(&mut scalar, n);
    let mut expr = LinExpr::new();
    for (v, c) in svars.iter().zip(coeffs.iter()) {
        expr = expr.term(*c, *v);
    }
    expr = expr.constant(3.0);
    let sobj = scalar.minimize(expr)?;
    assert_eq!(scalar.active_objective(), Some(sobj));

    // Same canonical cells (position-wise values, zeros dropped identically).
    let (bsense, bc, bterms) = objective_state(&bulk);
    let (ssense, sc, sterms) = objective_state(&scalar);
    assert_eq!(bsense, ssense);
    assert!((bc - sc).abs() < 1e-12, "constant {bc} vs {sc}");
    assert_eq!(bterms.len(), sterms.len(), "same surviving cells");
    for ((_, bv), (_, sv)) in bterms.iter().zip(sterms.iter()) {
        assert!((bv - sv).abs() < 1e-12, "{bv} vs {sv}");
    }
    assert_eq!(bulk.num_coefficients(), scalar.num_coefficients());
    Ok(())
}

#[test]
fn bulk_rejects_nonfinite_atomically() {
    let mut model = Model::new();
    let vars = add_vars(&mut model, 4);
    let rev = model.current_revision();
    let seq = model.changelog_sequence();

    let bad = vec![1.0, f64::NAN, 2.0, 3.0];
    let err = model
        .set_linear_objective_bulk(Sense::Maximize, &vars, &bad, 0.0)
        .unwrap_err();
    assert!(matches!(err, ModelError::NonFiniteValue(_)));

    assert_eq!(model.num_objectives(), 0, "no dangling objective");
    assert_eq!(model.num_coefficients(), 0);
    assert_eq!(model.current_revision(), rev);
    assert_eq!(model.changelog_sequence(), seq, "no journal residue");
    assert_eq!(model.active_objective(), None);
}

#[test]
fn bulk_rejects_stale_var_and_length_mismatch_atomically() {
    let mut model = Model::new();
    let vars = add_vars(&mut model, 3);
    let rev = model.current_revision();

    // Genuinely stale variable: removed, so its generation no longer matches.
    // (Foreign-model handles with coincidentally equal slot IDs are the
    // Python owner layer's job; core identifies by slot identity.)
    let doomed = model.add_var();
    model.remove_variable(doomed).unwrap();
    let mixed = vec![vars[0], vars[1], doomed];
    let err = model
        .set_linear_objective_bulk(Sense::Minimize, &mixed, &[1.0, 2.0, 3.0], 0.0)
        .unwrap_err();
    assert!(matches!(err, ModelError::VariableNotFound(_)));
    assert_eq!(model.num_objectives(), 0);
    assert_eq!(model.current_revision(), rev);

    let err = model
        .set_linear_objective_bulk(Sense::Minimize, &vars, &[1.0, 2.0], 0.0)
        .unwrap_err();
    assert!(matches!(err, ModelError::MismatchedBulkLengths { .. }));
    assert_eq!(model.num_objectives(), 0);
    assert_eq!(model.current_revision(), rev);
}

#[test]
fn bulk_duplicate_vars_combine_like_scalar() -> Result<(), ModelError> {
    // R2.2: one canonical cell per (target, variable); duplicates combine.
    let mut bulk = Model::new();
    let x = bulk.add_var();
    let y = bulk.add_var();
    bulk.set_linear_objective_bulk(Sense::Minimize, &[x, y, x], &[1.0, 2.0, 3.0], 0.0)?;

    let mut scalar = Model::new();
    let sx = scalar.add_var();
    let sy = scalar.add_var();
    scalar.minimize(LinExpr::new().term(1.0, sx).term(2.0, sy).term(3.0, sx))?;

    assert_eq!(bulk.num_coefficients(), 2);
    let (_, _, bterms) = objective_state(&bulk);
    let (_, _, sterms) = objective_state(&scalar);
    assert_eq!(bterms.len(), 2);
    assert_eq!(sterms.len(), 2);
    assert!((bterms[0].1 - 4.0).abs() < 1e-12);
    assert!((sterms[0].1 - 4.0).abs() < 1e-12);
    assert!((bterms[1].1 - sterms[1].1).abs() < 1e-12);
    Ok(())
}

#[test]
fn bulk_journal_replays_like_snapshot() -> Result<(), ModelError> {
    // Commuting square with the expected PACKED ops: replaying the committed
    // batch from the pre-commit snapshot must equal the post-commit snapshot.
    let mut model = Model::new();
    let vars = add_vars(&mut model, 8);
    let coeffs: Vec<f64> = (1..=8).map(|i| i as f64).collect();
    let snap_before = model.take_snapshot()?;

    let obj = model.set_linear_objective_bulk(Sense::Maximize, &vars, &coeffs, 1.5)?;
    model.commit()?;
    let snap_after = model.take_snapshot()?;

    let cells: Arc<[(VarId, f64)]> = vars
        .iter()
        .zip(coeffs.iter())
        .map(|(v, c)| (*v, *c))
        .collect::<Vec<_>>()
        .into();
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
        expected_backend.normalized_view(),
        "packed bulk ops must replay exactly like the snapshot"
    );
    Ok(())
}

#[test]
fn bulk_snapshots_deterministic() -> Result<(), ModelError> {
    // Two fresh builds from the same input produce identical snapshots.
    fn build() -> Result<roml::snapshot::ModelSnapshot, ModelError> {
        let mut m = Model::new();
        let vars = add_vars(&mut m, 16);
        let coeffs: Vec<f64> = (0..16).map(|i| i as f64 + 1.0).collect();
        m.set_linear_objective_bulk(Sense::Minimize, &vars, &coeffs, 0.0)?;
        m.take_snapshot()
    }
    assert_eq!(build()?, build()?);
    Ok(())
}

#[test]
fn scalar_parameterized_objective_still_works() -> Result<(), ModelError> {
    // Regression lock: the general path is untouched by the bulk addition.
    let mut model = Model::new();
    let p = model.add_parameter(2.0)?;
    let x = model.add_var();
    let y = model.add_var();
    let obj = model.maximize(LinExpr::new().term(ValueExpr::param(p), x).term(3.0, y))?;
    model.commit()?;
    model.set_parameter(p, 4.0)?;
    model.commit()?;
    let expr = model.objective_expression(obj)?;
    assert_eq!(expr.num_terms(), 2);
    Ok(())
}
