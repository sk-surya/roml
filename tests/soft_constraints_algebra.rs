//! P30 exact portable persistent soft-constraint algebra tests.

mod support {
    pub mod soft_constraints_reference;
}

use roml::advanced::{
    BackendCapabilitySet, BackendFeature, CompilationPolicy, CompilationSession, CompileError,
    EntityOrigin, FeatureSupport, GeneratedRole,
};
use roml::{continuous, Bounds, ConstraintExprExt, Model, PenaltyPolicy, ViolationPolicy};
use roml::{ConstraintBounds, PenaltyTarget, Sense, ValueExpr};
use support::soft_constraints_reference::{raw_violation, sides};

fn bridge_caps() -> BackendCapabilitySet {
    let mut caps = BackendCapabilitySet::new();
    caps.set(
        BackendFeature::Lp,
        FeatureSupport::native(Default::default()),
    );
    caps.set(
        BackendFeature::SoftConstraint,
        FeatureSupport::bridge(Default::default()),
    );
    caps
}

fn bridge_caps_with_incremental_updates() -> BackendCapabilitySet {
    let mut caps = bridge_caps();
    caps.set(
        BackendFeature::IncrementalBounds,
        FeatureSupport::native(Default::default()),
    );
    caps.set(
        BackendFeature::IncrementalCoefficients,
        FeatureSupport::native(Default::default()),
    );
    caps
}

#[test]
fn softening_widens_the_original_side_so_positive_violation_is_feasible() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    let constraint = model.add_constraint(x.le(3.0)).unwrap();
    let soft = model
        .soften_constraint(
            constraint,
            ViolationPolicy {
                max_violation: Some(10.0),
            },
            PenaltyPolicy::default(),
        )
        .unwrap();
    model.commit().unwrap();

    let snapshot = model.take_snapshot().unwrap();
    let mut session = CompilationSession::new();
    let compiled = session
        .compile_snapshot(
            model.instance(),
            &snapshot,
            &CompilationPolicy::Portable,
            &bridge_caps(),
        )
        .unwrap();

    let original_row = compiled.linear_rows[compiled
        .origin_map
        .constraints_for_origin(&EntityOrigin::UserConstraint(constraint))[0]
        .0 as usize]
        .clone();
    assert_eq!(original_row.bounds.upper, f64::INFINITY);

    let generated_row =
        compiled.linear_rows[compiled
            .origin_map
            .constraints_for_origin(&EntityOrigin::Construct {
                construct: soft.construct(),
                role: GeneratedRole::SoftConstraintUpperViolationRow,
            })[0]
            .0 as usize]
            .clone();
    assert_eq!(generated_row.bounds.upper, 3.0);
    // x = 5 and v_up = 2 satisfy the transformed upper side: 5 - 2 <= 3.
    assert!(5.0 - 2.0 <= generated_row.bounds.upper);
}

#[test]
fn softening_rebuilds_after_an_original_constraint_coefficient_changes() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    let constraint = model.add_empty_constraint(ConstraintBounds::le(3.0));
    model
        .add_constraint_coefficient(constraint, x, 1.0)
        .unwrap();
    model
        .soften_constraint(
            constraint,
            ViolationPolicy::default(),
            PenaltyPolicy::default(),
        )
        .unwrap();
    model.commit().unwrap();

    let initial = model.take_snapshot().unwrap();
    let mut session = CompilationSession::new();
    let compiled = session
        .compile_snapshot(
            model.instance(),
            &initial,
            &CompilationPolicy::Portable,
            &bridge_caps_with_incremental_updates(),
        )
        .unwrap();

    model
        .add_constraint_coefficient(constraint, x, 1.0)
        .unwrap();
    let revision = model.commit().unwrap();
    let delta = model
        .deltas_since(initial.revision)
        .unwrap()
        .iter()
        .find(|batch| batch.to == revision)
        .copied()
        .expect("coefficient update delta");

    let error = session
        .compile_delta(
            delta,
            compiled.compilation_id,
            model.instance(),
            &CompilationPolicy::Portable,
            &bridge_caps_with_incremental_updates(),
        )
        .expect_err("copied soft rows require a fresh rebuild");
    assert!(matches!(error, CompileError::RebuildRequired(_)));
}

