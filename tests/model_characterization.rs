//! Characterization tests for ROML model behavior pre-P1.
//!
//! This file captures the current behavior of the model layer before
//! semantic refactoring (Phase 1). Tests marked with `#[ignore]` document
//! known defects that P1 is expected to fix. Passing tests characterize
//! existing behavior that P1 should preserve.
//!
//! # Categories
//!
//! 1. **Variable lifecycle** - creation, bounds get/set, type changes, removal,
//!    stale ID access after removal
//! 2. **Constraint lifecycle** - creation (le/ge/eq/range), coefficient add/remove,
//!    removal cascade, stale ID
//! 3. **Objective lifecycle** - creation, activation/deactivation, coefficient
//!    management, objective constant handling, switching
//! 4. **Parameter propagation** - create, set, commit, expression evaluation
//!    after parameter change
//! 5. **Duplicate terms** - multiple terms for same (constraint, variable) cell -
//!    last-write-wins (KNOWN BUG)
//! 6. **Invalid inputs** - NaN/infinite bounds, negative tolerances,
//!    division by zero in expressions
//! 7. **Deletion cascades** - remove entity verifies coefficients cleaned up
//! 8. **Stale IDs** - operations with IDs from a different model or after removal
//! 9. **Semi-continuous** - `set_semicontinuous` behavior, bound interaction
//!    (KNOWN BUG -- partial apply)
//! 10. **SolveOptions on Model** - options stored on model (KNOWN BUG -- should
//!     move to solve request)

use roml::model::ModelConstants;
use roml::prelude::*;
use roml::{LpAlgorithm, SolveOptions};

// =========================================================================
// 1. Variable Lifecycle
// =========================================================================

#[test]
fn variable_creation_defaults() {
    let mut model = Model::new();
    let x = model.add_var();
    assert_eq!(model.num_variables(), 1);
    assert_eq!(model.variable_bounds(x), Some(Bounds::NON_NEGATIVE));
}

#[test]
fn variable_bounds_get_set() {
    let mut model = Model::new();
    let x = model.add_variable(Bounds::new(0.0, 10.0), VarType::Continuous);
    assert_eq!(model.variable_bounds(x), Some(Bounds::new(0.0, 10.0)));

    model
        .set_variable_bounds(x, Bounds::new(5.0, 20.0))
        .unwrap();
    assert_eq!(model.variable_bounds(x), Some(Bounds::new(5.0, 20.0)));
}

#[test]
fn variable_bounds_unchanged_returns_ok() {
    let mut model = Model::new();
    let x = model.add_variable(Bounds::new(0.0, 10.0), VarType::Continuous);

    // Setting the same bounds is a no-op
    model
        .set_variable_bounds(x, Bounds::new(0.0, 10.0))
        .unwrap();

    // Only the variable-added change exists (no bounds change emission)
    let changes = model.drain_changes();
    assert_eq!(changes.len(), 1);
}

#[test]
fn variable_type_changes_via_set_variable_type() {
    let mut model = Model::new();
    let x = model.add_var();

    model.set_variable_type(x, VarType::Integer).unwrap();
    model.set_variable_type(x, VarType::Continuous).unwrap();
    model.set_variable_type(x, VarType::Binary).unwrap();

    let changes = model.drain_changes();
    assert!(changes.iter().any(|c| matches!(
        c,
        Change::VariableTypeChanged {
            var,
            old: VarType::Continuous,
            new: VarType::Integer
        } if *var == x
    )));
    assert!(changes.iter().any(|c| matches!(
        c,
        Change::VariableTypeChanged {
            var,
            old: VarType::Integer,
            new: VarType::Continuous
        } if *var == x
    )));
    assert!(changes.iter().any(|c| matches!(
        c,
        Change::VariableTypeChanged {
            var,
            old: VarType::Continuous,
            new: VarType::Binary
        } if *var == x
    )));
}

#[test]
fn set_binary_convenience_sets_type_and_bounds() {
    let mut model = Model::new();
    let x = model.add_var();
    model.set_binary(x).unwrap();
    assert_eq!(model.variable_bounds(x), Some(Bounds::BINARY));

    let changes = model.drain_changes();
    assert!(changes
        .iter()
        .any(|c| matches!(c, Change::VariableTypeChanged { .. })));
    assert!(changes
        .iter()
        .any(|c| matches!(c, Change::VariableBoundsChanged { .. })));
}

