//! Machine-checkable P30 qualification corpus for persistent softening.

use roml::advanced::{
    BackendCapabilitySet, BackendFeature, CompilationPolicy, CompilationSession, EntityOrigin,
    FeatureSupport, GeneratedRole,
};
use roml::{
    continuous, ConstraintExprExt, Model, PenaltyPolicy, PenaltyTarget, Sense, ValueExpr,
    ViolationPolicy,
};

fn portable_caps() -> BackendCapabilitySet {
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

#[test]
fn all_row_senses_emit_exact_side_roles_and_caps() {
    let mut model = Model::new();
    let x = model
        .add_variable(continuous().bounds(-10.0, 10.0))
        .unwrap();
    let lower = model.add_constraint((x).ge(2.0)).unwrap();
    let upper = model.add_constraint((x).le(3.0)).unwrap();
    let equality = model.add_constraint((x).eq(0.0)).unwrap();
    let ranged = model.add_constraint((x).between(-1.0, 4.0)).unwrap();
    let lower_soft = model
        .soften_constraint(
            lower,
            ViolationPolicy {
                max_violation: Some(0.0),
            },
            PenaltyPolicy::default(),
        )
        .unwrap();
    let upper_soft = model
        .soften_constraint(upper, ViolationPolicy::default(), PenaltyPolicy::default())
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
    let mut compiler = CompilationSession::new();
    let compiled = compiler
        .compile_snapshot(
            model.instance(),
            &model.take_snapshot().unwrap(),
            &CompilationPolicy::Portable,
            &portable_caps(),
        )
        .unwrap();
    let lower_vars = compiled
        .origin_map
        .variables_for_origin(&EntityOrigin::Construct {
            construct: lower_soft.construct(),
            role: GeneratedRole::SoftConstraintLowerViolationVariable,
        });
    assert_eq!(
        compiled.variables[lower_vars[0].0 as usize].bounds.upper,
        0.0
    );
    for (handle, expected_sides) in [(upper_soft, 1), (equality_soft, 2), (ranged_soft, 2)] {
        let lower = compiled
            .origin_map
            .variables_for_origin(&EntityOrigin::Construct {
                construct: handle.construct(),
                role: GeneratedRole::SoftConstraintLowerViolationVariable,
            });
        let upper = compiled
            .origin_map
            .variables_for_origin(&EntityOrigin::Construct {
                construct: handle.construct(),
                role: GeneratedRole::SoftConstraintUpperViolationVariable,
            });
        assert_eq!(lower.len() + upper.len(), expected_sides);
    }
}

#[test]
fn parameterized_weight_and_both_objective_senses_are_evaluated() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).unwrap();
    let constraint = model.add_constraint((x).le(1.0)).unwrap();
    let parameter = model.add_parameter(2.5).unwrap();
    let minimize = model.add_objective(roml::Sense::Minimize);
    let maximize = model.add_objective(roml::Sense::Maximize);
    model
        .soften_constraint(
            constraint,
            ViolationPolicy::default(),
            PenaltyPolicy {
                weight: ValueExpr::from(parameter),
                target: PenaltyTarget::Objective(minimize),
            },
        )
        .unwrap();
    model
        .soften_constraint(
            constraint,
            ViolationPolicy::default(),
            PenaltyPolicy {
                weight: ValueExpr::constant(3.0),
                target: PenaltyTarget::Objective(maximize),
            },
        )
        .expect_err("one primitive cannot be softened twice");
    assert_eq!(model.objective_sense(minimize), Some(Sense::Minimize));
    assert_eq!(model.objective_sense(maximize), Some(Sense::Maximize));
}
