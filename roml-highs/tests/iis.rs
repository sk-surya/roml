//! Phase 29 bundled HiGHS IIS qualification smoke tests.

use std::collections::BTreeMap;

use roml::prelude::*;
use roml::{
    ConflictGuarantee, ContinuousLock, InfeasibilityMode, InfeasibilityOutcome, InfeasibilityPlan,
    InfeasibilityScope, LockSelector, MinMaxRelation, MinMaxSense, PrimalAssignment, SolutionLock,
    SolveStatus, SolverSession,
};
use roml_highs::HighsSession;

fn contradictory_lp() -> Model {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).expect("variable");
    model.add_constraint(x.ge(1.0)).expect("lower row");
    model.add_constraint(x.le(0.0)).expect("upper row");
    model
}

#[test]
fn portable_reducer_reports_verified_semantic_conflict() {
    let mut model = contradictory_lp();
    let mut session = SolverSession::new(HighsSession::try_new().expect("bundled HiGHS"));
    let plan = InfeasibilityPlan::portable_lp();
    let report = session
        .analyze_infeasibility(&model, &plan)
        .expect("portable IIS analysis");

    assert_eq!(report.outcome, InfeasibilityOutcome::Conflict);
    assert_eq!(report.guarantee, ConflictGuarantee::Irreducible);
    assert_eq!(report.members.len(), 2);
    assert!(report
        .provider_chain
        .iter()
        .any(|provider| provider.name == "ROML semantic reducer"));

    // The analysis session is isolated; the persistent solve session remains
    // usable after the report is produced.
    let solution = session.solve(&mut model).expect("persistent solve");
    assert!(matches!(solution.status(), SolveStatus::Infeasible));
}

#[test]
fn default_plan_keeps_an_exact_constraint_as_one_semantic_atom() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).expect("variable");
    model.add_constraint(x.eq(5.0)).expect("equality");
    model.add_constraint(x.le(4.0)).expect("contradiction");

    let mut session = SolverSession::new(HighsSession::try_new().expect("bundled HiGHS"));
    let report = session
        .analyze_infeasibility(&model, &InfeasibilityPlan::portable_lp())
        .expect("semantic IIS analysis");
    assert_eq!(
        report.candidate_universe.grouping,
        roml::ConflictGrouping::Semantic
    );
    assert_eq!(report.members.len(), 2);
    assert!(report.members.iter().any(|member| {
        matches!(
            member.declaration.origin,
            roml::advanced::ConflictOrigin::ConstraintEquality { .. }
        )
    }));
}

#[test]
fn auto_records_native_seed_and_semantic_reduction() {
    let model = contradictory_lp();
    let mut session = SolverSession::new(HighsSession::try_new().expect("bundled HiGHS"));
    let mut plan = InfeasibilityPlan::portable_lp();
    plan.mode = InfeasibilityMode::Auto;
    let report = session
        .analyze_infeasibility(&model, &plan)
        .expect("auto IIS analysis");

    assert_eq!(report.outcome, InfeasibilityOutcome::Conflict);
    assert!(report
        .provider_chain
        .iter()
        .any(|provider| provider.name == "ROML semantic reducer"));
    assert!(report
        .provider_chain
        .iter()
        .any(|provider| provider.name == "fresh semantic verifier"));
}

#[test]
fn full_universe_seed_policy_does_not_invoke_native_seeding() {
    let model = contradictory_lp();
    let mut session = SolverSession::new(HighsSession::try_new().expect("bundled HiGHS"));
    let mut plan = InfeasibilityPlan::portable_lp();
    plan.mode = InfeasibilityMode::Auto;
    plan.seed_policy = roml::advanced::SeedPolicy::FullUniverse;
    let report = session
        .analyze_infeasibility(&model, &plan)
        .expect("full-universe analysis");

    assert_eq!(report.outcome, InfeasibilityOutcome::Conflict);
    assert!(!report
        .provider_chain
        .iter()
        .any(|provider| provider.name.contains("native IIS")));
}

#[test]
fn native_then_roml_honors_native_mode_with_a_full_universe_seed_policy() {
    let model = contradictory_lp();
    let mut session = SolverSession::new(HighsSession::try_new().expect("bundled HiGHS"));
    let mut plan = InfeasibilityPlan::portable_lp();
    plan.mode = InfeasibilityMode::NativeThenRoml;
    plan.seed_policy = roml::advanced::SeedPolicy::FullUniverse;
    let report = session
        .analyze_infeasibility(&model, &plan)
        .expect("NativeThenRoml must attempt native seeding");
    assert_eq!(report.outcome, InfeasibilityOutcome::Conflict);
    assert!(report
        .provider_chain
        .iter()
        .any(|provider| provider.name.contains("native IIS")));
    assert!(report
        .provider_chain
        .iter()
        .any(|provider| provider.name == "ROML semantic reducer"));
}

#[test]
fn persistent_fixing_is_a_semantic_layer_that_can_be_reduced() {
    let mut model = Model::new();
    let x = model
        .add_variable(continuous().bounds(0.0, 10.0))
        .expect("variable");
    model.fix(x, 1.0).expect("persistent fixing");
    model.add_constraint(x.le(0.0)).expect("upper row");

    let mut session = SolverSession::new(HighsSession::try_new().expect("bundled HiGHS"));
    let report = session
        .analyze_infeasibility(&model, &InfeasibilityPlan::portable_lp())
        .expect("fixing IIS analysis");
    assert_eq!(report.guarantee, ConflictGuarantee::Irreducible);
    assert!(report.members.iter().any(|member| {
        matches!(
            member.declaration.origin,
            roml::advanced::ConflictOrigin::PersistentFixing { .. }
        )
    }));
}

