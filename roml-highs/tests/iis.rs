//! Phase 29 bundled HiGHS IIS qualification smoke tests.

use roml::prelude::*;
use roml::{
    ConflictGuarantee, InfeasibilityMode, InfeasibilityOutcome, InfeasibilityPlan,
    InfeasibilityScope, SolveStatus, SolverSession,
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
