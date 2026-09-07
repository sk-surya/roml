//! Direct-Rust BESS rolling MPC benchmark (MPY-05 comparison arm).
//!
//! Same mathematics as the Python `bess_mpc` example, built with the native
//! ROML modeling API and solved through a persistent `Highs` session with
//! batched parameter updates. Prints per-gate wall times and objectives as
//! JSON lines plus a summary: the direct-Rust counterpart for wrapper
//! overhead measurement.
//!
//! Run with: `cargo run -p roml-highs --example bess_mpc --release --
//! --steps 1000 --seed 20260907`

use std::time::Instant;

use roml::prelude::*;
use roml::{ConstraintBounds, ConstraintSpec, LinExpr, SolveOptions, ValueExpr};
use roml_highs::Highs;

const N: usize = 24;
const DT: f64 = 0.25;
const ENERGY_CAP: f64 = 4.0;
const POWER: f64 = 2.0;
const EFF: f64 = 0.95;
const TERMINAL: f64 = 30.0;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let mut steps = 1000usize;
    let mut seed = 20260907u64;
    let mut mode = String::from("closed");
    let mut forecasts_path: Option<String> = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--steps" => {
                steps = args[i + 1].parse()?;
                i += 2;
            }
            "--seed" => {
                seed = args[i + 1].parse()?;
                i += 2;
            }
            "--mode" => {
                mode = args[i + 1].clone();
                i += 2;
            }
            "--forecasts" => {
                forecasts_path = Some(args[i + 1].clone());
                i += 2;
            }
            other => panic!("unknown argument {other}"),
        }
    }
    if mode != "closed" && mode != "matched" {
        panic!("--mode must be closed or matched");
    }

    let mut model = Model::named("rolling-battery");
    let mut prices = Vec::with_capacity(N);
    for t in 0..N {
        prices.push(model.add_parameter(parameter(50.0).named(format!("price_{t}")))?);
    }
    let initial = model.add_parameter(parameter(2.0).named("initial_energy"))?;
    let mut charge = Vec::with_capacity(N);
    let mut discharge = Vec::with_capacity(N);
    let mut energy = Vec::with_capacity(N + 1);
    let mut direction = Vec::with_capacity(N);
    for t in 0..N {
        charge.push(model.add_variable(continuous().bounds(0.0, POWER).named(format!("c{t}")))?);
        discharge.push(
            model.add_variable(continuous().bounds(0.0, POWER).named(format!("d{t}")))?,
        );
        direction.push(model.add_variable(binary().named(format!("b{t}")))?);
    }
    for t in 0..=N {
        energy.push(
            model.add_variable(continuous().bounds(0.0, ENERGY_CAP).named(format!("e{t}")))?,
        );
    }
    // initial_soc tracks the level parameter: bounds re-applied per gate.
    let initial_soc = model.add_constraint(
        ConstraintSpec::new(
            LinExpr::new().term(1.0, energy[0]),
            ConstraintBounds::eq(2.0),
        )
        .named("initial_soc"),
    )?;
    for t in 0..N {
        let row = LinExpr::new()
            .term(1.0, energy[t + 1])
            .term(-1.0, energy[t])
            .term(-DT * EFF, charge[t])
            .term(DT / EFF, discharge[t]);
        model.add_constraint(ConstraintSpec::new(row, ConstraintBounds::eq(0.0)))?;
        let mode_c = LinExpr::new()
            .term(1.0, charge[t])
            .term(-POWER, direction[t]);
        model.add_constraint(ConstraintSpec::new(mode_c, ConstraintBounds::le(0.0)))?;
        let mode_d = LinExpr::new()
            .term(1.0, discharge[t])
            .term(POWER, direction[t]);
        model.add_constraint(ConstraintSpec::new(mode_d, ConstraintBounds::le(POWER)))?;
    }
    // Objective with parameter-dependent coefficients (native auto-update).
    let mut obj = LinExpr::new();
    for t in 0..N {
        let w = ValueExpr::param(prices[t]) * DT;
        obj = obj.term(w.clone(), discharge[t]);
        obj = obj.term(w * -1.0, charge[t]);
    }
    obj = obj.term(TERMINAL, energy[N]);
    model.maximize(obj)?;

    // Seeded synthetic forecasts (xorshift uniform shaking, same family).
    let mut rng = seed.max(1);
    let mut next_rand = || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        (rng as f64) / (u64::MAX as f64)
    };
    let horizon = steps + N;
    let realized: Vec<f64> = match &forecasts_path {
        Some(path) => {
            // Shared frozen stream (one value per line or comma-separated).
            let text = std::fs::read_to_string(path)?;
            text.split(|c| c == ',' || c == '\n')
                .filter_map(|piece| {
                    let piece = piece.trim();
                    if piece.is_empty() {
                        None
                    } else {
                        Some(piece.parse::<f64>().expect("forecast value"))
                    }
                })
                .collect()
        }
        None => {
            let mut realized = Vec::with_capacity(horizon);
            for t in 0..horizon {
                let noise: f64 = (0..12).map(|_| next_rand()).sum::<f64>() - 6.0;
                realized.push(50.0 + 60.0 * ((t as f64) / 4.0).sin() + 5.0 * noise / 2.0);
            }
            realized
        }
    };
    assert!(
        realized.len() >= horizon,
        "forecast stream too short for steps"
    );

    let mut highs = Highs::new()?;
    let options = SolveOptions::new().threads(1).output(false);
    let mut level = 2.0f64;
    let mut walls = Vec::with_capacity(steps);
    for k in 0..steps {
        for t in 0..N {
            model.set_parameter(prices[t], realized[k + t])?;
        }
        let gate_level = if mode == "matched" { 2.0 } else { level };
        model.set_parameter(initial, gate_level)?;
        model.set_constraint_bounds(initial_soc, ConstraintBounds::eq(gate_level))?;
        let t0 = Instant::now();
        let solved = highs.solve_with(&mut model, options.clone())?;
        let wall_ms = t0.elapsed().as_secs_f64() * 1000.0;
        assert!(solved.status().is_optimal());
        let ch0 = solved.value(charge[0]).unwrap_or(0.0);
        let dh0 = solved.value(discharge[0]).unwrap_or(0.0);
        level = (level + DT * (EFF * ch0 - dh0 / EFF)).clamp(0.0, ENERGY_CAP);
        walls.push(wall_ms);
        if k < 3 || k + 1 == steps {
            println!(
                "{{\"gate\":{k},\"wall_ms\":{wall_ms:.3},\"objective\":{:?}}}",
                solved.objective_value()
            );
        }
    }
    walls.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = walls[walls.len() / 2];
    let p95 = walls[(walls.len() as f64 * 0.95) as usize];
    println!("{{\"steps\":{steps},\"p50_ms\":{median:.3},\"p95_ms\":{p95:.3}}}");
    Ok(())
}
