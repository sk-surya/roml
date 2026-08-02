//! P23 Task 4 — Pre-1.0 deprecation-window compatibility (API-08.3).
//!
//! Every API that P23 deprecates must keep working for the chosen pre-1.0
//! window and remain tested. This suite pins that behavior: the raw
//! constructor wrappers, the effectful macros, and the `Model::constrain` /
//! `Model::constraint` / `Model::set_objective` aliases all still compile and
//! run. Each usage carries `#[allow(deprecated)]` and a pointer to the
//! replacement documented in `MIGRATION.md`.

#![allow(deprecated)]

use roml::model::{ConstraintBounds, Sense};
use roml::prelude::*;
use roml::{constrain, set_objective};

/// Raw constructor wrappers (`add_var`, `add_binary`, `add_integer`,
/// `add_parameter(f64)`) still compile and produce the documented entities.
/// Replacements: `add_variable(continuous())`, `add_variable(binary())`,
/// `add_variable(integer().bounds(...))`, `add_parameter(parameter(value))`.
#[test]
fn raw_constructor_wrappers_still_work() {
    let mut model = Model::new();
    let x = model.add_var();
    let y = model.add_binary();
    let z = model.add_integer(Bounds::new(0.0, 10.0)).expect("integer");
    let p = model.add_parameter(2.5).expect("parameter");

    assert_eq!(model.num_variables(), 3);
    assert_eq!(model.num_parameters(), 1);
    assert_eq!(model.variable_bounds(x), Some(Bounds::NON_NEGATIVE));
    assert_eq!(model.variable_bounds(y), Some(Bounds::BINARY));
    assert_eq!(model.variable_bounds(z), Some(Bounds::new(0.0, 10.0)));
    assert_eq!(model.parameter_value(p), Some(2.5));
}

/// The deprecated constraint aliases (`Model::constrain`, `Model::constraint`)
/// still add constraints. Replacement: `model.add_constraint(spec)`.
#[test]
fn constraint_aliases_still_work() {
    let mut model = Model::new();
    let x = model.add_var();
    let y = model.add_var();

    let c1 = model.constrain((x + y).le(4.0)).expect("constrain");
    let c2 = model.constraint((x).ge(1.0)).expect("constraint");
    assert_eq!(model.num_constraints(), 2);
    assert_eq!(model.constraint_bounds(c1), Some(ConstraintBounds::le(4.0)));
    assert_eq!(model.constraint_bounds(c2), Some(ConstraintBounds::ge(1.0)));
}

/// The effectful `constrain!` macro still applies constraints.
/// Replacement: `model.add_constraint(constraint!(...))`.
#[test]
fn effectful_constrain_macro_still_works() {
    let mut model = Model::new();
    let x = model.add_var();
    let y = model.add_var();

    let cap = constrain!(model, x + y <= 4.0).expect("cap");
    let band = constrain!(model, between: 0.0, x, 3.0).expect("band");
    assert_eq!(model.num_constraints(), 2);
    assert_eq!(
        model.constraint_bounds(cap),
        Some(ConstraintBounds::le(4.0))
    );
    assert_eq!(
        model.constraint_bounds(band),
        Some(ConstraintBounds::range(0.0, 3.0))
    );
}

/// The effectful `set_objective!` macro and the `Model::set_objective` alias
/// still create and activate an objective.
/// Replacement: `model.maximize(expr)` / `model.minimize(expr)`.
#[test]
fn effectful_objective_paths_still_work() {
    let mut model = Model::new();
    let x = model.add_var();
    let y = model.add_var();

    let obj1 = set_objective!(model, maximize: x + 2.0 * y + 3.0).expect("obj1");
    assert_eq!(model.active_objective(), Some(obj1));
    assert_eq!(model.objective_constant(obj1), Some(3.0));

    let obj2 = model
        .set_objective(roml::ObjectiveSpec::new(Sense::Maximize, x + 9.0))
        .expect("obj2");
    assert_eq!(model.active_objective(), Some(obj2));
    assert_eq!(model.objective_constant(obj2), Some(9.0));
}

/// The deprecated surface composes with the canonical surface in one model
/// (migration-in-progress callers).
#[test]
fn deprecated_and_canonical_compose() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).expect("x");
    let y = model.add_var(); // deprecated wrapper

    model.add_constraint((x + y).le(4.0)).expect("canonical");
    model.constrain((y).ge(0.0)).expect("deprecated alias");
    constrain!(model, x <= 3.0).expect("deprecated macro");

    let obj = model.maximize(x + 2.0 * y).expect("canonical objective");
    assert_eq!(model.num_constraints(), 3);
    assert_eq!(model.active_objective(), Some(obj));
    assert!(model.validate_invariants().is_ok());
}
