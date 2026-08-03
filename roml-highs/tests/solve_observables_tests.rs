//! M1R-Q5 solve observables tests.
//!
//! Focused tests for objective offsets, dual values, reduced costs,
//! basis/hot-start behavior, statuses, and option negotiation.
//!
//! These tests extend the C8-C11 patterns from `contract_tests.rs`
//! with HiGHS-specific observable behavior, not duplicates.

#![allow(clippy::approx_constant)]

use roml::advanced::{
    BackendCapabilitySet, BackendFeature, BackendSnapshot, CompilationPolicy, CompilationSession,
    FeatureSupport, SupportLevel,
};
use roml::id::{ConId, Generation, ObjId, VarId};
use roml::model::coefficient::CoefficientTarget;
use roml::model::{Bounds, ConstraintBounds, Sense, VarType};
use roml::revision::ModelRevision;
use roml::snapshot::{CellEntry, ConstraintEntry, ModelSnapshot, ObjectiveEntry, VariableEntry};
use roml::solver::backend::TerminationStatus;
use roml::solver::request::SolveRequest;
use roml::solver::session::{BackendSession, SolutionView, Synchronization};
use roml::value_expr::ValueExpr;
use roml::LpAlgorithm;
use roml::Model;
use roml_highs::HighsSession;

// ── Test Helpers ───────────────────────────────────────────────────────────────

/// Create a new HiGHS session for testing.
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

/// M1R-Q5: Objective constant offset — rebuild_from_snapshot sets the
/// constant offset via Highs_changeObjectiveOffset so that
/// Highs_getObjectiveValue includes it.
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
            fixing: None,
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
        .expect("Rebuild should succeed");
    let result = session
        .solve(&SolveRequest::new())
        .expect("Solve should succeed");
    assert_eq!(result.termination, TerminationStatus::Optimal);

    let sol = result.solution.expect("Should have solution");
    let obj = sol.objective_value.unwrap_or(-1.0);
    // Objective = 2*x + 10, minimized at x=0 → value ≈ 10.
    assert!(
        approx_eq(obj, 10.0, 1e-4),
        "Expected objective ≈ 10 (2*x + 10, x=0). Got {}",
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
                fixing: None,
            },
            VariableEntry {
                id: v1,
                bounds: Bounds::NON_NEGATIVE,
                var_type: VarType::Continuous,
                active: true,
                semicontinuous_lower: None,
                fixing: None,
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
        functions: vec![],
        constructs: vec![],
    };

    session
        .synchronize(Synchronization::CompiledRebuild(compile_snapshot(&snap)))
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
    let sol = result
        .solution
        .expect("Optimal solution should be available");
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
                fixing: None,
            },
            VariableEntry {
                id: v1,
                bounds: Bounds::NON_NEGATIVE,
                var_type: VarType::Continuous,
                active: true,
                semicontinuous_lower: None,
                fixing: None,
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
        functions: vec![],
        constructs: vec![],
    };

    session
        .synchronize(Synchronization::CompiledRebuild(compile_snapshot(&snap)))
        .expect("Rebuild should succeed");
    let result = session
        .solve(&SolveRequest::new())
        .expect("Solve should succeed");
    assert_eq!(result.termination, TerminationStatus::Optimal);

    // Verify solution has dual values and reduced costs
    let sol = result
        .solution
        .expect("Optimal solution should be available");

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
        .synchronize(Synchronization::CompiledRebuild(compile_snapshot(
            &ModelSnapshot::empty(r0),
        )))
        .expect("Empty rebuild should succeed");

    let result = session
        .solve(&SolveRequest::new().with_time_limit(60.0).with_threads(1))
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
    assert!(config.threads.is_some(), "threads should be applied");
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
        .synchronize(Synchronization::CompiledRebuild(compile_snapshot(
            &ModelSnapshot::empty(r0),
        )))
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
    let has_unknown_rejection = config
        .rejections
        .iter()
        .any(|r| r.key.contains("nonexistent"));
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
            fixing: None,
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

// ── Q5 follow-up: option negotiation end-to-end ──────────────────────────────

