//! MIR-00 characterization: the current BESS-shaped lowering route and
//! parameter-propagation cost, measured *before* any storage redesign.
//!
//! This test is deliberately observational. It does not assert a target
//! performance (that is MIR-08's job) and it does not claim the MIR fast path
//! exists. It records:
//!
//! - which lowering route the packed parametric objective takes;
//! - how many per-cell reverse-index entries that route materializes;
//! - how many per-cell lookups/evaluations one full repricing performs.
//!
//! Flagship cardinality: 300 batteries × 96 periods = 28,800 price
//! parameters driving 57,600 parameterized objective coefficient cells
//! (one charge + one discharge cell per price parameter).

#![allow(deprecated)]

use roml::prelude::*;
use roml::{ParamId, VarId};

const BATTERIES: usize = 300;
const PERIODS: usize = 96;
/// Price parameters (`BATTERIES * PERIODS`).
const PARAMS: usize = BATTERIES * PERIODS;
/// Objective cells driven by those parameters (charge + discharge).
const CELLS: usize = 2 * PARAMS;
/// Finite-difference scale applied to the price in the objective.
const DT: f64 = 0.25;

struct Bess {
    model: Model,
    price: Vec<ParamId>,
}

fn build_bess() -> Bess {
    let mut model = Model::new();
    let charge: Vec<VarId> = (0..PARAMS).map(|_| model.add_var()).collect();
    let discharge: Vec<VarId> = (0..PARAMS).map(|_| model.add_var()).collect();
    let price: Vec<ParamId> = (0..PARAMS)
        .map(|_| model.add_parameter(50.0).expect("parameter arena"))
        .collect();

    // `maximize DT * sum(price * (discharge - charge))` lowers to one
    // `scale × ParamId` cell per (variable, parameter): -DT for charge and
    // +DT for discharge.
    let mut vars = Vec::with_capacity(CELLS);
    let mut params = Vec::with_capacity(CELLS);
    let mut scales = Vec::with_capacity(CELLS);
    for (var, param) in charge.iter().zip(price.iter()) {
        vars.push(*var);
        params.push(*param);
        scales.push(-DT);
    }
    for (var, param) in discharge.iter().zip(price.iter()) {
        vars.push(*var);
        params.push(*param);
        scales.push(DT);
    }
    model
        .set_linear_objective_param_bulk(Sense::Maximize, &vars, &params, &scales, 0.0)
        .expect("packed parametric objective accepts distinct variables");

    Bess { model, price }
}

/// Characterize the construction route: the packed parametric bulk path is
/// selected, the general affine fallback is not, and the store materializes
/// exactly one reverse-index position per objective cell (the cost MIR-02
/// removes for eligible families).
#[test]
fn mir00_bess_objective_lowers_via_parametric_bulk() {
    let bess = build_bess();
    let lowering = bess.model.lowering_stats();

    assert_eq!(lowering.parametric_bulk, 1, "packed parametric objective");
    assert_eq!(lowering.general_affine, 0, "no general affine fallback");
    assert_eq!(lowering.numeric_bulk, 0, "no constant bulk insertion here");
    assert_eq!(lowering.param_dep_blocks, 0, "MIR-02 not implemented yet");
    assert_eq!(
        lowering.param_positions_cells as usize, CELLS,
        "one per-cell reverse-index position per objective cell"
    );
    assert_eq!(bess.model.num_parameters(), PARAMS);
    assert_eq!(bess.model.num_variables(), 2 * PARAMS);
}

/// One full repricing of all 28,800 price parameters. The current path walks
/// the per-cell reverse index twice (once to collect overlay candidates, once
/// to propagate packed cells) and materializes one `CoefficientValueChanged`
/// per changed cell. MIR-02's target is zero per-cell lookups and one packed
/// coefficient-patch batch for the eligible family.
#[test]
fn mir00_bess_reprice_walks_per_cell_reverse_index() {
    let mut bess = build_bess();
    // Flush construction so only propagation is measured.
    bess.model.commit().expect("construction commit");
    let base_revision = bess.model.current_revision();

    let repriced: Vec<f64> = (0..PARAMS).map(|i| 51.0 + (i % 17) as f64).collect();
    bess.model.reset_diagnostics();
    for (param, value) in bess.price.iter().zip(repriced.iter()) {
        bess.model
            .set_parameter(*param, *value)
            .expect("live param");
    }
    bess.model.commit().expect("reprice commit");

    // The committed delta is one batch: one `ParameterValueChanged` op per
    // parameter plus one coefficient op per changed cell. MIR-02's packed
    // commit replaces this with one packed parameter-value change plus one
    // packed coefficient-patch batch for the eligible family.
    let batches = bess
        .model
        .deltas_since(base_revision)
        .expect("retained reprice delta");
    let delta_ops: usize = batches.iter().map(|b| b.operations.len()).sum();
    assert_eq!(
        delta_ops,
        PARAMS + CELLS,
        "one parameter op per parameter plus one coefficient op per cell"
    );

    let propagation = bess.model.propagation_stats();
    assert_eq!(
        propagation.param_position_lookups as usize, CELLS,
        "packed reverse-index positions examined once per cell"
    );
    assert_eq!(
        propagation.overlay_lookups as usize, CELLS,
        "collecting overlay candidates walks the packed reverse index too"
    );
    assert_eq!(
        propagation.value_expr_evals, 0,
        "packed parametric cells evaluate scale*value, not a ValueExpr"
    );
    assert_eq!(
        propagation.coefficient_patch_batches, 0,
        "MIR-00 has no packed coefficient-patch batch"
    );

    // Report the measured shape so `--nocapture` evidence is explicit.
    println!(
        "MIR-00 reprice: params={PARAMS} cells={CELLS} \
         param_position_lookups={} overlay_lookups={} value_expr_evals={} delta_ops={delta_ops}",
        propagation.param_position_lookups,
        propagation.overlay_lookups,
        propagation.value_expr_evals,
    );
}
