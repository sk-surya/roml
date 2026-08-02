//! Incremental parameter update example — two solves with one `Highs` façade.
//!
//! A coefficient depends on the parameter `price`. The first solve runs with
//! `price = 1.0`; after `set_parameter`, the second solve runs on the same
//! `Highs` instance. The parameter delta is applied incrementally — no model
//! rebuild, no manual `commit`, `synchronize`, or `solve_model` calls.
//!
//! ```text
//! maximize price * x + y
//! subject to   x + y <= 4
//!              x, y >= 0
//!
//! price = 1.0 -> objective 4.0   (x + y = 4)
//! price = 3.0 -> objective 12.0  (x = 4, y = 0)
//! ```
//!
//! Run with: `cargo run -p roml-highs --example parameter_update`
//!
//! This example is verified by `roml-highs/tests/readme_incremental.rs`.

use roml::prelude::*;
use roml_highs::Highs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
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
    println!("price = 1.0 -> objective = {:?}", first.objective_value());

    // Change the parameter and re-solve on the same `Highs` instance.
    model.set_parameter(price, 3.0)?;

    let second = highs.solve(&mut model)?;
    assert!(second.status().is_optimal());
    assert_eq!(second.objective_value(), Some(12.0));
    println!("price = 3.0 -> objective = {:?}", second.objective_value());
    Ok(())
}
