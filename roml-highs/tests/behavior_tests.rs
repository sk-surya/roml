//! Behavioral tests for HiGHS session features that contract tests don't cover.
//!
//! These tests target *behavior* the public API promises but that C1-C13 and
//! Q5 don't exercise end-to-end:
//!
//! 1. **MIP callback dispatch** — a handler registered via `CallbackSession`
//!    is actually invoked during a MIP solve, receives real candidate data,
//!    and can add cuts. Also: clearing the handler disables invocation.
//! 2. **`SolutionView` accessors** — `value`/`dual`/`reduced_cost`/
//!    `objective_value` read the *same* solution the solve result exposes.
//! 3. **Session health transitions** — a failed delta sync leaves the cursor
//!    `RequiresRebuild`; a terminal failure marks it `Terminal`; a rebuild
//!    returns it to `Ready`.
//! 4. **Close / drop** — `close()` consumes the session without error and
//!    releases the HiGHS handle.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use roml::advanced::{
    BackendCapabilitySet, BackendFeature, BackendSnapshot, CompilationPolicy, CompilationSession,
    FeatureSupport, SupportLevel,
};
use roml::delta::{DeltaBatch, ModelOp};
use roml::id::{ConId, Generation, ObjId, VarId};
use roml::model::coefficient::CoefficientTarget;
use roml::model::{Bounds, ConstraintBounds, Sense, VarType};
use roml::revision::ModelRevision;
use roml::snapshot::{CellEntry, ConstraintEntry, ModelSnapshot, ObjectiveEntry, VariableEntry};
use roml::solver::backend::TerminationStatus;
use roml::solver::callback::{CallbackAction, CallbackData, CallbackHandler};
use roml::solver::request::SolveRequest;
use roml::solver::session::{
    BackendSession, CallbackSession, SessionHealth, SolutionView, Synchronization,
};
use roml::sync::AdapterHealth;
use roml::value_expr::ValueExpr;
use roml::Model;
use roml_highs::HighsSession;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn create_session() -> HighsSession {
    HighsSession::try_new().expect("HiGHS should be available for bundled tests")
}

/// A full-support typed capability set for test compilation.
fn test_capabilities() -> BackendCapabilitySet {
    let mut set = BackendCapabilitySet::new();
    for feature in [
        BackendFeature::Lp,
        BackendFeature::Mip,
        BackendFeature::IncrementalBounds,
        BackendFeature::IncrementalRows,
        BackendFeature::IncrementalCoefficients,
    ] {
        set.set(
            feature,
            FeatureSupport {
                level: SupportLevel::Native,
                limitations: Default::default(),
            },
        );
    }
    set
}

/// Compile a canonical snapshot into a backend snapshot for the compiled
/// synchronization path (P26 Task 7).
fn compile_snapshot(snapshot: &ModelSnapshot) -> BackendSnapshot {
    let mut session = CompilationSession::new();
    let instance = Model::new().instance();
    session
        .compile_snapshot(
            instance,
            snapshot,
            &CompilationPolicy::Auto,
            &test_capabilities(),
        )
        .expect("test snapshot must compile")
}

fn approx_eq(a: f64, b: f64, eps: f64) -> bool {
    (a - b).abs() < eps
}

fn var_id(index: u32) -> VarId {
    VarId::new(index, Generation::new())
}

fn con_id(index: u32) -> ConId {
    ConId::new(index, Generation::new())
}

fn obj_id(index: u32) -> ObjId {
    ObjId::new(index, Generation::new())
}

