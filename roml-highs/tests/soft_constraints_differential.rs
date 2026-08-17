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

#[test]
fn real_highs_relaxes_both_sides_of_ranged_row_when_lower_violation_is_required() {
    let mut model = Model::new();
    let x = model
        .add_variable(continuous().bounds(-10.0, 10.0))
        .unwrap();
    let ranged = model.add_constraint(x.between(0.0, 5.0)).unwrap();
    model.add_constraint(x.le(-1.0)).unwrap();
    let plan = FeasibilityRelaxationPlan {
        scope: RelaxationScope::Explicit(vec![
            RelaxationRestriction::ConstraintSide {
                constraint: ranged,
                side: roml::BoundSide::Lower,
            },
            RelaxationRestriction::ConstraintSide {
                constraint: ranged,
                side: roml::BoundSide::Upper,
            },
        ]),
        ..Default::default()
    };

    let report = SolverSession::new(HighsSession::try_new().expect("HiGHS should be available"))
        .solve_feasibility_relaxation(&mut model, plan)
        .expect("both sides of a ranged row must be relaxable together");

    assert_eq!(report.outcome, RelaxationOutcome::OptimalRepair);
    assert!((report.solution.values()[&x] + 1.0).abs() < 1e-7);
    let lower = report
        .members
        .iter()
        .find(|member| {
            member.restriction
                == RelaxationRestriction::ConstraintSide {
                    constraint: ranged,
                    side: roml::BoundSide::Lower,
                }
        })
        .expect("lower ranged-row member");
    let upper = report
        .members
        .iter()
        .find(|member| {
            member.restriction
                == RelaxationRestriction::ConstraintSide {
                    constraint: ranged,
                    side: roml::BoundSide::Upper,
                }
        })
        .expect("upper ranged-row member");
    assert!((lower.violation - 1.0).abs() < 1e-7);
    assert!(upper.violation.abs() < 1e-7);
    assert!((report.total_weighted_violation - 1.0).abs() < 1e-7);
    assert!((report.metadata.numerics.objective_value.unwrap() - 1.0).abs() < 1e-7);
}

#[test]
fn real_highs_relaxes_both_variable_bounds_when_lower_violation_is_required() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    model.add_constraint(x.le(-1.0)).unwrap();
    let plan = FeasibilityRelaxationPlan {
        scope: RelaxationScope::Explicit(vec![
            RelaxationRestriction::VariableBound {
                variable: x,
                side: roml::BoundSide::Lower,
            },
            RelaxationRestriction::VariableBound {
                variable: x,
                side: roml::BoundSide::Upper,
            },
        ]),
        ..Default::default()
    };

    let report = SolverSession::new(HighsSession::try_new().expect("HiGHS should be available"))
        .solve_feasibility_relaxation(&mut model, plan)
        .expect("both variable bounds must be relaxable together");

    assert_eq!(report.outcome, RelaxationOutcome::OptimalRepair);
    assert!((report.solution.values()[&x] + 1.0).abs() < 1e-7);
    let lower = report
        .members
        .iter()
        .find(|member| {
            member.restriction
                == RelaxationRestriction::VariableBound {
                    variable: x,
                    side: roml::BoundSide::Lower,
                }
        })
        .expect("lower variable-bound member");
    let upper = report
        .members
        .iter()
        .find(|member| {
            member.restriction
                == RelaxationRestriction::VariableBound {
                    variable: x,
                    side: roml::BoundSide::Upper,
                }
        })
        .expect("upper variable-bound member");
    assert!((lower.violation - 1.0).abs() < 1e-7);
    assert!(upper.violation.abs() < 1e-7);
    assert!((report.total_weighted_violation - 1.0).abs() < 1e-7);
    assert!((report.metadata.numerics.objective_value.unwrap() - 1.0).abs() < 1e-7);
}

#[test]
fn real_highs_relaxes_persistent_fixing_without_relaxing_declared_bounds() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    model.fix(x, 5.0).unwrap();
    model.add_constraint(x.le(0.0)).unwrap();
    let plan = FeasibilityRelaxationPlan {
        scope: RelaxationScope::Explicit(vec![RelaxationRestriction::PersistentFixing {
            variable: x,
        }]),
        ..Default::default()
    };

    let report = SolverSession::new(HighsSession::try_new().expect("HiGHS should be available"))
        .solve_feasibility_relaxation(&mut model, plan)
        .expect("persistent fixing must be independently relaxable");

    assert_eq!(report.outcome, RelaxationOutcome::OptimalRepair);
    assert!((report.solution.values()[&x]).abs() < 1e-7);
    assert_eq!(report.members.len(), 1);
    assert_eq!(
        report.members[0].restriction,
        RelaxationRestriction::PersistentFixing { variable: x }
    );
    assert!((report.members[0].violation - 5.0).abs() < 1e-7);
    assert!((report.total_weighted_violation - 5.0).abs() < 1e-7);
    assert!((report.metadata.numerics.objective_value.unwrap() - 5.0).abs() < 1e-7);
    assert_eq!(
        model.declared_bounds(x).unwrap(),
        roml::Bounds::new(0.0, 10.0)
    );
}
