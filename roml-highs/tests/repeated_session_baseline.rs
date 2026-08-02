//! P20 Task 5 — Baseline repeated-solve protocol behavior for `HighsSession`.
//!
//! This is the direct-session baseline that P21's `SolverSession<B>` / `Highs`
//! façade must reproduce end-to-end. It records, per solve attempt:
//!
//! - model revision (`SessionHealth::revision`),
//! - cursor health (`SessionHealth::health`),
//! - termination status and objective value (`SolveResult`/`SolutionView`),
//! - solution availability (`SolveSolution` presence and `SolutionView`).
//!
//! ## Expected baseline results (used by P21 façade tests)
//!
//! Model: `maximize price*x + y` subject to `x + y <= 4`, `x,y >= 0`.
//!
//! | Step | Revision | Health | Status | Objective |
//! |---|---|---|---|---|
//! | Rebuild from snapshot | r1 | Ready | Optimal | 12.0 (`price = 3.0`) |
//! | Apply parameter delta (`price 3.0 -> 5.0`) | r2 | Ready | Optimal | 20.0 |
//! | Failed delta (mismatched base) | r1 (unchanged) | RequiresRebuild | — | prior solution stays readable but stale |
//! | Snapshot rebuild (deterministic recovery) | r1 | Ready | Optimal | 12.0 |
//!
//! The last two rows are the "unsupported/dirty path": a delta whose base
//! revision does not match the cursor (a missed/unsupported incremental
//! update) is rejected before mutation, the cursor demands a rebuild, and a
//! deterministic snapshot rebuild restores `Ready` and a correct solve.
//!
//! Characterized current behavior on the rejected delta (asserted in
//! `dirty_path_recovers_via_deterministic_snapshot_rebuild`): the previously
//! reported solution REMAINS readable through `SolutionView` — the objective
//! still reads 12.0 — but it is stale relative to the advanced model. It is
//! not invalidated. P21's façade must never report that stale result as
//! current (API-01.5); this baseline records what the session layer does
//! today so P21 can decide the invalidation policy.

use roml::delta::{DeltaBatch, ModelOp};
use roml::expr::ConstraintExprExt;
use roml::id::VarId;
use roml::model::{Bounds, Model};
use roml::revision::ModelRevision;
use roml::solver::backend::TerminationStatus;
use roml::solver::request::SolveRequest;
use roml::solver::session::{BackendSession, SessionHealth, SolutionView, Synchronization};
use roml::sync::AdapterHealth;
use roml_highs::HighsSession;

fn approx(a: Option<f64>, expected: f64) {
    let got = a.unwrap_or_else(|| panic!("expected Some({expected}) but got None"));
    assert!(
        (got - expected).abs() < 1e-6,
        "objective {got} != expected {expected}"
    );
}

/// Build `maximize price*x + y` s.t. `x + y <= 4` with a parameterized
/// objective coefficient, commit it, and return the model plus the created IDs.
fn build_parameterized_model(price_value: f64) -> (Model, VarId, VarId, roml::id::ParamId) {
    let mut model = Model::new();
    let x = model.add_var();
    let y = model.add_var();
    let price = model.add_parameter(price_value);
    model
        .constrain((x + y).le(4.0))
        .expect("constraint should build");
    model
        .maximize(price * x + y)
        .expect("objective should build");
    (model, x, y, price)
}

/// Rebuild + solve, then apply one parameter delta and solve again, recording
/// revisions, health, status, objective, and solution availability at each
/// step. This is the golden repeated-solve baseline.
#[test]
fn repeated_solve_rebuild_then_parameter_delta() -> Result<(), Box<dyn std::error::Error>> {
    let (mut model, _x, _y, price) = build_parameterized_model(3.0);
    let r1 = model.commit()?;

    let mut session = HighsSession::try_new().expect("HiGHS should be available");

    // Rebuild from the committed snapshot at r1.
    let snap = model.take_snapshot()?;
    session
        .synchronize(Synchronization::Rebuild(snap))
        .expect("rebuild should succeed");
    assert_eq!(session.revision(), r1, "revision after rebuild");
    assert_eq!(session.health(), AdapterHealth::Ready);

    // First solve: price = 3.0 -> optimal objective 12.0.
    let first = session.solve(&SolveRequest::new()).expect("first solve");
    assert_eq!(first.termination, TerminationStatus::Optimal);
    assert!(
        first.solution.is_some(),
        "solution must be available after an optimal solve"
    );
    approx(first.solution.as_ref().unwrap().objective_value, 12.0);
    approx(session.objective_value(), 12.0);
    assert!(session.value(_x).is_some(), "variable value available");

    // Apply one parameter delta: price 3.0 -> 5.0.
    model.set_parameter(price, 5.0);
    let r2 = model.commit()?;
    let batches = model.deltas_since(r1)?;
    let batch = batches.last().expect("exactly the r1->r2 batch");
    assert_eq!(batch.to, r2, "delta target revision");

    session
        .synchronize(Synchronization::DeltaBatch((*batch).clone()))
        .expect("parameter delta should apply incrementally");
    assert_eq!(session.revision(), r2, "revision after delta");
    assert_eq!(session.health(), AdapterHealth::Ready);

    // Second solve: price = 5.0 -> optimal objective 20.0.
    let second = session.solve(&SolveRequest::new()).expect("second solve");
    assert_eq!(second.termination, TerminationStatus::Optimal);
    assert!(
        second.solution.is_some(),
        "solution must be available after re-solve"
    );
    approx(second.solution.as_ref().unwrap().objective_value, 20.0);
    approx(session.objective_value(), 20.0);

    Ok(())
}

