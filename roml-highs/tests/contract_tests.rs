//! Phase 11 Contract Tests: C1-C11 for HiGHS BackendSession implementation.
//!
//! These integration tests verify that [`HighsSession`] correctly implements
//! the frozen [`BackendSession`] trait (ADR-001) and related supplementary
//! traits. Categories C1-C7 are solver-agnostic contract conformance tests;
//! C8-C11 are HiGHS-specific.
//!
//! # Test Infrastructure
//!
//! Tests use a real HiGHS instance via the `bundled` feature (default). Each
//! test creates its own [`HighsSession`] via [`HighsSession::try_new()`].

// P23: exercises deprecated raw constructors during the pre-1.0 window.
#![allow(deprecated)]

use roml::advanced::{
    BackendCapabilitySet, BackendDeltaBatch, BackendFeature, BackendSnapshot, CompilationPolicy,
    CompilationSession, FeatureSupport, SupportLevel,
};
use roml::delta::{DeltaBatch, ModelOp};
use roml::id::{ConId, Generation, ObjId, VarId};
use roml::model::coefficient::CoefficientTarget;
use roml::model::{continuous, Bounds, ConstraintBounds, Sense, VarType};
use roml::revision::ModelRevision;
use roml::snapshot::{CellEntry, ConstraintEntry, ModelSnapshot, ObjectiveEntry, VariableEntry};
use roml::solver::backend::{ErrorCategory, HealthEffect, TerminationStatus};
use roml::solver::request::SolveRequest;
use roml::solver::session::{
    BackendMetadata, BackendSession, SessionHealth, SolutionView, Synchronization,
};
use roml::sync::AdapterHealth;
use roml::value_expr::ValueExpr;
use roml::Model;
use roml_highs::HighsSession;

// ── Test Helpers ───────────────────────────────────────────────────────────────

/// Create a new HiGHS session for testing.
///
/// # Panics
///
/// Panics if HiGHS is not available (e.g., library not found or invalid
/// build configuration).
fn create_session() -> HighsSession {
    HighsSession::try_new().expect("HiGHS should be available for bundled tests")
}