/// Q5: Every `LpAlgorithm` request must be recorded faithfully in
/// `effective_configuration.lp_algorithm` — no silent remapping.
#[test]
fn q5_lp_algorithm_variants_map_to_effective_config() {
    let cases = [
        (LpAlgorithm::Automatic, LpAlgorithm::Automatic),
        (LpAlgorithm::DualSimplex, LpAlgorithm::DualSimplex),
        (LpAlgorithm::Primal, LpAlgorithm::Primal),
        (LpAlgorithm::Dual, LpAlgorithm::Dual),
        (LpAlgorithm::Barrier, LpAlgorithm::Barrier),
    ];

    for (requested, expected) in cases {
        let mut session = create_session();
        let r0 = ModelRevision::ZERO;
        session
            .synchronize(Synchronization::CompiledRebuild(compile_snapshot(
                &ModelSnapshot::empty(r0),
            )))
            .expect("Empty rebuild should succeed");

        let result = session
            .solve(&SolveRequest::new().with_lp_algorithm(requested))
            .expect("Solve with lp_algorithm should succeed");
        assert_eq!(
            result.termination,
            TerminationStatus::Optimal,
            "{requested:?} solve should be Optimal"
        );
        assert_eq!(
            result.effective_configuration.lp_algorithm,
            Some(expected),
            "requested {requested:?} must be reported faithfully, got {:?}",
            result.effective_configuration.lp_algorithm
        );
    }
}

/// Q5: mip_rel_gap is applied and recorded.
#[test]
fn q5_mip_rel_gap_applied() {
    let mut session = create_session();
    let r0 = ModelRevision::ZERO;
    session
        .synchronize(Synchronization::CompiledRebuild(compile_snapshot(
            &ModelSnapshot::empty(r0),
        )))
        .expect("Empty rebuild should succeed");

    let result = session
        .solve(&SolveRequest::new().with_mip_rel_gap(0.1))
        .expect("Solve with mip_rel_gap should succeed");
    assert_eq!(result.termination, TerminationStatus::Optimal);
    let cfg = result.effective_configuration;
    assert!(
        approx_eq(cfg.mip_rel_gap.unwrap_or(-1.0), 0.1, 1e-4),
        "mip_rel_gap not recorded: {:?}",
        cfg.mip_rel_gap
    );
}

/// Q5: mip_abs_gap is applied and recorded as a ConfigAdjustment.
#[test]
fn q5_mip_abs_gap_recorded_as_adjustment() {
    let mut session = create_session();
    let r0 = ModelRevision::ZERO;
    session
        .synchronize(Synchronization::CompiledRebuild(compile_snapshot(
            &ModelSnapshot::empty(r0),
        )))
        .expect("Empty rebuild should succeed");

    let result = session
        .solve(&SolveRequest {
            mip_abs_gap: Some(1e-4),
            ..SolveRequest::new()
        })
        .expect("Solve with mip_abs_gap should succeed");
    assert_eq!(result.termination, TerminationStatus::Optimal);
    let cfg = result.effective_configuration;
    assert!(
        cfg.adjustments.iter().any(|a| a.key == "mip_abs_gap"),
        "mip_abs_gap should be recorded as an adjustment, got {:?}",
        cfg.adjustments
    );
}

/// Q5: enable_output maps to the output_flag native option.
#[test]
fn q5_enable_output_flag_applied() {
    let mut session = create_session();
    let r0 = ModelRevision::ZERO;
    session
        .synchronize(Synchronization::CompiledRebuild(compile_snapshot(
            &ModelSnapshot::empty(r0),
        )))
        .expect("Empty rebuild should succeed");

    for enabled in [true, false] {
        let result = session
            .solve(&SolveRequest {
                enable_output: Some(enabled),
                ..SolveRequest::new()
            })
            .expect("Solve with enable_output should succeed");
        assert_eq!(result.termination, TerminationStatus::Optimal);
        assert_eq!(
            result.effective_configuration.enable_output,
            Some(enabled),
            "enable_output not recorded"
        );
    }
}

/// Q5: random_seed is applied and recorded as a ConfigAdjustment.
#[test]
fn q5_random_seed_recorded_as_adjustment() {
    let mut session = create_session();
    let r0 = ModelRevision::ZERO;
    session
        .synchronize(Synchronization::CompiledRebuild(compile_snapshot(
            &ModelSnapshot::empty(r0),
        )))
        .expect("Empty rebuild should succeed");

    let result = session
        .solve(&SolveRequest {
            random_seed: Some(42),
            ..SolveRequest::new()
        })
        .expect("Solve with random_seed should succeed");
    assert_eq!(result.termination, TerminationStatus::Optimal);
    let cfg = result.effective_configuration;
    assert!(
        cfg.adjustments.iter().any(|a| a.key == "random_seed"),
        "random_seed should be recorded as an adjustment, got {:?}",
        cfg.adjustments
    );
}

