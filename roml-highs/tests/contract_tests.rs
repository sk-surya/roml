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

use roml_highs::HighsSession;
use roml::delta::{DeltaBatch, ModelOp};
use roml::id::{ConId, Generation, ObjId, VarId};
use roml::model::coefficient::CoefficientTarget;
use roml::model::{Bounds, ConstraintBounds, Sense, VarType};
use roml::revision::ModelRevision;
use roml::snapshot::{CellEntry, ConstraintEntry, ModelSnapshot, ObjectiveEntry, VariableEntry};
use roml::solver::backend::{ErrorCategory, HealthEffect, TerminationStatus};
use roml::solver::request::SolveRequest;
use roml::solver::session::{BackendSession, BackendMetadata, SessionHealth, Synchronization};
use roml::sync::AdapterHealth;
use roml::value_expr::ValueExpr;

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
        .synchronize(Synchronization::Rebuild(snapshot))
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
    };

    let receipt = session
        .synchronize(Synchronization::Rebuild(snapshot))
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

    let sol = result.solution.expect("Optimal solution should be available");
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
#[test]
fn c3_incremental_delta() {
    let mut session = create_session();
    let r0 = ModelRevision::ZERO;

    // Start from empty.
    let receipt = session
        .synchronize(Synchronization::Rebuild(ModelSnapshot::empty(r0)))
        .expect("Empty rebuild should succeed");
    assert_eq!(receipt.health, AdapterHealth::Ready);
    assert_eq!(session.revision(), r0);

    let v0 = var_id(0);
    let c0 = con_id(0);
    let o0 = obj_id(0);

    // Helper: apply a single-operation batch and verify readiness.
    let mut rev = r0;
    macro_rules! apply_op {
        ($op:expr) => {{
            let next = rev
                .next()
                .expect("Revision should not overflow during test");
            let batch =
                DeltaBatch::new(rev, next, vec![$op]).expect("DeltaBatch construction should succeed");
            session
                .synchronize(Synchronization::DeltaBatch(batch))
                .unwrap_or_else(|e| panic!("Delta sync r{}->r{} failed: {}", rev.as_u64(), next.as_u64(), e));
            assert_eq!(
                session.revision(),
                next,
                "Revision should advance to r{}",
                next.as_u64()
            );
            rev = next;
        }};
    }

    // 1. AddVariable (continuous, non-negative)
    apply_op!(ModelOp::AddVariable {
        var: v0,
        bounds: Bounds::NON_NEGATIVE,
        var_type: VarType::Continuous,
    });

    // 2. SetVariableBounds (tighten)
    apply_op!(ModelOp::SetVariableBounds {
        var: v0,
        bounds: Bounds::new(1.0, 10.0),
    });

    // 3. SetVariableActive { active: false }
    apply_op!(ModelOp::SetVariableActive {
        var: v0,
        active: false,
    });

    // 4. SetVariableActive { active: true }
    apply_op!(ModelOp::SetVariableActive {
        var: v0,
        active: true,
    });

    // 5. SetVariableType (change to integer)
    apply_op!(ModelOp::SetVariableType {
        var: v0,
        var_type: VarType::Integer,
    });

    // 6. AddConstraint (upper bound 20)
    apply_op!(ModelOp::AddConstraint {
        con: c0,
        bounds: ConstraintBounds::le(20.0),
    });

    // 7. SetConstraintBounds (change to [-inf, 30])
    apply_op!(ModelOp::SetConstraintBounds {
        con: c0,
        bounds: ConstraintBounds::le(30.0),
    });

    // 8. SetConstraintActive { active: false }
    apply_op!(ModelOp::SetConstraintActive {
        con: c0,
        active: false,
    });

    // 9. SetConstraintActive { active: true }
    apply_op!(ModelOp::SetConstraintActive {
        con: c0,
        active: true,
    });

    // 10. SetCell (add coefficient 3.0 at constraint)
    apply_op!(ModelOp::SetCell {
        cell_key: (CoefficientTarget::Constraint(c0), v0),
        value_expr: ValueExpr::constant(3.0),
        evaluated_value: 3.0,
    });

    // 11. RemoveCell (remove coefficient)
    apply_op!(ModelOp::RemoveCell {
        cell_key: (CoefficientTarget::Constraint(c0), v0),
    });

    // 12. AddObjective (maximize)
    apply_op!(ModelOp::AddObjective {
        obj: o0,
        sense: Sense::Maximize,
    });

    // 13. SetObjectiveCell (add coefficient 5.0 to objective)
    apply_op!(ModelOp::SetObjectiveCell {
        cell_key: (CoefficientTarget::Objective(o0), v0),
        value_expr: ValueExpr::constant(5.0),
        evaluated_value: 5.0,
        constant: 0.0,
    });

    // 14. SetActiveObjective (activate this objective)
    apply_op!(ModelOp::SetActiveObjective {
        obj: Some(o0),
    });

    // 15. RemoveObjective
    apply_op!(ModelOp::RemoveObjective { obj: o0 });

    // 16. RemoveVariable
    apply_op!(ModelOp::RemoveVariable { var: v0 });

    // 17. RemoveConstraint
    apply_op!(ModelOp::RemoveConstraint { con: c0 });

    // 18. SetParameter (no-op in HiGHS, but should not error)
    apply_op!(ModelOp::SetParameter {
        param: roml::id::ParamId::new(0, Generation::new()),
        value: 1.0,
    });

    // Final solve on empty model — should still be trivially optimal.
    let result = session
        .solve(&SolveRequest::new())
        .expect("Final solve after all delta ops should succeed");
    assert_eq!(
        result.termination,
        TerminationStatus::Optimal,
        "Empty model after all ops is trivially optimal"
    );
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
        cells: vec![
            CellEntry {
                cell_key: (CoefficientTarget::Objective(o0), v0),
                value_expr: ValueExpr::constant(1.0),
                evaluated_value: 1.0,
                dependencies: vec![],
            },
        ],
    };

    // Session A: rebuild from r0, then apply delta r0->r1.
    session_a
        .synchronize(Synchronization::Rebuild(snap_r0.clone()))
        .expect("Session A: rebuild from r0 should succeed");

    let delta = DeltaBatch::new(r0, r1, vec![
        ModelOp::SetCell {
            cell_key: (CoefficientTarget::Constraint(c0), v0),
            value_expr: ValueExpr::constant(2.0),
            evaluated_value: 2.0,
        },
    ])
    .expect("DeltaBatch r0->r1 should be valid");

    session_a
        .synchronize(Synchronization::DeltaBatch(delta))
        .expect("Session A: delta apply should succeed");

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
    };

    // Session B: rebuild directly from r1.
    session_b
        .synchronize(Synchronization::Rebuild(snap_r1))
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
    };

    // Rebuild.
    session
        .synchronize(Synchronization::Rebuild(snapshot))
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

    // Deactivate.
    let r1 = r0.next().unwrap();
    session
        .synchronize(Synchronization::DeltaBatch(
            DeltaBatch::new(
                r0,
                r1,
                vec![ModelOp::SetVariableActive {
                    var: v0,
                    active: false,
                }],
            )
            .unwrap(),
        ))
        .expect("Deactivate delta should succeed");

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

    // Reactivate.
    let r2 = r1.next().unwrap();
    session
        .synchronize(Synchronization::DeltaBatch(
            DeltaBatch::new(
                r1,
                r2,
                vec![ModelOp::SetVariableActive {
                    var: v0,
                    active: true,
                }],
            )
            .unwrap(),
        ))
        .expect("Reactivate delta should succeed");

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
    };

    session
        .synchronize(Synchronization::Rebuild(snapshot))
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

    // Add maximize objective and activate it (r0->r1).
    let o_max = obj_id(1);
    let r1 = r0.next().unwrap();
    session
        .synchronize(Synchronization::DeltaBatch(
            DeltaBatch::new(
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
                    ModelOp::SetActiveObjective {
                        obj: Some(o_max),
                    },
                ],
            )
            .unwrap(),
        ))
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
    let mut session = create_session();
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
    };

    let result = session.synchronize(Synchronization::Rebuild(snapshot));

    assert!(
        result.is_err(),
        "Semi-continuous snapshot should be rejected"
    );
    let err = result.err().expect("Already checked is_err");
    assert_eq!(
        err.category,
        ErrorCategory::Unsupported,
        "Error should be Unsupported"
    );
    assert!(
        err.message.contains("semi-continuous"),
        "Error message should mention semi-continuous: {}",
        err.message
    );
    assert_eq!(
        err.health_effect,
        HealthEffect::RequiresRebuild,
        "Health effect should be RequiresRebuild"
    );
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
    };

    session.synchronize(Synchronization::Rebuild(snap)).unwrap();
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
    };

    session.synchronize(Synchronization::Rebuild(snap)).unwrap();
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
    };

    session.synchronize(Synchronization::Rebuild(snap)).unwrap();
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
/// maximize 1.5*x + 3.0*y, x + y <= 10, x >= 0, y >= 0.
/// Expected: Optimal, x=0, y=10, objective=30.
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
                value_expr: ValueExpr::constant(1.5),
                evaluated_value: 1.5,
                dependencies: vec![],
            },
            CellEntry {
                cell_key: (CoefficientTarget::Objective(o0), v1),
                value_expr: ValueExpr::constant(3.0),
                evaluated_value: 3.0,
                dependencies: vec![],
            },
        ],
    };

    session.synchronize(Synchronization::Rebuild(snap)).unwrap();
    let result = session.solve(&SolveRequest::new()).unwrap();
    assert_eq!(result.termination, TerminationStatus::Optimal);

    let sol = result.solution.expect("Optimal LP should have solution");
    let obj = sol.objective_value.unwrap_or(0.0);
    assert!(
        approx_eq(obj, 30.0, 1e-4),
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
    };

    session.synchronize(Synchronization::Rebuild(snap)).unwrap();
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
    };

    session.synchronize(Synchronization::Rebuild(snap)).unwrap();
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
    };

    session.synchronize(Synchronization::Rebuild(snap)).unwrap();
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
/// maximize x + y, x + y <= 5, x >= 0, y >= 0.
/// Expected: x=5, y=0, objective=5.
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
                value_expr: ValueExpr::constant(1.0),
                evaluated_value: 1.0,
                dependencies: vec![],
            },
        ],
    };

    session.synchronize(Synchronization::Rebuild(snap)).unwrap();
    let result = session.solve(&SolveRequest::new()).unwrap();
    assert_eq!(result.termination, TerminationStatus::Optimal);

    let sol = result.solution.expect("Optimal LP should have solution");

    // Objective value.
    let obj = sol.objective_value.unwrap_or(0.0);
    assert!(
        approx_eq(obj, 5.0, 1e-4),
        "Expected objective ≈ 5, got {}",
        obj
    );

    // Variable values should contain both x0 and x1.
    assert_eq!(
        sol.variable_values.len(),
        1,
        "Expected exactly 1 non-zero variable value (y=0 is excluded via NaN filter)"
    );
}

