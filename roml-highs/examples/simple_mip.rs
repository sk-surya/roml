//! Simple MILP example — binary and integer definitions, MIP status.
//!
//! Demonstrates the canonical definition builders for a mixed-integer model
//! and a HiGHS MIP solve:
//!
//! ```text
//! maximize 5x + 4y
//! subject to  2x + 3y <= 15
//!             x binary
//!             0 <= y <= 10, integer
//! ```
//!
//! The optimum is `x = 1`, `y = 4`, objective `21`.
//!
//! Run with: `cargo run -p roml-highs --example simple_mip`

use roml::prelude::*;
use roml_highs::Highs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut model = Model::named("simple_mip");

    // `binary()` gives a validated variable fixed to the unit interval;
    // `integer()` is non-negative unless bounds are overridden.
    let x = model.add_variable(binary().named("x"))?;
    let y = model.add_variable(integer().bounds(0.0, 10.0).named("y"))?;

    model.add_constraint((2.0 * x + 3.0 * y).le(15.0).named("resource"))?;

    model.maximize(5.0 * x + 4.0 * y)?;

    let mut highs = Highs::new()?;
    let solution = highs.solve(&mut model)?;

    println!("status:    {:?}", solution.status());
    println!("x =        {:?}", solution.value(x));
    println!("y =        {:?}", solution.value(y));
    println!("objective = {:?}", solution.objective_value());

    // A mathematical termination (here: optimal) returns `Ok(Solution)`.
    assert!(solution.status().is_optimal());

    // The binary variable is 0 or 1; the integer variable is integral.
    let xv = solution.value_or_zero(x);
    let yv = solution.value_or_zero(y);
    assert!(xv == 0.0 || xv == 1.0, "binary variable violated its domain: {xv}");
    assert_eq!(yv.fract(), 0.0, "integer variable was not integral: {yv}");
    assert_eq!(xv, 1.0);
    assert_eq!(yv, 4.0);
    assert_eq!(solution.objective_value(), Some(21.0));
    Ok(())
}
