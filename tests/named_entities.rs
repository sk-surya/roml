//! P22 — Entity names and diagnostics: lifecycle and formatting tests.
//!
//! Pins the M2 name semantics (D6/D8): semantic aliases are plain type aliases
//! so expression operators work unchanged; names are first-class, queryable
//! metadata that survive model lifecycle operations; names are diagnostics,
//! not unique keys; and `pprint` prefers names while never panicking on
//! removed/stale entities (API-05, API-06).

use roml::id::{ConId, Generation, ObjId, ParamId, VarId};
use roml::prelude::*;

fn fake_var() -> VarId {
    VarId::new(999, Generation::new())
}

fn fake_param() -> ParamId {
    ParamId::new(999, Generation::new())
}

fn fake_con() -> ConId {
    ConId::new(999, Generation::new())
}

fn fake_obj() -> ObjId {
    ObjId::new(999, Generation::new())
}

// ────────────────────────────────────────────────────────────────────────────
// Task 2 — Semantic aliases and names
// ────────────────────────────────────────────────────────────────────────────

/// D8: the semantic aliases are plain type aliases, so all existing expression
/// operators compile and behave unchanged.
#[test]
fn aliases_support_expression_operators_unchanged() {
    let mut model = Model::new();
    let x: Variable = model.add_variable(continuous()).expect("x");
    let y: Variable = model.add_variable(continuous()).expect("y");
    let p: Parameter = model.add_parameter(parameter(2.0)).expect("p");

    // +, *, -, scalar, and parameter operators on alias handles.
    let expr: LinExpr = 3.0 * x + 2.0 * y - p * x + 5.0;
    assert_eq!(expr.num_terms(), 3);
    assert_eq!(expr.get_constant(), 5.0);

    let _con: Constraint = model.add_constraint(expr.le(20.0)).expect("con");
    let _obj: Objective = model.maximize(2.0 * x + y).expect("obj");
    assert_eq!(model.num_constraints(), 1);
    assert_eq!(model.num_objectives(), 1);
}

/// Named creation for all four entity types (D6, API-05).
#[test]
fn named_creation_for_all_entity_types() {
    let mut model = Model::named("model");
    let x = model.add_variable(continuous().named("x")).expect("x");
    let p = model
        .add_parameter(parameter(1.0).named("price"))
        .expect("p");
    let con = model
        .add_constraint((x).le(10.0).named("capacity"))
        .expect("con");
    let obj = model
        .set_objective((p * x).maximize().named("profit"))
        .expect("obj");

    assert_eq!(model.variable_name(x).expect("var name"), Some("x"));
    assert_eq!(model.parameter_name(p).expect("param name"), Some("price"));
    assert_eq!(
        model.constraint_name(con).expect("constraint name"),
        Some("capacity")
    );
    assert_eq!(
        model.objective_name(obj).expect("objective name"),
        Some("profit")
    );
}

/// Name getters return `Ok(None)` for valid, unnamed entities.
#[test]
fn name_getters_return_none_for_unnamed_valid_ids() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).expect("x");
    let p = model.add_parameter(1.0).expect("p");
    let con = model
        .add_constraint(ConstraintBounds::le(1.0))
        .expect("con");
    let obj = model.maximize(x).expect("obj");

    assert_eq!(model.variable_name(x).expect("var name"), None);
    assert_eq!(model.parameter_name(p).expect("param name"), None);
    assert_eq!(model.constraint_name(con).expect("constraint name"), None);
    assert_eq!(model.objective_name(obj).expect("objective name"), None);
}

/// Name getters reject stale IDs with typed errors (D10/API-06.3).
#[test]
fn name_getters_reject_stale_ids_with_typed_errors() {
    let model = Model::new();
    assert_eq!(
        model.variable_name(fake_var()).expect_err("stale var"),
        ModelError::VariableNotFound(fake_var())
    );
    assert_eq!(
        model.parameter_name(fake_param()).expect_err("stale param"),
        ModelError::ParameterNotFound(fake_param())
    );
    assert_eq!(
        model.constraint_name(fake_con()).expect_err("stale con"),
        ModelError::ConstraintNotFound(fake_con())
    );
    assert_eq!(
        model.objective_name(fake_obj()).expect_err("stale obj"),
        ModelError::ObjectiveNotFound(fake_obj())
    );
}

/// Duplicate names are permitted: names are diagnostics, not unique keys (D6).
#[test]
fn duplicate_names_are_diagnostics_not_unique_keys() {
    let mut model = Model::new();
    let a = model.add_variable(continuous().named("dup")).expect("a");
    let b = model.add_variable(continuous().named("dup")).expect("b");
    assert_ne!(a, b, "distinct handles for distinct entities");
    assert_eq!(model.variable_name(a).expect("a name"), Some("dup"));
    assert_eq!(model.variable_name(b).expect("b name"), Some("dup"));
    assert_eq!(model.num_variables(), 2);
}