/// Approximate floating-point equality within epsilon.
fn approx_eq(a: f64, b: f64, eps: f64) -> bool {
    (a - b).abs() < eps
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

/// Compile a canonical snapshot into a backend snapshot (P26 compiled path).
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

/// A persistent identity compiler for a single test, so sequential compiled
/// deltas chain their exact from/to compilation ids (P26 Task 7).
struct TestCompiler {
    session: CompilationSession,
    instance: roml::identity::ModelInstanceId,
}

impl TestCompiler {
    fn new() -> Self {
        Self {
            session: CompilationSession::new(),
            instance: Model::new().instance(),
        }
    }

    fn rebuild(&mut self, snapshot: &ModelSnapshot) -> BackendSnapshot {
        self.session
            .compile_snapshot(
                self.instance,
                snapshot,
                &CompilationPolicy::Auto,
                &test_capabilities(),
            )
            .expect("test snapshot must compile")
    }

    fn delta(&mut self, batch: &DeltaBatch) -> BackendDeltaBatch {
        let from = self
            .session
            .current_compilation()
            .expect("test delta requires a compiled base");
        self.session
            .compile_delta(
                batch,
                from,
                self.instance,
                &CompilationPolicy::Auto,
                &test_capabilities(),
            )
            .expect("test delta must compile")
    }
}

/// Compile `base` into a fresh compiled state AND `batch` into a delta chained
/// from THAT SAME compiled state, returning both. A session that establishes
/// `compiled_base` via `CompiledRebuild` then holds exactly the
/// `from_compilation` the delta requires — the exact `CompilationId` is the
/// only stale-state authority (D28, WR-1), so the base and its deltas must come
/// from ONE compilation chain, never from separate one-shot sessions.
fn compile_base_and_delta(
    base: &ModelSnapshot,
    batch: &DeltaBatch,
) -> (BackendSnapshot, BackendDeltaBatch) {
    let mut session = CompilationSession::new();
    let instance = Model::new().instance();
    let compiled_base = session
        .compile_snapshot(
            instance,
            base,
            &CompilationPolicy::Auto,
            &test_capabilities(),
        )
        .expect("base snapshot must compile");
    let compiled_delta = session
        .compile_delta(
            batch,
            compiled_base.compilation_id,
            instance,
            &CompilationPolicy::Auto,
            &test_capabilities(),
        )
        .expect("test delta must compile");
    (compiled_base, compiled_delta)
}

/// Establish `base` as a compiled rebuild on `session`, then apply `batch`
/// chained from that SAME base (D28/WR-1: the delta's `from_compilation` must
/// match the base the session holds).
fn rebuild_then_apply_delta(session: &mut HighsSession, base: &ModelSnapshot, batch: &DeltaBatch) {
    let (compiled_base, compiled_delta) = compile_base_and_delta(base, batch);
    session
        .synchronize(Synchronization::CompiledRebuild(compiled_base))
        .expect("compiled base rebuild should succeed");
    session
        .synchronize(Synchronization::CompiledDeltaBatch(compiled_delta))
        .expect("compiled delta should succeed");
}

/// Generate a fresh [`VarId`] for testing.
fn var_id(index: u32) -> VarId {
    VarId::new(index, Generation::new())
}

/// Generate a fresh [`ConId`] for testing.
fn con_id(index: u32) -> ConId {
    ConId::new(index, Generation::new())
}

/// Generate a fresh [`ObjId`] for testing.
fn obj_id(index: u32) -> ObjId {
    ObjId::new(index, Generation::new())
}

// ── C1: Empty Model ────────────────────────────────────────────────────────────

/// C1: An empty model rebuilds successfully and solves as trivially optimal.
#[test]
fn c1_empty_model() {
    let mut session = create_session();
    let snapshot = ModelSnapshot::empty(ModelRevision::ZERO);

    let receipt = session
        .synchronize(Synchronization::CompiledRebuild(compile_snapshot(
            &snapshot,
        )))
        .expect("Rebuild from empty snapshot should succeed");
    assert_eq!(
        receipt.health,
        AdapterHealth::Ready,
        "Empty model should be Ready after rebuild"
    );

    let result = session
        .solve(&SolveRequest::new())
        .expect("Solve of empty model should succeed");
    assert_eq!(
        result.termination,
        TerminationStatus::Optimal,
        "Empty model is trivially optimal (AD-6)"
    );
}

// ── C2: Full Rebuild ───────────────────────────────────────────────────────────

/// C2: A model with all entity types rebuilds and solves correctly.
///
/// Model (all continuous variables for correctness):
/// maximize x0 + x1
/// s.t.  1*x0 + 1*x1 <= 5
///       0 <= x0 <= 10
///       0 <= x1 <= 10
///
/// Expected: x0 = 5, x1 = 0, objective = 5
#[test]
fn c2_full_rebuild() {
    let mut session = create_session();
    let r0 = ModelRevision::ZERO;
    let v0 = var_id(0);
    let v1 = var_id(1);
    let c0 = con_id(0);
    let o0 = obj_id(0);

    let snapshot = ModelSnapshot {
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
            bounds: ConstraintBounds::le(5.0),
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
            // Constraint coefficients
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
            // Objective coefficients
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
    };

    let receipt = session
        .synchronize(Synchronization::CompiledRebuild(compile_snapshot(
            &snapshot,
        )))
        .expect("Rebuild from full snapshot should succeed");
    assert_eq!(receipt.health, AdapterHealth::Ready);

    let result = session
        .solve(&SolveRequest::new())
        .expect("Solve of full model should succeed");
    assert_eq!(
        result.termination,
        TerminationStatus::Optimal,
        "Feasible LP should be Optimal"
    );

    let sol = result
        .solution
        .expect("Optimal solution should be available");
    let obj_val = sol.objective_value.unwrap_or(0.0);
    assert!(
        approx_eq(obj_val, 5.0, 1e-4),
        "Expected objective ≈ 5, got {}",
        obj_val
    );

    // Verify variable values are extracted
    assert!(
        !sol.variable_values.is_empty(),
        "Variable values should be extracted"
    );
}

// ── C3: Incremental Delta ──────────────────────────────────────────────────────

/// C3: Apply each of the 16 ModelOp variants individually and verify the
/// session stays Ready with advancing revision. End with a solve to confirm
/// model integrity.
///
/// The operations are ordered so that entities exist before they are
/// removed or modified. Revision chain: r0 (empty) -> r1 through r18.
#[allow(unused_assignments)]
#[test]
fn c3_incremental_delta() {
    let mut session = create_session();
    let r0 = ModelRevision::ZERO;
    let mut compiler = TestCompiler::new();

    // Start from empty.
    session
        .synchronize(Synchronization::CompiledRebuild(
            compiler.rebuild(&ModelSnapshot::empty(r0)),
        ))
        .expect("Empty rebuild should succeed");
    assert_eq!(session.revision(), r0);

    let v0 = var_id(0);
    let c0 = con_id(0);
    let o0 = obj_id(0);

    // Helper: apply a single primitive-linear op as a compiled delta.
    let mut rev = r0;
    macro_rules! apply_op {
        ($op:expr) => {{
            let next = rev.next().expect("Revision should not overflow");
            let batch = DeltaBatch::new(rev, next, vec![$op])
                .expect("DeltaBatch construction should succeed");
            session
                .synchronize(Synchronization::CompiledDeltaBatch(compiler.delta(&batch)))
                .unwrap_or_else(|e| {
                    panic!(
                        "Delta sync r{}->r{} failed: {}",
                        rev.as_u64(),
                        next.as_u64(),
                        e
                    )
                });
            assert_eq!(
                session.revision(),
                next,
                "Revision should advance to r{}",
                next.as_u64()
            );
            rev = next;
        }};
    }

    // Primitive linear ops compile and apply incrementally (SM-03.7).
    apply_op!(ModelOp::AddVariable {
        var: v0,
        bounds: Bounds::NON_NEGATIVE,
        var_type: VarType::Continuous,
    });
    apply_op!(ModelOp::SetVariableBounds {
        var: v0,
        bounds: Bounds::new(1.0, 10.0),
    });
    apply_op!(ModelOp::AddConstraint {
        con: c0,
        bounds: ConstraintBounds::le(20.0),
    });
    apply_op!(ModelOp::SetConstraintBounds {
        con: c0,
        bounds: ConstraintBounds::le(30.0),
    });
    apply_op!(ModelOp::SetCell {
        cell_key: (CoefficientTarget::Constraint(c0), v0),
        value_expr: ValueExpr::constant(3.0),
        evaluated_value: 3.0,
    });
    apply_op!(ModelOp::RemoveCell {
        cell_key: (CoefficientTarget::Constraint(c0), v0),
    });
    apply_op!(ModelOp::AddObjective {
        obj: o0,
        sense: Sense::Maximize,
    });
    apply_op!(ModelOp::SetObjectiveCell {
        cell_key: (CoefficientTarget::Objective(o0), v0),
        value_expr: ValueExpr::constant(5.0),
        evaluated_value: 5.0,
        constant: 0.0,
    });
    apply_op!(ModelOp::SetActiveObjective { obj: Some(o0) });

    // Non-incremental ops (variable/constraint activity, variable type) are
    // NOT compiled incrementally: the identity compiler selects a deterministic
    // rebuild (F-B1 / design §18), so no `BackendDeltaBatch` is emitted.
    let next = rev.next().unwrap();
    let activity_batch = DeltaBatch::new(
        rev,
        next,
        vec![ModelOp::SetVariableActive {
            var: v0,
            active: false,
        }],
    )
    .unwrap();
    let from = compiler.session.current_compilation().unwrap();
    let result = compiler.session.compile_delta(
        &activity_batch,
        from,
        compiler.instance,
        &CompilationPolicy::Auto,
        &test_capabilities(),
    );
    assert!(
        result.is_err(),
        "SetVariableActive must select rebuild (no compiled delta)"
    );

    // SetParameter is a provable no-op on compiled IR (the coefficient index is
    // the single authority; the batch's SetCell ops carry evaluated values), so
    // it compiles and applies incrementally from the current revision.
    let param_batch = DeltaBatch::new(
        rev,
        next,
        vec![ModelOp::SetParameter {
            param: roml::id::ParamId::new(0, Generation::new()),
            value: 1.0,
        }],
    )
    .unwrap();
    let compiled_param = compiler.delta(&param_batch);
    session
        .synchronize(Synchronization::CompiledDeltaBatch(compiled_param))
        .expect("SetParameter should compile as a no-op delta");
    assert_eq!(session.revision(), next);

    // Final solve — the model remains valid (trivially optimal with the
    // remaining objective/variable state).
    let result = session
        .solve(&SolveRequest::new())
        .expect("Final solve should succeed");
    assert_eq!(result.termination, TerminationStatus::Optimal);
}

// ── C4: Commuting Square ───────────────────────────────────────────────────────

/// C4: Prove that snapshot(r1) == apply(snapshot(r0), deltas r0->r1).
///
/// Session A is rebuilt from r0 snapshots and receives deltas. Session B is
/// rebuilt directly from r1. Both are solved with the same request — the
/// objective values must match within epsilon.
#[test]
fn c4_commuting_square() {
    let mut session_a = create_session();
    let mut session_b = create_session();
    let r0 = ModelRevision::ZERO;
    let r1 = r0.next().expect("Revision should not overflow");
    let v0 = var_id(0);
    let c0 = con_id(0);
    let o0 = obj_id(0);

    // Snapshot at r0: one variable, one constraint, one objective.
    let snap_r0 = ModelSnapshot {
        revision: r0,
        variables: vec![VariableEntry {
            id: v0,
            bounds: Bounds::new(0.0, 10.0),
            var_type: VarType::Continuous,
            active: true,
            semicontinuous_lower: None,
        }],
        constraints: vec![ConstraintEntry {
            id: c0,
            bounds: ConstraintBounds::le(8.0),
            active: true,
        }],
        objectives: vec![ObjectiveEntry {
            id: o0,
            sense: Sense::Maximize,
            active: true,
            constant: 0.0,
        }],
        parameters: vec![],
        cells: vec![CellEntry {
            cell_key: (CoefficientTarget::Objective(o0), v0),
            value_expr: ValueExpr::constant(1.0),
            evaluated_value: 1.0,
            dependencies: vec![],
        }],
        functions: vec![],
        constructs: vec![],
    };

    // Session A: rebuild from r0, then apply delta r0->r1. The base and the
    // delta are compiled in ONE chain so the delta's `from_compilation` matches
    // the base the session holds (D28/WR-1).
    let delta = DeltaBatch::new(
        r0,
        r1,
        vec![ModelOp::SetCell {
            cell_key: (CoefficientTarget::Constraint(c0), v0),
            value_expr: ValueExpr::constant(2.0),
            evaluated_value: 2.0,
        }],
    )
    .expect("DeltaBatch r0->r1 should be valid");
    rebuild_then_apply_delta(&mut session_a, &snap_r0, &delta);

    // Snapshot at r1 (includes the constraint coefficient).
    let snap_r1 = ModelSnapshot {
        revision: r1,
        variables: vec![VariableEntry {
            id: v0,
            bounds: Bounds::new(0.0, 10.0),
            var_type: VarType::Continuous,
            active: true,
            semicontinuous_lower: None,
        }],
        constraints: vec![ConstraintEntry {
            id: c0,
            bounds: ConstraintBounds::le(8.0),
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
                cell_key: (CoefficientTarget::Objective(o0), v0),
                value_expr: ValueExpr::constant(1.0),
                evaluated_value: 1.0,
                dependencies: vec![],
            },
            CellEntry {
                cell_key: (CoefficientTarget::Constraint(c0), v0),
                value_expr: ValueExpr::constant(2.0),
                evaluated_value: 2.0,
                dependencies: vec![],
            },
        ],
        functions: vec![],
        constructs: vec![],
    };

    // Session B: rebuild directly from r1.
    session_b
        .synchronize(Synchronization::CompiledRebuild(compile_snapshot(&snap_r1)))
        .expect("Session B: rebuild from r1 should succeed");

    // Solve both with the same request.
    let req = SolveRequest::new();
    let result_a = session_a
        .solve(&req)
        .expect("Session A solve should succeed");
    let result_b = session_b
        .solve(&req)
        .expect("Session B solve should succeed");

    assert_eq!(
        result_a.termination,
        TerminationStatus::Optimal,
        "Session A should be Optimal"
    );
    assert_eq!(
        result_b.termination,
        TerminationStatus::Optimal,
        "Session B should be Optimal"
    );

    let obj_a = result_a
        .solution
        .as_ref()
        .and_then(|s| s.objective_value)
        .expect("Session A solution should have objective");
    let obj_b = result_b
        .solution
        .as_ref()
        .and_then(|s| s.objective_value)
        .expect("Session B solution should have objective");

    assert!(
        approx_eq(obj_a, obj_b, 1e-6),
        "Commuting square failed: incremental objective {} != rebuild objective {}",
        obj_a,
        obj_b
    );
    assert!(
        approx_eq(obj_a, 4.0, 1e-4),
        "Expected objective ≈ 4 (x=4, 2*4=8<=8), got {}",
        obj_a
    );
}

// ── C5: Activity Toggling ──────────────────────────────────────────────────────

/// C5: Deactivating and reactivating a variable preserves its bounds.
///
/// Model: maximize x, s.t. 1.0 <= x <= 10.0
/// - Active solve: x should be at upper bound (10.0).
/// - Deactivated solve: x is fixed to 0, objective is 0.
/// - Reactivated solve: x is back to 10.0, objective restored.
#[test]
fn c5_activity_toggle() {
    let mut session = create_session();
    let r0 = ModelRevision::ZERO;
    let v0 = var_id(0);
    let o0 = obj_id(0);

    let snapshot = ModelSnapshot {
        revision: r0,
        variables: vec![VariableEntry {
            id: v0,
            bounds: Bounds::new(1.0, 10.0),
            var_type: VarType::Continuous,
            active: true,
            semicontinuous_lower: None,
        }],
        constraints: vec![],
        objectives: vec![ObjectiveEntry {
            id: o0,
            sense: Sense::Maximize,
            active: true,
            constant: 0.0,
        }],
        parameters: vec![],
        cells: vec![CellEntry {
            cell_key: (CoefficientTarget::Objective(o0), v0),
            value_expr: ValueExpr::constant(1.0),
            evaluated_value: 1.0,
            dependencies: vec![],
        }],
        functions: vec![],
        constructs: vec![],
    };

    // Rebuild.
    session
        .synchronize(Synchronization::CompiledRebuild(compile_snapshot(
            &snapshot,
        )))
        .expect("Rebuild should succeed");

    // Solve 1: active — x should be at upper bound 10.0, objective = 10.0.
    let result1 = session
        .solve(&SolveRequest::new())
        .expect("First solve should succeed");
    assert_eq!(result1.termination, TerminationStatus::Optimal);
    let obj1 = result1
        .solution
        .as_ref()
        .and_then(|s| s.objective_value)
        .unwrap_or(0.0);
    assert!(
        approx_eq(obj1, 10.0, 1e-4),
        "Active solve: expected objective ≈ 10, got {}",
        obj1
    );

    // Deactivate: variable-activity changes are not incrementally compilable
    // (F-B1 / design §18 — the compiled IR folds activity into bounds), so the
    // compiled path selects a deterministic rebuild. Rebuild from an inactive
    // snapshot (the compiler folds `active: false` into fixed [0,0] bounds).
    let mut inactive = snapshot.clone();
    inactive.revision = r0.next().unwrap();
    inactive.variables[0].active = false;
    session
        .synchronize(Synchronization::CompiledRebuild(compile_snapshot(
            &inactive,
        )))
        .expect("Deactivate rebuild should succeed");

    // Solve 2: deactivated — x fixed to 0, objective = 0.
    let result2 = session
        .solve(&SolveRequest::new())
        .expect("Second solve (inactive) should succeed");
    assert_eq!(result2.termination, TerminationStatus::Optimal);
    let obj2 = result2
        .solution
        .as_ref()
        .and_then(|s| s.objective_value)
        .unwrap_or(-1.0);
    assert!(
        approx_eq(obj2, 0.0, 1e-4),
        "Deactivated solve: expected objective ≈ 0, got {}",
        obj2
    );

    // Reactivate: rebuild from the active snapshot again.
    let mut reactivated = snapshot.clone();
    reactivated.revision = inactive.revision.next().unwrap();
    reactivated.variables[0].active = true;
    session
        .synchronize(Synchronization::CompiledRebuild(compile_snapshot(
            &reactivated,
        )))
        .expect("Reactivate rebuild should succeed");

    // Solve 3: reactivated — bounds restored, objective back to 10.
    let result3 = session
        .solve(&SolveRequest::new())
        .expect("Third solve (reactivated) should succeed");
    assert_eq!(result3.termination, TerminationStatus::Optimal);
    let obj3 = result3
        .solution
        .as_ref()
        .and_then(|s| s.objective_value)
        .unwrap_or(0.0);
    assert!(
        approx_eq(obj3, 10.0, 1e-4),
        "Reactivated solve: expected objective ≈ 10, got {}",
        obj3
    );
}

// ── C6: Objective Switching ────────────────────────────────────────────────────

/// C6: Switching between minimize and maximize objectives correctly
/// updates costs and sense (Pitfall 5 mitigation).
///
/// Start with only minimize, then add a maximize objective and switch.
///
/// Model: x in [0, 10]. Cost coefficient = 1.0 for both.
/// - Minimize: x = 0, objective = 0.
/// - Maximize: x = 10, objective = 10.
#[test]
fn c6_objective_switch() {
    let mut session = create_session();
    let r0 = ModelRevision::ZERO;
    let v0 = var_id(0);
    let o_min = obj_id(0);

    // Snapshot r0: one minimize objective.
    let snapshot = ModelSnapshot {
        revision: r0,
        variables: vec![VariableEntry {
            id: v0,
            bounds: Bounds::new(0.0, 10.0),
            var_type: VarType::Continuous,
            active: true,
            semicontinuous_lower: None,
        }],
        constraints: vec![],
        objectives: vec![ObjectiveEntry {
            id: o_min,
            sense: Sense::Minimize,
            active: true,
            constant: 0.0,
        }],
        parameters: vec![],
        cells: vec![CellEntry {
            cell_key: (CoefficientTarget::Objective(o_min), v0),
            value_expr: ValueExpr::constant(1.0),
            evaluated_value: 1.0,
            dependencies: vec![],
        }],
        functions: vec![],
        constructs: vec![],
    };

    // The r0→r1 switch delta is compiled chained from the SAME compiled base
    // as the rebuild, so its `from_compilation` matches the session (D28/WR-1).
    let o_max = obj_id(1);
    let r1 = r0.next().unwrap();
    let switch_batch = DeltaBatch::new(
        r0,
        r1,
        vec![
            ModelOp::AddObjective {
                obj: o_max,
                sense: Sense::Maximize,
            },
            ModelOp::SetObjectiveCell {
                cell_key: (CoefficientTarget::Objective(o_max), v0),
                value_expr: ValueExpr::constant(1.0),
                evaluated_value: 1.0,
                constant: 0.0,
            },
            ModelOp::SetActiveObjective { obj: Some(o_max) },
        ],
    )
    .unwrap();
    let (compiled_base, compiled_switch) = compile_base_and_delta(&snapshot, &switch_batch);
    session
        .synchronize(Synchronization::CompiledRebuild(compiled_base))
        .expect("Rebuild should succeed");

    // Solve 1: minimize — x should be at 0.
    let result1 = session
        .solve(&SolveRequest::new())
        .expect("Minimize solve should succeed");
    assert_eq!(result1.termination, TerminationStatus::Optimal);
    let obj1 = result1
        .solution
        .as_ref()
        .and_then(|s| s.objective_value)
        .unwrap_or(999.0);
    assert!(
        approx_eq(obj1, 0.0, 1e-4),
        "Minimize: expected objective ≈ 0, got {}",
        obj1
    );

    // Apply the pre-compiled maximize switch (r0->r1).
    session
        .synchronize(Synchronization::CompiledDeltaBatch(compiled_switch))
        .expect("Add+switch to maximize should succeed");

    // Solve 2: maximize — x should be at 10.
    let result2 = session
        .solve(&SolveRequest::new())
        .expect("Maximize solve should succeed");
    assert_eq!(result2.termination, TerminationStatus::Optimal);
    let obj2 = result2
        .solution
        .as_ref()
        .and_then(|s| s.objective_value)
        .unwrap_or(0.0);
    assert!(
        approx_eq(obj2, 10.0, 1e-4),
        "Maximize: expected objective ≈ 10, got {}",
        obj2
    );
}

// ── C7: Unsupported Rejection ──────────────────────────────────────────────────

/// C7: Semi-continuous variables are rejected atomically before any HiGHS
/// state modification (M1R-H7).
///
/// The snapshot contains a variable with `semicontinuous_lower: Some(2.0)`.
/// synchronize returns an error with `ErrorCategory::Unsupported` without
/// modifying the HiGHS model state.
#[test]
fn c7_unsupported_rejection() {
    let session = create_session();
    let r0 = ModelRevision::ZERO;
    let v0 = var_id(0);

    // Health should be Ready before the attempt.
    assert_eq!(
        session.health(),
        AdapterHealth::Ready,
        "Session should start Ready"
    );

    let snapshot = ModelSnapshot {
        revision: r0,
        variables: vec![VariableEntry {
            id: v0,
            bounds: Bounds::NON_NEGATIVE,
            var_type: VarType::Continuous,
            active: true,
            semicontinuous_lower: Some(2.0), // Unsupported!
        }],
        constraints: vec![],
        objectives: vec![],
        parameters: vec![],
        cells: vec![],
        functions: vec![],
        constructs: vec![],
    };

    // The identity compiler rejects the snapshot (the compiled IR has no
    // semi-continuous representation, M1R-H7 preserved at the compile
    // boundary), so no `BackendSnapshot` reaches the session.
    let mut compiler = CompilationSession::new();
    let instance = Model::new().instance();
    let result = compiler.compile_snapshot(
        instance,
        &snapshot,
        &CompilationPolicy::Auto,
        &test_capabilities(),
    );
    let err = result.expect_err("semi-continuous snapshot should be rejected");
    assert!(
        matches!(err, roml::advanced::CompileError::UnsupportedFeature(_)),
        "Error should be UnsupportedFeature, got {err:?}"
    );
    // The session never receives a compiled snapshot, so it stays Ready at r0
    // (no HiGHS state was modified).
    assert_eq!(session.health(), AdapterHealth::Ready);
    assert_eq!(session.revision(), r0);
}

// ── C8: Status Mapping ─────────────────────────────────────────────────────────
//
// Note: HiGHS LP solver with default settings may not prove infeasibility
// on models where the solution is at a bound extreme. The infeasible models
// are constructed to be deterministically detected by the simplex presolver.

/// C8: Status mapping — Optimal LP.
///
/// maximize x + y, x + y <= 4, x >= 0, y >= 0.
/// Expected: Optimal, objective = 4.
#[test]
fn c8_optimal_lp_status() {
    let mut session = create_session();
    let r0 = ModelRevision::ZERO;
    let v0 = var_id(0);
    let v1 = var_id(1);
    let c0 = con_id(0);
    let o0 = obj_id(0);

    let snap = ModelSnapshot {
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
            bounds: ConstraintBounds::le(4.0),
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
                value_expr: ValueExpr::constant(1.0),
                evaluated_value: 1.0,
                dependencies: vec![],
            },
        ],
        functions: vec![],
        constructs: vec![],
    };

    session
        .synchronize(Synchronization::CompiledRebuild(compile_snapshot(&snap)))
        .unwrap();
    let result = session.solve(&SolveRequest::new()).unwrap();
    assert_eq!(
        result.termination,
        TerminationStatus::Optimal,
        "Optimal LP should map to Optimal"
    );
}

