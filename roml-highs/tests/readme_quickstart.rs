//! Compiled-and-run fixture for the README quick-start example (API-09.1).
//!
//! The README's primary code block (section "Quick start — build and solve an
//! LP with HiGHS") is extracted here and run against the real HiGHS backend.
//! If this test stops compiling or passing, the README example has drifted
//! from the accepted M2 golden path and must be corrected.

use roml::prelude::*;
use roml_highs::Highs;

/// The README quick-start model: `maximize 3x + y` subject to
/// `x + y <= 4`, `x <= 3`, `x, y >= 0`. Solves to `x = 3, y = 1, obj = 10`.
#[test]
fn readme_quickstart_compiles_and_runs() -> Result<(), Box<dyn std::error::Error>> {
    let mut model = Model::named("production");

    let x = model.add_variable(continuous().named("x"))?;
    let y = model.add_variable(continuous().named("y"))?;

    model.add_constraint((x + y).le(4.0).named("capacity"))?;
    model.add_constraint(x.le(3.0))?;

    model.maximize(3.0 * x + y)?;

    let mut highs = Highs::new()?;
    let solution = highs.solve(&mut model)?;

    assert!(solution.status().is_optimal());
    assert_eq!(solution.value(x), Some(3.0));
    assert_eq!(solution.value(y), Some(1.0));
    assert_eq!(solution.objective_value(), Some(10.0));
    Ok(())
}
