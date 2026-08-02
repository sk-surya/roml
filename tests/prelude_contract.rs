//! P23 Task 1 — The curated default prelude is sufficient for ordinary model
//! authors and excludes protocol/backend-extension internals (API-07.1/07.2).
//!
//! This file imports ONLY `roml::prelude::*`. If a golden-path modeling
//! workflow needs something not in the prelude, that is a prelude defect
//! (API-07.1) — fix the prelude, not this file. The negative inventory (the
//! protocol/backend types that must be ABSENT) is enforced by the
//! `compile_fail` doctests attached to the prelude module documentation in
//! `src/lib.rs`.

use roml::prelude::*;

/// The full ordinary modeling workflow compiles and runs using only the
/// prelude: named model, definition builders, names, canonical constraints,
/// the pure `constraint!` builder, canonical objectives, and introspection.
#[test]
fn prelude_supports_full_ordinary_workflow() -> Result<(), Box<dyn std::error::Error>> {
    let mut model = Model::named("production");

    let x = model.add_variable(continuous().named("x"))?;
    let y = model.add_variable(integer().bounds(0.0, 10.0).named("y"))?;
    let z = model.add_variable(binary())?;
    let price = model.add_parameter(parameter(1.0).named("price"))?;

    // The pure `constraint!` builder is re-exported by the prelude.
    let cap = constraint!(x + y <= 4.0).named("capacity");
    model.add_constraint(cap)?;
    model.add_constraint((price * x + z).ge(1.0))?;

    let obj = model.maximize(3.0 * x + price * y)?;

    assert_eq!(model.num_variables(), 3);
    assert_eq!(model.num_parameters(), 1);
    assert_eq!(model.num_constraints(), 2);
    assert_eq!(model.variable_name(x)?, Some("x"));
    assert_eq!(model.parameter_value(price), Some(1.0));
    assert_eq!(model.active_objective(), Some(obj));

    let bounds = model.variable_bounds(z).expect("z bounds");
    assert_eq!(bounds, Bounds::BINARY);
    Ok(())
}

/// Solve-option building, status naming, and error naming are available from
/// the prelude without any extra import (API-07.1 solver/solution/error
/// vocabulary).
#[test]
fn prelude_covers_solve_and_solution_vocabulary() {
    // Build a non-trivial option set entirely from prelude-named types.
    let options = SolveOptions::new()
        .threads(2)
        .relative_gap(0.01)
        .output(true)
        .random_seed(7)
        .backend_option("presolve", "off")
        .time_limit(std::time::Duration::from_secs(1));
    let _ = options.clone();

    let status = SolveStatus::Optimal;
    assert!(status.is_optimal());
    assert!(!SolveStatus::Infeasible.is_optimal());

    // The error type is nameable and constructible.
    let _err: SolveError = SolveError::NoActiveObjective;

    let sol = Solution::new(SolveStatus::Infeasible);
    assert!(!sol.has_values());
}

/// Semantic aliases are the prelude's entity handles; `Bounds`, `Sense`, and
/// `VarType` remain available for ordinary modeling, and the definition
/// builder result types are nameable.
#[test]
fn prelude_provides_semantic_aliases_and_model_vocabulary() {
    let mut model = Model::new();
    let x: Variable = model
        .add_variable(continuous().bounds(0.0, 5.0))
        .expect("x");
    let b: Bounds = model.variable_bounds(x).expect("bounds");
    assert!(b.is_valid());

    let _ = VarType::Integer;
    let _ = Sense::Maximize;
    let _def: VariableDef = continuous().lower_bound(-1.0).named("free");
    let _pdef: ParameterDef = parameter(1.0);
}
