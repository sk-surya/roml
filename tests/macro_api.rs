use roml::{constrain, constraint, objective, set_objective, ConstraintBounds, Model};

#[test]
fn constraint_macro_supports_infix_and_range_forms() {
    let mut model = Model::new();
    let x = model.add_var();
    let y = model.add_var();

    let capacity = constraint!(2.0 * x + y <= 10.0);
    assert_eq!(capacity.bounds, ConstraintBounds::le(10.0));
    assert_eq!(capacity.expr.num_terms(), 2);
    assert_eq!(capacity.expr.get_constant(), 0.0);

    let floor = constraint!(x >= 1.0);
    assert_eq!(floor.bounds, ConstraintBounds::ge(1.0));
    assert_eq!(floor.expr.num_terms(), 1);

    let balance = constraint!(x + y == 4.0);
    assert_eq!(balance.bounds, ConstraintBounds::eq(4.0));
    assert_eq!(balance.expr.num_terms(), 2);

    let band = constraint!(between: 0.0, x + y, 5.0);
    assert_eq!(band.bounds, ConstraintBounds::range(0.0, 5.0));
    assert_eq!(band.expr.num_terms(), 2);
}

#[test]
fn objective_macro_builds_specs_for_model_entry_points() {
    let mut model = Model::new();
    let x = model.add_var();
    let y = model.add_var();

    model.constrain(constraint!(x + y <= 4.0)).unwrap();

    let obj = model
        .set_objective(objective!(maximize: x + 2.0 * y + 3.0))
        .unwrap();

    assert_eq!(model.active_objective(), Some(obj));
    assert_eq!(model.objective_constant(obj), Some(3.0));

    let expr = model.objective_expression(obj).unwrap();
    assert_eq!(expr.num_terms(), 2);
    assert_eq!(expr.get_constant(), 3.0);
}

#[test]
fn effectful_constraint_macro_adds_constraints() {
    let mut model = Model::new();
    let x = model.add_var();
    let y = model.add_var();

    let cap = constrain!(model, x + y <= 4.0).unwrap();
    let band = constrain!(model, between: 0.0, x, 3.0).unwrap();

    assert_eq!(model.num_constraints(), 2);
    assert_eq!(model.constraint_expression(cap).unwrap().num_terms(), 2);
    assert_eq!(model.constraint_expression(band).unwrap().num_terms(), 1);
}

#[test]
fn effectful_objective_macro_sets_active_objective() {
    let mut model = Model::new();
    let x = model.add_var();
    let y = model.add_var();

    constrain!(model, x + y <= 4.0).unwrap();
    let obj = set_objective!(model, maximize: x + 2.0 * y + 3.0).unwrap();

    assert_eq!(model.active_objective(), Some(obj));
    assert_eq!(model.objective_constant(obj), Some(3.0));
    assert_eq!(model.objective_expression(obj).unwrap().get_constant(), 3.0);
}