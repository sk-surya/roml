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

use std::time::Duration;

use roml::advanced::{
    BackendCapabilitySet, BackendFeature, BackendSnapshot, CompilationPolicy, CompilationSession,
    FeatureSupport, SupportLevel,
};
use roml::model::{Bounds, Model};
use roml::prelude::*;
use roml::solver::session::{BackendSession, SessionHealth, Synchronization};
use roml::sync::AdapterHealth;
use roml::SynchronizationMode;
use roml_highs::{Highs, HighsSession};

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

/// Options must not leak across solves: HiGHS options persist on the native
/// session, so each request is self-contained — unspecified options are reset
/// to HiGHS defaults before the request's explicit values are applied. A
/// default solve after a configured `solve_with` must report (and run with)
/// defaults, not the previous time limit/threads.
#[test]
fn default_solve_after_solve_with_resets_options() -> Result<(), Box<dyn std::error::Error>> {
    let (mut model, _x, _y) = quickstart_model();
    let mut highs = Highs::new()?;

    // Configured solve: time limit + threads are applied and reported.
    let limited = highs.solve_with(
        &mut model,
        SolveOptions::new()
            .time_limit(Duration::from_secs(60))
            .threads(1),
    )?;
    assert!(limited.status().is_optimal());
    assert_eq!(
        limited.metadata().effective_configuration.time_limit_secs,
        Some(60.0)
    );
    assert_eq!(limited.metadata().effective_configuration.threads, Some(1));

    // Default solve: the previous options must not be retained.
    let again = highs.solve(&mut model)?;
    assert!(again.status().is_optimal());
    approx(again.objective_value(), 12.0);
    assert_eq!(
        again.metadata().effective_configuration.time_limit_secs,
        None,
        "time limit must not leak into a default solve"
    );
    assert_eq!(
        again.metadata().effective_configuration.threads,
        None,
        "threads must not leak into a default solve"
    );
    Ok(())
}

/// Arbitrary `backend_option(key, value)` entries must not leak across
/// solves either: they persist on the native HiGHS handle, so the per-request
/// reset must cover them (via `Highs_resetOptions`). Successful extra
/// options are also recorded in the effective metadata.
#[test]
fn backend_option_is_recorded_and_reset_on_next_solve() -> Result<(), Box<dyn std::error::Error>> {
    let (mut model, _x, _y) = quickstart_model();
    let mut highs = Highs::new()?;

    // A successful extra option is applied AND recorded in metadata.
    let with_opt = highs.solve_with(
        &mut model,
        SolveOptions::new().backend_option("presolve", "off"),
    )?;
    assert!(with_opt.status().is_optimal());
    assert!(
        with_opt
            .metadata()
            .effective_configuration
            .adjustments
            .iter()
            .any(|a| a.key == "presolve" && a.applied == "off"),
        "successful backend_option must be recorded in effective metadata"
    );

    // A default solve must not retain the previous extra option.
    let again = highs.solve(&mut model)?;
    assert!(again.status().is_optimal());
    assert!(
        again
            .metadata()
            .effective_configuration
            .adjustments
            .iter()
            .all(|a| a.key != "presolve"),
        "backend_option must not leak into a default solve"
    );
    Ok(())
}

/// Session-level health regression (PR #21 review round 2): a rejected delta
/// whose error is NOT terminal must mark the session RequiresRebuild — never
/// Terminal. (The terminal mapping itself is unit-tested in
/// `roml-highs/src/session.rs`.)
#[test]
fn rejected_unsupported_delta_marks_session_rebuild_not_terminal(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut model = Model::new();
    let x = model.add_variable(continuous())?;
    model.add_constraint(x.le(10.0))?;
    model.maximize(x)?;
    let r1 = model.commit()?;

    let mut session = HighsSession::try_new()?;
    // Compile the canonical snapshot into backend IR for the compiled session
    // (P26 Task 7); the session no longer accepts a canonical `ModelSnapshot`.
    let mut capabilities = BackendCapabilitySet::new();
    for feature in [
        BackendFeature::Lp,
        BackendFeature::Mip,
        BackendFeature::IncrementalBounds,
        BackendFeature::IncrementalRows,
        BackendFeature::IncrementalCoefficients,
    ] {
        capabilities.set(
            feature,
            FeatureSupport {
                level: SupportLevel::Native,
                limitations: Default::default(),
            },
        );
    }
    let mut compiler = CompilationSession::new();
    let compiled: BackendSnapshot = compiler
        .compile_snapshot(
            model.instance(),
            &model.take_snapshot()?,
            &CompilationPolicy::Auto,
            &capabilities,
        )
        .expect("snapshot must compile");
    session.synchronize(Synchronization::CompiledRebuild(compiled))?;
    assert_eq!(session.health(), AdapterHealth::Ready);

    // Advance the model with an unsupported operation (semi-continuous
    // domain): the projection rejects the delta with HealthEffect::RequiresRebuild.
    model.set_semicontinuous(x, 1.0)?;
    let r2 = model.commit()?;
    let batches = model.deltas_since(r1)?;
    let batch = batches.last().expect("r1->r2 batch");
    assert_eq!(batch.to, r2);

    let err = session.synchronize(Synchronization::DeltaBatch((*batch).clone()));
    assert!(err.is_err(), "semi-continuous delta must be rejected");
    assert_eq!(
        session.health(),
        AdapterHealth::RequiresRebuild,
        "unsupported/recoverable failure demands a rebuild, not terminal"
    );
    Ok(())
}

/// F4: the no-sync branch must require a compiled base. A fresh revision-zero
/// model (an untouched `Model::new()`) reaching its FIRST solve must NOT take
/// the no-sync path against a backend with no compiled state — that would hit
/// HiGHS's "solve called before any compiled synchronization" error. The
/// façade must force the snapshot rebuild so the compiled base is established
/// before solve.
#[test]
fn first_solve_of_revision_zero_model_establishes_compiled_base(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut model = Model::new();
    assert_eq!(
        model.current_revision(),
        roml::revision::ModelRevision::ZERO,
        "an untouched model is at revision zero"
    );
    let mut highs = Highs::new()?;
    let solution = highs.solve(&mut model)?;
    assert_eq!(
        solution.metadata().synchronization,
        SynchronizationMode::Rebuild,
        "a fresh revision-zero model must compile via a snapshot rebuild on first solve"
    );
    Ok(())
}

/// F4: once the compiled base is established, a second no-change solve of the
/// same revision-zero model takes the no-sync path.
#[test]
fn second_solve_of_revision_zero_model_is_no_sync() -> Result<(), Box<dyn std::error::Error>> {
    let mut model = Model::new();
    let mut highs = Highs::new()?;
    let first = highs.solve(&mut model)?;
    assert_eq!(
        first.metadata().synchronization,
        SynchronizationMode::Rebuild
    );
    let second = highs.solve(&mut model)?;
    assert_eq!(
        second.metadata().synchronization,
        SynchronizationMode::NoChange,
        "a second no-change solve takes the no-sync path once a compiled base exists"
    );
    Ok(())
}