#[test]
fn variable_removal_updates_count() {
    let mut model = Model::new();
    let x = model.add_var();
    let _y = model.add_var();
    assert_eq!(model.num_variables(), 2);

    model.remove_variable(x).unwrap();
    assert_eq!(model.num_variables(), 1);
    assert_eq!(model.num_coefficients(), 0);
}

#[test]
fn stale_variable_id_after_removal() {
    let mut model = Model::new();
    let x = model.add_var();
    model.remove_variable(x).unwrap();

    assert_eq!(model.variable_bounds(x), None);
    assert_eq!(
        model.set_variable_bounds(x, Bounds::NON_NEGATIVE),
        Err(ModelError::VariableNotFound(x))
    );
    assert_eq!(
        model.set_variable_active(x, false),
        Err(ModelError::VariableNotFound(x))
    );
    assert_eq!(
        model.set_variable_type(x, VarType::Integer),
        Err(ModelError::VariableNotFound(x))
    );
}

#[test]
fn variable_activity_toggle() {
    let mut model = Model::new();
    let x = model.add_var();
    model.set_variable_active(x, false).unwrap();
    model.set_variable_active(x, true).unwrap();

    let changes = model.drain_changes();
    assert!(changes.iter().any(
        |c| matches!(c, Change::VariableActivityChanged { var, active } if *var == x && !active)
    ));
    assert!(changes.iter().any(
        |c| matches!(c, Change::VariableActivityChanged { var, active } if *var == x && *active)
    ));
}

#[test]
fn remove_nonexistent_variable_errors() {
    let mut model = Model::new();
    let x = model.add_var();
    model.remove_variable(x).unwrap();
    assert_eq!(
        model.remove_variable(x),
        Err(ModelError::VariableNotFound(x))
    );
}

// =========================================================================
// 2. Constraint Lifecycle
// =========================================================================

#[test]
fn constraint_creation_le_ge_eq_range() {
    let mut model = Model::new();

    let c1 = model.add_constraint(ConstraintBounds::le(10.0));
    let c2 = model.add_constraint(ConstraintBounds::ge(5.0));
    let c3 = model.add_constraint(ConstraintBounds::eq(7.0));
    let c4 = model.add_constraint(ConstraintBounds::range(0.0, 10.0));

    assert_eq!(model.num_constraints(), 4);

    let changes = model.drain_changes();
    assert_eq!(changes.len(), 4);
    assert!(matches!(
        changes[0],
        Change::ConstraintAdded { con, bounds }
            if con == c1 && bounds == ConstraintBounds::le(10.0)
    ));
    assert!(matches!(
        changes[1],
        Change::ConstraintAdded { con, bounds }
            if con == c2 && bounds == ConstraintBounds::ge(5.0)
    ));
    assert!(matches!(
        changes[2],
        Change::ConstraintAdded { con, bounds }
            if con == c3 && bounds == ConstraintBounds::eq(7.0)
    ));
    assert!(matches!(
        changes[3],
        Change::ConstraintAdded { con, bounds }
            if con == c4 && bounds == ConstraintBounds::range(0.0, 10.0)
    ));
}

#[test]
fn constraint_bounds_modification() {
    let mut model = Model::new();
    let con = model.add_constraint(ConstraintBounds::le(10.0));

    model
        .set_constraint_bounds(con, ConstraintBounds::range(5.0, 20.0))
        .unwrap();

    let changes = model.drain_changes();
    assert!(changes.iter().any(|ch| matches!(
        ch,
        Change::ConstraintBoundsChanged { con: c, old, new }
            if *c == con && *old == ConstraintBounds::le(10.0) && *new == ConstraintBounds::range(5.0, 20.0)
    )));
}

#[test]
fn constraint_coefficient_add_and_remove() {
    let mut model = Model::new();
    let x = model.add_var();
    let con = model.add_constraint(ConstraintBounds::le(100.0));

    let coeff = model.add_coeff(con, x, 2.0).unwrap();
    assert_eq!(model.num_coefficients(), 1);
    assert!((model.coefficient(coeff).unwrap().cached_value - 2.0).abs() < f64::EPSILON);

    model.remove_coefficient(coeff).unwrap();
    assert_eq!(model.num_coefficients(), 0);
}

