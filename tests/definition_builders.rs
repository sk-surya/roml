//! P21 — Definition builders and fallible model entry points.
//!
//! The P20 target fixtures (`tests/ui/target_*.rs`) freeze the accepted
//! target API: `continuous()` / `integer()` / `binary()` / `parameter(value)`
//! definition builders (D7), `Model::named`, fallible `add_variable`,
//! `add_parameter`, `add_constraint`, and `set_parameter`, and the semantic
//! aliases `Variable` / `Parameter` (D8). These tests pin that behavior so
//! the fixtures can compile and execute.

use roml::id::{ConId, Generation, ObjId, ParamId, VarId};
use roml::model::{Bounds, ConstraintBounds};
use roml::prelude::*;

fn fake_param() -> ParamId {
    ParamId::new(999, Generation::new())
}

/// `Model::named` is the name-bearing constructor.
#[test]
fn model_named_constructor_sets_name() {
    let model = Model::named("production");
    assert_eq!(model.name.as_deref(), Some("production"));
}

/// `add_variable(continuous())` returns a `Variable` (alias of `VarId`) and
/// records non-negative bounds.
#[test]
fn continuous_builder_adds_non_negative_continuous_variable() {
    let mut model = Model::new();
    let x: Variable = model.add_variable(continuous()).expect("valid def");
    assert_eq!(model.variable_bounds(x), Some(Bounds::NON_NEGATIVE));
    assert_eq!(model.num_variables(), 1);
}

/// `add_variable(integer().bounds(0.0, 10.0).named("y"))` records the bounds
/// and the name survives construction.
#[test]
fn integer_builder_with_bounds_and_name() {
    let mut model = Model::new();
    let y: Variable = model
        .add_variable(integer().bounds(0.0, 10.0).named("y"))
        .expect("valid def");
    assert_eq!(model.variable_bounds(y), Some(Bounds::new(0.0, 10.0)));
    assert_eq!(model.num_variables(), 1);
}

/// Binary builder adds a [0,1] variable.
#[test]
fn binary_builder_adds_binary_bounds() {
    let mut model = Model::new();
    let z: Variable = model.add_variable(binary()).expect("valid def");
    assert_eq!(model.variable_bounds(z), Some(Bounds::BINARY));
}

/// Invalid (inverted) bounds are rejected before model mutation (API-06.1).
#[test]
fn add_variable_rejects_invalid_bounds_atomically() {
    let mut model = Model::new();
    let before = model.num_variables();
    let err = model
        .add_variable(continuous().bounds(10.0, 0.0))
        .expect_err("inverted bounds must be rejected");
    assert_eq!(err, ModelError::InvalidBounds);
    assert_eq!(model.num_variables(), before, "no mutation on rejection");
}

/// NaN bounds are rejected (API-06.2).
#[test]
fn add_variable_rejects_nan_bounds() {
    let mut model = Model::new();
    let err = model
        .add_variable(continuous().bounds(f64::NAN, 0.0))
        .expect_err("NaN bounds must be rejected");
    assert_eq!(err, ModelError::InvalidBounds);
}

/// `add_parameter(parameter(1.0).named("price"))` returns a `Parameter` and
/// stores the initial value.
#[test]
fn parameter_builder_adds_named_parameter() {
    let mut model = Model::new();
    let price: Parameter = model
        .add_parameter(parameter(1.0).named("price"))
        .expect("valid parameter");
    assert_eq!(model.parameter_value(price), Some(1.0));
}

/// `add_parameter(f64)` still compiles through the `From<f64>` bridge.
#[test]
fn add_parameter_accepts_plain_f64() {
    let mut model = Model::new();
    let p: Parameter = model.add_parameter(2.5).expect("valid parameter");
    assert_eq!(model.parameter_value(p), Some(2.5));
}

/// Non-finite parameter values are rejected (API-06.2, D10).
#[test]
fn add_parameter_rejects_non_finite() {
    let mut model = Model::new();
    assert!(model.add_parameter(f64::NAN).is_err());
    assert!(model.add_parameter(f64::INFINITY).is_err());
    assert_eq!(model.num_parameters(), 0);
}

