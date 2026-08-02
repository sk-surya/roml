//! P21 Tasks 5-6 — End-to-end behavior of the user-facing `Highs` façade.
//!
//! `Highs` is the M2 golden-path solver type (API-01, D3): it wraps
//! [`SolverSession`] with a `HighsSession` backend and exposes only
//! `new`/`solve`/`solve_with`. Users never touch commits, snapshots, delta
//! batches, cursors, or synchronization (D2/D5).
//!
//! These tests qualify the full repeated-solve lifecycle on the real HiGHS
//! backend: first solve, no-change re-solve, bound and parameter deltas,
//! objective switch, unsupported-model error behavior (never a fabricated
//! result), and failed-option-validation state preservation (plan Task 6).
//!
//! Rebuild-required recovery semantics (at most one snapshot rebuild retry,
//! then a correct solve) are exercised in `tests/solver_facade.rs` with
//! fault-injecting reference backends; with real HiGHS every supported model
//! change applies as a delta (the projection implements all 18 `ModelOp`
//! variants), so the only delta-rejection — semi-continuous domains — is
//! also rejected by snapshot rebuild and must surface as an error, never as
//! a stale or fabricated result.

use roml::model::{Bounds, Model};
use roml::prelude::*;
use roml::SynchronizationMode;
use roml_highs::Highs;

fn approx(a: Option<f64>, expected: f64) {
    let got = a.unwrap_or_else(|| panic!("expected Some({expected}) but got None"));
    assert!(
        (got - expected).abs() < 1e-6,
        "objective {got} != expected {expected}"
    );
}

/// The canonical M2 quickstart model: named LP/MILP, method-first modeling.
fn quickstart_model() -> (Model, roml::Variable, roml::Variable) {
    let mut model = Model::named("production");
    let x = model
        .add_variable(continuous().named("x"))
        .expect("continuous variable");
    let y = model
        .add_variable(integer().bounds(0.0, 10.0).named("y"))
        .expect("integer variable");
    model
        .add_constraint((x + y).le(4.0).named("capacity"))
        .expect("constraint");
    model.maximize(3.0 * x + y).expect("objective should build");
    (model, x, y)
}

/// The parameterized incremental model from the P20 repeated-solve baseline:
/// `maximize price*x + y` s.t. `x + y <= 4`, `x, y >= 0`.
fn parameterized_model(price_value: f64) -> (Model, roml::Variable, roml::Parameter) {
    let mut model = Model::new();
    let x = model
        .add_variable(continuous())
        .expect("continuous variable");
    let _y = model
        .add_variable(continuous())
        .expect("continuous variable");
    let price = model
        .add_parameter(parameter(price_value).named("price"))
        .expect("parameter");
    model.add_constraint((x + _y).le(4.0)).expect("constraint");
    model
        .maximize(price * x + _y)
        .expect("objective should build");
    (model, x, price)
}

/// Task 6a — First solve from a new model: the quickstart solves to optimal
/// and the solution carries metadata (backend name, committed revision, and
/// a synchronization mode).
#[test]
fn first_solve_from_new_model() -> Result<(), Box<dyn std::error::Error>> {
    let (mut model, x, _y) = quickstart_model();
    let mut highs = Highs::new()?;

    let solution = highs.solve(&mut model)?;
    assert!(solution.status().is_optimal());
    approx(solution.objective_value(), 12.0);
    assert!(solution.value(x).is_some(), "x value available");
    assert_eq!(
        solution.metadata().model_revision,
        model.current_revision(),
        "metadata carries the committed model revision"
    );
    assert!(
        !solution.metadata().backend_name.is_empty(),
        "metadata identifies the backend"
    );
    // A fresh backend (revision r0) behind a committed model is synchronized
    // through the retained r0->r1 delta chain when the journal provides it
    // (delta-first orchestration); a journal without that chain would rebuild
    // from a snapshot. Both are valid orchestration outcomes — the contract
    // is "synchronized and correct", not a specific mode.
    assert!(
        matches!(
            solution.metadata().synchronization,
            SynchronizationMode::Delta | SynchronizationMode::Rebuild
        ),
        "first solve synchronizes via delta chain or snapshot rebuild"
    );
    Ok(())
}

/// Task 6b — No-change second solve: the façade detects the backend is
/// already current and skips synchronization entirely.
#[test]
fn no_change_second_solve_is_no_sync() -> Result<(), Box<dyn std::error::Error>> {
    let (mut model, _x, _y) = quickstart_model();
    let mut highs = Highs::new()?;

    let first = highs.solve(&mut model)?;
    assert!(first.status().is_optimal());
    approx(first.objective_value(), 12.0);

    let second = highs.solve(&mut model)?;
    assert!(second.status().is_optimal());
    approx(second.objective_value(), 12.0);
    assert_eq!(
        second.metadata().synchronization,
        SynchronizationMode::NoChange,
        "no model change -> no synchronization"
    );
    Ok(())
}

