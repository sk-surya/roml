//! Core entity-insertion cost probe (MPY bulk-breakdown evidence).
//!
//! Measures raw `add_variable` / `add_constraint` throughput on the core
//! model with no facade, binding, or solver involved, so the identical
//! core work shared by bulk and scalar construction arms can be
//! quantified. Not a benchmark harness (no statistics); run release and
//! take the reported medians.
//!
//! Run with: `cargo run -p roml --example sync_cost_probe --release`

use std::time::Instant;

use roml::prelude::*;
use roml::{ConstraintBounds, ConstraintSpec, LinExpr};

const NVARS: usize = 100_000;
const NROWS: usize = 10_000;
const PER_ROW: usize = 10;

fn median(mut xs: Vec<f64>) -> f64 {
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    xs[xs.len() / 2]
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut var_times = Vec::new();
    let mut row_times = Vec::new();
    for _ in 0..5 {
        let mut model = Model::named("probe");
        let t0 = Instant::now();
        let mut vars = Vec::with_capacity(NVARS);
        for i in 0..NVARS {
            vars.push(model.add_variable(continuous().bounds(0.0, 5.0).named(format!("x{i}")))?);
        }
        var_times.push(t0.elapsed().as_secs_f64());
        // 10-coefficient rows over the variable window.
        let t1 = Instant::now();
        for r in 0..NROWS {
            let mut lin = LinExpr::new();
            for k in 0..PER_ROW {
                lin = lin.term(1.0, vars[(10 * r + k) % NVARS]);
            }
            model.add_constraint(ConstraintSpec::new(lin, ConstraintBounds::le(10.0)))?;
        }
        row_times.push(t1.elapsed().as_secs_f64());
    }
    println!(
        "{{\"vars_100k_s\":{:.4},\"rows_10x10_s\":{:.4}}}",
        median(var_times),
        median(row_times)
    );
    Ok(())
}