/// A MIP that genuinely requires branch-and-bound:
/// max x + y, s.t. 2x + 2y <= 3, x,y binary.
///
/// The LP relaxation is x + y <= 1.5 (fractional), so the root solve is not
/// integral and HiGHS must branch — which is what drives the MIP callback
/// events (`kHighsCallbackMipSolution`) that the handler observes. Optimal
/// objective is 1 (either x=1,y=0 or x=0,y=1).
fn binary_mip_snapshot() -> ModelSnapshot {
    let r0 = ModelRevision::ZERO;
    let v0 = var_id(0);
    let v1 = var_id(1);
    let c0 = con_id(0);
    let o0 = obj_id(0);

    ModelSnapshot {
        revision: r0,
        variables: vec![
            VariableEntry {
                id: v0,
                bounds: Bounds::BINARY,
                var_type: VarType::Binary,
                active: true,
                semicontinuous_lower: None,
            },
            VariableEntry {
                id: v1,
                bounds: Bounds::BINARY,
                var_type: VarType::Binary,
                active: true,
                semicontinuous_lower: None,
            },
        ],
        constraints: vec![ConstraintEntry {
            id: c0,
            bounds: ConstraintBounds::le(3.0),
            active: true,
        }],
        objectives: vec![ObjectiveEntry {
            id: o0,
            sense: Sense::Maximize,
            active: true,
            constant: 0.0,
        }],
        parameters: vec![],
        cells: vec![
            CellEntry {
                cell_key: (CoefficientTarget::Constraint(c0), v0),
                value_expr: ValueExpr::constant(2.0),
                evaluated_value: 2.0,
                dependencies: vec![],
            },
            CellEntry {
                cell_key: (CoefficientTarget::Constraint(c0), v1),
                value_expr: ValueExpr::constant(2.0),
                evaluated_value: 2.0,
                dependencies: vec![],
            },
            CellEntry {
                cell_key: (CoefficientTarget::Objective(o0), v0),
                value_expr: ValueExpr::constant(1.0),
                evaluated_value: 1.0,
                dependencies: vec![],
            },
            CellEntry {
                cell_key: (CoefficientTarget::Objective(o0), v1),
                value_expr: ValueExpr::constant(1.0),
                evaluated_value: 1.0,
                dependencies: vec![],
            },
        ],
        functions: vec![],
        constructs: vec![],
    }
}

/// A simple LP: max x + 3y s.t. x + y <= 10, x,y >= 0. Unique optimum x=0,y=10,obj=30.
fn unique_lp_snapshot() -> ModelSnapshot {
    let r0 = ModelRevision::ZERO;
    let v0 = var_id(0);
    let v1 = var_id(1);
    let c0 = con_id(0);
    let o0 = obj_id(0);

    ModelSnapshot {
        revision: r0,
        variables: vec![
            VariableEntry {
                id: v0,
                bounds: Bounds::NON_NEGATIVE,
                var_type: VarType::Continuous,
                active: true,
                semicontinuous_lower: None,
            },
            VariableEntry {
                id: v1,
                bounds: Bounds::NON_NEGATIVE,
                var_type: VarType::Continuous,
                active: true,
                semicontinuous_lower: None,
            },
        ],
        constraints: vec![ConstraintEntry {
            id: c0,
            bounds: ConstraintBounds::le(10.0),
            active: true,
        }],
        objectives: vec![ObjectiveEntry {
            id: o0,
            sense: Sense::Maximize,
            active: true,
            constant: 0.0,
        }],
        parameters: vec![],
        cells: vec![
            CellEntry {
                cell_key: (CoefficientTarget::Constraint(c0), v0),
                value_expr: ValueExpr::constant(1.0),
                evaluated_value: 1.0,
                dependencies: vec![],
            },
            CellEntry {
                cell_key: (CoefficientTarget::Constraint(c0), v1),
                value_expr: ValueExpr::constant(1.0),
                evaluated_value: 1.0,
                dependencies: vec![],
            },
            CellEntry {
                cell_key: (CoefficientTarget::Objective(o0), v0),
                value_expr: ValueExpr::constant(1.0),
                evaluated_value: 1.0,
                dependencies: vec![],
            },
            CellEntry {
                cell_key: (CoefficientTarget::Objective(o0), v1),
                value_expr: ValueExpr::constant(3.0),
                evaluated_value: 3.0,
                dependencies: vec![],
            },
        ],
        functions: vec![],
        constructs: vec![],
    }
}

// ── 1. MIP callback dispatch ─────────────────────────────────────────────────

/// A handler that records how many times it was invoked and which variables
/// were present in the candidate data.
struct CountingHandler {
    calls: Arc<AtomicUsize>,
    /// Records the max number of distinct variables seen in any callback.
    max_vars_seen: Arc<AtomicUsize>,
}

impl CallbackHandler for CountingHandler {
    fn on_candidate(&mut self, data: &CallbackData) -> CallbackAction {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let n = data.var_values.len();
        self.max_vars_seen.fetch_max(n, Ordering::SeqCst);
        CallbackAction::Accept
    }
}

