//! P21 gate — the frozen incremental re-solve contract now compiles and
//! executes.
//!
//! This test is the promoted P20 target contract (previously
//! `tests/ui/target_incremental.rs`). One `Highs` instance is reused across
//! solves; `set_parameter` is fallible; the second `solve` re-optimizes
//! after the parameter change without any user-facing `commit`,
//! `synchronize`, snapshot, or cursor calls (D5/D7).
//!
//! Do NOT weaken these signatures — they are the M2 contract (plan Task 3).

use roml::prelude::*;
use roml_highs::Highs;

/// The M2 incremental workflow: one `Highs`, two solves, a parameter change
/// in between — synchronization is automatic.
#[test]
fn incremental_compiles_and_runs() -> Result<(), Box<dyn std::error::Error>> {
    let mut model = Model::new();
    let x = model.add_variable(continuous())?;
    let price = model.add_parameter(parameter(1.0).named("price"))?;
    model.add_constraint(x.le(10.0))?;
    model.maximize(price * x)?;
    let mut highs = Highs::new()?;
    let _first = highs.solve(&mut model)?;
    model.set_parameter(price, 3.0)?;
    let _second = highs.solve(&mut model)?;
    Ok(())
}
