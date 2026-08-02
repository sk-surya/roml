//! P21 gate — the frozen golden-path quickstart contract now compiles and
//! executes.
//!
//! This test is the promoted P20 target contract (previously
//! `tests/ui/target_quickstart.rs`). It pins the EXACT target API accepted
//! for M2 (`DECISIONS.md` D1/D3/D4/D7) and proves it end-to-end on the real
//! HiGHS backend: canonical method-first modeling, definition builders,
//! entity names, `add_constraint(spec)`, and the `roml_highs::Highs` façade.
//!
//! Do NOT weaken these signatures — they are the M2 contract (plan Task 3).

use roml::prelude::*;
use roml_highs::Highs;

/// The M2 quickstart: build a named LP/MILP, solve with HiGHS, inspect the
/// solution — no revisions, snapshots, deltas, or synchronization calls.
#[test]
fn quickstart_compiles_and_runs() -> Result<(), Box<dyn std::error::Error>> {
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
