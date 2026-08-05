//! Piecewise-linear costs and min/max constructs (P33, P32).
//!
//! A small production planning model. Output `q` incurs a convex
//! piecewise-linear cost (three marginal price tiers) modeled with an exact
//! `Epigraph` PWL constraint — the compiler turns a convex epigraph into
//! zero-binary supporting-inequality rows (SM-14.3) — and output is capped
//! by the minimum of two capacity lines via a `Min`/`Hypograph` construct.
//! Both constructs return stable `Construct` handles and compile through the
//! portable bridge.
//!
//! Run with: `cargo run -p roml-highs --example pwl_production_planning`

use roml::construct::{
    ExtrapolationPolicy, FormulationPreference, MinMaxRelation, MinMaxSense, PwlPoint, PwlRelation,
};
use roml::prelude::*;
use roml_highs::Highs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut model = Model::named("pwl_production_planning");

    // Output, and the variable holding the PWL cost.
    let q = model.add_variable(continuous().bounds(0.0, 100.0).named("q"))?;

    // Marginal tiers: [0, 40] at 2.0/unit, [40, 80] at 3.0/unit,
    // [80, 100] at 5.0/unit — a CONVEX piecewise-linear total cost
    // (0, 80, 200, 300). The `Epigraph` relation (`cost >= f(q)`) compiles
    // with zero binaries; minimizing cost makes the rows bind exactly.
    let (_cost_construct, cost) = model.add_piecewise_linear(
        LinExpr::from(q),
        vec![(0.0, 0.0), (40.0, 80.0), (80.0, 200.0), (100.0, 300.0)]
            .into_iter()
            .map(PwlPoint::from)
            .collect(),
        PwlRelation::Epigraph,
        ExtrapolationPolicy::Linear, // outside [0, 100] the end tier extends
        Some(FormulationPreference::Portable),
    )?;

    // Capacity: `cap = min(60 - a, 50 - b)` with `a, b` controllable.
    let a = model.add_variable(continuous().bounds(0.0, 30.0).named("a"))?;
    let b = model.add_variable(continuous().bounds(0.0, 30.0).named("b"))?;
    let (_cap_construct, cap) = model.add_minmax(
        vec![
            LinExpr::from_constant(60.0) - a,
            LinExpr::from_constant(50.0) - b,
        ],
        MinMaxSense::Min,
        MinMaxRelation::Hypograph, // `q <= min(...)` — zero-binary one-sided row
        Some(FormulationPreference::Portable),
    )?;
    model.add_constraint((LinExpr::from(q) - cap).le(0.0))?;

    // Net reward: push output up against capacity while paying the PWL cost.
    // The reward rate (4.0/unit) exceeds ALL three marginal tiers (2, 3, 5),
    // so the objective is strictly increasing in q: the optimum is UNIQUE at
    // the capacity bound q = cap = 50, inside the second tier. The epigraph
    // rows bind exactly and the min/max capacity construct is active.
    model.maximize(4.0 * q - cost)?;

    let mut highs = Highs::new()?;
    let solution = highs.solve(&mut model)?;

    println!("q    = {:.2}", solution.value(q).unwrap_or(f64::NAN));
    println!("cost = {:.2}", solution.value(cost).unwrap_or(f64::NAN));
    println!("cap  = {:.2}", solution.value(cap).unwrap_or(f64::NAN));
    println!(
        "objective (4q - cost) = {:.2}",
        solution.objective_value().unwrap_or(f64::NAN)
    );

    // The convex epigraph is exact: the reported cost equals the PWL function
    // evaluated at the reported output.
    let qv = solution.value(q).unwrap_or(f64::NAN);
    let f = |x: f64| {
        if x <= 40.0 {
            2.0 * x
        } else if x <= 80.0 {
            80.0 + 3.0 * (x - 40.0)
        } else {
            200.0 + 5.0 * (x - 80.0)
        }
    };
    assert!(
        (solution.value(cost).unwrap_or(f64::NAN) - f(qv)).abs() < 1e-6,
        "the PWL epigraph must reproduce the exact cost function"
    );
    // Deterministic optimum (review P1-2): the reward strictly dominates every
    // marginal tier, so q is pushed to the capacity bound.
    assert!(
        (qv - 50.0).abs() < 1e-6,
        "q must reach the capacity bound 50, got {qv}"
    );
    assert!(
        (solution.value(cap).unwrap_or(f64::NAN) - 50.0).abs() < 1e-6,
        "the min/max capacity construct must bind at 50"
    );
    assert!(
        (qv - solution.value(cap).unwrap_or(f64::NAN)).abs() < 1e-6,
        "q must equal the capacity bound exactly"
    );
    assert!(
        (solution.value(cost).unwrap_or(f64::NAN) - 110.0).abs() < 1e-6,
        "cost must equal f(50) = 110"
    );
    assert_eq!(solution.status(), SolveStatus::Optimal);
    Ok(())
}
