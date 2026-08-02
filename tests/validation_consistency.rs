//! P23 Task 3 — Validation is consistent across all build profiles (D10,
//! API-06). Every public model mutation and solve-option builder rejects
//! NaN, infinities, inverted bounds, and stale IDs with a typed error, in
//! debug AND release. Failed mutations are atomic: counts, changelog, and
//! revision are unchanged (API-06.5).
//!
//! These tests cover the P22 deferred surface-consistency items:
//!  - `set_variable_bounds` validation (deferred item 2);
//!  - `VarId - VarId` expression operator (deferred item 3);
//!  - raw `*_coefficient` non-finite rejection (deferred item 5).

use roml::model::{CoefficientTarget, ConstraintBounds};
use roml::prelude::*;

/// Snapshot the mutation-sensitive state of a model.
struct State {
    vars: usize,
    cons: usize,
    objs: usize,
    params: usize,
    coeffs: usize,
    seq: u64,
    rev: roml::ModelRevision,
}

fn snapshot(model: &Model) -> State {
    State {
        vars: model.num_variables(),
        cons: model.num_constraints(),
        objs: model.num_objectives(),
        params: model.num_parameters(),
        coeffs: model.num_coefficients(),
        seq: model.changelog_sequence(),
        rev: model.current_revision(),
    }
}

fn assert_unchanged(model: &Model, before: &State) {
    assert_eq!(model.num_variables(), before.vars, "variables");
    assert_eq!(model.num_constraints(), before.cons, "constraints");
    assert_eq!(model.num_objectives(), before.objs, "objectives");
    assert_eq!(model.num_parameters(), before.params, "parameters");
    assert_eq!(model.num_coefficients(), before.coeffs, "coefficients");
    assert_eq!(model.changelog_sequence(), before.seq, "changelog");
    assert_eq!(model.current_revision(), before.rev, "revision");
}

// ── set_variable_bounds (deferred item 2) ──────────────────────────────────

/// Inverted bounds are rejected before mutation.
#[test]
fn set_variable_bounds_rejects_inverted() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).expect("x");
    let before = snapshot(&model);
    let err = model.set_variable_bounds(x, Bounds::new(10.0, 0.0));
    assert_eq!(err, Err(ModelError::InvalidBounds));
    assert_unchanged(&model, &before);
}

/// NaN bounds are rejected (mirrors `add_variable`).
#[test]
fn set_variable_bounds_rejects_nan() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).expect("x");
    let before = snapshot(&model);
    assert_eq!(
        model.set_variable_bounds(x, Bounds::new(f64::NAN, 1.0)),
        Err(ModelError::InvalidBounds)
    );
    assert_eq!(
        model.set_variable_bounds(x, Bounds::new(0.0, f64::NAN)),
        Err(ModelError::InvalidBounds)
    );
    assert_unchanged(&model, &before);
}

/// A +inf lower bound (with +inf upper) or -inf upper bound is a non-finite
/// misuse, not a legitimate unbounded side.
#[test]
fn set_variable_bounds_rejects_misused_infinities() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).expect("x");
    assert_eq!(
        model.set_variable_bounds(x, Bounds::new(f64::INFINITY, f64::INFINITY)),
        Err(ModelError::NonFiniteValue("variable lower bound"))
    );
    assert_eq!(
        model.set_variable_bounds(x, Bounds::new(f64::NEG_INFINITY, f64::NEG_INFINITY)),
        Err(ModelError::NonFiniteValue("variable upper bound"))
    );
}

/// Binary variables must keep bounds inside [0, 1] (API-06.4).
#[test]
fn set_variable_bounds_rejects_binary_outside_unit_interval() {
    let mut model = Model::new();
    let x = model.add_variable(binary()).expect("x");
    let before = snapshot(&model);
    assert_eq!(
        model.set_variable_bounds(x, Bounds::new(0.0, 2.0)),
        Err(ModelError::InvalidBinaryBounds)
    );
    assert_eq!(
        model.set_variable_bounds(x, Bounds::new(-1.0, 1.0)),
        Err(ModelError::InvalidBinaryBounds)
    );
    assert_unchanged(&model, &before);
    // Subsets of [0, 1] remain accepted.
    assert!(model.set_variable_bounds(x, Bounds::new(0.0, 1.0)).is_ok());
    assert!(model
        .set_variable_bounds(x, Bounds::new(0.25, 0.75))
        .is_ok());
}

/// A stale variable ID is rejected atomically.
#[test]
fn set_variable_bounds_stale_id_is_atomic() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).expect("x");
    model.remove_variable(x).expect("remove");
    let before = snapshot(&model);
    assert_eq!(
        model.set_variable_bounds(x, Bounds::new(0.0, 5.0)),
        Err(ModelError::VariableNotFound(x))
    );
    assert_unchanged(&model, &before);
}

// ── set_constraint_bounds / constraint bounds at insertion ─────────────────

/// NaN constraint bounds are rejected at insertion (atomic) and on mutation.
#[test]
fn nan_constraint_bounds_rejected_atomically() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).expect("x");
    let before = snapshot(&model);

    let err = model.add_constraint(ConstraintBounds::le(f64::NAN));
    assert_eq!(err, Err(ModelError::NonFiniteValue("constraint bound")));
    assert_unchanged(&model, &before);

    let err = model.add_constraint_expr(x, ConstraintBounds::le(f64::NAN));
    assert_eq!(err, Err(ModelError::NonFiniteValue("constraint bound")));
    assert_unchanged(&model, &before);
}