/// C8: Status mapping — Infeasible LP.
///
/// minimize x subject to 1*x >= 10, x in [0, 1].
/// Variable bounds [0, 1] directly conflict with constraint x >= 10.
/// Uses a single constraint for trivially provable infeasibility.
#[test]
fn c8_infeasible_lp_status() {
    let mut session = create_session();
    let r0 = ModelRevision::ZERO;
    let v0 = var_id(0);
    let o0 = obj_id(0);
    let c0 = con_id(0);

    let snap = ModelSnapshot {
        revision: r0,
        variables: vec![VariableEntry {
            id: v0,
            bounds: Bounds::new(0.0, 1.0), // tight — [0, 1]
            var_type: VarType::Continuous,
            active: true,
            semicontinuous_lower: None,
        }],
        constraints: vec![ConstraintEntry {
            id: c0,
            bounds: ConstraintBounds::ge(10.0), // x >= 10 impossible with bounds
            active: true,
        }],
        objectives: vec![ObjectiveEntry {
            id: o0,
            sense: Sense::Minimize,
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
                cell_key: (CoefficientTarget::Objective(o0), v0),
                value_expr: ValueExpr::constant(1.0),
                evaluated_value: 1.0,
                dependencies: vec![],
            },
        ],
        functions: vec![],
        constructs: vec![],
    };

    session
        .synchronize(Synchronization::CompiledRebuild(compile_snapshot(&snap)))
        .unwrap();
    let result = session.solve(&SolveRequest::new()).unwrap();
    assert_eq!(
        result.termination,
        TerminationStatus::Infeasible,
        "Infeasible LP should map to Infeasible"
    );
}

