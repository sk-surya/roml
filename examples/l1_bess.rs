//! L1 BESS energy-arbitrage model (MIR-04, IR-24).
//!
//! Built entirely with the array surface: structured variable/parameter
//! handles, metadata-only slicing, explicit residual composition, and the
//! automatic packed objective. There are no raw variable ids and no manual
//! linear-expression construction in ordinary model code.
//!
//! Run with: `cargo run -p roml --example l1_bess`

use roml::Model;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (buses, periods) = (4usize, 24usize);
    let dt = 0.25;
    let efficiency = 0.95;
    let power = 2.0;
    let capacity = 4.0;

    let mut model = Model::named("l1-bess");
    let charge = model
        .var("charge", [buses, periods])
        .bounds(0.0, power)
        .build()?;
    let discharge = model
        .var("discharge", [buses, periods])
        .bounds(0.0, power)
        .build()?;
    let energy = model
        .var("energy", [buses, periods])
        .bounds(0.0, capacity)
        .build()?;
    let prices: Vec<f64> = (0..buses * periods)
        .map(|k| 40.0 + ((k % 12) as f64) * 3.0)
        .collect();
    let price = model.param("price", [buses, periods], &prices)?;

    // energy[:, 1:] == energy[:, :-1] + dt * (eta * charge[:, 1:] - discharge[:, 1:] / eta)
    let next_energy = energy.slice(1, 1, periods - 1)?;
    let previous_energy = energy.slice(1, 0, periods - 1)?;
    let next_charge = charge.slice(1, 1, periods - 1)?;
    let next_discharge = discharge.slice(1, 1, periods - 1)?;
    let inventory = previous_energy.clone()
        + dt * (efficiency * next_charge.clone() - next_discharge.clone() / efficiency);
    model.add_row((next_energy.clone() - inventory).eq(0.0))?;

    // maximize dt * price * (discharge - charge)
    let objective = price
        .try_mul(&(discharge.clone() - charge.clone()))?
        .expect("conservative IR covers price * (discharge - charge)")
        * dt;
    model.maximize_array(&objective)?;

    let lowering = model.lowering_stats();
    println!(
        "bess: variables={} constraints={} cells={}",
        model.num_variables(),
        model.num_constraints(),
        model.num_coefficients()
    );
    println!(
        "bess: param_blocks={} param_cells={} general_affine={}",
        lowering.param_dep_blocks, lowering.param_positions_cells, lowering.general_affine
    );
    println!(
        "bess: fingerprint={}",
        model.normalized_ordinal_fingerprint()?
    );
    Ok(())
}
