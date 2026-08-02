//! P20 Task 3 — Frozen target contract: golden-path quickstart.
//!
//! This fixture pins the EXACT target API accepted for M2 (see
//! `README.md` of the M2 packet and `DECISIONS.md` D1/D3/D4/D7). It uses the
//! canonical method-first style, definition builders, entity names, and the
//! `roml_highs::Highs` façade.
//!
//! # Why this file is a "UI" fixture, not a default test
//!
//! Files under `tests/ui/` are NOT auto-discovered by Cargo, so this
//! intentionally-non-compiling target contract never breaks
//! `cargo test --all-targets` (API-10.1).
//!
//! # Status
//!
//! **Does not compile today.** `Model::named`, `continuous`, `integer`,
//! `add_variable(VariableDef)`, `add_constraint(ConstraintSpec)`,
//! `roml_highs::Highs`, `Highs::new`, `Highs::solve`, and
//! `Solution::status().is_optimal()` are P21/P22 work.
//!
//! P21 implements `roml_highs::Highs` + `Solution`/`SolveStatus`; P22
//! implements `Model::named`, `add_variable(VariableDef)`, and `.named(...)`.
//! When those land, this fixture becomes a compile-pass test. Do NOT weaken
//! these signatures to make the fixture pass early without amending
//! `DECISIONS.md` and review (plan Task 3).
//!
//! # Promote to a compile test
//!
//! ```text
//! cp tests/ui/target_quickstart.rs tests/target_quickstart_compile.rs
//! cargo test -p roml --test target_quickstart_compile
//! ```

use roml::prelude::*;
use roml_highs::Highs;

#[allow(dead_code)]
fn quickstart() -> Result<(), Box<dyn std::error::Error>> {
    let mut model = Model::named("production");
    let x = model.add_variable(continuous().named("x"))?;
    let y = model.add_variable(integer().bounds(0.0, 10.0).named("y"))?;
    model.add_constraint((x + y).le(4.0).named("capacity"))?;
    model.maximize(3.0 * x + y)?;
    let mut highs = Highs::new()?;
    let solution = highs.solve(&mut model)?;
    assert!(solution.status().is_optimal());
    Ok(())
}
