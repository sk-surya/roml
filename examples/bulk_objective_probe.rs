//! P0 bulk-objective probe: scalar vs bulk construction comparison.
//!
//! Builds the sparse workload's objective both ways in one process
//! (release) and reports the owner's required columns: expression build,
//! simplify, validation+insertion (scalar `minimize` total), bulk API total,
//! `commit()`, snapshot compile, and peak RSS. BEFORE numbers should match
//! `results/research/p0-objective` in roml-bench; AFTER numbers gate P0.
//!
//! Run with: `cargo run -p roml --example bulk_objective_probe --release
//! -- <N> [constant|parameterized]`
//!
//! The `parameterized` mode builds the scalar objective with a single shared
//! parameter (control); bulk is constant-only by design.

use std::time::Instant;

use roml::prelude::*;
use roml::{ConstraintBounds, ConstraintSpec, LinExpr, Sense, VarId};

/// Current VmRSS and process peak (VmHWM) in bytes from /proc/self/status.
fn rss_bytes() -> (u64, u64) {
    let mut rss = 0u64;
    let mut hwm = 0u64;
    if let Ok(text) = std::fs::read_to_string("/proc/self/status") {
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("VmRSS:") {
                rss = rest
                    .split_whitespace()
                    .next()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0)
                    * 1024;
            } else if let Some(rest) = line.strip_prefix("VmHWM:") {
                hwm = rest
                    .split_whitespace()
                    .next()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0)
                    * 1024;
            }
        }
    }
    (rss, hwm)
}

