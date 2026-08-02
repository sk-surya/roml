//! Compiled-and-run fixture for the `MODELING_API.md` guide snippets (API-09.3).
//!
//! Every inline snippet the guide teaches (canonical path first, advanced
//! escape hatches labeled) is exercised here against the real HiGHS backend.
//! If a guide snippet stops compiling or asserting, this file pinpoints the
//! drift. The guide's major workflows additionally link the compiled examples
//! in `roml-highs/examples/`.

use std::time::Duration;

use roml::prelude::*;
use roml_highs::Highs;

/// Chapter 1 — model and entity definitions.
#[test]
fn guide_entities_definitions_and_names() -> Result<(), Box<dyn std::error::Error>> {
    let mut model = Model::named("guide");

    let x = model.add_variable(continuous().named("x"))?;
    let y = model.add_variable(integer().bounds(0.0, 10.0).named("y"))?;
    let z = model.add_variable(binary().named("z"))?;
    let w = model.add_variable(continuous().lower_bound(1.0).named("w"))?;
    let price = model.add_parameter(parameter(1.0).named("price"))?;

    assert_eq!(model.num_variables(), 4);
    assert_eq!(model.num_parameters(), 1);
    assert_eq!(model.variable_name(x)?, Some("x"));
    assert_eq!(model.variable_name(y)?, Some("y"));
    assert_eq!(model.variable_name(z)?, Some("z"));
    assert_eq!(model.parameter_name(price)?, Some("price"));

    // Names are optional; unnamed entities still exist.
    let _anon = model.add_variable(continuous())?;
    assert_eq!(model.num_variables(), 5);
    Ok(())
}

/// Chapters 2 and 3 — expressions, constraints, and objectives.
#[test]
fn guide_constraints_and_objectives() -> Result<(), Box<dyn std::error::Error>> {
    let mut model = Model::new();
    let x = model.add_variable(continuous())?;
    let y = model.add_variable(continuous())?;
    let price = model.add_parameter(parameter(1.0))?;

    // Fluent constraint builders are canonical (API-04.3).
    model.add_constraint((x + y).le(4.0).named("capacity"))?;
    model.add_constraint(x.ge(1.0))?;
    model.add_constraint(y.between(0.0, 3.0))?;
    model.add_constraint((2.0 * x - y).eq(1.0))?;

    // `constraint!` is an optional PURE spec builder (API-04.4).
    let spec = constraint!(x + y <= 4.0);
    model.add_constraint(spec)?;
    assert_eq!(model.num_constraints(), 5);

    // Parameters participate in expressions.
    let _expr = price * x + y;

    // Canonical single-objective mutations (API-04.2); constants fold once.
    let obj = model.maximize(3.0 * x + y + 2.0)?;
    assert_eq!(model.objective_constant(obj), Some(2.0));
    assert_eq!(model.active_objective(), Some(obj));
    Ok(())
}

/// Chapter 4 — names and diagnostics.
#[test]
fn guide_names_and_diagnostics() -> Result<(), Box<dyn std::error::Error>> {
    let mut model = Model::named("diag");
    let x = model.add_variable(continuous().named("x"))?;
    model.add_constraint(x.le(3.0).named("cap"))?;
    model.maximize(x)?;

    let text = model.pprint();
    assert!(text.contains("diag"), "pprint should show the model name");
    assert!(text.contains("cap"), "pprint should show constraint names");
    assert!(model.variable_name(x).is_ok());
    Ok(())
}

/// Chapter 5 — solving with HiGHS.
#[test]
fn guide_solve_and_solution_access() -> Result<(), Box<dyn std::error::Error>> {
    let mut model = Model::named("solve");
    let x = model.add_variable(continuous())?;
    model.add_constraint(x.le(10.0))?;
    model.maximize(x)?;

    let mut highs = Highs::new()?;
    let solution = highs.solve(&mut model)?;

    assert!(solution.status().is_optimal());
    assert_eq!(solution.value(x), Some(10.0));
    assert_eq!(solution.objective_value(), Some(10.0));
    assert_eq!(solution.objective_id(), model.active_objective());

    let metadata = solution.metadata();
    assert!(!metadata.backend_name.is_empty());
    Ok(())
}