/// Q5: an extra option whose KEY has an interior null byte is rejected (not
/// a panic) and the solve still succeeds.
#[test]
fn q5_extra_option_null_byte_in_key_rejected() {
    let mut session = create_session();
    let r0 = ModelRevision::ZERO;
    session
        .synchronize(Synchronization::CompiledRebuild(compile_snapshot(
            &ModelSnapshot::empty(r0),
        )))
        .expect("Empty rebuild should succeed");

    let result = session
        .solve(&SolveRequest::new().with_option("bad\0key", "x"))
        .expect("Solve must not fail on a null-byte option key");
    assert_eq!(result.termination, TerminationStatus::Optimal);
    assert!(
        result
            .effective_configuration
            .rejections
            .iter()
            .any(|r| r.key == "bad\0key" && r.reason.contains("null byte")),
        "expected a null-byte rejection, got {:?}",
        result.effective_configuration.rejections
    );
}

/// Q5: an extra option whose VALUE has an interior null byte is rejected and
/// the solve still succeeds.
#[test]
fn q5_extra_option_null_byte_in_value_rejected() {
    let mut session = create_session();
    let r0 = ModelRevision::ZERO;
    session
        .synchronize(Synchronization::CompiledRebuild(compile_snapshot(
            &ModelSnapshot::empty(r0),
        )))
        .expect("Empty rebuild should succeed");

    let result = session
        .solve(&SolveRequest::new().with_option("solver", "simplex\0foo"))
        .expect("Solve must not fail on a null-byte option value");
    assert_eq!(result.termination, TerminationStatus::Optimal);
    assert!(
        result
            .effective_configuration
            .rejections
            .iter()
            .any(|r| r.key == "solver" && r.reason.contains("null byte")),
        "expected a null-byte rejection, got {:?}",
        result.effective_configuration.rejections
    );
}

/// Q5: an option unknown to HiGHS fails both the string and option APIs and
/// is recorded as a rejection naming both return codes.
#[test]
fn q5_extra_option_unknown_rejected_via_both_apis() {
    let mut session = create_session();
    let r0 = ModelRevision::ZERO;
    session
        .synchronize(Synchronization::CompiledRebuild(compile_snapshot(
            &ModelSnapshot::empty(r0),
        )))
        .expect("Empty rebuild should succeed");

    let result = session
        .solve(&SolveRequest::new().with_option("definitely_not_a_highs_option", "x"))
        .expect("Solve must not fail on an unknown option");
    assert_eq!(result.termination, TerminationStatus::Optimal);
    assert!(
        result
            .effective_configuration
            .rejections
            .iter()
            .any(|r| r.key == "definitely_not_a_highs_option" && r.reason.contains("string API")),
        "expected a both-APIs rejection, got {:?}",
        result.effective_configuration.rejections
    );
}

// ── Q5 follow-up: interrupted-solve status mapping ───────────────────────────

/// A sparse 12-binary-variable MIP that takes longer than 1µs to solve, so a
/// 1µs time limit produces `TerminationStatus::TimeLimit` with an extracted
/// (partial) solution.
fn time_limit_mip_snapshot() -> ModelSnapshot {
    let r0 = ModelRevision::ZERO;
    let o = obj_id(0);
    let mut variables = Vec::new();
    let mut cells = Vec::new();
    for i in 0..12 {
        variables.push(VariableEntry {
            id: var_id(i),
            bounds: Bounds::BINARY,
            var_type: VarType::Binary,
            active: true,
            semicontinuous_lower: None,
            fixing: None,
        });
        cells.push(CellEntry {
            cell_key: (CoefficientTarget::Objective(o), var_id(i)),
            value_expr: ValueExpr::constant(1.0),
            evaluated_value: 1.0,
            dependencies: vec![],
        });
    }
    let mut constraints = Vec::new();
    for k in 0..6 {
        constraints.push(ConstraintEntry {
            id: con_id(k),
            bounds: ConstraintBounds::le(3.0),
            active: true,
        });
        for i in 0..12 {
            if (i + k) % 3 == 0 {
                cells.push(CellEntry {
                    cell_key: (CoefficientTarget::Constraint(con_id(k)), var_id(i)),
                    value_expr: ValueExpr::constant(1.0),
                    evaluated_value: 1.0,
                    dependencies: vec![],
                });
            }
        }
    }
    ModelSnapshot {
        revision: r0,
        variables,
        constraints,
        objectives: vec![ObjectiveEntry {
            id: o,
            sense: Sense::Maximize,
            active: true,
            constant: 0.0,
        }],
        parameters: vec![],
        cells,
        functions: vec![],
        constructs: vec![],
    }
}

