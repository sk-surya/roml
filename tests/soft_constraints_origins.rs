//! P30 provenance and capability tests for persistent soft constraints.

use roml::advanced::{
    BackendCapabilitySet, BackendFeature, CompilationPolicy, CompilationSession, CompileError,
    EntityOrigin, FeatureSupport, GeneratedRole,
};
use roml::{continuous, ConstraintExprExt, Model, PenaltyPolicy, ViolationPolicy};

fn caps() -> BackendCapabilitySet {
    let mut result = BackendCapabilitySet::new();
    result.set(
        BackendFeature::Lp,
        FeatureSupport::native(Default::default()),
    );
    result.set(
        BackendFeature::SoftConstraint,
        FeatureSupport::bridge(Default::default()),
    );
    result
}

#[test]
fn unsupported_native_required_policy_is_typed_before_rows() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    let con = model.add_constraint((x).le(3.0)).unwrap();
    let _soft = model
        .soften_constraint(con, ViolationPolicy::default(), PenaltyPolicy::default())
        .unwrap();
    model.commit().unwrap();

    let snapshot = model.take_snapshot().unwrap();
    let mut session = CompilationSession::new();
    let error = session
        .compile_snapshot(
            model.instance(),
            &snapshot,
            &CompilationPolicy::NativeRequired,
            &caps(),
        )
        .expect_err("portable-only softening must not claim native support");
    assert!(matches!(error, CompileError::UnsupportedFeature(_)));
}

#[test]
fn both_sides_have_distinct_stable_origins() {
    let mut model = Model::new();
    let x = model
        .add_variable(continuous().bounds(-10.0, 10.0))
        .unwrap();
    let con = model.add_constraint((x).between(-2.0, 2.0)).unwrap();
    let soft = model
        .soften_constraint(con, ViolationPolicy::default(), PenaltyPolicy::default())
        .unwrap();
    model.commit().unwrap();

    let snapshot = model.take_snapshot().unwrap();
    let mut session = CompilationSession::new();
    let compiled = session
        .compile_snapshot(
            model.instance(),
            &snapshot,
            &CompilationPolicy::Portable,
            &caps(),
        )
        .unwrap();
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
