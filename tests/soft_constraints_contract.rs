//! P30-00 RED contract: persistent softening is a canonical construct.

use roml::construct::{
    ConstructKind, PenaltyPolicy, PenaltyTarget, ViolationPolicy, ViolationSide,
};
use roml::value_expr::ValueExpr;
use roml::{ConstraintBounds, ConstraintExprExt, Model, ModelOp};

#[test]
fn persistent_softening_is_public_stable_and_revisioned() {
    let mut model = Model::new();
    let x = model.add_variable(roml::continuous()).expect("variable");
    let constraint = model
        .add_constraint((x).between(0.0, 1.0))
        .expect("constraint");
    model.commit().expect("initial revision");
    let before = model.current_revision();

    let soft = model
        .soften_constraint(
            constraint,
            ViolationPolicy::default(),
            PenaltyPolicy {
                weight: ValueExpr::constant(1.0),
                target: PenaltyTarget::None,
            },
        )
        .expect("live constraint can be softened");

    assert_eq!(soft.original_constraint(), constraint);
    assert_eq!(soft.lower_violation().side(), ViolationSide::Lower);
    assert_eq!(soft.upper_violation().side(), ViolationSide::Upper);

    let revision = model.commit().expect("softening revision");
    assert!(revision > before);
    let snapshot = model.take_snapshot().expect("snapshot");
    let entry = snapshot
        .constructs
        .iter()
        .find(|entry| entry.id == soft.construct())
        .expect("soft construct in snapshot");
    assert!(matches!(entry.kind, ConstructKind::SoftConstraint(_)));

    let batches = model.deltas_since(before).expect("delta history");
    let batch = batches.last().expect("softening delta");
    assert!(batch.operations.iter().any(|op| {
        matches!(
            op,
            ModelOp::AddConstruct { construct, .. } if *construct == soft.construct()
        )
    }));
}

#[test]
fn frozen_p30_policy_surface_has_no_priority_target() {
    let _ = PenaltyTarget::None;
    let _ = PenaltyTarget::Objective(roml::Objective::new(0, roml::id::Generation::new()));
    let _ = ConstraintBounds::le(1.0);
}
