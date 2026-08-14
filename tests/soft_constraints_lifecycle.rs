//! P30-01 canonical lifecycle and atomic validation regressions.

use roml::construct::{PenaltyPolicy, PenaltyTarget, ViolationPolicy};
use roml::id::{ConId, Generation};
use roml::value_expr::ValueExpr;
use roml::{ConstraintExprExt, Model, ModelError};

fn softened_model() -> (Model, roml::Constraint, roml::SoftConstraint) {
    let mut model = Model::new();
    let x = model.add_variable(roml::continuous()).expect("variable");
    let constraint = model
        .add_constraint(x.between(0.0, 1.0))
        .expect("constraint");
    let soft = model
        .soften_constraint(
            constraint,
            ViolationPolicy::default(),
            PenaltyPolicy::default(),
        )
        .expect("softening");
    model.commit().expect("revision");
    (model, constraint, soft)
}

#[test]
fn clone_preserves_soft_semantics_but_allocates_a_new_instance() {
    let (model, _constraint, soft) = softened_model();
    let clone = model.clone();

    assert_eq!(clone.lineage(), model.lineage());
    assert_ne!(clone.instance(), model.instance());
    assert_eq!(
        clone.soft_constraint(soft).unwrap(),
        model.soft_constraint(soft).unwrap()
    );
    assert_eq!(
        clone.take_snapshot().unwrap().constructs,
        model.take_snapshot().unwrap().constructs
    );
}

#[test]
fn removing_the_original_constraint_cascades_the_persistent_soft_construct() {
    let (mut model, constraint, soft) = softened_model();
    model
        .remove_constraint(constraint)
        .expect("remove constraint");

    assert_eq!(model.num_constructs(), 0);
    assert_eq!(
        model.soft_constraint(soft),
        Err(ModelError::ConstructNotFound(soft.construct()))
    );
    assert!(model.validate_invariants().is_ok());
}

#[test]
fn invalid_softening_inputs_are_atomic_and_typed() {
    let mut model = Model::new();
    let x = model.add_variable(roml::continuous()).expect("variable");
    let constraint = model.add_constraint(x.le(1.0)).expect("constraint");
    model.commit().expect("baseline revision");

    let assert_unchanged = |model: &Model, before: &roml::ModelSnapshot| {
        assert_eq!(model.take_snapshot().unwrap(), *before);
        assert_eq!(model.changelog_sequence(), 3);
        assert_eq!(model.num_constructs(), 0);
        assert!(!model.has_pending_changes());
    };
    let baseline = model.take_snapshot().unwrap();

    let error = model
        .soften_constraint(
            constraint,
            ViolationPolicy {
                max_violation: Some(-1.0),
            },
            PenaltyPolicy::default(),
        )
        .unwrap_err();
    assert_eq!(error, ModelError::InvalidMaxViolation(-1.0));
    assert_unchanged(&model, &baseline);

    model.set_constraint_active(constraint, false).unwrap();
    let after_activity = model.take_snapshot().unwrap();
    let error = model
        .soften_constraint(
            constraint,
            ViolationPolicy::default(),
            PenaltyPolicy::default(),
        )
        .unwrap_err();
    assert_eq!(error, ModelError::InactiveConstraint(constraint));
    assert_eq!(model.take_snapshot().unwrap(), after_activity);
    model.set_constraint_active(constraint, true).unwrap();

    let fake = ConId::new(999, Generation::new());
    let error = model
        .soften_constraint(fake, ViolationPolicy::default(), PenaltyPolicy::default())
        .unwrap_err();
    assert_eq!(error, ModelError::ConstraintNotFound(fake));

    let error = model
        .soften_constraint(
            constraint,
            ViolationPolicy::default(),
            PenaltyPolicy {
                weight: ValueExpr::constant(-0.5),
                target: PenaltyTarget::None,
            },
        )
        .unwrap_err();
    assert_eq!(error, ModelError::InvalidPenaltyWeight(-0.5));

    let missing = roml::ParamId::new(998, Generation::new());
    let error = model
        .soften_constraint(
            constraint,
            ViolationPolicy::default(),
            PenaltyPolicy {
                weight: ValueExpr::param(missing),
                target: PenaltyTarget::None,
            },
        )
        .unwrap_err();
    assert_eq!(error, ModelError::ParameterNotFound(missing));
}