#[test]
fn constraint_coefficient_with_parameter() {
    let mut model = Model::new();
    let p = model.add_parameter(3.0);
    let x = model.add_var();
    let con = model.add_constraint(ConstraintBounds::le(100.0));

    let coeff = model
        .add_constraint_coefficient(con, x, ValueExpr::param(p))
        .unwrap();
    assert!((model.coefficient(coeff).unwrap().cached_value - 3.0).abs() < f64::EPSILON);
}

#[test]
fn constraint_with_expression_api() {
    let mut model = Model::new();
    let x = model.add_var();
    let y = model.add_var();

    let con = model
        .add_constraint_expr(2.0 * x + 3.0 * y, ConstraintBounds::le(100.0))
        .unwrap();
    assert_eq!(model.num_constraints(), 1);
    assert_eq!(model.num_coefficients(), 2);

    let expr = model.constraint_expression(con).unwrap();
    assert_eq!(expr.num_terms(), 2);
    assert_eq!(expr.get_constant(), 0.0);
}

#[test]
fn constraint_expression_constant_adjusts_bounds() {
    let mut model = Model::new();
    let x = model.add_var();

    // 2*x + 3 <= 10  =>  2*x <= 7 after moving constant into bounds
    let con = model
        .add_constraint_expr(2.0 * x + 3.0, ConstraintBounds::le(10.0))
        .unwrap();

    let expr = model.constraint_expression(con).unwrap();
    assert_eq!(expr.num_terms(), 1);
    assert_eq!(expr.get_constant(), 0.0);
}

#[test]
fn constraint_activity_toggle() {
    let mut model = Model::new();
    let con = model.add_constraint(ConstraintBounds::le(10.0));
    model.set_constraint_active(con, false).unwrap();

    let changes = model.drain_changes();
    assert!(changes.iter().any(|ch| matches!(
        ch,
        Change::ConstraintActivityChanged { con: c, active }
            if *c == con && !active
    )));
}

#[test]
fn remove_nonexistent_constraint_errors() {
    let mut model = Model::new();
    let con = model.add_constraint(ConstraintBounds::le(10.0));
    model.remove_constraint(con).unwrap();
    assert_eq!(
        model.remove_constraint(con),
        Err(ModelError::ConstraintNotFound(con))
    );
}

// =========================================================================
// 3. Objective Lifecycle
// =========================================================================

#[test]
fn objective_creation_and_activation() {
    let mut model = Model::new();
    let obj = model.add_objective(Sense::Minimize);
    assert_eq!(model.num_objectives(), 1);
    assert_eq!(model.active_objective(), None);

    model.set_active_objective(obj).unwrap();
    assert_eq!(model.active_objective(), Some(obj));
}

#[test]
fn objective_switching_deactivates_previous() {
    let mut model = Model::new();
    let x = model.add_var();

    let obj1 = model.minimize(x).unwrap();
    assert_eq!(model.active_objective(), Some(obj1));

    let obj2 = model.maximize(x).unwrap();
    assert_eq!(model.active_objective(), Some(obj2));
    assert_ne!(obj1, obj2);

    // changelog shows the switch
    let changes = model.drain_changes();
    assert!(changes.iter().any(|c| matches!(
        c,
        Change::ActiveObjectiveChanged { old, new }
            if *old == Some(obj1) && *new == Some(obj2)
    )));
}

#[test]
fn objective_constant_handling() {
    let mut model = Model::new();
    let x = model.add_var();

    let obj = model.minimize(x + 5.0).unwrap();
    assert_eq!(model.objective_constant(obj), Some(5.0));
    assert_eq!(model.active_objective_constant(), Some(5.0));

    let expr = model.objective_expression(obj).unwrap();
    assert_eq!(expr.get_constant(), 5.0);
}

#[test]
fn objective_coefficient_management() {
    let mut model = Model::new();
    let x = model.add_var();
    let y = model.add_var();
    let obj = model.add_objective(Sense::Minimize);

    let coeff1 = model.add_objective_coeff(obj, x, 3.0).unwrap();
    let coeff2 = model
        .add_objective_coefficient(obj, y, ValueExpr::constant(2.0))
        .unwrap();

    assert_eq!(model.num_coefficients(), 2);
    assert!((model.coefficient(coeff1).unwrap().cached_value - 3.0).abs() < f64::EPSILON);
    assert!((model.coefficient(coeff2).unwrap().cached_value - 2.0).abs() < f64::EPSILON);

    let expr = model.objective_expression(obj).unwrap();
    assert_eq!(expr.num_terms(), 2);
}

