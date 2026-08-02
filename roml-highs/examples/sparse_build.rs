//! Sparse construction example — explicit cell semantics without `CoeffId`.
//!
//! Larger models are often assembled coefficient-by-coefficient from sparse
//! matrix data. ROML's advanced cell APIs name matrix coordinates, not
//! implementation identities:
//!
//! - `set_coefficient(target, var, value)` — replace the canonical cell
//!   `(target, var)`;
//! - `add_to_coefficient(target, var, value)` — algebraically add to it;
//! - `remove_coefficient_at(target, var)` — remove it by coordinate.
//!
//! These live under `roml::advanced` because ordinary models use expression
//! builders (`(x + y).le(...)`). `CoeffId` never appears in user code.
//!
//! ```text
//! maximize 4x + y + 2z
//! subject to  2x + 3y      <= 8
//!             x            <= 4
//!                        z <= 5
//!             x, y, z >= 0
//! ```
//!
//! Run with: `cargo run -p roml-highs --example sparse_build`

use roml::advanced::{CoefficientTarget, ConstraintBounds};
use roml::prelude::*;
use roml_highs::Highs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut model = Model::named("sparse_build");

    let x = model.add_variable(continuous().named("x"))?;
    let y = model.add_variable(continuous().named("y"))?;
    let z = model.add_variable(continuous().named("z"))?;

    // Create an empty constraint row, then populate its cells explicitly.
    let resource = model.add_constraint(ConstraintSpec::new(
        LinExpr::new(),
        ConstraintBounds::le(8.0),
    ))?;
    model.set_coefficient(CoefficientTarget::Constraint(resource), x, 2.0)?;
    model.set_coefficient(CoefficientTarget::Constraint(resource), y, 3.0)?;

    // `add_to_coefficient` accumulates into one canonical cell:
    // the cell `(resource, x)` becomes 2.0 + 1.0 = 3.0.
    model.add_to_coefficient(CoefficientTarget::Constraint(resource), x, 1.0)?;

    // A second sparse row.
    let x_cap = model.add_constraint(ConstraintSpec::new(
        LinExpr::new(),
        ConstraintBounds::le(4.0),
    ))?;
    model.set_coefficient(CoefficientTarget::Constraint(x_cap), x, 1.0)?;

    let z_cap = model.add_constraint(ConstraintSpec::new(
        LinExpr::new(),
        ConstraintBounds::le(5.0),
    ))?;
    model.set_coefficient(CoefficientTarget::Constraint(z_cap), z, 1.0)?;

    model.maximize(4.0 * x + y + 2.0 * z)?;

    let mut highs = Highs::new()?;
    let solution = highs.solve(&mut model)?;

    println!("status:    {:?}", solution.status());
    println!("x =        {:?}", solution.value(x));
    println!("y =        {:?}", solution.value(y));
    println!("z =        {:?}", solution.value(z));
    println!("objective = {:?}", solution.objective_value());

    // Constraint: 3x + 3y <= 8, x <= 4, z <= 5. Optimal: x = 8/3, y = 0,
    // z = 5, objective = 4*(8/3) + 10 = 32/3 + 30/3 = 62/3.
    assert!(solution.status().is_optimal());
    assert!((solution.value_or_zero(x) - 8.0 / 3.0).abs() < 1e-6);
    assert!((solution.value_or_zero(z) - 5.0).abs() < 1e-6);
    Ok(())
}
