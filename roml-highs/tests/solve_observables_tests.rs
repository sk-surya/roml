//! M1R-Q5 solve observables tests.
//!
//! Focused tests for objective offsets, dual values, reduced costs,
//! basis/hot-start behavior, statuses, and option negotiation.
//!
//! These tests extend the C8-C11 patterns from `contract_tests.rs`
//! with HiGHS-specific observable behavior, not duplicates.

#![allow(clippy::approx_constant)]

use roml_highs::HighsSession;
use roml::id::{ConId, Generation, ObjId, VarId};
use roml::model::coefficient::CoefficientTarget;
use roml::model::{Bounds, ConstraintBounds, Sense, VarType};
use roml::revision::ModelRevision;
use roml::snapshot::{CellEntry, ConstraintEntry, ModelSnapshot, ObjectiveEntry, VariableEntry};
use roml::solver::backend::TerminationStatus;
use roml::solver::request::SolveRequest;
use roml::solver::session::{BackendSession, SolutionView, Synchronization};
use roml::value_expr::ValueExpr;

// ── Test Helpers ───────────────────────────────────────────────────────────────

/// Create a new HiGHS session for testing.
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

// ── Q5 Tests ───────────────────────────────────────────────────────────────────

/// M1R-Q5: Objective constant is stored in ROML model but not applied to HiGHS
/// via Highs_changeObjectiveOffset (AD-9 gap).
///
/// When AD-9 is implemented, the expected value should become
/// objective + constant_offset.
#[test]
fn q5_objective_offset() {
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

    session
        .synchronize(Synchronization::Rebuild(snap))
        .expect("Rebuild should succeed");
    let result = session
        .solve(&SolveRequest::new())
        .expect("Solve should succeed");
    assert_eq!(result.termination, TerminationStatus::Optimal);

    let sol = result.solution.expect("Should have solution");
    let obj = sol.objective_value.unwrap_or(-1.0);
    // AD-9: HiGHS does not include objective constant offset.
    // Expected raw objective = 2*x = 0 at x=0 (minimized).
    // When AD-9 is implemented, this should become ≈ 10.
    assert!(
        approx_eq(obj, 0.0, 1e-4),
        "Expected objective ≈ 0 (raw 2*x without constant). \
         When AD-9 offset is implemented, expect ≈ 10. Got {}",
        obj
    );
}

/// M1R-Q5: Dual values for binding constraints.
///
/// maximize x + y, s.t. x + y <= 5, x >= 0, y >= 0.
/// The binding constraint x + y <= 5 has a non-zero dual.
#[test]
fn q5_dual_values() {
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

    session
        .synchronize(Synchronization::Rebuild(snap))
        .expect("Rebuild should succeed");
    let result = session
        .solve(&SolveRequest::new())
        .expect("Solve should succeed");
    assert_eq!(result.termination, TerminationStatus::Optimal);

    // Extract dual value for the binding constraint via SolutionView
    let dual = session.dual(c0);
    assert!(
        dual.is_some(),
        "Binding constraint should have a dual value"
    );
    let dual_val = dual.unwrap();
    assert!(
        dual_val.abs() > 1e-6,
        "Binding constraint should have non-zero dual, got {}",
        dual_val
    );

    // Verify solution exists with objective = 5
    let sol = result.solution.expect("Optimal solution should be available");
    let obj = sol.objective_value.unwrap_or(0.0);
    assert!(
        approx_eq(obj, 5.0, 1e-4),
        "Expected objective ≈ 5, got {}",
        obj
    );
}