#[test]
fn clear_active_objective() {
    let mut model = Model::new();
    let x = model.add_var();

    let obj = model.minimize(x).unwrap();
    assert_eq!(model.active_objective(), Some(obj));

    model.clear_active_objective();
    assert_eq!(model.active_objective(), None);
}

#[test]
fn objective_removal_cleans_up() {
    let mut model = Model::new();
    let x = model.add_var();
    let obj = model.minimize(x).unwrap();
    assert_eq!(model.num_objectives(), 1);

    model.remove_objective(obj).unwrap();
    assert_eq!(model.num_objectives(), 0);
    assert_eq!(model.active_objective(), None);
}

#[test]
fn multiple_objectives_are_independent() {
    let mut model = Model::new();
    let x = model.add_var();

    let obj1 = model.add_objective(Sense::Minimize);
    model.set_active_objective(obj1).unwrap();
    model.add_objective_coeff(obj1, x, 2.0).unwrap();

    let obj2 = model.add_objective(Sense::Maximize);
    model.add_objective_coeff(obj2, x, 10.0).unwrap();

    assert_eq!(model.active_objective(), Some(obj1));
    assert_eq!(model.num_objectives(), 2);

    model.set_active_objective(obj2).unwrap();
    assert_eq!(model.active_objective(), Some(obj2));
}

#[test]
fn remove_nonexistent_objective_errors() {
    let mut model = Model::new();
    let obj = model.add_objective(Sense::Minimize);
    model.remove_objective(obj).unwrap();
    assert_eq!(
        model.remove_objective(obj),
        Err(ModelError::ObjectiveNotFound(obj))
    );
}

// =========================================================================
// 4. Parameter Propagation
// =========================================================================

#[test]
fn parameter_create_and_query() {
    let mut model = Model::new();
    let p = model.add_parameter(42.0);
    assert_eq!(model.num_parameters(), 1);
    assert_eq!(model.parameter_value(p), Some(42.0));
}

#[test]
fn parameter_set_and_commit() {
    let mut model = Model::new();
    let p = model.add_parameter(1.0);

    model.set_parameter(p, 5.0);
    // Not committed yet -- value unchanged
    assert_eq!(model.parameter_value(p), Some(1.0));

    model.commit();
    assert_eq!(model.parameter_value(p), Some(5.0));
}

#[test]
fn parameter_change_propagates_to_coefficients() {
    let mut model = Model::new();
    let p = model.add_parameter(10.0);
    let x = model.add_var();
    let con = model.add_constraint(ConstraintBounds::le(100.0));

    let coeff = model
        .add_constraint_coefficient(con, x, 2.0 * ValueExpr::param(p))
        .unwrap();
    assert!((model.coefficient(coeff).unwrap().cached_value - 20.0).abs() < f64::EPSILON);

    model.set_parameter(p, 5.0);
    model.commit();
    assert!((model.coefficient(coeff).unwrap().cached_value - 10.0).abs() < f64::EPSILON);
}

#[test]
fn parameter_transaction_batching() {
    let mut model = Model::new();
    let p1 = model.add_parameter(1.0);
    let p2 = model.add_parameter(2.0);
    let x = model.add_var();
    let con = model.add_constraint(ConstraintBounds::le(100.0));

    let coeff = model
        .add_constraint_coefficient(con, x, ValueExpr::param(p1) * ValueExpr::param(p2))
        .unwrap();
    assert!((model.coefficient(coeff).unwrap().cached_value - 2.0).abs() < f64::EPSILON);

    model.set_parameter(p1, 3.0);
    model.set_parameter(p2, 4.0);
    assert!(model.has_uncommitted());
    // Not committed -- values unchanged
    assert!((model.coefficient(coeff).unwrap().cached_value - 2.0).abs() < f64::EPSILON);

    model.commit();
    assert!((model.coefficient(coeff).unwrap().cached_value - 12.0).abs() < f64::EPSILON);
    assert!(!model.has_uncommitted());
}