/// A bound change applied as a delta must also re-solve correctly without a
/// rebuild — the second supported delta flavor the P21 façade must keep on the
/// incremental path.
#[test]
fn repeated_solve_bound_delta_updates_optimal() -> Result<(), Box<dyn std::error::Error>> {
    let (mut model, x, _y, _price) = build_parameterized_model(3.0);
    let r1 = model.commit()?;

    let mut session = HighsSession::try_new().expect("HiGHS should be available");
    session
        .synchronize(Synchronization::Rebuild(model.take_snapshot()?))
        .expect("rebuild should succeed");
    let first = session.solve(&SolveRequest::new()).expect("first solve");
    assert_eq!(first.termination, TerminationStatus::Optimal);
    approx(first.solution.as_ref().unwrap().objective_value, 12.0);

    // Tighten x's upper bound to 2.0: max 3x + y, x+y<=4, x<=2 -> x=2,y=2,obj=8.
    model.set_variable_bounds(x, Bounds::new(0.0, 2.0))?;
    let r2 = model.commit()?;
    let batches = model.deltas_since(r1)?;
    let batch = batches.last().expect("r1->r2 batch");
    assert_eq!(batch.to, r2);

    session
        .synchronize(Synchronization::DeltaBatch((*batch).clone()))
        .expect("bound delta should apply incrementally");
    assert_eq!(session.revision(), r2);
    assert_eq!(session.health(), AdapterHealth::Ready);

    let second = session.solve(&SolveRequest::new()).expect("second solve");
    assert_eq!(second.termination, TerminationStatus::Optimal);
    approx(second.solution.as_ref().unwrap().objective_value, 8.0);
    approx(session.objective_value(), 8.0);

    Ok(())
}

/// Unsupported/dirty path: a delta whose base revision does not match the
/// cursor is rejected before mutation, the cursor requires a rebuild, and a
/// deterministic snapshot rebuild restores `Ready` and a correct solve.
#[test]
fn dirty_path_recovers_via_deterministic_snapshot_rebuild() -> Result<(), Box<dyn std::error::Error>>
{
    let (mut model, x, _y, _price) = build_parameterized_model(3.0);
    let r1 = model.commit()?;

    let mut session = HighsSession::try_new().expect("HiGHS should be available");
    let snap = model.take_snapshot()?;
    session
        .synchronize(Synchronization::Rebuild(snap.clone()))
        .expect("rebuild should succeed");
    session
        .solve(&SolveRequest::new())
        .expect("first solve succeeds");
    assert_eq!(session.health(), AdapterHealth::Ready);

    // A delta batch whose base revision is ahead of the cursor simulates a
    // missed or unsupported incremental update.
    let r10 = ModelRevision::from_u64(10);
    let r11 = r10.next().expect("next revision");
    let bad_batch = DeltaBatch::new(
        r10,
        r11,
        vec![ModelOp::SetVariableBounds {
            var: x,
            bounds: Bounds::new(0.0, 5.0),
        }],
    )
    .expect("valid batch");

    let res = session.synchronize(Synchronization::DeltaBatch(bad_batch));
    assert!(res.is_err(), "mismatched-base delta must be rejected");
    assert_eq!(
        session.health(),
        AdapterHealth::RequiresRebuild,
        "cursor must require a rebuild after a rejected delta"
    );
    assert_eq!(session.revision(), r1, "revision unchanged after rejection");

    // Characterized current behavior: the prior optimal solution REMAINS
    // readable through SolutionView after the rejected delta — the objective
    // still reads 12.0 — but it is stale relative to the advanced model
    // (revision r1 cannot satisfy the model at r11). It is not invalidated.
    // P21 (API-01.5) must decide that the façade never reports this stale
    // result as current; this assertion freezes what the session layer does
    // today as the parity target for that decision.
    assert_eq!(
        session.objective_value(),
        Some(12.0),
        "prior solution stays readable (but stale) after a rejected delta"
    );
    assert!(
        session.value(x).is_some(),
        "prior variable values also stay readable after a rejected delta"
    );

    // Deterministic recovery: rebuild from the snapshot restores Ready and a
    // correct, non-stale solve.
    session
        .synchronize(Synchronization::Rebuild(snap))
        .expect("rebuild after failure should succeed");
    assert_eq!(session.health(), AdapterHealth::Ready);
    assert_eq!(session.revision(), r1);

    let recovered = session
        .solve(&SolveRequest::new())
        .expect("recovered solve");
    assert_eq!(recovered.termination, TerminationStatus::Optimal);
    assert!(recovered.solution.is_some());
    approx(recovered.solution.as_ref().unwrap().objective_value, 12.0);
    approx(session.objective_value(), 12.0);

    Ok(())
}
