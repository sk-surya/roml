//! Solve options example — duration, gap, output, and effective configuration.
//!
//! `solve_with` passes an ergonomically built [`SolveOptions`] to the backend.
//! The returned [`Solution`] carries [`SolveMetadata`] with the effective
//! configuration, including any adjustments and rejections the backend
//! negotiated. Options are validated before synchronization, so an invalid
//! option never touches the model or backend state.
//!
//! Run with: `cargo run -p roml-highs --example solve_options`

use std::time::Duration;

use roml::prelude::*;
use roml_highs::Highs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut model = Model::named("solve_options");

    let x = model.add_variable(continuous().named("x"))?;
    model.add_constraint(x.le(10.0))?;
    model.maximize(x)?;

    let options = SolveOptions::new()
        .time_limit(Duration::from_secs(30))
        .relative_gap(0.05)
        .output(false);

    let mut highs = Highs::new()?;
    let solution = highs.solve_with(&mut model, options)?;

    assert!(solution.status().is_optimal());
    assert_eq!(solution.value(x), Some(10.0));

    let metadata = solution.metadata();
    println!("backend:            {}", metadata.backend_name);
    println!("model revision:     {:?}", metadata.model_revision);
    println!("synchronization:    {:?}", metadata.synchronization);
    println!(
        "effective threads:  {:?}",
        metadata.effective_configuration.threads
    );
    println!(
        "effective gap:      {:?}",
        metadata.effective_configuration.mip_rel_gap
    );
    println!(
        "adjustments:        {:?}",
        metadata.effective_configuration.adjustments
    );
    println!(
        "rejections:         {:?}",
        metadata.effective_configuration.rejections
    );
    Ok(())
}