/// Task 6c — Bound delta second solve: tightening a bound re-solves through
/// an incremental delta (P20 baseline: 12.0 -> 8.0).
#[test]
fn bound_delta_second_solve() -> Result<(), Box<dyn std::error::Error>> {
    let (mut model, x, _y) = quickstart_model();
    let mut highs = Highs::new()?;

    let first = highs.solve(&mut model)?;
    approx(first.objective_value(), 12.0);

    model.set_variable_bounds(x, Bounds::new(0.0, 2.0))?;
    let second = highs.solve(&mut model)?;
    assert!(second.status().is_optimal());
    approx(second.objective_value(), 8.0);
    assert_eq!(
        second.metadata().synchronization,
        SynchronizationMode::Delta,
        "bound change applies as a delta"
    );
    Ok(())
}

/// Task 6d — Parameter-driven coefficient delta second solve (P20 baseline:
/// price 3.0 -> 12.0, price 5.0 -> 20.0). One `Highs`, no user-facing sync.
#[test]
fn parameter_delta_second_solve() -> Result<(), Box<dyn std::error::Error>> {
    let (mut model, _x, price) = parameterized_model(3.0);
    let mut highs = Highs::new()?;

    let first = highs.solve(&mut model)?;
    assert!(first.status().is_optimal());
    approx(first.objective_value(), 12.0);

    model.set_parameter(price, 5.0)?;
    let second = highs.solve(&mut model)?;
    assert!(second.status().is_optimal());
    approx(second.objective_value(), 20.0);
    assert_eq!(
        second.metadata().synchronization,
        SynchronizationMode::Delta,
        "parameter change applies as a delta"
    );
    Ok(())
}

/// Task 6e — Objective switch: replacing the active objective re-solves
/// against the new objective through the same `Highs` instance.
#[test]
fn objective_switch_second_solve() -> Result<(), Box<dyn std::error::Error>> {
    let (mut model, x, _y) = quickstart_model();
    let mut highs = Highs::new()?;

    let first = highs.solve(&mut model)?;
    approx(first.objective_value(), 12.0);

    model.minimize(x)?;
    let second = highs.solve(&mut model)?;
    assert!(second.status().is_optimal());
    approx(second.objective_value(), 0.0);
    Ok(())
}

/// Task 6f — An unsupported model change surfaces as an error, never as a
/// stale or fabricated result. Semi-continuous domains are rejected by the
/// HiGHS projection on both the delta and the rebuild path, so the façade
/// refuses the solve (API-03.3 operational-failure semantics) instead of
/// reporting the previous solution as current (API-01.5).
#[test]
fn unsupported_model_returns_error_never_stale() -> Result<(), Box<dyn std::error::Error>> {
    let (mut model, x, _y) = quickstart_model();
    let mut highs = Highs::new()?;

    let first = highs.solve(&mut model)?;
    assert!(first.status().is_optimal());
    approx(first.objective_value(), 12.0);

    // Semi-continuous domains are unsupported by HiGHS end-to-end (the
    // projection rejects the delta batch AND the snapshot rebuild). The
    // façade must return an error — after at most one rebuild retry — and
    // must not return the previous 12.0 solution as if it were current.
    model.set_semicontinuous(x, 1.0)?;
    let err = highs.solve(&mut model);
    assert!(
        matches!(err, Err(SolveError::Synchronization(_))),
        "unsupported model must surface as a synchronization error: {err:?}"
    );
    Ok(())
}

/// Task 6g — Failed option validation leaves the model and backend state
/// unchanged: an invalid option set errors before synchronization, and a
/// subsequent valid solve still produces the correct result.
#[test]
fn failed_option_validation_leaves_state_unchanged() -> Result<(), Box<dyn std::error::Error>> {
    let (mut model, _x, _y) = quickstart_model();
    let mut highs = Highs::new()?;

    let first = highs.solve(&mut model)?;
    approx(first.objective_value(), 12.0);

    let err = highs.solve_with(&mut model, SolveOptions::new().threads(0));
    assert!(
        matches!(err, Err(SolveError::InvalidOptions(_))),
        "threads(0) must fail validation before any backend mutation"
    );

    // Model and backend are untouched: the next valid solve is correct.
    let again = highs.solve(&mut model)?;
    assert!(again.status().is_optimal());
    approx(again.objective_value(), 12.0);
    Ok(())
}
