use roml::expr::LinExpr;
use roml::model::{Change, CoefficientTarget};
use roml::{Bounds, ConstraintBounds, Model, Sense, ValueExpr, VarType};

#[test]
fn changelog_captures_mutations() {
    let mut model = Model::new();

    let x = model.add_variable(Bounds::NON_NEGATIVE, VarType::Continuous);
    let con = model.add_constraint(ConstraintBounds::le(10.0));
    let obj = model.add_objective(Sense::Minimize);
    model.set_active_objective(obj).unwrap();

    let coeff = model.add_coeff(con, x, 2.0).unwrap();

    model
        .set_variable_bounds(x, Bounds::new(-1.0, 5.0))
        .unwrap();
    model
        .set_constraint_bounds(con, ConstraintBounds::range(0.0, 10.0))
        .unwrap();
    model.set_variable_active(x, false).unwrap();
    model.set_constraint_active(con, false).unwrap();

    let param = model.add_parameter(3.0);
    let coeff_param = model
        .add_constraint_coefficient(con, x, ValueExpr::param(param))
        .unwrap();

    model.set_parameter(param, 4.0);
    model.commit();

    let changes = model.drain_changes();
    assert_eq!(changes.len(), 12);

    assert!(matches!(
        changes[0],
        Change::VariableAdded { var, bounds, var_type }
            if var == x && bounds == Bounds::NON_NEGATIVE && var_type == VarType::Continuous
    ));
    assert!(matches!(
        changes[1],
        Change::ConstraintAdded { con: id, bounds }
            if id == con && bounds == ConstraintBounds::le(10.0)
    ));
    assert!(matches!(
        changes[2],
        Change::ObjectiveAdded { obj: id, sense }
            if id == obj && sense == Sense::Minimize
    ));
    assert!(matches!(
        changes[3],
        Change::ActiveObjectiveChanged { old, new }
            if old.is_none() && new == Some(obj)
    ));
    assert!(matches!(
        changes[4],
        Change::CoefficientAdded { coeff: id, var, target, value }
            if id == coeff
                && var == x
                && target == CoefficientTarget::Constraint(con)
                && (value - 2.0).abs() < f64::EPSILON
    ));
    assert!(matches!(
        changes[5],
        Change::VariableBoundsChanged { var, old, new }
            if var == x
                && old == Bounds::NON_NEGATIVE
                && new == Bounds::new(-1.0, 5.0)
    ));
    assert!(matches!(
        changes[6],
        Change::ConstraintBoundsChanged { con: id, old, new }
            if id == con
                && old == ConstraintBounds::le(10.0)
                && new == ConstraintBounds::range(0.0, 10.0)
    ));
    assert!(matches!(
        changes[7],
        Change::VariableActivityChanged { var, active }
            if var == x && !active
    ));
    assert!(matches!(
        changes[8],
        Change::ConstraintActivityChanged { con: id, active }
            if id == con && !active
    ));
    assert!(matches!(
        changes[9],
        Change::CoefficientAdded { coeff: id, var, target, value }
            if id == coeff_param
                && var == x
                && target == CoefficientTarget::Constraint(con)
                && (value - 3.0).abs() < f64::EPSILON
    ));
    assert!(matches!(
        changes[10],
        Change::ParameterValueChanged { param: id, old, new }
            if id == param
                && (old - 3.0).abs() < f64::EPSILON
                && (new - 4.0).abs() < f64::EPSILON
    ));
    assert!(matches!(
        changes[11],
        Change::CoefficientValueChanged { coeff: id, var, target, old, new }
            if id == coeff_param
                && var == x
                && target == CoefficientTarget::Constraint(con)
                && (old - 3.0).abs() < f64::EPSILON
                && (new - 4.0).abs() < f64::EPSILON
    ));
}

#[test]
fn drain_changes_auto_commits_parameters() {
    let mut model = Model::new();

    let x = model.add_var();
    let con = model.add_constraint(ConstraintBounds::le(5.0));
    let param = model.add_parameter(2.0);

    let coeff = model
        .add_constraint_coefficient(con, x, ValueExpr::param(param))
        .unwrap();

    model.set_parameter(param, 6.0);

    let changes = model.drain_changes();
    assert_eq!(changes.len(), 5);

    assert!(matches!(
        changes[0],
        Change::VariableAdded { var, .. } if var == x
    ));
    assert!(matches!(
        changes[1],
        Change::ConstraintAdded { con: id, .. } if id == con
    ));
    assert!(matches!(
        changes[2],
        Change::CoefficientAdded { coeff: id, var, target, value }
            if id == coeff
                && var == x
                && target == CoefficientTarget::Constraint(con)
                && (value - 2.0).abs() < f64::EPSILON
    ));
    assert!(matches!(
        changes[3],
        Change::ParameterValueChanged { param: id, old, new }
            if id == param
                && (old - 2.0).abs() < f64::EPSILON
                && (new - 6.0).abs() < f64::EPSILON
    ));
    assert!(matches!(
        changes[4],
        Change::CoefficientValueChanged { coeff: id, var, target, old, new }
            if id == coeff
                && var == x
                && target == CoefficientTarget::Constraint(con)
                && (old - 2.0).abs() < f64::EPSILON
                && (new - 6.0).abs() < f64::EPSILON
    ));
}

#[test]
fn indexed_model_with_parameter_arrays() {
    let mut model = Model::new();

    // lets call add_variable in a loop to simulate indexed variables
    // and store the ids in a vector
    let plan_mw: Vec<_> = (0..5).map(|_| model.add_var()).collect();

    // similarly, create a parameter array for energy prices (generate random numbers)
    let rt_energy_price: Vec<_> = (0..5)
        .map(|_| model.add_parameter(rand::random::<f64>() * 100.0))
        .collect();

    let rt_energy_price_scaler: Vec<_> = (0..5)
        .map(|_| model.add_parameter(rand::random::<f64>() * 10.0))
        .collect();

    // lets create an objective expression that uses these parameters
    let mut obj_expr = LinExpr::new().constant(0.0);
    for i in 0..5 {
        let coeff =
            ValueExpr::param(rt_energy_price[i]) * ValueExpr::param(rt_energy_price_scaler[i]);
        obj_expr = obj_expr.add_term_with(coeff, plan_mw[i]);
    }

    let obj = model.add_objective(Sense::Minimize);
    model.set_objective_expr(obj, obj_expr).unwrap();
    model.set_active_objective(obj).unwrap();
    model.commit();
    let changes = model.drain_changes();
    assert_eq!(changes.len(), 5 + 1 + 5 + 1); // 5 for variables, 1 Obj Added, 5 coefficient adds in objective, 1 ActiveObjectiveChanged
}
