//! P30 exact portable persistent soft-constraint algebra tests.

mod support {
    pub mod soft_constraints_reference;
}

use roml::advanced::{
    BackendCapabilitySet, BackendFeature, CompilationPolicy, CompilationSession, EntityOrigin,
    FeatureSupport, GeneratedRole,
};
use roml::{continuous, ConstraintExprExt, Model, PenaltyPolicy, ViolationPolicy};
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