#[test]
fn explicit_mip_relaxation_is_an_lp_analysis_scope() {
    let mut model = Model::new();
    let x = model
        .add_variable(integer().bounds(0.0, 1.0))
        .expect("integer variable");
    model.add_constraint(x.ge(2.0)).expect("contradictory row");

    let mut session = SolverSession::new(HighsSession::try_new().expect("bundled HiGHS"));
    let mut plan = InfeasibilityPlan::portable_lp();
    plan.scope = InfeasibilityScope::LpRelaxation;
    let report = session
        .analyze_infeasibility(&model, &plan)
        .expect("explicit LP relaxation analysis");

    assert_eq!(report.scope, InfeasibilityScope::LpRelaxation);
    assert_eq!(report.outcome, InfeasibilityOutcome::Conflict);
}

#[test]
fn original_lp_rejects_generated_integer_columns() {
    let mut model = Model::new();
    let x = model
        .add_variable(continuous().bounds(0.0, 1.0))
        .expect("first operand");
    let y = model
        .add_variable(continuous().bounds(0.0, 1.0))
        .expect("second operand");
    model
        .add_minmax(
            vec![x.into(), y.into()],
            MinMaxSense::Max,
            MinMaxRelation::Exact,
            None,
        )
        .expect("exact min/max construct");

    let mut session = SolverSession::new(HighsSession::try_new().expect("bundled HiGHS"));
    let error = session
        .analyze_infeasibility(&model, &InfeasibilityPlan::portable_lp())
        .expect_err("OriginalLp must inspect compiled variable types");
    assert!(matches!(
        error,
        roml::advanced::InfeasibilityError::Unsupported { .. }
    ));
}

#[test]
fn portable_oracle_neutralizes_an_unbounded_objective() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).expect("variable");
    model.maximize(x).expect("objective");

    let mut session = SolverSession::new(HighsSession::try_new().expect("bundled HiGHS"));
    let report = session
        .analyze_infeasibility(&model, &InfeasibilityPlan::portable_lp())
        .expect("feasibility analysis");

    assert_eq!(report.outcome, InfeasibilityOutcome::NoConflict);
}

#[test]
fn oracle_call_budget_includes_the_authoritative_full_universe_check() {
    for limit in [0, 1, 2] {
        let model = contradictory_lp();
        let mut session = SolverSession::new(HighsSession::try_new().expect("bundled HiGHS"));
        let mut plan = InfeasibilityPlan::portable_lp();
        plan.budget.max_oracle_calls = Some(limit);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            session.analyze_infeasibility(&model, &plan)
        }));
        let analysis = result
            .expect("oracle-call budgeting must never panic")
            .expect("budgeted analysis should return a typed report");
        assert!(analysis.statistics.oracle_calls <= limit);
    }
}

#[test]
fn solve_overlay_fixing_is_an_analyzable_semantic_layer() {
    let mut model = Model::new();
    let x = model
        .add_variable(continuous().bounds(0.0, 10.0))
        .expect("variable");
    model.add_constraint(x.le(0.0)).expect("constraint");
    let mut fixings = BTreeMap::new();
    fixings.insert(x, 1.0);
    let overlay =
        roml::SolveOverlay::new(fixings, Vec::new(), Vec::new(), Vec::new()).expect("overlay");

    let mut plan = InfeasibilityPlan::portable_lp();
    plan.overlay = Some(overlay);
    let mut session = SolverSession::new(HighsSession::try_new().expect("bundled HiGHS"));
    let report = session
        .analyze_infeasibility(&model, &plan)
        .expect("overlay IIS analysis");
    assert!(report.members.iter().any(|member| {
        matches!(
            member.declaration.origin,
            roml::advanced::ConflictOrigin::TemporaryFixing { .. }
        )
    }));
}

#[test]
fn solve_overlay_lock_is_an_analyzable_semantic_layer() {
    let mut model = Model::new();
    let x = model
        .add_variable(continuous().bounds(0.0, 10.0))
        .expect("variable");
    model.add_constraint(x.le(0.0)).expect("constraint");
    let assignment = PrimalAssignment {
        lineage: model.lineage(),
        source_instance: Some(model.instance()),
        source_revision: Some(model.current_revision()),
        values: BTreeMap::from([(x, 1.0)]),
    };
    let lock = SolutionLock {
        assignment,
        selector: LockSelector::AllAssigned,
        continuous: ContinuousLock::Exact,
    };
    let overlay = roml::SolveOverlay::new(BTreeMap::new(), vec![lock], Vec::new(), Vec::new())
        .expect("overlay");

    let mut plan = InfeasibilityPlan::portable_lp();
    plan.overlay = Some(overlay);
    let mut session = SolverSession::new(HighsSession::try_new().expect("bundled HiGHS"));
    let report = session
        .analyze_infeasibility(&model, &plan)
        .expect("overlay lock IIS analysis");
    assert!(report.members.iter().any(|member| {
        matches!(
            member.declaration.origin,
            roml::advanced::ConflictOrigin::SolveLock { .. }
        )
    }));
}