#[test]
fn softening_rebuilds_after_an_original_constraint_parameter_changes() {
    let mut model = Model::new();
    let parameter = model.add_parameter(1.0).unwrap();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    let constraint = model.add_empty_constraint(ConstraintBounds::le(3.0));
    model
        .add_constraint_coefficient(constraint, x, ValueExpr::param(parameter))
        .unwrap();
    model
        .soften_constraint(
            constraint,
            ViolationPolicy::default(),
            PenaltyPolicy::default(),
        )
        .unwrap();
    model.commit().unwrap();

    let initial = model.take_snapshot().unwrap();
    let mut session = CompilationSession::new();
    let compiled = session
        .compile_snapshot(
            model.instance(),
            &initial,
            &CompilationPolicy::Portable,
            &bridge_caps_with_incremental_updates(),
        )
        .unwrap();

    model.set_parameter(parameter, 2.0).unwrap();
    let revision = model.commit().unwrap();
    let delta = model
        .deltas_since(initial.revision)
        .unwrap()
        .iter()
        .find(|batch| batch.to == revision)
        .copied()
        .expect("parameter update delta");

    let error = session
        .compile_delta(
            delta,
            compiled.compilation_id,
            model.instance(),
            &CompilationPolicy::Portable,
            &bridge_caps_with_incremental_updates(),
        )
        .expect_err("parameterized copied soft rows require a fresh rebuild");
    assert!(matches!(error, CompileError::RebuildRequired(_)));
}

#[test]
fn softening_rebuilds_after_a_referenced_variable_lifecycle_change() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    let constraint = model.add_constraint(x.le(3.0)).unwrap();
    model
        .soften_constraint(
            constraint,
            ViolationPolicy::default(),
            PenaltyPolicy::default(),
        )
        .unwrap();
    model.commit().unwrap();

    let initial = model.take_snapshot().unwrap();
    let mut session = CompilationSession::new();
    let compiled = session
        .compile_snapshot(
            model.instance(),
            &initial,
            &CompilationPolicy::Portable,
            &bridge_caps_with_incremental_updates(),
        )
        .unwrap();

    model.set_variable_bounds(x, Bounds::new(0.0, 8.0)).unwrap();
    let revision = model.commit().unwrap();
    let delta = model
        .deltas_since(initial.revision)
        .unwrap()
        .iter()
        .find(|batch| batch.to == revision)
        .copied()
        .expect("variable update delta");

    let error = session
        .compile_delta(
            delta,
            compiled.compilation_id,
            model.instance(),
            &CompilationPolicy::Portable,
            &bridge_caps_with_incremental_updates(),
        )
        .expect_err("referenced-variable changes require a fresh soft-row rebuild");
    assert!(matches!(error, CompileError::RebuildRequired(_)));
}