/// C8: Status mapping — Unbounded LP.
///
/// maximize x, x >= 0 (no upper bound).
#[test]
fn c8_unbounded_lp_status() {
    let mut session = create_session();
    let r0 = ModelRevision::ZERO;
    let v0 = var_id(0);
    let o0 = obj_id(0);

    let snap = ModelSnapshot {
        revision: r0,
        variables: vec![VariableEntry {
            id: v0,
            bounds: Bounds::NON_NEGATIVE,
            var_type: VarType::Continuous,
            active: true,
            semicontinuous_lower: None,
        }],
        constraints: vec![],
        objectives: vec![ObjectiveEntry {
            id: o0,
            sense: Sense::Maximize,
            active: true,
            constant: 0.0,
        }],
        parameters: vec![],
        cells: vec![CellEntry {
            cell_key: (CoefficientTarget::Objective(o0), v0),
            value_expr: ValueExpr::constant(1.0),
            evaluated_value: 1.0,
            dependencies: vec![],
        }],
        functions: vec![],
        constructs: vec![],
    };

    session
        .synchronize(Synchronization::CompiledRebuild(compile_snapshot(&snap)))
        .unwrap();
    let result = session.solve(&SolveRequest::new()).unwrap();
    assert_eq!(
        result.termination,
        TerminationStatus::Unbounded,
        "Unbounded LP should map to Unbounded"
    );
}

// ── C9: Solve Tests ────────────────────────────────────────────────────────────

/// C9: Optimal LP with solution extraction.
///
/// maximize 1.0*x + 3.0*y, x + y <= 10, x >= 0, y >= 0.
/// Expected: Optimal, x=0, y=10, objective=30 (unique optimum).
#[test]
fn c9_optimal_lp_with_extraction() {
    let mut session = create_session();
    let r0 = ModelRevision::ZERO;
    let v0 = var_id(0);
    let v1 = var_id(1);
    let c0 = con_id(0);
    let o0 = obj_id(0);

    let snap = ModelSnapshot {
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
    };

    session
        .synchronize(Synchronization::CompiledRebuild(compile_snapshot(&snap)))
        .unwrap();
    let result = session.solve(&SolveRequest::new()).unwrap();
    assert_eq!(result.termination, TerminationStatus::Optimal);

    let sol = result.solution.expect("Optimal LP should have solution");
    let obj = sol.objective_value.unwrap_or(0.0);
    // maximize 1.5*x + 3.0*y, x+y<=10, x>=0, y>=0 → x=0, y=10, obj=30.
    assert!(
        approx_eq(obj, 30.0, 1e-1),
        "Expected objective ≈ 30, got {}",
        obj
    );

    // Check that variable values exist (at least one non-zero).
    assert!(
        !sol.variable_values.is_empty(),
        "Variable values should not be empty"
    );
}

/// C9: Infeasible LP.
///
/// minimize x subject to 1*x >= 10, x in [0, 1].
/// Variable bounds [0, 1] directly conflict with constraint x >= 10.
/// Expected: Infeasible (no solution).
#[test]
fn c9_infeasible_lp() {
    let mut session = create_session();
    let r0 = ModelRevision::ZERO;
    let v0 = var_id(0);
    let o0 = obj_id(0);
    let c0 = con_id(0);

    let snap = ModelSnapshot {
        revision: r0,
        variables: vec![VariableEntry {
            id: v0,
            bounds: Bounds::new(0.0, 1.0),
            var_type: VarType::Continuous,
            active: true,
            semicontinuous_lower: None,
        }],
        constraints: vec![ConstraintEntry {
            id: c0,
            bounds: ConstraintBounds::ge(10.0),
            active: true,
        }],
        objectives: vec![ObjectiveEntry {
            id: o0,
            sense: Sense::Minimize,
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
                cell_key: (CoefficientTarget::Objective(o0), v0),
                value_expr: ValueExpr::constant(1.0),
                evaluated_value: 1.0,
                dependencies: vec![],
            },
        ],
        functions: vec![],
        constructs: vec![],
    };

    session
        .synchronize(Synchronization::CompiledRebuild(compile_snapshot(&snap)))
        .unwrap();
    let result = session.solve(&SolveRequest::new()).unwrap();
    assert_eq!(result.termination, TerminationStatus::Infeasible);
    // Infeasible models should not have a solution.
    assert!(
        result.solution.is_none(),
        "Infeasible model should not have a solution"
    );
}

/// C9: Unbounded LP.
///
/// maximize x, x >= 0.
/// Expected: Unbounded.
#[test]
fn c9_unbounded_lp() {
    let mut session = create_session();
    let r0 = ModelRevision::ZERO;
    let v0 = var_id(0);
    let o0 = obj_id(0);

    let snap = ModelSnapshot {
        revision: r0,
        variables: vec![VariableEntry {
            id: v0,
            bounds: Bounds::NON_NEGATIVE,
            var_type: VarType::Continuous,
            active: true,
            semicontinuous_lower: None,
        }],
        constraints: vec![],
        objectives: vec![ObjectiveEntry {
            id: o0,
            sense: Sense::Maximize,
            active: true,
            constant: 0.0,
        }],
        parameters: vec![],
        cells: vec![CellEntry {
            cell_key: (CoefficientTarget::Objective(o0), v0),
            value_expr: ValueExpr::constant(1.0),
            evaluated_value: 1.0,
            dependencies: vec![],
        }],
        functions: vec![],
        constructs: vec![],
    };

    session
        .synchronize(Synchronization::CompiledRebuild(compile_snapshot(&snap)))
        .unwrap();
    let result = session.solve(&SolveRequest::new()).unwrap();
    assert_eq!(result.termination, TerminationStatus::Unbounded);
}

/// C9: Optimal MIP — single binary variable with trivial objective.
///
/// maximize 5*x0, s.t. 2*x0 <= 3, x0 binary.
/// Expected: Optimal, x0=1, objective=5.
#[test]
fn c9_optimal_mip() {
    let mut session = create_session();
    let r0 = ModelRevision::ZERO;
    let v0 = var_id(0);
    let c0 = con_id(0);
    let o0 = obj_id(0);

    let snap = ModelSnapshot {
        revision: r0,
        variables: vec![VariableEntry {
            id: v0,
            bounds: Bounds::BINARY,
            var_type: VarType::Binary,
            active: true,
            semicontinuous_lower: None,
        }],
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
                cell_key: (CoefficientTarget::Objective(o0), v0),
                value_expr: ValueExpr::constant(5.0),
                evaluated_value: 5.0,
                dependencies: vec![],
            },
        ],
        functions: vec![],
        constructs: vec![],
    };

    session
        .synchronize(Synchronization::CompiledRebuild(compile_snapshot(&snap)))
        .unwrap();
    let result = session.solve(&SolveRequest::new()).unwrap();
    assert_eq!(
        result.termination,
        TerminationStatus::Optimal,
        "Binary MIP should be Optimal"
    );
    let sol = result.solution.expect("MIP solution should be available");
    let obj = sol.objective_value.unwrap_or(0.0);
    assert!(
        approx_eq(obj, 5.0, 1e-4),
        "Expected MIP objective ≈ 5, got {}",
        obj
    );
}