/// C9: Objective offset — verify objective with constant term.
///
/// NOTE: Per AD-9, the objective constant is not currently applied to
/// Highs_getObjectiveValue. This test documents the current behaviour:
/// the constant offset (10) is stored in ROML's objective cache but not
/// applied to HiGHS, so Highs_getObjectiveValue returns 2*x = 0 for x=0.
/// When AD-9 is implemented (manual offset application), this test should
/// expect objective ≈ 10 instead.
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
    };

    session.synchronize(Synchronization::Rebuild(snap)).unwrap();
    let result = session.solve(&SolveRequest::new()).unwrap();
    assert_eq!(result.termination, TerminationStatus::Optimal);

    let sol = result.solution.expect("Should have solution");
    let obj = sol.objective_value.unwrap_or(-1.0);
    // HiGHS Highs_getObjectiveValue does NOT include the constant offset
    // because ROML doesn't set it via Highs_changeObjectiveOffset.
    // The raw objective is 2*x, minimized to 0 at x=0.
    // When AD-9 is implemented, this should become ≈ 10.
    assert!(
        approx_eq(obj, 0.0, 1e-4),
        "Expected objective ≈ 0 (2*x without constant), got {}. \
         When AD-9 offset is implemented, expect ≈ 10.",
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
    assert!(
        !caps.semiinteger,
        "HiGHS should NOT support semi-integer"
    );
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
