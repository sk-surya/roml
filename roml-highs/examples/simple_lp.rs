//! Simple LP example — the ROML golden path (README quick start).
//!
//! Builds a small production model, solves it with HiGHS, and inspects the
//! solution:
//!
//! ```text
//! maximize 3x + y
//! subject to  x + y <= 4
//!             x     <= 3
//!             x, y >= 0
//! ```
//!
//! The optimum is `x = 3`, `y = 1`, objective `10`.
//!
//! Run with: `cargo run -p roml-highs --example simple_lp`
//!
//! This example is verified by `roml-highs/tests/readme_quickstart.rs`.

use roml::prelude::*;
use roml_highs::Highs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut model = Model::named("production");

    let x = model.add_variable(continuous().named("x"))?;
    let y = model.add_variable(continuous().named("y"))?;

    model.add_constraint((x + y).le(4.0).named("capacity"))?;
    model.add_constraint(x.le(3.0))?;

    model.maximize(3.0 * x + y)?;

    let mut highs = Highs::new()?;
    let solution = highs.solve(&mut model)?;

    println!("status:    {:?}", solution.status());
    println!("x =        {:?}", solution.value(x));
    println!("y =        {:?}", solution.value(y));
    println!("objective = {:?}", solution.objective_value());

    assert!(solution.status().is_optimal());
    assert_eq!(solution.value(x), Some(3.0));
    assert_eq!(solution.value(y), Some(1.0));
    assert_eq!(solution.objective_value(), Some(10.0));
    Ok(())
}
