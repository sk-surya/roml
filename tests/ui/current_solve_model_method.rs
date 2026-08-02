//! P20 Task 2 — Characterization fixture: the documented `solve_model` method
//! does not exist on the real session type.
//!
//! `README.md` (and `MODELING_API.md`) document `adapter.solve_model(&mut
//! model)`. The import-level drift (`HighsAdapter` does not exist) is frozen
//! in `tests/ui/current_readme_drift.rs` (E0432). This second fixture
//! freezes the method-level drift: calling the documented method on the
//! actual public session type `roml_highs::HighsSession` fails with
//! `error[E0599]: no method named `solve_model``.
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
//! cp tests/ui/current_solve_model_method.rs roml-highs/tests/zz_solve_model_method_capture.rs
//! cargo check -p roml-highs --test zz_solve_model_method_capture   # expect: E0599 no method `solve_model`
//! rm roml-highs/tests/zz_solve_model_method_capture.rs
//! ```
//!
//! Or run `scripts/p20-capture-drift.sh`, which reproduces both drift
//! failures (E0432 and E0599) from the committed fixtures and asserts the
//! expected error codes.
//!
//! P21 introduces `roml_highs::Highs` (façade over `HighsSession`) with the
//! `solve` method; the README is rewritten against the accepted target API
//! in P24.

use roml::prelude::*;
use roml_highs::HighsSession;

#[allow(dead_code)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut model = Model::new();

    let x = model.add_var();
    let y = model.add_var();

    model
        .constrain((x + y).le(4.0))
        .expect("constraint should build");
    model
        .maximize(x + 2.0 * y + 5.0)
        .expect("objective should build");
    model.commit();

    let mut adapter = HighsSession::try_new()?;
    let solution = adapter.solve_model(&mut model)?;

    assert!(solution.is_optimal());
    Ok(())
}
