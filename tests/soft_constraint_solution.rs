//! P30 original-constraint violation and explicit signed-correction tests.

use roml::{
    continuous, ConstraintExprExt, Model, PenaltyPolicy, SolutionBuilder, SolveMetadata,
    ViolationPolicy,
};

fn solution_for(model: &Model, variable: roml::VarId, value: f64) -> roml::Solution {
    SolutionBuilder::new()
        .value(variable, value)
        .metadata(SolveMetadata {
            model_instance: model.instance(),
            model_revision: model.current_revision(),
            ..SolveMetadata::default()
        })
        .build()
}

#[test]
fn raw_lower_upper_and_total_violations_use_original_constraint_terms() {
    let mut model = Model::new();
    let x = model
        .add_variable(continuous().bounds(-10.0, 10.0))
        .unwrap();
    let con = model.add_constraint((x).between(-1.0, 3.0)).unwrap();
    let soft = model
        .soften_constraint(con, ViolationPolicy::default(), PenaltyPolicy::default())
        .unwrap();
    model.commit().unwrap();

    let solution = solution_for(&model, x, 5.0);
    let raw = solution.soft_constraint_violation(&model, soft).unwrap();
    assert_eq!(raw.lower, 0.0);
    assert_eq!(raw.upper, 2.0);
    assert_eq!(raw.total(), 2.0);
    assert_eq!(
        solution.constraint_violation(&model, con).unwrap().total(),
        raw.total()
    );
}

#[test]
fn tolerance_adjustment_is_presentation_only_and_signed_correction_is_explicit() {
    let mut model = Model::new();
    let x = model
        .add_variable(continuous().bounds(-10.0, 10.0))
        .unwrap();
    let con = model.add_constraint((x).between(-1.0, 3.0)).unwrap();
    model
        .soften_constraint(con, ViolationPolicy::default(), PenaltyPolicy::default())
        .unwrap();
    model.commit().unwrap();

    let solution = solution_for(&model, x, 5.0);
    let presentation = solution
        .constraint_violation_with_tolerance(&model, con, 2.0)
        .unwrap();
    assert_eq!(presentation.raw.upper, 2.0);
    assert_eq!(presentation.adjusted.upper, 0.0);
    assert_eq!(presentation.raw.total(), 2.0);
    assert_eq!(presentation.adjusted.total(), 0.0);

    let correction = model.signed_correction(con, 5.0).unwrap();
    assert_eq!(correction.positive, 0.0);
    assert_eq!(correction.negative, 2.0);
    assert_eq!(correction.net(), -2.0);
}

#[test]
fn stale_solution_identity_is_typed() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    let con = model.add_constraint((x).le(3.0)).unwrap();
    model.commit().unwrap();
    let stale = solution_for(&model, x, 1.0);

    let mut changed = model.clone();
    changed
        .set_variable_bounds(x, roml::Bounds::new(0.0, 9.0))
        .unwrap();
    changed.commit().unwrap();
    let error = stale
        .constraint_violation(&changed, con)
        .expect_err("a solution from an older revision must be rejected");
    assert!(matches!(
        error,
        roml::ViolationError::ModelInstanceMismatch { .. }
    ));
}
