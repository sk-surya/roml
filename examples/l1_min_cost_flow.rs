//! L1 min-cost flow on a line network (MIR-04, IR-24).
//!
//! Source/sink fixing and interior conservation are cell-wise rows over
//! metadata-only slices; the parametric cost objective packs automatically.
//! No raw variable ids and no manual linear-expression construction appear in
//! ordinary model code.
//!
//! Run with: `cargo run -p roml --example l1_min_cost_flow`

use roml::Model;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let edges = 8usize;
    let throughput = 5.0;
    // Interior net demands (sum to zero so source == sink is consistent).
    let demands = [1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 0.0];

    let mut model = Model::named("l1-min-cost-flow");
    let flow = model.var("flow", edges).bounds(0.0, 10.0).build()?;
    let costs: Vec<f64> = (0..edges).map(|k| 1.0 + ((k % 3) as f64)).collect();
    let cost = model.param("cost", edges, &costs)?;

    // Source and sink fix the throughput.
    model.add_row(flow.slice(0, 0, 1)?.expr()?.eq(throughput))?;
    model.add_row(flow.slice(0, edges - 1, 1)?.expr()?.eq(throughput))?;

    // Interior conservation: flow[k - 1] - flow[k] == demand[k] for k = 1..edges.
    let inlet = flow.slice(0, 0, edges - 1)?;
    let outlet = flow.slice(0, 1, edges - 1)?;
    let conservation = inlet.clone() - outlet.clone();
    model.add_row(conservation.eq_each(&demands)?)?;

    // minimize Σ cost * flow.
    let objective = cost
        .try_mul(&flow.expr()?)?
        .expect("conservative IR covers cost * flow");
    model.minimize_array(&objective)?;

    let lowering = model.lowering_stats();
    println!(
        "min_cost_flow: variables={} constraints={} cells={}",
        model.num_variables(),
        model.num_constraints(),
        model.num_coefficients()
    );
    println!(
        "min_cost_flow: param_blocks={} param_cells={} general_affine={}",
        lowering.param_dep_blocks, lowering.param_positions_cells, lowering.general_affine
    );
    println!(
        "min_cost_flow: fingerprint={}",
        model.normalized_ordinal_fingerprint()?
    );
    Ok(())
}