/// `set_constraint_bounds` rejects NaN but accepts legitimate ±inf sides.
#[test]
fn set_constraint_bounds_rejects_nan() {
    let mut model = Model::new();
    let con = model
        .add_constraint(ConstraintBounds::le(4.0))
        .expect("con");
    let before = snapshot(&model);

    assert_eq!(
        model.set_constraint_bounds(con, ConstraintBounds::le(f64::NAN)),
        Err(ModelError::NonFiniteValue("constraint bound"))
    );
    assert_eq!(
        model.set_constraint_bounds(con, ConstraintBounds::range(f64::NAN, 5.0)),
        Err(ModelError::NonFiniteValue("constraint bound"))
    );
    assert_unchanged(&model, &before);

    // ±inf sides remain valid (le/ge forms).
    assert!(model
        .set_constraint_bounds(con, ConstraintBounds::le(10.0))
        .is_ok());
    assert!(model
        .set_constraint_bounds(con, ConstraintBounds::ge(-5.0))
        .is_ok());
}

// ── set_semicontinuous ─────────────────────────────────────────────────────

/// NaN semi-continuous lower bounds are rejected.
#[test]
fn set_semicontinuous_rejects_nan() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).expect("x");
    let before = snapshot(&model);
    assert_eq!(
        model.set_semicontinuous(x, f64::NAN),
        Err(ModelError::NonFiniteValue("semi-continuous lower bound"))
    );
    assert_unchanged(&model, &before);
}

// ── raw *_coefficient mutators (deferred item 5) ───────────────────────────

/// `add_constraint_coefficient` rejects non-finite constants atomically.
#[test]
fn add_constraint_coefficient_rejects_non_finite() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).expect("x");
    let con = model
        .add_constraint(ConstraintBounds::le(100.0))
        .expect("con");
    let before = snapshot(&model);

    let err = model.add_constraint_coefficient(con, x, roml::ValueExpr::constant(f64::NAN));
    assert_eq!(err, Err(ModelError::NonFiniteValue("coefficient value")));
    assert_unchanged(&model, &before);

    let err = model.add_constraint_coefficient(con, x, roml::ValueExpr::constant(f64::INFINITY));
    assert_eq!(err, Err(ModelError::NonFiniteValue("coefficient value")));
    assert_unchanged(&model, &before);
}

/// `add_objective_coefficient` rejects non-finite constants atomically.
#[test]
fn add_objective_coefficient_rejects_non_finite() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).expect("x");
    let obj = model.add_objective(roml::Sense::Maximize);
    let before = snapshot(&model);

    let err = model.add_objective_coefficient(obj, x, roml::ValueExpr::constant(f64::NAN));
    assert_eq!(err, Err(ModelError::NonFiniteValue("coefficient value")));
    assert_unchanged(&model, &before);
}

/// A NaN term coefficient in a constraint spec is rejected BEFORE the row is
/// inserted — no dangling row, no changelog event, no revision (API-06.5).
#[test]
fn expr_with_nan_coefficient_is_rejected_atomically() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).expect("x");
    let before = snapshot(&model);

    let err = model.add_constraint((f64::NAN * x).le(10.0));
    assert_eq!(err, Err(ModelError::NonFiniteValue("coefficient value")));
    assert_unchanged(&model, &before);
    assert!(!model.has_uncommitted());

    let err = model.maximize(f64::INFINITY * x);
    assert_eq!(err, Err(ModelError::NonFiniteValue("coefficient value")));
    assert_unchanged(&model, &before);
}

/// The D11 sparse trio continues to reject non-finite values in all profiles.
#[test]
fn sparse_trio_rejects_non_finite() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).expect("x");
    let con = model
        .add_constraint(ConstraintBounds::le(100.0))
        .expect("con");
    let before = snapshot(&model);

    assert_eq!(
        model.set_coefficient(CoefficientTarget::Constraint(con), x, f64::NAN),
        Err(ModelError::NonFiniteValue("coefficient value"))
    );
    assert_eq!(
        model.add_to_coefficient(CoefficientTarget::Constraint(con), x, f64::INFINITY),
        Err(ModelError::NonFiniteValue("coefficient value"))
    );
    assert_unchanged(&model, &before);
}

// ── VarId - VarId operator (deferred item 3) ───────────────────────────────

/// `x - y` (two bare variable handles) compiles and evaluates like the
/// existing `x + y` form.
#[test]
fn var_id_sub_var_id_produces_expression() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).expect("x");
    let y = model.add_variable(continuous()).expect("y");

    let expr: roml::LinExpr = x - y;
    assert_eq!(expr.num_terms(), 2);
    assert_eq!(expr.get_constant(), 0.0);

    let con = model.add_constraint(expr.le(1.0)).expect("con");
    assert_eq!(model.num_constraints(), 1);
    assert_eq!(
        model.constraint_bounds(con),
        Some(ConstraintBounds::le(1.0))
    );

    // (x - y) composes with scalar multiplication.
    let scaled: roml::LinExpr = (x - y) * 2.0;
    assert_eq!(scaled.num_terms(), 2);
}
