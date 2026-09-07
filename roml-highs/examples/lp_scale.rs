//! LP-scale workload in direct Rust (MPY-05 PY-27 comparison arm).
//!
//! 96 periods x 100 batteries with shaped-index bookkeeping, affine
//! balances, and 9,600 price cells updated per gate. Reports update,
//! solve, and extraction splits as JSON. Matches
//! `python/benchmarks/bench_lpscale.py` mathematically.
//!
//! Run with: `cargo run -p roml-highs --example lp_scale --release --
//! --steps 20 --repeats 10`

use std::time::Instant;

use roml::prelude::*;
use roml::{ConstraintBounds, ConstraintSpec, LinExpr, SolveOptions, ValueExpr};
use roml_highs::Highs;

const PERIODS: usize = 96;
const BATTERIES: usize = 100;
const DT: f64 = 0.25;
const EFF: f64 = 0.95;
const ENERGY_CAP: f64 = 4.0;
const POWER: f64 = 2.0;

fn idx(b: usize, t: usize) -> usize {
    b * PERIODS + t
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let mut steps = 20usize;
    let mut repeats = 10usize;
    let mut seed = 20260907u64;
    let mut forecasts_path: Option<String> = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--steps" => {
                steps = args[i + 1].parse()?;
                i += 2;
            }
            "--repeats" => {
                repeats = args[i + 1].parse()?;
                i += 2;
            }
            "--seed" => {
                seed = args[i + 1].parse()?;
                i += 2;
            }
            "--forecasts" => {
                forecasts_path = Some(args[i + 1].clone());
                i += 2;
            }
            other => panic!("unknown argument {other}"),
        }
    }

    // Deterministic price streams (xorshift uniform shaking).
    let mut rng = seed.max(1);
    let mut next_rand = || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        (rng as f64) / (u64::MAX as f64)
    };
    // Shared frozen stream when provided (identical numerics across
    // arms); otherwise the internal recipe (same distribution family).
    let streams: Vec<Vec<f64>> = match &forecasts_path {
        Some(path) => {
            let text = std::fs::read_to_string(path)?;
            let values: Vec<f64> = text
                .split(|c| c == ',' || c == '\n')
                .filter_map(|piece| {
                    let piece = piece.trim();
                    if piece.is_empty() {
                        None
                    } else {
                        Some(piece.parse::<f64>().expect("forecast value"))
                    }
                })
                .collect();
            assert!(
                values.len() >= steps * BATTERIES * PERIODS,
                "forecast stream too short"
            );
            (0..steps)
                .map(|k| {
                    values[k * BATTERIES * PERIODS..(k + 1) * BATTERIES * PERIODS].to_vec()
                })
                .collect()
        }
        None => {
            let mut streams = Vec::with_capacity(steps);
            for _ in 0..steps {
                let mut grid = Vec::with_capacity(BATTERIES * PERIODS);
                for _ in 0..BATTERIES * PERIODS {
                    grid.push(50.0 + 10.0 * (next_rand() - 0.5) * 2.0);
                }
                streams.push(grid);
            }
            streams
        }
    };

    let mut model = Model::named("lp-scale");
    let mut price_of = Vec::with_capacity(BATTERIES * PERIODS);
    for b in 0..BATTERIES {
        for t in 0..PERIODS {
            price_of.push(
                model.add_parameter(parameter(50.0).named(format!("p{b}_{t}")))?,
            );
        }
    }
    let mut charge = Vec::with_capacity(BATTERIES * PERIODS);
    let mut discharge = Vec::with_capacity(BATTERIES * PERIODS);
    let mut energy = Vec::with_capacity(BATTERIES * (PERIODS + 1));
    for b in 0..BATTERIES {
        for t in 0..PERIODS {
            charge.push(model.add_variable(continuous().bounds(0.0, POWER))?);
            discharge.push(model.add_variable(continuous().bounds(0.0, POWER))?);
        }
        for t in 0..=PERIODS {
            energy.push(model.add_variable(continuous().bounds(0.0, ENERGY_CAP))?);
        }
    }
    let eoff = |b: usize| b * (PERIODS + 1);
    for b in 0..BATTERIES {
        let row = LinExpr::new().term(1.0, energy[eoff(b)]);
        model.add_constraint(ConstraintSpec::new(row, ConstraintBounds::eq(2.0)))?;
        for t in 0..PERIODS {
            let row = LinExpr::new()
                .term(1.0, energy[eoff(b) + t + 1])
                .term(-1.0, energy[eoff(b) + t])
                .term(-DT * EFF, charge[idx(b, t)])
                .term(DT / EFF, discharge[idx(b, t)]);
            model.add_constraint(ConstraintSpec::new(row, ConstraintBounds::eq(0.0)))?;
        }
        for t in 0..PERIODS {
            let row = LinExpr::new()
                .term(1.0, charge[idx(b, t)])
                .term(1.0, discharge[idx(b, t)]);
            model.add_constraint(ConstraintSpec::new(row, ConstraintBounds::le(POWER)))?;
        }
    }
    let mut obj = LinExpr::new();
    for b in 0..BATTERIES {
        for t in 0..PERIODS {
            let w = ValueExpr::param(price_of[idx(b, t)]) * DT;
            obj = obj.term(w.clone(), discharge[idx(b, t)]);
            obj = obj.term(w * -1.0, charge[idx(b, t)]);
        }
    }
    model.maximize(obj)?;

    let mut highs = Highs::new()?;
    let options = SolveOptions::new().threads(1).output(false);
    // Warmup.
    for k in 0..3.min(steps) {
        for (p, v) in price_of.iter().zip(streams[k % steps].iter()) {
            model.set_parameter(*p, *v)?;
        }
        highs.solve_with(&mut model, options.clone())?;
    }
    let mut end_to_end = Vec::new();
    let (mut update_ms, mut solve_ms, mut extract_ms) = (0.0, 0.0, 0.0);
    for _ in 0..repeats {
        let rep_start = Instant::now();
        for k in 0..steps {
            let u0 = Instant::now();
            for (p, v) in price_of.iter().zip(streams[k].iter()) {
                model.set_parameter(*p, *v)?;
            }
            let u1 = Instant::now();
            let solved = highs.solve_with(&mut model, options.clone())?;
            let s1 = Instant::now();
            assert!(solved.status().is_optimal());
            let mut acc = 0.0;
            for v in discharge.iter() {
                acc += solved.value(*v).unwrap_or(0.0);
            }
            let e1 = Instant::now();
            update_ms += (u1 - u0).as_secs_f64() * 1000.0;
            solve_ms += (s1 - u1).as_secs_f64() * 1000.0;
            extract_ms += (e1 - s1).as_secs_f64() * 1000.0;
            let _ = acc;
        }
        end_to_end.push((Instant::now() - rep_start).as_secs_f64() * 1000.0 / steps as f64);
    }
    end_to_end.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let gates = (repeats * steps) as f64;
    println!(
        "{{\"end_to_end_p50_ms\":{:.2},\"update_ms_per_gate\":{:.2},\"solve_ms_per_gate\":{:.2},\"extract_ms_per_gate\":{:.2}}}",
        end_to_end[end_to_end.len() / 2],
        update_ms / gates,
        solve_ms / gates,
        extract_ms / gates,
    );
    Ok(())
}