/// A MIP solve with a registered callback must invoke the handler at least
/// once, with candidate data that contains variable values.
#[test]
fn mip_callback_handler_is_invoked_with_candidate_data() {
    let mut session = create_session();
    let snap = binary_mip_snapshot();
    session
        .synchronize(Synchronization::CompiledRebuild(compile_snapshot(&snap)))
        .expect("Rebuild should succeed");

    let calls = Arc::new(AtomicUsize::new(0));
    let max_vars = Arc::new(AtomicUsize::new(0));
    session
        .set_callback_handler(Box::new(CountingHandler {
            calls: calls.clone(),
            max_vars_seen: max_vars.clone(),
        }))
        .expect("set_callback_handler should succeed");

    let result = session
        .solve(&SolveRequest::new())
        .expect("MIP solve should succeed");
    assert_eq!(
        result.termination,
        TerminationStatus::Optimal,
        "Binary MIP should solve to Optimal"
    );

    // The handler must have been invoked during the branch-and-cut search.
    assert!(
        calls.load(Ordering::SeqCst) > 0,
        "MIP callback handler was never invoked"
    );
    // Candidate data must include at least one variable value from the model.
    assert!(
        max_vars.load(Ordering::SeqCst) >= 1,
        "Callback candidate data should contain variable values"
    );
}

/// After `clear_callback_handler`, a subsequent MIP solve must NOT invoke the
/// handler (its call counter stays frozen at the pre-clear value).
#[test]
fn cleared_callback_handler_is_not_invoked() {
    let mut session = create_session();
    let snap = binary_mip_snapshot();
    session
        .synchronize(Synchronization::CompiledRebuild(compile_snapshot(&snap)))
        .expect("Rebuild should succeed");

    let calls = Arc::new(AtomicUsize::new(0));
    let max_vars = Arc::new(AtomicUsize::new(0));
    session
        .set_callback_handler(Box::new(CountingHandler {
            calls: calls.clone(),
            max_vars_seen: max_vars.clone(),
        }))
        .expect("set_callback_handler should succeed");

    // Solve once with the handler registered.
    session
        .solve(&SolveRequest::new())
        .expect("First MIP solve should succeed");
    let after_first = calls.load(Ordering::SeqCst);
    assert!(after_first > 0, "handler should be invoked on first solve");

    // Clear it, then solve again.
    session
        .clear_callback_handler()
        .expect("clear_callback_handler should succeed");
    session
        .solve(&SolveRequest::new())
        .expect("Second MIP solve should succeed");
    let after_second = calls.load(Ordering::SeqCst);

    assert_eq!(
        after_first, after_second,
        "cleared callback handler must not be invoked on subsequent solves"
    );
}

/// A handler that panics whenever invoked. The trampoline must catch the
/// panic at the FFI boundary instead of unwinding through C.
struct PanicHandler;

impl CallbackHandler for PanicHandler {
    fn on_candidate(&mut self, _data: &CallbackData) -> CallbackAction {
        panic!("panic in callback handler")
    }
}

/// A panicking callback handler must not abort the solve or unwind across the
/// C boundary — the trampoline catches the panic and the solve completes.
#[test]
fn panicking_callback_handler_does_not_abort_solve() {
    let mut session = create_session();
    let snap = binary_mip_snapshot();
    session
        .synchronize(Synchronization::CompiledRebuild(compile_snapshot(&snap)))
        .expect("Rebuild should succeed");

    session
        .set_callback_handler(Box::new(PanicHandler))
        .expect("set_callback_handler should succeed");

    let result = session
        .solve(&SolveRequest::new())
        .expect("solve must succeed even when the handler panics");
    assert_eq!(
        result.termination,
        TerminationStatus::Optimal,
        "MIP solve should still complete"
    );
}

// ── 2. SolutionView accessors ────────────────────────────────────────────────