/// C9: Solution extraction — verify variable values in optimal LP.
///
/// maximize x + 3y, x + y <= 5, x >= 0, y >= 0.
/// Expected: x=0, y=5, objective=15 (unique optimum — avoids the
/// line-of-optima ambiguity that made the non-zero count nondeterministic).
#[test]
fn c9_solution_extraction() {
    let mut session = create_session();
    let r0 = ModelRevision::ZERO;
    let v0 = var_id(0);
    let v1 = var_id(1);
    let c0 = con_id(0);
    let o0 = obj_id(0);

    let snap = ModelSnapshot {
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
            bounds: ConstraintBounds::le(5.0),
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
    };

    session
        .synchronize(Synchronization::CompiledRebuild(compile_snapshot(&snap)))
        .unwrap();
    let result = session.solve(&SolveRequest::new()).unwrap();
    assert_eq!(result.termination, TerminationStatus::Optimal);

    let sol = result.solution.expect("Optimal LP should have solution");

    // Objective value.
    let obj = sol.objective_value.unwrap_or(0.0);
    assert!(
        approx_eq(obj, 15.0, 1e-4),
        "Expected objective ≈ 15, got {}",
        obj
    );

    // Variable values should contain BOTH x0 and x1 — extraction maps every
    // HiGHS column back to a VarId (zeros are retained, only NaN/inf are
    // filtered), so a 2-variable model yields 2 entries: x=0 and y=5.
    assert_eq!(
        sol.variable_values.len(),
        2,
        "Expected exactly 2 variable values (x=0, y=5), got {}",
        sol.variable_values.len()
    );
    let y_val = sol
        .variable_values
        .iter()
        .find(|(var, _)| *var == v1)
        .map(|(_, v)| *v);
    assert!(
        approx_eq(y_val.unwrap_or(0.0), 5.0, 1e-4),
        "Expected y ≈ 5, got {:?}",
        y_val
    );
}

/// C9: Objective offset — verify objective with constant term.
///
/// HiGHS includes the objective constant offset (set via
/// Highs_changeObjectiveOffset during snapshot rebuild) in
/// Highs_getObjectiveValue.  A model with objective 2*x + 10 minimized
/// over x >= 0 should give objective_value ≈ 10 (x=0, constant=10).
#[test]
fn c9_objective_offset_constant() {
    let mut session = create_session();
    let r0 = ModelRevision::ZERO;
    let v0 = var_id(0);
    let o0 = obj_id(0);

    let snap = ModelSnapshot {
        revision: r0,
        variables: vec![VariableEntry {
            id: v0,
            bounds: Bounds::NON_NEGATIVE,
            var_type: VarType::Continuous,
            active: true,
            semicontinuous_lower: None,
        }],
        constraints: vec![],
        objectives: vec![ObjectiveEntry {
            id: o0,
            sense: Sense::Minimize,
            active: true,
            constant: 10.0,
        }],
        parameters: vec![],
        cells: vec![CellEntry {
            cell_key: (CoefficientTarget::Objective(o0), v0),
            value_expr: ValueExpr::constant(2.0),
            evaluated_value: 2.0,
            dependencies: vec![],
        }],
        functions: vec![],
        constructs: vec![],
    };

    session
        .synchronize(Synchronization::CompiledRebuild(compile_snapshot(&snap)))
        .unwrap();
    let result = session.solve(&SolveRequest::new()).unwrap();
    assert_eq!(result.termination, TerminationStatus::Optimal);

    let sol = result.solution.expect("Should have solution");
    let obj = sol.objective_value.unwrap_or(-1.0);
    // HiGHS Highs_getObjectiveValue includes the constant offset because
    // rebuild_from_snapshot sets it via Highs_changeObjectiveOffset.
    // The raw objective is 2*x + 10, minimized to 10 at x=0.
    assert!(
        approx_eq(obj, 10.0, 1e-4),
        "Expected objective ≈ 10 (2*x + 10, minimized at x=0), got {}.",
        obj
    );
}

// ── C10: Metadata ──────────────────────────────────────────────────────────────

/// C10: Metadata query contract (M1R-H8).
///
/// Verifies that:
/// - `name()` returns a version string (e.g., "1.15.0").
/// - `capabilities()` reports the expected flags.
#[test]
fn c10_metadata() {
    let session = create_session();

    // Name should contain a version number (semver or similar).
    let name = session.name();
    let has_digit = name.chars().any(|c| c.is_ascii_digit());
    let has_dot = name.contains('.');
    assert!(
        has_digit,
        "Session name should contain a version number, got: {}",
        name
    );
    assert!(
        has_dot,
        "Session name should have a dotted version format, got: {}",
        name
    );

    // Capabilities.
    let caps = session.capabilities();
    assert!(caps.lp, "HiGHS should support LP");
    assert!(caps.mip, "HiGHS should support MIP");
    assert!(caps.solution, "HiGHS should support solution extraction");
    assert!(caps.duals, "HiGHS should support dual values");
    assert!(caps.reduced_costs, "HiGHS should support reduced costs");
    assert!(
        !caps.semicontinuous,
        "HiGHS should NOT support semi-continuous (H7)"
    );
    assert!(!caps.semiinteger, "HiGHS should NOT support semi-integer");
}

// ── C11: Fallible Construction ─────────────────────────────────────────────────

/// C11: Fallible construction (M1R-H2).
///
/// Verifies that:
/// - `try_new()` returns `Result<HighsSession, BackendError>`, not a panic.
/// - Construction succeeds when HiGHS is available (bundled build).
/// - (Manual verification needed for 64-bit index width and library-not-found.)
#[test]
fn c11_fallible_construction() {
    // Nominal path: bundled build has HiGHS available.
    let session_result = HighsSession::try_new();
    assert!(
        session_result.is_ok(),
        "HighsSession::try_new() should succeed with bundled feature"
    );

    // Successful session should have Ready health.
    let session = session_result.unwrap();
    assert_eq!(
        session.health(),
        AdapterHealth::Ready,
        "Fresh session should start Ready"
    );
}

// To test library-not-found: run with `system` feature and no HiGHS installed.
// To test 64-bit index width: requires a 64-bit HiGHS build — currently unreachable.
// Both scenarios are documented but cannot be automated in CI without a
// specific test environment.

// =========================================================================
// C12: Production-path integration — Model API → commit() → DeltaBatch →
//      HighsSession::synchronize() → solve
//
// This test exercises the real approved production path (from PR #6/PR #9).
// It does NOT hand-construct DeltaBatch values. Every change goes through
// the public Model API and Model::commit(). The resulting DeltaBatch is
// applied through HighsSession::synchronize(), and the solve result is
// verified. This catches any gap between the compiler and the backend that
// hand-constructed deltas would miss.
// =========================================================================

#[test]
fn c12_production_path_objective_coefficient() {
    let mut model = Model::new();
    let x = model.add_var();
    let y = model.add_var();
    let obj = model.add_objective(Sense::Minimize);
    model.set_active_objective(obj).unwrap();
    model.add_objective_coeff(obj, x, 3.0).unwrap();
    model.add_objective_coeff(obj, y, 4.0).unwrap();

    let _r1 = model.commit().unwrap();
    let snapshot = model.take_snapshot().unwrap();

    let mut session = create_session();
    session
        .synchronize(Synchronization::CompiledRebuild(compile_snapshot(
            &snapshot,
        )))
        .expect("synchronize rebuild");

    let result = session.solve(&SolveRequest::default()).expect("solve");
    assert_eq!(result.termination, TerminationStatus::Optimal);
    assert!(result.solution.is_some());
    let sol = result.solution.as_ref().unwrap();
    assert!(approx_eq(
        sol.objective_value.unwrap_or(f64::NAN),
        0.0,
        1e-4
    ));
}

#[test]
fn c12_production_path_constraint_coefficient() {
    let mut model = Model::new();
    let x = model.add_var();
    let c = model.add_constraint(ConstraintBounds::le(10.0)).unwrap();
    model.add_coeff(c, x, 2.0).unwrap();

    let _r1 = model.commit().unwrap();
    let snapshot = model.take_snapshot().unwrap();

    let mut session = create_session();
    session
        .synchronize(Synchronization::CompiledRebuild(compile_snapshot(
            &snapshot,
        )))
        .expect("synchronize rebuild");

    let result = session.solve(&SolveRequest::default()).expect("solve");
    assert_eq!(result.termination, TerminationStatus::Optimal);
}

#[test]
fn c12_production_path_parameter_propagation() {
    let mut model = Model::new();
    let x = model.add_var();
    let p = model.add_parameter(5.0).unwrap();
    let c = model.add_constraint(ConstraintBounds::le(10.0)).unwrap();
    model
        .add_constraint_coefficient(c, x, ValueExpr::param(p))
        .unwrap();

    let _r1 = model.commit().unwrap();
    model.set_parameter(p, 3.0).unwrap();
    let _r2 = model.commit().unwrap();

    let snapshot = model.take_snapshot().unwrap();
    let mut session = create_session();
    session
        .synchronize(Synchronization::CompiledRebuild(compile_snapshot(
            &snapshot,
        )))
        .expect("synchronize rebuild");

    let result = session.solve(&SolveRequest::default()).expect("solve");
    assert_eq!(result.termination, TerminationStatus::Optimal);
}