/// Chapter 6 — solve options and effective configuration.
#[test]
fn guide_solve_options_and_effective_configuration() -> Result<(), Box<dyn std::error::Error>> {
    let mut model = Model::named("options");
    let x = model.add_variable(continuous())?;
    model.add_constraint(x.le(10.0))?;
    model.maximize(x)?;

    let options = SolveOptions::new()
        .time_limit(Duration::from_secs(60))
        .relative_gap(0.1)
        .threads(1)
        .output(false);

    let mut highs = Highs::new()?;
    let solution = highs.solve_with(&mut model, options)?;
    assert!(solution.status().is_optimal());

    let effective = &solution.metadata().effective_configuration;
    assert_eq!(effective.mip_rel_gap, Some(0.1));
    assert_eq!(effective.threads, Some(1));
    Ok(())
}

/// Chapter 7 — solution/status semantics.
#[test]
fn guide_status_semantics_math_vs_operational() -> Result<(), Box<dyn std::error::Error>> {
    // Operational failure (invalid options) returns Err, never a Solution, and
    // leaves the model and backend state unchanged.
    let mut model = Model::new();
    let a = model.add_variable(continuous())?;
    model.add_constraint(a.le(1.0))?;
    model.maximize(a)?;
    let mut highs = Highs::new()?;
    let invalid = SolveOptions::new().relative_gap(-1.0);
    let err = highs.solve_with(&mut model, invalid).unwrap_err();
    assert!(matches!(err, SolveError::InvalidOptions(_)));

    // Mathematical termination (infeasible) returns Ok(Solution) with status
    // and no primal values.
    let mut infeasible = Model::new();
    let b = infeasible.add_variable(continuous())?;
    infeasible.add_constraint(b.le(1.0))?;
    infeasible.add_constraint(b.ge(2.0))?;
    infeasible.maximize(b)?;
    let solution = highs.solve(&mut infeasible)?;
    assert_eq!(solution.status(), SolveStatus::Infeasible);
    assert!(!solution.has_values());
    Ok(())
}

/// Chapter 8 — parameters and repeated solves.
#[test]
fn guide_parameters_and_repeated_solves() -> Result<(), Box<dyn std::error::Error>> {
    let mut model = Model::named("repeated");
    let x = model.add_variable(continuous())?;
    let y = model.add_variable(continuous())?;
    let price = model.add_parameter(parameter(1.0))?;

    model.add_constraint((x + y).le(4.0))?;
    model.maximize(price * x + y)?;

    let mut highs = Highs::new()?;
    let first = highs.solve(&mut model)?;
    assert_eq!(first.objective_value(), Some(4.0));

    // Fallible parameter mutation; stale IDs are rejected (API-06.3).
    model.set_parameter(price, 3.0)?;
    let second = highs.solve(&mut model)?;
    assert_eq!(second.objective_value(), Some(12.0));
    Ok(())
}

/// Chapter 9 — sparse construction (advanced escape hatch, labeled).
#[test]
fn guide_sparse_cells() -> Result<(), Box<dyn std::error::Error>> {
    use roml::advanced::{CoefficientTarget, ConstraintBounds};

    let mut model = Model::named("sparse");
    let x = model.add_variable(continuous())?;
    let y = model.add_variable(continuous())?;

    // Start from an empty row and populate cells by coordinate (D11).
    let con = model.add_constraint(ConstraintSpec::new(
        LinExpr::new(),
        ConstraintBounds::le(8.0),
    ))?;
    model.set_coefficient(CoefficientTarget::Constraint(con), x, 2.0)?;
    model.add_to_coefficient(CoefficientTarget::Constraint(con), x, 1.0)?;
    model.set_coefficient(CoefficientTarget::Constraint(con), y, 3.0)?;
    model.remove_coefficient_at(CoefficientTarget::Constraint(con), y)?;

    model.maximize(x)?;
    let mut highs = Highs::new()?;
    let solution = highs.solve(&mut model)?;
    assert!(solution.status().is_optimal());
    assert!((solution.value_or_zero(x) - 8.0 / 3.0).abs() < 1e-6);
    Ok(())
}

/// Chapter 11 — validation and common errors.
#[test]
fn guide_validation_and_errors() -> Result<(), Box<dyn std::error::Error>> {
    let mut model = Model::new();

    // Inverted bounds are rejected before any model mutation.
    assert!(matches!(
        model.add_variable(continuous().bounds(5.0, 1.0)),
        Err(ModelError::InvalidBounds)
    ));

    // Binary definitions must stay within the unit interval.
    assert!(matches!(
        model.add_variable(binary().bounds(-1.0, 1.0)),
        Err(ModelError::InvalidBinaryBounds)
    ));

    // Stale IDs are rejected with typed errors.
    let x = model.add_variable(continuous())?;
    model.remove_variable(x)?;
    assert!(model.set_variable_bounds(x, Bounds::new(0.0, 2.0)).is_err());
    Ok(())
}