/// `set_parameter` is fallible and applies the queued change on commit.
#[test]
fn set_parameter_is_fallible_and_applies() {
    let mut model = Model::new();
    let price = model.add_parameter(parameter(1.0)).expect("valid");
    model.set_parameter(price, 3.0).expect("valid value");
    model.commit().expect("commit succeeds");
    assert_eq!(model.parameter_value(price), Some(3.0));
}

/// `set_parameter` rejects a stale/unknown parameter id (API-06.3).
#[test]
fn set_parameter_rejects_unknown_parameter() {
    let mut model = Model::new();
    let err = model
        .set_parameter(fake_param(), 3.0)
        .expect_err("unknown parameter must be rejected");
    assert_eq!(err, ModelError::ParameterNotFound(fake_param()));
}

/// `set_parameter` rejects non-finite values without queueing a change.
#[test]
fn set_parameter_rejects_non_finite() {
    let mut model = Model::new();
    let price = model.add_parameter(parameter(1.0)).expect("valid");
    assert!(model.set_parameter(price, f64::NAN).is_err());
    model.commit().expect("commit succeeds");
    assert_eq!(model.parameter_value(price), Some(1.0), "unchanged");
}

/// `add_constraint(spec)` accepts the fluent constraint spec (API-04.1).
#[test]
fn add_constraint_accepts_fluent_spec() {
    let mut model = Model::new();
    let x: Variable = model.add_variable(continuous()).expect("var");
    let y: Variable = model.add_variable(continuous()).expect("var");
    let con = model
        .add_constraint((x + y).le(4.0))
        .expect("constraint spec accepted");
    assert_eq!(model.num_constraints(), 1);
    assert_eq!(
        model.constraint_bounds(con),
        Some(ConstraintBounds::le(4.0))
    );
}

/// `add_constraint(ConstraintBounds)` keeps compiling through the
/// `From<ConstraintBounds>` bridge and is fallible.
#[test]
fn add_constraint_accepts_raw_bounds() {
    let mut model = Model::new();
    let con: ConId = model
        .add_constraint(ConstraintBounds::le(4.0))
        .expect("raw bounds accepted");
    assert_eq!(
        model.constraint_bounds(con),
        Some(ConstraintBounds::le(4.0))
    );
}

/// `add_constraint` with a named spec compiles and stores the constraint.
#[test]
fn add_constraint_with_named_spec_compiles() {
    let mut model = Model::new();
    let x: Variable = model.add_variable(continuous()).expect("var");
    let con = model
        .add_constraint((x).le(10.0).named("capacity"))
        .expect("named spec accepted");
    assert_eq!(model.num_constraints(), 1);
    assert_eq!(
        model.constraint_bounds(con),
        Some(ConstraintBounds::le(10.0))
    );
}

/// `Model::constrain` still works and routes through the spec path.
#[test]
fn constrain_route_still_compiles() {
    let mut model = Model::new();
    let x: Variable = model.add_variable(continuous()).expect("var");
    let con = model.constrain((x).ge(1.0)).expect("constrain works");
    assert_eq!(model.num_constraints(), 1);
    assert_eq!(
        model.constraint_bounds(con),
        Some(ConstraintBounds::ge(1.0))
    );
}

/// `Model::maximize` with the new variable/parameter aliases compiles and
/// activates the objective (the fixture algebra).
#[test]
fn maximize_works_with_variable_and_parameter_aliases() {
    let mut model = Model::new();
    let x: Variable = model.add_variable(continuous()).expect("var");
    let price: Parameter = model.add_parameter(parameter(1.0)).expect("param");
    let obj = model.maximize(price * x).expect("objective accepted");
    assert_eq!(model.active_objective(), Some(obj));
}

/// The semantic aliases are the raw id types (D8: aliases, not wrappers).
#[test]
fn semantic_aliases_are_the_raw_id_types() {
    let mut model = Model::new();
    let x: Variable = model.add_variable(continuous()).expect("var");
    let p: Parameter = model.add_parameter(1.0).expect("param");
    let c: Constraint = model.add_constraint((x).le(1.0)).expect("constraint");
    let o: Objective = model.maximize(p * x).expect("objective");
    // Type-level proof: an alias is the same type as the id.
    let _as_var: VarId = x;
    let _as_param: ParamId = p;
    let _as_con: ConId = c;
    let _as_obj: ObjId = o;
}