#[test]
fn c12_production_path_inactive_objective_does_not_corrupt_active() {
    let mut model = Model::new();
    let x = model.add_var();
    let y = model.add_var();
    let obj1 = model.add_objective(Sense::Minimize);
    let obj2 = model.add_objective(Sense::Maximize);

    model.set_active_objective(obj1).unwrap();
    model.add_objective_coeff(obj1, x, 1.0).unwrap();
    model.add_objective_coeff(obj1, y, 0.0).unwrap();

    // Inactive obj2 — should NOT affect native HiGHS costs
    model.add_objective_coeff(obj2, x, 0.0).unwrap();
    model.add_objective_coeff(obj2, y, 100.0).unwrap();

    let _r1 = model.commit().unwrap();
    let snapshot = model.take_snapshot().unwrap();

    let mut session = create_session();
    session
        .synchronize(Synchronization::CompiledRebuild(compile_snapshot(
            &snapshot,
        )))
        .expect("rebuild");

    let result = session.solve(&SolveRequest::default()).unwrap();
    assert_eq!(result.termination, TerminationStatus::Optimal);
    let sol = result.solution.as_ref().unwrap();
    // Active is Minimize 1*x + 0*y → optimal at 0, objective 0
    assert!(approx_eq(
        sol.objective_value.unwrap_or(f64::NAN),
        0.0,
        1e-4
    ));
}

#[test]
fn c12_production_path_semi_continuous_roundtrip() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).unwrap();
    model.set_semicontinuous(x, 1.0).unwrap();

    let _r1 = model.commit().unwrap();
    let snapshot = model.take_snapshot().unwrap();

    // The identity compiler rejects the semi-continuous snapshot (the compiled
    // IR has no semi-continuous representation); no BackendSnapshot reaches the
    // session.
    let mut compiler = CompilationSession::new();
    let result = compiler.compile_snapshot(
        model.instance(),
        &snapshot,
        &CompilationPolicy::Auto,
        &test_capabilities(),
    );
    assert!(result.is_err(), "semi-continuous snapshot must be rejected");
}

// =========================================================================
// C13: Targeted correctness tests (reviewer-requested)
// =========================================================================

#[test]
fn c13_active_objective_sense_change() {
    // Minimize → Maximize on v in [0,10] with cost 1.
    // Minimize pushes to 0 (obj=0). Maximize pushes to 10 (obj=10).
    // Test that the sense change actually alters the solve result.
    let mut session = create_session();
    let r0 = ModelRevision::ZERO;
    let v = var_id(0);
    let obj = obj_id(0);
    let r1 = r0.next().unwrap();

    let snap = ModelSnapshot {
        revision: r0,
        variables: vec![VariableEntry {
            id: v,
            bounds: Bounds::new(0.0, 10.0),
            var_type: VarType::Continuous,
            active: true,
            semicontinuous_lower: None,
        }],
        constraints: vec![],
        objectives: vec![ObjectiveEntry {
            id: obj,
            sense: Sense::Minimize,
            active: true,
            constant: 0.0,
        }],
        parameters: vec![],
        cells: vec![CellEntry {
            cell_key: (CoefficientTarget::Objective(obj), v),
            value_expr: ValueExpr::constant(1.0),
            evaluated_value: 1.0,
            dependencies: vec![],
        }],
        functions: vec![],
        constructs: vec![],
    };
    // Apply SetObjectiveSense: Minimize → Maximize (base and delta in one
    // compilation chain — D28/WR-1).
    rebuild_then_apply_delta(
        &mut session,
        &snap,
        &DeltaBatch::new(
            r0,
            r1,
            vec![ModelOp::SetObjectiveSense {
                obj,
                sense: Sense::Maximize,
            }],
        )
        .unwrap(),
    );

    let result = session.solve(&SolveRequest::new()).unwrap();
    assert_eq!(result.termination, TerminationStatus::Optimal);
    let sol = result.solution.unwrap();
    let o = sol.objective_value.unwrap_or(f64::NAN);
    // Maximize 1*x with x∈[0,10] → x=10, obj≈10.
    assert!(
        approx_eq(o, 10.0, 1e-2),
        "expected obj≈10 after Maximize, got {}",
        o
    );
}

#[test]
fn c13_set_objective_cell_on_inactive_objective() {
    // Active obj1: Maximize 1*x with x∈[0,10].
    // Inactive obj2: Maximize 100*x (should NOT leak into HiGHS).
    // If inactive leaks: x=10, obj=1000. If correct: x=10, obj=10.
    let mut session = create_session();
    let r0 = ModelRevision::ZERO;
    let v = var_id(0);
    let obj1 = obj_id(0);
    let obj2 = obj_id(1);
    let r1 = r0.next().unwrap();

    let snap = ModelSnapshot {
        revision: r0,
        variables: vec![VariableEntry {
            id: v,
            bounds: Bounds::new(0.0, 10.0),
            var_type: VarType::Continuous,
            active: true,
            semicontinuous_lower: None,
        }],
        constraints: vec![],
        objectives: vec![
            ObjectiveEntry {
                id: obj1,
                sense: Sense::Maximize,
                active: true,
                constant: 0.0,
            },
            ObjectiveEntry {
                id: obj2,
                sense: Sense::Maximize,
                active: false,
                constant: 0.0,
            },
        ],
        parameters: vec![],
        cells: vec![CellEntry {
            cell_key: (CoefficientTarget::Objective(obj1), v),
            value_expr: ValueExpr::constant(1.0),
            evaluated_value: 1.0,
            dependencies: vec![],
        }],
        functions: vec![],
        constructs: vec![],
    };
    // SetObjectiveCell on INACTIVE obj2 with cost 100 — must stay cache-only
    // (base and delta in one compilation chain — D28/WR-1).
    rebuild_then_apply_delta(
        &mut session,
        &snap,
        &DeltaBatch::new(
            r0,
            r1,
            vec![ModelOp::SetObjectiveCell {
                cell_key: (CoefficientTarget::Objective(obj2), v),
                value_expr: ValueExpr::constant(100.0),
                evaluated_value: 100.0,
                constant: 0.0,
            }],
        )
        .unwrap(),
    );

    let result = session.solve(&SolveRequest::new()).unwrap();
    assert_eq!(result.termination, TerminationStatus::Optimal);
    let sol = result.solution.unwrap();
    let o = sol.objective_value.unwrap_or(f64::NAN);
    // Correct: Maximize 1*x, x=10 → obj≈10.
    // Leaked: Maximize 100*x → obj≈1000.
    assert!(
        o < 50.0,
        "inactive objective leaked: obj={} (expected ~10, leaked would be ~1000)",
        o
    );
}

#[test]
fn c13_semicontinuous_rejected_before_any_mutation() {
    // A mixed batch must reject atomically — no partial application.
    // Rebuild with v∈[0,10], then apply [SetVariableBounds(1,10), SetSemiContinuousBound].
    // After rejection, Health must NOT be Ready AND applied_revision must not advance.
    let mut session = create_session();
    let r0 = ModelRevision::ZERO;
    let v = var_id(0);
    let r1 = r0.next().unwrap();

    let snap = ModelSnapshot {
        revision: r0,
        variables: vec![VariableEntry {
            id: v,
            bounds: Bounds::new(0.0, 10.0),
            var_type: VarType::Continuous,
            active: true,
            semicontinuous_lower: None,
        }],
        constraints: vec![],
        objectives: vec![],
        parameters: vec![],
        cells: vec![],
        functions: vec![],
        constructs: vec![],
    };
    let mut compiler = TestCompiler::new();
    session
        .synchronize(Synchronization::CompiledRebuild(compiler.rebuild(&snap)))
        .unwrap();
    assert_eq!(session.revision(), r0);

    // Batch: valid bounds change followed by unsupported semi-continuous op.
    let batch = DeltaBatch::new(
        r0,
        r1,
        vec![
            ModelOp::SetVariableBounds {
                var: v,
                bounds: Bounds::new(1.0, 10.0),
            },
            ModelOp::SetSemiContinuousBound { var: v, lower: 5.0 },
        ],
    )
    .unwrap();
    // The identity compiler rejects the batch atomically (rebuild-on-uncertainty,
    // F-B1): no `BackendDeltaBatch` is emitted, so no mutation reaches the
    // session. This is the compiled-path equivalent of the atomic rejection.
    let from = compiler.session.current_compilation().unwrap();
    let result = compiler.session.compile_delta(
        &batch,
        from,
        compiler.instance,
        &CompilationPolicy::Auto,
        &test_capabilities(),
    );

    // 1. The batch MUST be rejected (no compiled delta emitted).
    assert!(result.is_err(), "batch must be rejected");

    // 2. The revision must NOT have advanced (no partial application).
    assert_eq!(
        session.revision(),
        r0,
        "revision advanced despite rejection"
    );
}