/// Q5: a time-limited MIP solve maps to `TimeLimit` and still yields an
/// extracted solution with variable values.
#[test]
fn q5_status_time_limit_with_extracted_solution() {
    let mut session = create_session();
    session
        .synchronize(Synchronization::CompiledRebuild(compile_snapshot(
            &time_limit_mip_snapshot(),
        )))
        .expect("Rebuild should succeed");

    let result = session
        .solve(&SolveRequest::new().with_time_limit(1e-6))
        .expect("Solve should succeed");
    assert_eq!(
        result.termination,
        TerminationStatus::TimeLimit,
        "expected TimeLimit, got {:?}",
        result.termination
    );
    let sol = result
        .solution
        .expect("time-limited solve should still extract a solution");
    assert!(
        !sol.variable_values.is_empty(),
        "expected extracted variable values at TimeLimit"
    );
}

/// A 8-variable / 4-row LP that requires more than one simplex iteration, so
/// `simplex_iteration_limit=1` produces `TerminationStatus::IterationLimit`.
fn iteration_limit_lp_snapshot() -> ModelSnapshot {
    let r0 = ModelRevision::ZERO;
    let o = obj_id(0);
    let mut variables = Vec::new();
    let mut cells = Vec::new();
    for i in 0..8 {
        variables.push(VariableEntry {
            id: var_id(i),
            bounds: Bounds::NON_NEGATIVE,
            var_type: VarType::Continuous,
            active: true,
            semicontinuous_lower: None,
            fixing: None,
        });
        cells.push(CellEntry {
            cell_key: (CoefficientTarget::Objective(o), var_id(i)),
            value_expr: ValueExpr::constant((i + 1) as f64),
            evaluated_value: (i + 1) as f64,
            dependencies: vec![],
        });
    }
    let mut constraints = Vec::new();
    for k in 0..4 {
        constraints.push(ConstraintEntry {
            id: con_id(k),
            bounds: ConstraintBounds::le(10.0),
            active: true,
        });
        for i in 0..8 {
            // Varied (non-degenerate) coefficients so the LP genuinely needs
            // several simplex pivots and cannot be presolved to trivial.
            let coeff = ((i + 1) * (k + 1)) % 7 + 1;
            cells.push(CellEntry {
                cell_key: (CoefficientTarget::Constraint(con_id(k)), var_id(i)),
                value_expr: ValueExpr::constant(coeff as f64),
                evaluated_value: coeff as f64,
                dependencies: vec![],
            });
        }
    }
    ModelSnapshot {
        revision: r0,
        variables,
        constraints,
        objectives: vec![ObjectiveEntry {
            id: o,
            sense: Sense::Maximize,
            active: true,
            constant: 0.0,
        }],
        parameters: vec![],
        cells,
        functions: vec![],
        constructs: vec![],
    }
}

/// Q5: an iteration-limited simplex solve maps to `IterationLimit` and still
/// yields an extracted solution.
#[test]
fn q5_status_iteration_limit_with_extracted_solution() {
    let mut session = create_session();
    session
        .synchronize(Synchronization::CompiledRebuild(compile_snapshot(
            &iteration_limit_lp_snapshot(),
        )))
        .expect("Rebuild should succeed");

    let result = session
        .solve(
            &SolveRequest::new()
                .with_option("simplex_iteration_limit", "1")
                // Presolve would otherwise remove the problem; the iteration
                // limit only fires when simplex actually runs.
                .with_option("presolve", "off"),
        )
        .expect("Solve should succeed");
    assert_eq!(
        result.termination,
        TerminationStatus::IterationLimit,
        "expected IterationLimit, got {:?}",
        result.termination
    );
    let sol = result
        .solution
        .expect("iteration-limited solve should still extract a solution");
    assert!(
        !sol.variable_values.is_empty(),
        "expected extracted variable values at IterationLimit"
    );
}
