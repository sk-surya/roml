//! Compiled-and-run fixture for the README incremental example (API-09.2).
//!
//! The README's "Incremental parameter updates" code block is extracted here:
//! one `Highs` instance, two solves, a parameter change in between. The second
//! solve applies the parameter delta incrementally and re-optimizes.
//!
//! Model: `maximize price * x + y` subject to `x + y <= 4`, `x, y >= 0`.
//! With `price = 1.0` the optimum is `x + y = 4` (objective 4.0); with
//! `price = 3.0` it shifts to `x = 4, y = 0` (objective 12.0).

use roml::prelude::*;
use roml_highs::Highs;

#[test]
fn readme_incremental_compiles_and_runs() -> Result<(), Box<dyn std::error::Error>> {
    let mut model = Model::named("pricing");

    let x = model.add_variable(continuous().named("x"))?;
    let y = model.add_variable(continuous().named("y"))?;
    let price = model.add_parameter(parameter(1.0).named("price"))?;

    model.add_constraint((x + y).le(4.0).named("capacity"))?;
    model.maximize(price * x + y)?;

    let mut highs = Highs::new()?;

    let first = highs.solve(&mut model)?;
    assert!(first.status().is_optimal());
    assert_eq!(first.objective_value(), Some(4.0));

    model.set_parameter(price, 3.0)?;

    let second = highs.solve(&mut model)?;
    assert!(second.status().is_optimal());
    assert_eq!(second.objective_value(), Some(12.0));
    Ok(())
}