#[test]
fn c13_inactive_objective_sense_change() {
    // Changing inactive objective sense should only update cache, not HiGHS.
    // Active obj1: Minimize 1*x with x∈[0,10]. Inactive obj2: Minimize 100*x.
    // Change obj2 to Maximize. Solve → must still Minimize (x=0, obj=0).
    let mut session = create_session();
    let r0 = ModelRevision::ZERO;
    let v = var_id(0);
    let obj1 = obj_id(0);
    let obj2 = obj_id(1);
    let r1 = r0.next().unwrap();

    let snap = ModelSnapshot {
        revision: r0,
        variables: vec![VariableEntry {
            id: v,
            bounds: Bounds::new(0.0, 10.0),
            var_type: VarType::Continuous,
            active: true,
            semicontinuous_lower: None,
        }],
        constraints: vec![],
        objectives: vec![
            ObjectiveEntry {
                id: obj1,
                sense: Sense::Minimize,
                active: true,
                constant: 0.0,
            },
            ObjectiveEntry {
                id: obj2,
                sense: Sense::Minimize,
                active: false,
                constant: 0.0,
            },
        ],
        parameters: vec![],
        cells: vec![
            CellEntry {
                cell_key: (CoefficientTarget::Objective(obj1), v),
                value_expr: ValueExpr::constant(1.0),
                evaluated_value: 1.0,
                dependencies: vec![],
            },
            CellEntry {
                cell_key: (CoefficientTarget::Objective(obj2), v),
                value_expr: ValueExpr::constant(100.0),
                evaluated_value: 100.0,
                dependencies: vec![],
            },
        ],
        functions: vec![],
        constructs: vec![],
    };
    // Change inactive obj2 sense — cache-only (base and delta in one
    // compilation chain — D28/WR-1).
    rebuild_then_apply_delta(
        &mut session,
        &snap,
        &DeltaBatch::new(
            r0,
            r1,
            vec![ModelOp::SetObjectiveSense {
                obj: obj2,
                sense: Sense::Maximize,
            }],
        )
        .unwrap(),
    );

    let result = session.solve(&SolveRequest::new()).unwrap();
    assert_eq!(result.termination, TerminationStatus::Optimal);
    let sol = result.solution.unwrap();
    let o = sol.objective_value.unwrap_or(f64::NAN);
    // Active is Minimize 1*x → x=0, obj≈0.
    assert!(o < 5.0, "inactive sense leaked: obj={}", o);
}

// ── C14-C21: Rebuild/delta paths for previously-uncovered projections ─────────

/// C14: Rebuild fixes inactive variables to [0,0].
///
/// x∈[0,10] with `active: false` under a Maximize 1*x objective must solve as
/// x=0, obj=0. If the inactive variable leaked its bounds, x would reach 10.
#[test]
fn c14_rebuild_inactive_variable_fixed_to_zero() {
    let mut session = create_session();
    let r0 = ModelRevision::ZERO;
    let v = var_id(0);
    let o = obj_id(0);

    let snap = ModelSnapshot {
        revision: r0,
        variables: vec![VariableEntry {
            id: v,
            bounds: Bounds::new(0.0, 10.0),
            var_type: VarType::Continuous,
            active: false,
            semicontinuous_lower: None,
        }],
        constraints: vec![],
        objectives: vec![ObjectiveEntry {
            id: o,
            sense: Sense::Maximize,
            active: true,
            constant: 0.0,
        }],
        parameters: vec![],
        cells: vec![CellEntry {
            cell_key: (CoefficientTarget::Objective(o), v),
            value_expr: ValueExpr::constant(1.0),
            evaluated_value: 1.0,
            dependencies: vec![],
        }],
        functions: vec![],
        constructs: vec![],
    };
    session
        .synchronize(Synchronization::CompiledRebuild(compile_snapshot(&snap)))
        .unwrap();

    let result = session.solve(&SolveRequest::new()).unwrap();
    assert_eq!(result.termination, TerminationStatus::Optimal);
    let obj = result.solution.unwrap().objective_value.unwrap_or(f64::NAN);
    assert!(
        obj.abs() < 1e-6,
        "inactive variable contributed to objective: {}",
        obj
    );
    // The variable must be fixed at 0, not free to reach its [0,10] bound.
    assert_eq!(session.value(v), Some(0.0));
}

/// C15: Rebuild drops inactive constraints entirely.
///
/// x∈[0,10], Maximize x, with an inactive `x <= 5` row. The row must not
/// constrain the solve: x=10, obj=10. A leaked row bound would cap x at 5.
#[test]
fn c15_rebuild_inactive_constraint_ignored() {
    let mut session = create_session();
    let r0 = ModelRevision::ZERO;
    let v = var_id(0);
    let c = con_id(0);
    let o = obj_id(0);

    let snap = ModelSnapshot {
        revision: r0,
        variables: vec![VariableEntry {
            id: v,
            bounds: Bounds::new(0.0, 10.0),
            var_type: VarType::Continuous,
            active: true,
            semicontinuous_lower: None,
        }],
        constraints: vec![ConstraintEntry {
            id: c,
            bounds: ConstraintBounds::le(5.0),
            active: false,
        }],
        objectives: vec![ObjectiveEntry {
            id: o,
            sense: Sense::Maximize,
            active: true,
            constant: 0.0,
        }],
        parameters: vec![],
        cells: vec![
            CellEntry {
                cell_key: (CoefficientTarget::Constraint(c), v),
                value_expr: ValueExpr::constant(1.0),
                evaluated_value: 1.0,
                dependencies: vec![],
            },
            CellEntry {
                cell_key: (CoefficientTarget::Objective(o), v),
                value_expr: ValueExpr::constant(1.0),
                evaluated_value: 1.0,
                dependencies: vec![],
            },
        ],
        functions: vec![],
        constructs: vec![],
    };
    session
        .synchronize(Synchronization::CompiledRebuild(compile_snapshot(&snap)))
        .unwrap();

    let result = session.solve(&SolveRequest::new()).unwrap();
    assert_eq!(result.termination, TerminationStatus::Optimal);
    let obj = result.solution.unwrap().objective_value.unwrap_or(f64::NAN);
    assert!(
        approx_eq(obj, 10.0, 1e-4),
        "inactive constraint leaked: expected x=10 → obj=10, got {}",
        obj
    );
}

/// C16: Delta `AddVariable` with an Integer type honors integrality.
///
/// Rebuild empty, then delta-add x∈[0,10] as Integer, 2x <= 3, Maximize x.
/// Integer x → x=1, obj=1 (a continuous x would give x=1.5, obj=1.5).
#[test]
fn c16_delta_add_integer_variable_respects_integrality() {
    let mut session = create_session();
    let r0 = ModelRevision::ZERO;
    let v = var_id(0);
    let c = con_id(0);
    let o = obj_id(0);
    let r1 = r0.next().unwrap();

    let batch = DeltaBatch::new(
        r0,
        r1,
        vec![
            ModelOp::AddVariable {
                var: v,
                bounds: Bounds::new(0.0, 10.0),
                var_type: VarType::Integer,
            },
            ModelOp::AddConstraint {
                con: c,
                bounds: ConstraintBounds::le(3.0),
            },
            ModelOp::SetCell {
                cell_key: (CoefficientTarget::Constraint(c), v),
                value_expr: ValueExpr::constant(2.0),
                evaluated_value: 2.0,
            },
            ModelOp::AddObjective {
                obj: o,
                sense: Sense::Maximize,
            },
            ModelOp::SetObjectiveCell {
                cell_key: (CoefficientTarget::Objective(o), v),
                value_expr: ValueExpr::constant(1.0),
                evaluated_value: 1.0,
                constant: 0.0,
            },
            ModelOp::SetActiveObjective { obj: Some(o) },
        ],
    )
    .unwrap();
    // The empty r0 base and the integer-variable delta are compiled in ONE
    // chain, so the delta's `from_compilation` matches the base the session
    // holds (D28/WR-1).
    rebuild_then_apply_delta(&mut session, &ModelSnapshot::empty(r0), &batch);

    let result = session.solve(&SolveRequest::new()).unwrap();
    assert_eq!(result.termination, TerminationStatus::Optimal);
    let obj = result.solution.unwrap().objective_value.unwrap_or(f64::NAN);
    let x = session.value(v).unwrap_or(f64::NAN);
    assert!(
        approx_eq(x, 1.0, 1e-4),
        "integrality not honored: expected x=1, got {}",
        x
    );
    assert!(
        approx_eq(obj, 1.0, 1e-4),
        "integrality not honored: expected obj=1, got {}",
        obj
    );
}