/// Names survive clone and ordinary mutation (bounds/activity changes).
///
/// Snapshot/rebuild is a numeric reconstruction path — names are not part of
/// the rebuild identity (D6 non-goal: names are not stable serialized
/// identities), so they are only asserted to survive clone and mutation.
#[test]
fn names_survive_clone_and_ordinary_mutation() {
    let mut model = Model::named("base");
    let x = model.add_variable(continuous().named("x")).expect("x");
    let p = model
        .add_parameter(parameter(1.0).named("price"))
        .expect("p");
    let con = model.add_constraint((x).le(5.0).named("cap")).expect("con");
    let obj = model
        .set_objective((p * x).maximize().named("profit"))
        .expect("obj");

    // Ordinary mutation must not wipe names.
    model
        .set_variable_bounds(x, Bounds::new(0.0, 3.0))
        .expect("bounds");
    model.set_constraint_active(con, false).expect("active");
    assert_eq!(model.variable_name(x).expect("var name"), Some("x"));
    assert_eq!(model.parameter_name(p).expect("param name"), Some("price"));
    assert_eq!(model.constraint_name(con).expect("con name"), Some("cap"));
    assert_eq!(model.objective_name(obj).expect("obj name"), Some("profit"));

    // Clone preserves names and the model name.
    let cloned = model.clone();
    assert_eq!(cloned.name.as_deref(), Some("base"));
    assert_eq!(cloned.variable_name(x).expect("cloned var name"), Some("x"));
    assert_eq!(
        cloned.parameter_name(p).expect("cloned param name"),
        Some("price")
    );
    assert_eq!(
        cloned.constraint_name(con).expect("cloned con name"),
        Some("cap")
    );
    assert_eq!(
        cloned.objective_name(obj).expect("cloned obj name"),
        Some("profit")
    );
}

// ────────────────────────────────────────────────────────────────────────────
// Task 6 — Model diagnostics
// ────────────────────────────────────────────────────────────────────────────

/// `pprint` prefers names for variables, parameters, constraints, objectives,
/// and their expression terms, falling back to stable debug handles (x[N]) for
/// unnamed entities (D6, API-05.5).
#[test]
fn pprint_prefers_names() {
    let mut model = Model::named("production");
    let x = model.add_variable(continuous().named("units")).expect("x");
    let y = model.add_variable(continuous()).expect("y");
    let p = model
        .add_parameter(parameter(2.0).named("rate"))
        .expect("p");
    let _con = model
        .add_constraint((x + 2.0 * y).le(10.0).named("capacity"))
        .expect("con");
    let obj = model
        .set_objective((3.0 * x + p * y).maximize().named("profit"))
        .expect("obj");
    assert_eq!(model.active_objective(), Some(obj));

    let out = model.pprint();
    // Entity names appear in the headers.
    assert!(out.contains("\"units\""), "named variable header");
    assert!(out.contains("\"rate\""), "named parameter header");
    assert!(out.contains("\"capacity\""), "named constraint header");
    assert!(out.contains("\"profit\""), "named objective header");

    // Reconstructed expressions prefer the named variable and fall back to the
    // stable debug handle for the unnamed one (term order is not guaranteed).
    let con_line = out
        .lines()
        .find(|l| l.contains("capacity"))
        .expect("constraint line");
    assert!(
        con_line.contains("units"),
        "named variable in constraint expr"
    );
    assert!(
        con_line.contains("x[1]"),
        "unnamed variable debug fallback in constraint expr"
    );

    let obj_line = out
        .lines()
        .find(|l| l.contains("profit"))
        .expect("objective line");
    assert!(
        obj_line.contains("units"),
        "named variable in objective expr"
    );
    assert!(
        obj_line.contains("x[1]"),
        "unnamed variable debug fallback in objective expr"
    );
}

/// Formatting must never panic when entities are removed/stale.
#[test]
fn pprint_never_panics_on_removed_entities() {
    let mut model = Model::named("m");
    let x = model.add_variable(continuous().named("x")).expect("x");
    let y = model.add_variable(continuous().named("y")).expect("y");
    let con = model
        .add_constraint((x + y).le(10.0).named("cap"))
        .expect("con");
    model.maximize(x).expect("obj");
    assert!(model.pprint().contains("\"cap\""));

    // Removing a constraint must not break formatting.
    model.remove_constraint(con).expect("remove con");
    let out = model.pprint();
    assert!(
        !out.contains("cap"),
        "removed constraint no longer rendered"
    );

    // Removing a variable (cascading to its cells) must not break formatting.
    model.remove_variable(x).expect("remove x");
    let out = model.pprint();
    assert!(out.contains("\"y\""), "remaining named variable rendered");
}