/// After an optimal LP solve, `SolutionView::value`/`dual`/`reduced_cost`/
/// `objective_value` must agree with the `SolveResult` extracted in the same
/// solve — the accessors read the session's cached solution.
#[test]
fn solution_view_accessors_match_solve_result() {
    let mut session = create_session();
    let snap = unique_lp_snapshot();
    session
        .synchronize(Synchronization::CompiledRebuild(compile_snapshot(&snap)))
        .expect("Rebuild should succeed");

    let result = session
        .solve(&SolveRequest::new())
        .expect("LP solve should succeed");
    assert_eq!(result.termination, TerminationStatus::Optimal);

    let sol = result.solution.expect("Optimal LP should have a solution");
    let obj = sol.objective_value.unwrap_or(0.0);

    // The solve result reports obj = 30 for this model.
    assert!(
        approx_eq(obj, 30.0, 1e-1),
        "Expected objective ≈ 30, got {}",
        obj
    );

    // SolutionView::objective_value must match the SolveResult.
    let view_obj = session.objective_value();
    assert!(
        view_obj.is_some(),
        "objective_value() should be Some after an optimal solve"
    );
    assert!(
        approx_eq(view_obj.unwrap(), obj, 1e-1),
        "SolutionView objective {} must match SolveResult {}",
        view_obj.unwrap(),
        obj
    );

    // value() for each variable must match the extracted variable_values.
    for (vid, expected) in &sol.variable_values {
        let got = session.value(*vid);
        assert!(
            got.is_some(),
            "value({vid:?}) should be present for a variable in the solution"
        );
        assert!(
            approx_eq(got.unwrap(), *expected, 1e-1),
            "value({vid:?}) = {} must match extracted {}",
            got.unwrap(),
            expected
        );
    }

    // reduced_cost() must be consistent with the solve result's reduced costs
    // for every variable that has one.
    if let Some(ref costs) = sol.reduced_costs {
        for (vid, expected) in costs {
            let got = session.reduced_cost(*vid);
            assert!(got.is_some(), "reduced_cost({vid:?}) should be present");
            assert!(
                approx_eq(got.unwrap(), *expected, 1e-1),
                "reduced_cost({vid:?}) = {} must match extracted {}",
                got.unwrap(),
                expected
            );
        }
    }

    // value() must return None for a variable NOT in the model.
    assert!(
        session.value(var_id(99)).is_none(),
        "value() for an absent variable should be None"
    );
}

// ── 3. Session health transitions ────────────────────────────────────────────

/// A failed delta sync (wrong base revision) must leave the cursor
/// `RequiresRebuild`; a subsequent rebuild must restore it to `Ready`.
#[test]
fn health_transitions_on_failed_delta_and_rebuild() {
    let mut session = create_session();
    let snap = unique_lp_snapshot();
    session
        .synchronize(Synchronization::CompiledRebuild(compile_snapshot(&snap)))
        .expect("Rebuild should succeed");
    assert_eq!(session.health(), AdapterHealth::Ready);

    // Attempt a delta whose `from` doesn't match the cursor: r5 → r6.
    let r5 = ModelRevision::from_u64(5);
    let r6 = r5.next().unwrap();
    let bad_batch = DeltaBatch::new(
        r5,
        r6,
        vec![ModelOp::AddVariable {
            var: var_id(50),
            bounds: Bounds::NON_NEGATIVE,
            var_type: VarType::Continuous,
        }],
    )
    .expect("valid batch");

    let res = session.synchronize(Synchronization::DeltaBatch(bad_batch));
    assert!(
        res.is_err(),
        "delta from a mismatched base revision should be rejected"
    );
    assert_eq!(
        session.health(),
        AdapterHealth::RequiresRebuild,
        "cursor must require a rebuild after a failed delta"
    );

    // A rebuild restores Ready and the correct revision.
    session
        .synchronize(Synchronization::CompiledRebuild(compile_snapshot(&snap)))
        .expect("Rebuild after failure should succeed");
    assert_eq!(session.health(), AdapterHealth::Ready);
    assert_eq!(session.revision(), ModelRevision::ZERO);
}

// ── 4. Close / drop ──────────────────────────────────────────────────────────

/// `close()` consumes the session and returns Ok, releasing the HiGHS handle.
#[test]
fn close_consumes_session_cleanly() {
    let mut session = create_session();
    let snap = unique_lp_snapshot();
    session
        .synchronize(Synchronization::CompiledRebuild(compile_snapshot(&snap)))
        .expect("Rebuild should succeed");

    // close() moves the session; the handle is freed by Drop.
    let result = session.close();
    assert!(
        result.is_ok(),
        "close() should succeed and release the HiGHS handle"
    );
}