#[test]
fn upper_side_compiles_to_signed_violation_row_and_reference() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    let constraint = model.add_constraint((x).le(3.0)).unwrap();
    let soft = model
        .soften_constraint(
            constraint,
            ViolationPolicy::default(),
            PenaltyPolicy::default(),
        )
        .unwrap();
    model.commit().unwrap();

    let snapshot = model.take_snapshot().unwrap();
    let mut session = CompilationSession::new();
    let compiled = session
        .compile_snapshot(
            model.instance(),
            &snapshot,
            &CompilationPolicy::Portable,
            &bridge_caps(),
        )
        .expect("upper-side soft constraint compiles");

    let variable_ids = compiled
        .origin_map
        .variables_for_origin(&EntityOrigin::Construct {
            construct: soft.construct(),
            role: GeneratedRole::SoftConstraintUpperViolationVariable,
        });
    let row_ids = compiled
        .origin_map
        .constraints_for_origin(&EntityOrigin::Construct {
            construct: soft.construct(),
            role: GeneratedRole::SoftConstraintUpperViolationRow,
        });
    assert_eq!(variable_ids.len(), 1);
    assert_eq!(row_ids.len(), 1);
    let violation = &compiled.variables[variable_ids[0].0 as usize];
    assert_eq!(violation.bounds.lower, 0.0);
    let row = &compiled.linear_rows[row_ids[0].0 as usize];
    assert_eq!(row.bounds.upper, 3.0);
    assert_eq!(row.coefficients.len(), 2);
    assert!(row
        .coefficients
        .iter()
        .any(|(_, coefficient)| *coefficient == -1.0));

    let reference = sides(snapshot.constraints[0].bounds, None);
    assert_eq!(reference.len(), 1);
    assert_eq!(reference[0].expression_sign, -1.0);
    assert_eq!(
        raw_violation(5.0, snapshot.constraints[0].bounds),
        (0.0, 2.0)
    );
}

#[test]
fn softened_upper_side_widens_original_row_for_positive_violation() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(2.0, 2.0)).unwrap();
    let constraint = model.add_constraint((x).le(1.0)).unwrap();
    let soft = model
        .soften_constraint(
            constraint,
            ViolationPolicy::default(),
            PenaltyPolicy::default(),
        )
        .unwrap();
    model.commit().unwrap();

    let mut session = CompilationSession::new();
    let compiled = session
        .compile_snapshot(
            model.instance(),
            &model.take_snapshot().unwrap(),
            &CompilationPolicy::Portable,
            &bridge_caps(),
        )
        .unwrap();

    let original = &compiled.linear_rows[0];
    assert_eq!(original.bounds.lower, f64::NEG_INFINITY);
    assert_eq!(
        original.bounds.upper,
        f64::INFINITY,
        "the original finite upper side must no longer remain hard"
    );

    let generated_id = compiled
        .origin_map
        .constraints_for_origin(&EntityOrigin::Construct {
            construct: soft.construct(),
            role: GeneratedRole::SoftConstraintUpperViolationRow,
        })[0];
    let generated = &compiled.linear_rows[generated_id.0 as usize];
    assert_eq!(generated.bounds.upper, 1.0);
    assert!(generated
        .coefficients
        .iter()
        .any(|(variable, coefficient)| *coefficient == -1.0
            && compiled.variables[variable.0 as usize].bounds.lower == 0.0));
}

#[test]
fn lower_equality_and_ranged_sides_keep_distinct_roles_and_caps() {
    let mut model = Model::new();
    let x = model
        .add_variable(continuous().bounds(-10.0, 10.0))
        .unwrap();
    let lower = model.add_constraint((x).ge(2.0)).unwrap();
    let equality = model.add_constraint((x).eq(0.0)).unwrap();
    let ranged = model.add_constraint((x).between(-1.0, 3.0)).unwrap();
    let lower_soft = model
        .soften_constraint(
            lower,
            ViolationPolicy {
                max_violation: Some(0.0),
            },
            PenaltyPolicy::default(),
        )
        .unwrap();
    let equality_soft = model
        .soften_constraint(
            equality,
            ViolationPolicy::default(),
            PenaltyPolicy::default(),
        )
        .unwrap();
    let ranged_soft = model
        .soften_constraint(ranged, ViolationPolicy::default(), PenaltyPolicy::default())
        .unwrap();
    model.commit().unwrap();

    let mut session = CompilationSession::new();
    let compiled = session
        .compile_snapshot(
            model.instance(),
            &model.take_snapshot().unwrap(),
            &CompilationPolicy::Portable,
            &bridge_caps(),
        )
        .unwrap();
    let lower_var = compiled
        .origin_map
        .variables_for_origin(&EntityOrigin::Construct {
            construct: lower_soft.construct(),
            role: GeneratedRole::SoftConstraintLowerViolationVariable,
        });
    assert_eq!(lower_var.len(), 1);
    assert_eq!(
        compiled.variables[lower_var[0].0 as usize].bounds.upper,
        0.0
    );

    for soft in [equality_soft, ranged_soft] {
        assert_eq!(
            compiled
                .origin_map
                .variables_for_origin(&EntityOrigin::Construct {
                    construct: soft.construct(),
                    role: GeneratedRole::SoftConstraintLowerViolationVariable,
                })
                .len(),
            1
        );
        assert_eq!(
            compiled
                .origin_map
                .variables_for_origin(&EntityOrigin::Construct {
                    construct: soft.construct(),
                    role: GeneratedRole::SoftConstraintUpperViolationVariable,
                })
                .len(),
            1
        );
    }
    assert_eq!(
        ConstraintBounds::range(-1.0, 3.0),
        model.constraint_bounds(ranged).unwrap()
    );
}

