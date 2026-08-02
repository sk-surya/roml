//! P20 Task 2 — Characterization fixture: the README's documented solve path
//! does not compile against the current public surface.
//!
//! `README.md` (and `MODELING_API.md`) document `roml_highs::HighsAdapter`
//! with `adapter.solve_model(&mut model)`. Neither `HighsAdapter` nor
//! `solve_model` exists in the current `roml-highs` crate — the only public
//! session type is `roml_highs::HighsSession` (see
//! `docs/release/evidence/M2_P20_BASELINE.md`). This fixture freezes the exact
//! documented code so the drift is visible and reproducible.
//!
//! # Why this file is a "UI" fixture, not a default test
//!
//! Files under `tests/ui/` are NOT auto-discovered by Cargo as integration
//! tests, so this intentionally-non-compiling fixture never breaks
//! `cargo test --all-targets` (API-10.1). It is characterization evidence
//! referenced by `M2_P20_BASELINE.md`.
//!
//! # Reproduce the failure
//!
//! ```text
//! cp tests/ui/current_readme_drift.rs tests/ui_tmp_zz_drift.rs
//! cargo check --test ui_tmp_zz_drift   # expect: unresolved import `HighsAdapter`
//! rm tests/ui_tmp_zz_drift.rs
//! ```
//!
//! P21 introduces `roml_highs::Highs` (façade over `HighsSession`); the README
//! is rewritten against the accepted target API in P24.

use roml::prelude::*;
use roml::{constrain, set_objective};
use roml_highs::HighsAdapter;

#[allow(dead_code)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut model = Model::new();

    let x = model.add_var();
    let y = model.add_var();
    let price = model.add_parameter(1.0);

    constrain!(model, x + y <= 4.0)?;
    constrain!(model, x <= 3.0)?;
    constrain!(model, between: 0.0, y, 3.0)?;

    let _obj = set_objective!(model, maximize: price * x + y + 2.0)?;

    model.set_parameter(price, 3.0);
    model.commit();

    let mut adapter = HighsAdapter::new();
    let solution = adapter.solve_model(&mut model)?;

    assert!(solution.is_optimal());
    Ok(())
}