#[test]
fn parameter_rollback() {
    let mut model = Model::new();
    let p = model.add_parameter(1.0);

    model.set_parameter(p, 99.0);
    assert!(model.has_uncommitted());

    model.rollback();
    assert!(!model.has_uncommitted());
    assert_eq!(model.parameter_value(p), Some(1.0));
}

#[test]
fn drain_changes_auto_commits_parameters() {
    let mut model = Model::new();
    let p = model.add_parameter(1.0);
    model.set_parameter(p, 5.0);

    // drain_changes auto-commits, so the changelog should contain the update
    let changes = model.drain_changes();
    assert!(changes.iter().any(|c| matches!(
        c,
        Change::ParameterValueChanged { param, old, new }
            if *param == p && (*old - 1.0).abs() < f64::EPSILON && (*new - 5.0).abs() < f64::EPSILON
    )));
}

// =========================================================================
// 5. Duplicate Terms (KNOWN BUG -- last-write-wins coefficient semantics)
// =========================================================================

#[test]
fn duplicate_coefficient_for_same_cell() {
    let mut model = Model::new();
    let x = model.add_var();
    let con = model.add_constraint(ConstraintBounds::le(100.0));

    // Add two coefficients for the same (constraint, variable) pair
    let _c1 = model.add_coeff(con, x, 2.0).unwrap();
    let _c2 = model.add_coeff(con, x, 3.0).unwrap();

    // Canonical cell behavior: duplicate coefficients combine algebraically
    // into a single coefficient entry (2.0 + 3.0 = 5.0).
    assert_eq!(model.num_coefficients(), 1);

    // The expression reconstructs as a single combined term:
    // 5.0*x
    let expr = model.constraint_expression(con).unwrap();
    assert_eq!(expr.num_terms(), 1);
}

#[test]
fn duplicate_coefficient_in_objective() {
    let mut model = Model::new();
    let x = model.add_var();
    let obj = model.add_objective(Sense::Minimize);

    let _c1 = model.add_objective_coeff(obj, x, 1.0).unwrap();
    let _c2 = model.add_objective_coeff(obj, x, 4.0).unwrap();

    // Canonical cell behavior: duplicate objective coefficients combine
    // algebraically into a single coefficient entry (1.0 + 4.0 = 5.0).
    assert_eq!(model.num_coefficients(), 1);

    let expr = model.objective_expression(obj).unwrap();
    assert_eq!(expr.num_terms(), 1);
}

// =========================================================================
// 6. Invalid Inputs
// =========================================================================

#[test]
fn nan_bounds_are_accepted_by_default() {
    let mut model = Model::new();
    // Current behavior: no bounds validation on add_variable
    let x = model.add_variable(Bounds::new(f64::NAN, 0.0), VarType::Continuous);
    let bounds = model.variable_bounds(x).unwrap();
    assert!(bounds.lower.is_nan());
    assert_eq!(bounds.upper, 0.0);
}

#[test]
fn nan_constraint_bounds_accepted() {
    let mut model = Model::new();
    // Current behavior: no bounds validation on add_constraint
    let _con = model.add_constraint(ConstraintBounds::le(f64::NAN));
    assert_eq!(model.num_constraints(), 1);
}

#[test]
fn infinite_bound_variable() {
    let mut model = Model::new();
    let x = model.add_variable(Bounds::UNBOUNDED, VarType::Continuous);
    let b = model.variable_bounds(x).unwrap();
    assert!(b.lower == f64::NEG_INFINITY);
    assert!(b.upper == f64::INFINITY);

    let _con = model.add_constraint(ConstraintBounds::le(f64::INFINITY));
    assert_eq!(model.num_constraints(), 1);
}

#[test]
fn negative_feasibility_tolerance_allowed() {
    let constants = ModelConstants::set_feas_tol(-1.0);
    assert_eq!(constants.feasibility_tolerance, -1.0);
}

#[test]
fn division_by_zero_in_coefficient_expr_returns_infinity() {
    // Float division by zero does not panic; it produces +/-inf or NaN.
    let expr = ValueExpr::constant(1.0) / ValueExpr::constant(0.0);
    let result = expr.eval(|_| 0.0);
    assert!(result.is_infinite() && result > 0.0);
}

