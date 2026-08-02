//! P20 Task 3 — Frozen target contract: incremental re-solve on one `Highs`.
//!
//! This fixture pins the exact target API for the parameterized incremental
//! workflow (M2 outcome in the packet README, `DECISIONS.md` D5/D7). One
//! `Highs` instance is reused across solves; `set_parameter` is fallible; the
//! second `solve` re-optimizes after the parameter change without any
//! user-facing `commit`, `synchronize`, snapshot, or cursor calls.
//!
//! # Why this file is a "UI" fixture, not a default test
//!
//! Files under `tests/ui/` are NOT auto-discovered by Cargo, so this
//! intentionally-non-compiling target contract never breaks
//! `cargo test --all-targets` (API-10.1).
//!
//! # Status
//!
//! **Does not compile today.** `Model::new`, `continuous`, `add_variable`,
//! `parameter`, `add_parameter`, `add_constraint`, `roml_highs::Highs`,
//! `Highs::new`, `Highs::solve`, and fallible `set_parameter` are P21/P22
//! work. When they land, this fixture becomes a compile-pass test. Do NOT
//! weaken these signatures to make the fixture pass early without amending
//! `DECISIONS.md` and review (plan Task 3).
//!
//! # Promote to a compile test
//!
//! ```text
//! cp tests/ui/target_incremental.rs tests/target_incremental_compile.rs
//! cargo test -p roml --test target_incremental_compile
//! ```

use roml::prelude::*;
use roml_highs::Highs;

#[allow(dead_code)]
fn incremental() -> Result<(), Box<dyn std::error::Error>> {
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
