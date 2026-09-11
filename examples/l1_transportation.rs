//! L1 transportation model (MIR-04, IR-24).
//!
//! Supply and demand are leading-axis reduction rows (`Σ_j plan[i, j]`), and
//! the parametric cost objective is packed automatically. No raw variable ids
//! and no manual linear-expression construction appear in ordinary model code.
//!
//! Run with: `cargo run -p roml --example l1_transportation`

use roml::Model;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let supply_nodes = 3usize;
    let demand_nodes = 4usize;
    let supply = [15.0, 20.0, 25.0];
    let demand = [10.0, 12.0, 18.0, 20.0];

    let mut model = Model::named("l1-transportation");
    let shipment = model
        .var("shipment", [supply_nodes, demand_nodes])
        .bounds(0.0, 100.0)
        .build()?;
    let costs: Vec<f64> = (0..supply_nodes * demand_nodes)
        .map(|k| 1.0 + ((k % 5) as f64))
        .collect();
    let cost = model.param("cost", [supply_nodes, demand_nodes], &costs)?;

    // Supply: Σ_j shipment[i, j] == supply[i].
    model.add_rows(shipment.expr()?.rows_eq(&supply)?)?;
    // Demand: Σ_i shipment[i, j] == demand[j] (transpose the leading axis).
    model.add_rows(shipment.transpose(0, 1)?.expr()?.rows_eq(&demand)?)?;

    // minimize Σ cost * shipment.
    let objective = cost
        .try_mul(&shipment.expr()?)?
        .expect("conservative IR covers cost * shipment");
    model.minimize_array(&objective)?;

    let lowering = model.lowering_stats();
    println!(
        "transportation: variables={} constraints={} cells={}",
        model.num_variables(),
        model.num_constraints(),
        model.num_coefficients()
    );
    println!(
        "transportation: param_blocks={} param_cells={} general_affine={}",
        lowering.param_dep_blocks, lowering.param_positions_cells, lowering.general_affine
    );
    println!(
        "transportation: fingerprint={}",
        model.normalized_ordinal_fingerprint()?
    );
    Ok(())
}