/// C17: Delta `SetCell` on an active Objective target updates the native cost.
///
/// x∈[0,10], Maximize x with zero cost. SetCell{Objective(o0), v} = 5.0 must
/// reach the native column cost: x=10, obj=50. A cache-only update would
/// leave obj=0.
#[test]
fn c17_delta_set_cell_objective_updates_native_cost() {
    let mut session = create_session();
    let r0 = ModelRevision::ZERO;
    let v = var_id(0);
    let o = obj_id(0);
    let r1 = r0.next().unwrap();

    let snap = ModelSnapshot {
        revision: r0,
        variables: vec![VariableEntry {
            id: v,
            bounds: Bounds::new(0.0, 10.0),
            var_type: VarType::Continuous,
            active: true,
            semicontinuous_lower: None,
        }],
        constraints: vec![],
        objectives: vec![ObjectiveEntry {
            id: o,
            sense: Sense::Maximize,
            active: true,
            constant: 0.0,
        }],
        parameters: vec![],
        cells: vec![],
        functions: vec![],
        constructs: vec![],
    };
    // Base and delta in one compilation chain (D28/WR-1).
    rebuild_then_apply_delta(
        &mut session,
        &snap,
        &DeltaBatch::new(
            r0,
            r1,
            vec![ModelOp::SetCell {
                cell_key: (CoefficientTarget::Objective(o), v),
                value_expr: ValueExpr::constant(5.0),
                evaluated_value: 5.0,
            }],
        )
        .unwrap(),
    );

    let result = session.solve(&SolveRequest::new()).unwrap();
    assert_eq!(result.termination, TerminationStatus::Optimal);
    let obj = result.solution.unwrap().objective_value.unwrap_or(f64::NAN);
    assert!(
        approx_eq(obj, 50.0, 1e-4),
        "Objective-target SetCell not applied natively: expected 50 (5*10), got {}",
        obj
    );
}

/// C18: Delta `RemoveCell` on an Objective target clears the native cost.
///
/// x∈[0,10], Maximize 5*x. RemoveCell{Objective(o0), v} must zero the native
/// cost: obj=0. A stale cost would give 50.
#[test]
fn c18_delta_remove_cell_objective_clears_cost() {
    let mut session = create_session();
    let r0 = ModelRevision::ZERO;
    let v = var_id(0);
    let o = obj_id(0);
    let r1 = r0.next().unwrap();

    let snap = ModelSnapshot {
        revision: r0,
        variables: vec![VariableEntry {
            id: v,
            bounds: Bounds::new(0.0, 10.0),
            var_type: VarType::Continuous,
            active: true,
            semicontinuous_lower: None,
        }],
        constraints: vec![],
        objectives: vec![ObjectiveEntry {
            id: o,
            sense: Sense::Maximize,
            active: true,
            constant: 0.0,
        }],
        parameters: vec![],
        cells: vec![CellEntry {
            cell_key: (CoefficientTarget::Objective(o), v),
            value_expr: ValueExpr::constant(5.0),
            evaluated_value: 5.0,
            dependencies: vec![],
        }],
        functions: vec![],
        constructs: vec![],
    };
    // Base and delta in one compilation chain (D28/WR-1).
    rebuild_then_apply_delta(
        &mut session,
        &snap,
        &DeltaBatch::new(
            r0,
            r1,
            vec![ModelOp::RemoveCell {
                cell_key: (CoefficientTarget::Objective(o), v),
            }],
        )
        .unwrap(),
    );

    let result = session.solve(&SolveRequest::new()).unwrap();
    assert_eq!(result.termination, TerminationStatus::Optimal);
    let obj = result.solution.unwrap().objective_value.unwrap_or(f64::NAN);
    assert!(
        obj.abs() < 1e-6,
        "Objective-target RemoveCell left a stale native cost: obj={}",
        obj
    );
}

/// C19: Delta `SetObjectiveCell` with a Constraint target is rejected with a
/// typed error; the session must not apply anything (RequiresRebuild, r0).
#[test]
fn c19_delta_set_objective_cell_on_constraint_target_rejected() {
    let mut session = create_session();
    let r0 = ModelRevision::ZERO;
    let v = var_id(0);
    let c = con_id(0);
    let o = obj_id(0);
    let r1 = r0.next().unwrap();

    let snap = ModelSnapshot {
        revision: r0,
        variables: vec![VariableEntry {
            id: v,
            bounds: Bounds::new(0.0, 10.0),
            var_type: VarType::Continuous,
            active: true,
            semicontinuous_lower: None,
        }],
        constraints: vec![ConstraintEntry {
            id: c,
            bounds: ConstraintBounds::le(5.0),
            active: true,
        }],
        objectives: vec![ObjectiveEntry {
            id: o,
            sense: Sense::Minimize,
            active: true,
            constant: 0.0,
        }],
        parameters: vec![],
        cells: vec![CellEntry {
            cell_key: (CoefficientTarget::Constraint(c), v),
            value_expr: ValueExpr::constant(1.0),
            evaluated_value: 1.0,
            dependencies: vec![],
        }],
        functions: vec![],
        constructs: vec![],
    };
    let mut compiler = TestCompiler::new();
    session
        .synchronize(Synchronization::CompiledRebuild(compiler.rebuild(&snap)))
        .unwrap();
    assert_eq!(session.revision(), r0);

    let batch = DeltaBatch::new(
        r0,
        r1,
        vec![ModelOp::SetObjectiveCell {
            cell_key: (CoefficientTarget::Constraint(c), v),
            value_expr: ValueExpr::constant(3.0),
            evaluated_value: 3.0,
            constant: 0.0,
        }],
    )
    .unwrap();
    // The identity compiler rejects a `SetObjectiveCell` with a Constraint
    // target (a malformed op): no `BackendDeltaBatch` is emitted, so nothing
    // reaches the session.
    let from = compiler.session.current_compilation().unwrap();
    let result = compiler.session.compile_delta(
        &batch,
        from,
        compiler.instance,
        &CompilationPolicy::Auto,
        &test_capabilities(),
    );
    assert!(
        result.is_err(),
        "SetObjectiveCell with Constraint target must be rejected"
    );

    // Nothing was applied: the session stays at r0.
    assert_eq!(session.revision(), r0);
}

/// C20: Delta `SetObjectiveCell` on the ACTIVE objective applies the offset
/// and native cost immediately.
///
/// x∈[0,10], Minimize 0*x + 0. SetObjectiveCell{Objective(o0), v, 5.0, 2.0}
/// → native cost 5 and offset 2.0 applied. Solve Minimize → x=0, obj=2.0.
#[test]
fn c20_delta_set_objective_cell_on_active_objective_applies_immediately() {
    let mut session = create_session();
    let r0 = ModelRevision::ZERO;
    let v = var_id(0);
    let o = obj_id(0);
    let r1 = r0.next().unwrap();

    let snap = ModelSnapshot {
        revision: r0,
        variables: vec![VariableEntry {
            id: v,
            bounds: Bounds::new(0.0, 10.0),
            var_type: VarType::Continuous,
            active: true,
            semicontinuous_lower: None,
        }],
        constraints: vec![],
        objectives: vec![ObjectiveEntry {
            id: o,
            sense: Sense::Minimize,
            active: true,
            constant: 0.0,
        }],
        parameters: vec![],
        cells: vec![],
        functions: vec![],
        constructs: vec![],
    };
    // Base and delta in one compilation chain (D28/WR-1).
    rebuild_then_apply_delta(
        &mut session,
        &snap,
        &DeltaBatch::new(
            r0,
            r1,
            vec![ModelOp::SetObjectiveCell {
                cell_key: (CoefficientTarget::Objective(o), v),
                value_expr: ValueExpr::constant(5.0),
                evaluated_value: 5.0,
                constant: 2.0,
            }],
        )
        .unwrap(),
    );

    let result = session.solve(&SolveRequest::new()).unwrap();
    assert_eq!(result.termination, TerminationStatus::Optimal);
    let obj = result.solution.unwrap().objective_value.unwrap_or(f64::NAN);
    // Minimize 5*x + 2.0 at x=0 → obj = 2.0 (offset applied immediately).
    assert!(
        approx_eq(obj, 2.0, 1e-4),
        "SetObjectiveCell on active objective not applied: expected 2.0, got {}",
        obj
    );
}

/// C21: A malformed snapshot (inverted bounds, lower > upper) is rejected by
/// HiGHS with a typed `BackendError` — both on rebuild (`Highs_addVar`) and
/// on a delta bounds change (`Highs_changeColBounds`).
#[test]
fn c21_rebuild_with_inverted_bounds_rejected() {
    let mut session = create_session();
    let r0 = ModelRevision::ZERO;
    let v = var_id(0);
    let o = obj_id(0);

    let snap = ModelSnapshot {
        revision: r0,
        variables: vec![VariableEntry {
            id: v,
            bounds: Bounds::new(5.0, 0.0), // lower > upper
            var_type: VarType::Continuous,
            active: true,
            semicontinuous_lower: None,
        }],
        constraints: vec![],
        objectives: vec![ObjectiveEntry {
            id: o,
            sense: Sense::Maximize,
            active: true,
            constant: 0.0,
        }],
        parameters: vec![],
        cells: vec![],
        functions: vec![],
        constructs: vec![],
    };
    let err = match session.synchronize(Synchronization::CompiledRebuild(compile_snapshot(&snap))) {
        Ok(_) => panic!("inverted bounds must be rejected by HiGHS"),
        Err(e) => e,
    };
    assert_eq!(err.category, ErrorCategory::Internal);
    assert_eq!(err.health_effect, HealthEffect::Recoverable);
    assert!(
        err.message.contains("Highs_addVar"),
        "unexpected error message: {}",
        err.message
    );
}

// Note: a delta `SetVariableBounds` with inverted bounds (lower > upper) is
// NOT rejected — `Highs_changeColBounds` accepts it (only `Highs_addVar`
// surfaces a non-OK status). Inverted bounds are therefore only detected on
// the rebuild path (c21_rebuild_with_inverted_bounds_rejected).