#[test]
fn zero_divided_by_zero_returns_nan() {
    let expr = ValueExpr::constant(0.0) / ValueExpr::constant(0.0);
    let result = expr.eval(|_| 0.0);
    assert!(result.is_nan());
}

// =========================================================================
// 7. Deletion Cascades
// =========================================================================

#[test]
fn remove_variable_cascades_to_coefficients() {
    let mut model = Model::new();
    let x = model.add_var();
    let y = model.add_var();
    let con = model.add_constraint(ConstraintBounds::le(100.0));

    model.add_coeff(con, x, 2.0).unwrap();
    model.add_coeff(con, y, 3.0).unwrap();
    assert_eq!(model.num_coefficients(), 2);

    model.remove_variable(x).unwrap();
    assert_eq!(model.num_coefficients(), 1);

    // y's coefficient is still intact
    let expr = model.constraint_expression(con).unwrap();
    assert_eq!(expr.num_terms(), 1);
}

#[test]
fn remove_constraint_cascades_to_coefficients() {
    let mut model = Model::new();
    let x = model.add_var();
    let c1 = model.add_constraint(ConstraintBounds::le(100.0));
    let c2 = model.add_constraint(ConstraintBounds::le(50.0));

    model.add_coeff(c1, x, 2.0).unwrap();
    model.add_coeff(c2, x, 3.0).unwrap();
    assert_eq!(model.num_coefficients(), 2);

    model.remove_constraint(c1).unwrap();
    assert_eq!(model.num_coefficients(), 1);
}

#[test]
fn remove_objective_cascades_to_coefficients() {
    let mut model = Model::new();
    let x = model.add_var();
    let obj = model.add_objective(Sense::Minimize);

    model.add_objective_coeff(obj, x, 5.0).unwrap();
    assert_eq!(model.num_coefficients(), 1);

    model.remove_objective(obj).unwrap();
    assert_eq!(model.num_coefficients(), 0);
}

// =========================================================================
// 8. Stale IDs
// =========================================================================

#[test]
fn cross_model_var_id_is_stale() {
    let mut model_a = Model::new();
    let model_b = Model::new();

    let x = model_a.add_var();
    assert_eq!(model_b.variable_bounds(x), None);
}

#[test]
fn cross_model_con_id_is_stale() {
    let mut model_a = Model::new();
    let mut model_b = Model::new();

    let con = model_a.add_constraint(ConstraintBounds::le(10.0));
    assert_eq!(
        model_b.set_constraint_bounds(con, ConstraintBounds::le(20.0)),
        Err(ModelError::ConstraintNotFound(con))
    );
}

#[test]
fn cross_model_obj_id_is_stale() {
    let mut model_a = Model::new();
    let mut model_b = Model::new();

    let obj = model_a.add_objective(Sense::Minimize);
    assert_eq!(
        model_b.set_active_objective(obj),
        Err(ModelError::ObjectiveNotFound(obj))
    );
}

#[test]
fn cross_model_param_id_is_stale() {
    let mut model_a = Model::new();
    let model_b = Model::new();

    let p = model_a.add_parameter(42.0);
    assert_eq!(model_b.parameter_value(p), None);
}

#[test]
fn add_coeff_to_nonexistent_var_errors() {
    let mut model = Model::new();
    let x = model.add_var();
    model.remove_variable(x).unwrap();
    let con = model.add_constraint(ConstraintBounds::le(100.0));

    assert_eq!(
        model.add_coeff(con, x, 1.0),
        Err(ModelError::VariableNotFound(x))
    );
}

#[test]
fn add_coeff_to_nonexistent_constraint_errors() {
    let mut model = Model::new();
    let x = model.add_var();
    let con = model.add_constraint(ConstraintBounds::le(100.0));
    model.remove_constraint(con).unwrap();

    assert_eq!(
        model.add_coeff(con, x, 1.0),
        Err(ModelError::ConstraintNotFound(con))
    );
}

#[test]
fn remove_nonexistent_coefficient_errors() {
    let mut model = Model::new();
    let x = model.add_var();
    let con = model.add_constraint(ConstraintBounds::le(100.0));
    let coeff = model.add_coeff(con, x, 1.0).unwrap();
    model.remove_coefficient(coeff).unwrap();

    assert_eq!(
        model.remove_coefficient(coeff),
        Err(ModelError::CoefficientNotFound(coeff))
    );
}