fn build_base(n: usize) -> Result<(Model, Vec<VarId>), Box<dyn std::error::Error>> {
    let mut model = Model::named("probe");
    let mut vars = Vec::with_capacity(n);
    for _ in 0..n {
        vars.push(model.add_variable(continuous())?);
    }
    let rows = n / 10;
    for r in 0..rows {
        let base = 10 * r;
        let mut expr = LinExpr::new();
        for k in 0..10 {
            expr = expr.term(1.0, vars[base + k]);
        }
        model.add_constraint(ConstraintSpec::new(expr, ConstraintBounds::le(10.0)))?;
    }
    Ok((model, vars))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let n: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(100_000);
    let parameterized = args.get(2).map(|s| s == "parameterized").unwrap_or(false);
    // Which arm(s) to run: separate processes give honest peak-RSS numbers
    // (VmHWM is process-cumulative). `rows` measures the rows-only commit
    // baseline so objective-attributable commit cost can be isolated.
    let which: String = args.get(3).cloned().unwrap_or("both".to_string());

    // --- BEFORE: scalar path ---
    if which == "both" || which == "scalar" {
        let (mut model, vars) = build_base(n)?;
        let t0 = Instant::now();
        let mut total = LinExpr::new();
        let p = if parameterized {
            Some(model.add_parameter(1.0)?)
        } else {
            None
        };
        for v in &vars {
            match p {
                Some(p) => {
                    total = total.term(roml::ValueExpr::param(p), *v);
                }
                None => {
                    total = total.term(1.0, *v);
                }
            }
        }
        let t_expr = t0.elapsed();
        // Standalone simplify on a clone (diagnostic only; the real insertion
        // re-simplifies internally).
        let t1 = Instant::now();
        let _ = total.clone().simplify();
        let t_simplify = t1.elapsed();
        let t2 = Instant::now();
        model.minimize(total)?;
        let t_minimize = t2.elapsed();
        let (_, hwm_scalar) = rss_bytes();
        println!(
            "{{\"arm\":\"scalar\",\"n\":{n},\"mode\":\"{}\",\"stage\":\"constructed\",\
             \"expr_ms\":{:.1},\"simplify_ms\":{:.1},\"minimize_ms\":{:.1},\
             \"peak_rss_mb\":{:.0}}}",
            if parameterized {
                "parameterized"
            } else {
                "constant"
            },
            t_expr.as_secs_f64() * 1e3,
            t_simplify.as_secs_f64() * 1e3,
            t_minimize.as_secs_f64() * 1e3,
            hwm_scalar as f64 / 2f64.powi(20),
        );
        use std::io::Write as _;
        let _ = std::io::stdout().flush();
        let t3 = Instant::now();
        model.commit()?;
        let t_commit = t3.elapsed();
        let t4 = Instant::now();
        let snap_scalar = model.take_snapshot()?;
        let t_snapshot = t4.elapsed();
        let (_, hwm_scalar) = rss_bytes();
        println!(
            "{{\"arm\":\"scalar\",\"n\":{n},\"mode\":\"{}\",\"stage\":\"committed\",\
             \"commit_ms\":{:.1},\"snapshot_ms\":{:.1},\
             \"peak_rss_mb\":{:.0},\"cells\":{}}}",
            if parameterized {
                "parameterized"
            } else {
                "constant"
            },
            t_commit.as_secs_f64() * 1e3,
            t_snapshot.as_secs_f64() * 1e3,
            hwm_scalar as f64 / 2f64.powi(20),
            snap_scalar.cells.len(),
        );
    }

    // --- Snapshot-only arm (construction + snapshot, no commit) ---
    if which == "snapshot" {
        let (mut bmodel, bvars) = build_base(n)?;
        let bcoeffs = vec![1.0; n];
        bmodel.set_linear_objective_bulk(Sense::Minimize, &bvars, &bcoeffs, 0.0)?;
        let t0 = Instant::now();
        let snap = bmodel.take_snapshot()?;
        let t_snapshot = t0.elapsed();
        let (_, hwm) = rss_bytes();
        println!(
            "{{\"arm\":\"snapshot\",\"n\":{n},\"snapshot_ms\":{:.1},\
             \"peak_rss_mb\":{:.0},\"cells\":{}}}",
            t_snapshot.as_secs_f64() * 1e3,
            hwm as f64 / 2f64.powi(20),
            snap.cells.len(),
        );
    }
    if which == "rows" {
        let (mut model, _) = build_base(n)?;
        let t0 = Instant::now();
        model.commit()?;
        let t_commit = t0.elapsed();
        let (_, hwm) = rss_bytes();
        println!(
            "{{\"arm\":\"rows\",\"n\":{n},\"commit_ms\":{:.1},\"peak_rss_mb\":{:.0}}}",
            t_commit.as_secs_f64() * 1e3,
            hwm as f64 / 2f64.powi(20),
        );
    }

    // --- P1A: native bulk rows (vars scalar, rows via add_linear_rows_bulk).
    if which == "rowsbulk" {
        let mut model = Model::named("probe");
        let t0 = Instant::now();
        let mut vars = Vec::with_capacity(n);
        for _ in 0..n {
            vars.push(model.add_variable(continuous())?);
        }
        let t_vars = t0.elapsed();
        let nrows = n / 10;
        let mut row_ptr: Vec<u32> = Vec::with_capacity(nrows + 1);
        let mut flat_vars: Vec<VarId> = Vec::with_capacity(n);
        let mut flat_vals: Vec<f64> = Vec::with_capacity(n);
        let mut bounds: Vec<ConstraintBounds> = Vec::with_capacity(nrows);
        row_ptr.push(0);
        for r in 0..nrows {
            for k in 0..10 {
                flat_vars.push(vars[10 * r + k]);
                flat_vals.push(1.0);
            }
            row_ptr.push(flat_vars.len() as u32);
            bounds.push(ConstraintBounds::le(10.0));
        }
        let t1 = Instant::now();
        let cons = model.add_linear_rows_bulk(&row_ptr, &flat_vars, &flat_vals, &bounds)?;
        let t_api = t1.elapsed();
        assert_eq!(cons.len(), nrows);
        let (_, hwm) = rss_bytes();
        println!(
            "{{\"arm\":\"rowsbulk\",\"n\":{n},\"vars_ms\":{:.1},\"api_ms\":{:.1},\"peak_rss_mb\":{:.0}}}",
            t_vars.as_secs_f64() * 1e3,
            t_api.as_secs_f64() * 1e3,
            hwm as f64 / 2f64.powi(20),
        );
        use std::io::Write as _;
        let _ = std::io::stdout().flush();
        let t2 = Instant::now();
        model.commit()?;
        let t_commit = t2.elapsed();
        let t3 = Instant::now();
        let snap = model.take_snapshot()?;
        let t_snapshot = t3.elapsed();
        let (_, hwm) = rss_bytes();
        println!(
            "{{\"arm\":\"rowsbulk\",\"n\":{n},\"stage\":\"committed\",\"commit_ms\":{:.1},\
             \"snapshot_ms\":{:.1},\"peak_rss_mb\":{:.0},\"cells\":{}}}",
            t_commit.as_secs_f64() * 1e3,
            t_snapshot.as_secs_f64() * 1e3,
            hwm as f64 / 2f64.powi(20),
            snap.cells.len(),
        );
    }

    // --- AFTER: bulk path (constant only) ---
    if which == "both" || which == "bulk" {
        let (mut bmodel, bvars) = build_base(n)?;
        let bcoeffs = vec![1.0; n];
        let t5 = Instant::now();
        let bobj = bmodel.set_linear_objective_bulk(Sense::Minimize, &bvars, &bcoeffs, 0.0)?;
        let t_bulk = t5.elapsed();
        let (_, hwm_bulk) = rss_bytes();
        let _ = bobj;
        println!(
            "{{\"arm\":\"bulk\",\"n\":{n},\"stage\":\"constructed\",\
             \"api_ms\":{:.1},\"peak_rss_mb\":{:.0}}}",
            t_bulk.as_secs_f64() * 1e3,
            hwm_bulk as f64 / 2f64.powi(20),
        );
        use std::io::Write as _;
        let _ = std::io::stdout().flush();
        let t6 = Instant::now();
        bmodel.commit()?;
        let t_bcommit = t6.elapsed();
        let t7 = Instant::now();
        let snap_bulk = bmodel.take_snapshot()?;
        let t_bsnapshot = t7.elapsed();
        let (_, hwm_bulk) = rss_bytes();
        println!(
            "{{\"arm\":\"bulk\",\"n\":{n},\"stage\":\"committed\",\
             \"commit_ms\":{:.1},\"snapshot_ms\":{:.1},\
             \"peak_rss_mb\":{:.0},\"cells\":{}}}",
            t_bcommit.as_secs_f64() * 1e3,
            t_bsnapshot.as_secs_f64() * 1e3,
            hwm_bulk as f64 / 2f64.powi(20),
            snap_bulk.cells.len(),
        );
    }
    Ok(())
}