/// M1R-Q5: Reduced costs are available in solve results.
///
/// Same model as dual test. Reduced costs are extracted from HiGHS
/// after solve. At least one non-basic variable has a non-zero
/// reduced cost.
#[test]
fn q5_reduced_costs() {
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

    session
        .synchronize(Synchronization::Rebuild(snap))
        .expect("Rebuild should succeed");
    let result = session
        .solve(&SolveRequest::new())
        .expect("Solve should succeed");
    assert_eq!(result.termination, TerminationStatus::Optimal);

    // Verify solution has dual values and reduced costs
    let sol = result.solution.expect("Optimal solution should be available");

    // Check reduced costs are available in the solve result
    assert!(
        sol.reduced_costs.is_some(),
        "Reduced costs should be available"
    );
    let costs = sol.reduced_costs.as_ref().unwrap();
    assert!(!costs.is_empty(), "Reduced costs should not be empty");

    // Verify objective is correct (5.0)
    let obj = sol.objective_value.unwrap_or(0.0);
    assert!(
        approx_eq(obj, 5.0, 1e-4),
        "Expected objective ≈ 5, got {}",
        obj
    );
}

/// M1R-Q5: Standard option negotiation — time limit and threads applied.
///
/// Empty model, trivially optimal. Options should be applied
/// (not rejected).
#[test]
fn q5_option_negotiation_applied() {
    let mut session = create_session();
    let r0 = ModelRevision::ZERO;

    session
        .synchronize(Synchronization::Rebuild(ModelSnapshot::empty(r0)))
        .expect("Empty rebuild should succeed");

    let result = session
        .solve(
            &SolveRequest::new()
                .with_time_limit(60.0)
                .with_threads(1),
        )
        .expect("Solve with options should succeed");
    assert_eq!(result.termination, TerminationStatus::Optimal);

    // effective_configuration should have the applied options
    let config = &result.effective_configuration;
    assert!(
        config.time_limit_secs.is_some(),
        "time_limit_secs should be applied"
    );
    if let Some(tl) = config.time_limit_secs {
        assert!(
            approx_eq(tl, 60.0, 1e-4),
            "Expected time limit 60, got {}",
            tl
        );
    }
    assert!(
        config.threads.is_some(),
        "threads should be applied"
    );
    if let Some(t) = config.threads {
        assert_eq!(t, 1, "Expected threads = 1, got {}", t);
    }

    // Options should not be rejected
    assert!(
        config.rejections.is_empty(),
        "Standard options should not be rejected, got: {:?}",
        config.rejections
    );
}

/// M1R-Q5: Extra option negotiation — unknown option is rejected,
/// solve still succeeds.
#[test]
fn q5_option_negotiation_extra() {
    let mut session = create_session();
    let r0 = ModelRevision::ZERO;

    session
        .synchronize(Synchronization::Rebuild(ModelSnapshot::empty(r0)))
        .expect("Empty rebuild should succeed");

    let result = session
        .solve(
            &SolveRequest::new()
                .with_option("output_flag", "false")
                .with_option("nonexistent_option_xyz", "1"),
        )
        .expect("Solve with extra options should still succeed");
    assert_eq!(result.termination, TerminationStatus::Optimal);

    // output_flag is a valid HiGHS option — it should be applied
    // nonexistent_option_xyz should be rejected
    let config = &result.effective_configuration;
    assert!(
        !config.rejections.is_empty(),
        "Unknown option should produce a rejection"
    );

    // Verify the unknown option is in the rejections list
    let has_unknown_rejection = config.rejections.iter().any(|r| r.key.contains("nonexistent"));
    assert!(
        has_unknown_rejection,
        "Rejections should include the unknown option: {:?}",
        config.rejections
    );
}

/// M1R-Q5: Unbounded model maps to TerminationStatus::Unbounded.
///
/// maximize x, x >= 0 (no upper bound).
#[test]
fn q5_status_infeasible_or_unbounded() {
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

    session
        .synchronize(Synchronization::Rebuild(snap))
        .expect("Rebuild should succeed");
    let result = session
        .solve(&SolveRequest::new())
        .expect("Solve should succeed");
    assert_eq!(
        result.termination,
        TerminationStatus::Unbounded,
        "Unbounded LP should map to Unbounded, got {:?}",
        result.termination
    );
}