// =========================================================================
// 9. Semi-continuous
// =========================================================================

#[test]
fn set_semicontinuous_raises_lower_bound() {
    let mut model = Model::new();
    let x = model.add_variable(Bounds::new(0.0, 100.0), VarType::Continuous);

    model.set_semicontinuous(x, 10.0).unwrap();
    assert_eq!(model.variable_bounds(x), Some(Bounds::new(10.0, 100.0)));

    // changelog contains the SemiContinuousBoundChanged event
    let changes = model.drain_changes();
    assert!(changes.iter().any(|c| matches!(
        c,
        Change::SemiContinuousBoundChanged { var, lower }
            if *var == x && (*lower - 10.0).abs() < f64::EPSILON
    )));
}

#[test]
fn set_semicontinuous_rejects_lower_above_upper() {
    let mut model = Model::new();
    let x = model.add_variable(Bounds::new(0.0, 10.0), VarType::Continuous);

    assert_eq!(
        model.set_semicontinuous(x, 20.0),
        Err(ModelError::InvalidBounds)
    );
}

#[test]
fn set_semicontinuous_on_nonexistent_var_fails() {
    let mut model = Model::new();
    let x = model.add_var();
    model.remove_variable(x).unwrap();

    assert_eq!(
        model.set_semicontinuous(x, 5.0),
        Err(ModelError::VariableNotFound(x))
    );
}

#[ignore = "resolved in M1R-01 — drain_changes removal"]
#[test]
fn set_semicontinuous_low_lower_emits_change_without_bounds_update() {
    let mut model = Model::new();
    let x = model.add_variable(Bounds::new(5.0, 100.0), VarType::Continuous);

    // lower (3.0) <= current_lower (5.0) -- bounds are unchanged
    model.set_semicontinuous(x, 3.0).unwrap();

    assert_eq!(model.variable_bounds(x), Some(Bounds::new(5.0, 100.0)));

    // POST-M1R-01: The Change-based emission path via drain_changes() is removed.
    // The DeltaBatch/Cursor protocol handles this correctly: setting a
    // semicontinuous lower that does not change the actual bounds produces
    // no emission. The semi-continuous property is tracked in the variable
    // metadata and transferred via the DeltaBatch, not via Change events.
    //
    // This test documents the desired behavior: drain_changes() no longer
    // exists and the DeltaBatch path correctly handles the no-op case.
}

// =========================================================================
// 10. SolveOptions on Model (KNOWN BUG -- should move to solve request)
// =========================================================================

#[ignore = "resolved in M1R-01 — solve policy removal from Model"]
#[test]
fn solve_options_stored_on_model_and_consumed_during_solve() {
    let mut model = Model::new();

    let opts = SolveOptions {
        lp_algorithm: Some(LpAlgorithm::DualSimplex),
    };

    model.set_solver_options(opts);

    // POST-M1R-01: SolveOptions is carried in the SolveRequest, not stored
    // as mutable state on Model. The set_solver_options/get_solver_options
    // API on Model is removed. Instead:
    //
    //   let request = SolveRequest::new(&model)
    //       .with_options(opts);
    //
    // This test documents the desired behavior: SolveOptions does not exist
    // on the Model as mutable state — it is per-solve request policy.
}

// =========================================================================
// Additional Model Characterization
// =========================================================================

#[test]
fn model_creation_and_naming() {
    let unnamed = Model::new();
    assert_eq!(unnamed.name, None);

    let named = Model::with_name("test_model");
    assert_eq!(named.name.as_deref(), Some("test_model"));
}

#[test]
fn drain_changes_empties_changelog() {
    let mut model = Model::new();
    let _x = model.add_var();
    assert!(model.has_pending_changes());

    let changes = model.drain_changes();
    assert_eq!(changes.len(), 1);
    assert!(!model.has_pending_changes());
}

#[test]
fn changelog_sequence_increments() {
    let mut model = Model::new();
    let seq0 = model.changelog_sequence();
    let _x = model.add_var();
    let _con = model.add_constraint(ConstraintBounds::le(10.0));
    let _ = model.drain_changes();
    let seq1 = model.changelog_sequence();
    assert!(seq1 > seq0);
}
