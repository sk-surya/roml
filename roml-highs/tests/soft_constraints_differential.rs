//! Standard compiled-row HiGHS qualification for P30 soft constraints.

use roml::advanced::{
    BackendCapabilitySet, BackendFeature, CompilationPolicy, CompilationSession, FeatureSupport,
};
use roml::{continuous, ConstraintExprExt, Model, PenaltyPolicy, ViolationPolicy};

#[test]
fn compiled_portable_soft_constraint_is_accepted_by_highs_session() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    let con = model.add_constraint((x).le(3.0)).unwrap();
    model
        .soften_constraint(con, ViolationPolicy::default(), PenaltyPolicy::default())
        .unwrap();
    model.commit().unwrap();

    let mut caps = BackendCapabilitySet::new();
    caps.set(
        BackendFeature::Lp,
        FeatureSupport::native(Default::default()),
    );
    caps.set(
        BackendFeature::SoftConstraint,
        FeatureSupport::bridge(Default::default()),
    );
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
}
