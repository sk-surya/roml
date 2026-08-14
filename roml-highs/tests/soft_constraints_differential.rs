//! Standard compiled-row HiGHS qualification for P30 soft constraints.

use roml::advanced::{BackendFeature, CompilationPolicy, CompilationSession};
use roml::solver::session::{BackendSession, Synchronization};
use roml::{
    continuous, ConstraintExprExt, FeasibilityRelaxationPlan, Model, PenaltyPolicy,
    RelaxationOutcome, RelaxationRestriction, RelaxationScope, SolverSession, ViolationPolicy,
};
use roml_highs::{highs_capability_set, HighsSession};

#[test]
fn highs_capability_set_qualifies_p30_bridge_surfaces() {
    let capabilities = highs_capability_set(1, 15, 0);

    assert!(capabilities.is_bridge(BackendFeature::SoftConstraint));
    assert!(capabilities.is_bridge(BackendFeature::FeasibilityRelaxation));
    assert!(!capabilities.supports(BackendFeature::SoftConstraint));
    assert!(!capabilities.supports(BackendFeature::FeasibilityRelaxation));
}

#[test]
fn compiled_portable_soft_constraint_is_accepted_by_highs_session() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    let con = model.add_constraint((x).le(3.0)).unwrap();
    model
        .soften_constraint(con, ViolationPolicy::default(), PenaltyPolicy::default())
        .unwrap();
    model.commit().unwrap();

    let caps = highs_capability_set(1, 15, 0);
    let mut compiler = CompilationSession::new();
    let compiled = compiler
        .compile_snapshot(
            model.instance(),
            &model.take_snapshot().unwrap(),
            &CompilationPolicy::Portable,
            &caps,
        )
        .expect("standard compiled-row projection must compile");
    assert_eq!(compiled.linear_rows.len(), 2);

    let mut highs = HighsSession::try_new().expect("HiGHS should be available");
    highs
        .synchronize(Synchronization::CompiledRebuild(compiled))
        .expect("a real HiGHS session must accept the qualified P30 bridge");
}

#[test]
fn real_highs_session_runs_portable_repair_and_rolls_back_overlay() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 1.0)).unwrap();
    let y = model.add_variable(continuous().bounds(0.0, 2.0)).unwrap();
    let x_constraint = model.add_constraint((x).ge(2.0)).unwrap();
    let y_constraint = model.add_constraint((y).ge(3.0)).unwrap();
    let plan = FeasibilityRelaxationPlan {
        scope: RelaxationScope::Explicit(vec![
            RelaxationRestriction::ConstraintSide {
                constraint: x_constraint,
                side: roml::BoundSide::Lower,
            },
            RelaxationRestriction::ConstraintSide {
                constraint: y_constraint,
                side: roml::BoundSide::Lower,
            },
        ]),
        ..Default::default()
    };
    let mut session =
        SolverSession::new(HighsSession::try_new().expect("HiGHS should be available"));

    let first = session
        .solve_feasibility_relaxation(&mut model, plan.clone())
        .expect("portable repair should execute through HighS");
    assert_eq!(first.outcome, RelaxationOutcome::OptimalRepair);
    assert_eq!(first.members.len(), 2);
    assert!(first
        .members
        .iter()
        .all(|member| (member.violation - 1.0).abs() < 1e-7));
    assert!((first.total_weighted_violation - 2.0).abs() < 1e-7);
    assert!((first.metadata.numerics.objective_value.unwrap() - 2.0).abs() < 1e-7);

    let second = session
        .solve_feasibility_relaxation(&mut model, plan)
        .expect("a rolled-back overlay must not contaminate the next repair");
    assert_eq!(second.outcome, RelaxationOutcome::OptimalRepair);
    assert!((second.total_weighted_violation - 2.0).abs() < 1e-7);
}