#[test]
fn invalid_caps_are_rejected_atomically_and_weights_are_sense_normalized() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    let con = model.add_constraint((x).le(3.0)).unwrap();
    let before = model.take_snapshot().unwrap();
    for cap in [Some(-1.0), Some(f64::NAN), Some(f64::INFINITY)] {
        assert!(model
            .soften_constraint(
                con,
                ViolationPolicy { max_violation: cap },
                PenaltyPolicy::default()
            )
            .is_err());
        assert_eq!(
            model.take_snapshot().unwrap().constructs.len(),
            before.constructs.len()
        );
    }

    let parameter = model.add_parameter(2.5).unwrap();
    let objective = model.minimize(x).unwrap();
    let soft = model
        .soften_constraint(
            con,
            ViolationPolicy::default(),
            PenaltyPolicy {
                weight: ValueExpr::param(parameter),
                target: PenaltyTarget::Objective(objective),
            },
        )
        .unwrap();
    model.commit().unwrap();
    let mut session = CompilationSession::new();
    let compiled = session
        .compile_snapshot(
            model.instance(),
            &model.take_snapshot().unwrap(),
            &CompilationPolicy::Portable,
            &bridge_caps(),
        )
        .unwrap();
    let violation = compiled
        .origin_map
        .variables_for_origin(&EntityOrigin::Construct {
            construct: soft.construct(),
            role: GeneratedRole::SoftConstraintUpperViolationVariable,
        })[0];
    assert!(compiled.objectives[0]
        .coefficients
        .contains(&(violation, 2.5)));
    assert!(compiled
        .report
        .formulation_decisions
        .iter()
        .any(|decision| decision.decision == "soft_constraint.penalty"
            && decision.selection.contains("2.5")));

    let mut maximizing = Model::new();
    let y = maximizing
        .add_variable(continuous().bounds(0.0, 10.0))
        .unwrap();
    let max_objective = maximizing.maximize(y).unwrap();
    let max_con = maximizing.add_constraint((y).le(3.0)).unwrap();
    let max_soft = maximizing
        .soften_constraint(
            max_con,
            ViolationPolicy::default(),
            PenaltyPolicy {
                weight: ValueExpr::constant(2.5),
                target: PenaltyTarget::Objective(max_objective),
            },
        )
        .unwrap();
    maximizing.commit().unwrap();
    let mut max_session = CompilationSession::new();
    let max_compiled = max_session
        .compile_snapshot(
            maximizing.instance(),
            &maximizing.take_snapshot().unwrap(),
            &CompilationPolicy::Portable,
            &bridge_caps(),
        )
        .unwrap();
    let max_violation = max_compiled
        .origin_map
        .variables_for_origin(&EntityOrigin::Construct {
            construct: max_soft.construct(),
            role: GeneratedRole::SoftConstraintUpperViolationVariable,
        })[0];
    assert!(max_compiled.objectives[0]
        .coefficients
        .contains(&(max_violation, -2.5)));
    assert_eq!(max_compiled.objectives[0].sense, Sense::Maximize);
}
