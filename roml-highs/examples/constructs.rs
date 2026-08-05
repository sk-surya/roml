//! Semantic constructs: indicator, binary product, absolute value, min/max (P32).
//!
//! The high-level construct library compiles exact semantics through the
//! portable bridge: an indicator coupling a binary decision to a constraint,
//! a binary-times-linear product, an absolute-value epigraph, and a min/max
//! epigraph. Each builder returns a stable [`Construct`] handle (and, where
//! applicable, the result-variable handle) so formulations stay inspectable
//! and origin-complete (SM-12.8, SM-02.5).
//!
//! Run with: `cargo run -p roml-highs --example constructs`

use roml::construct::{
    AbsoluteValueVariant, FormulationPreference, IndicatorDirection, MinMaxRelation, MinMaxSense,
};
use roml::prelude::*;
use roml_highs::Highs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut model = Model::named("constructs");

    let x = model.add_variable(continuous().bounds(-10.0, 10.0).named("x"))?;
    let on = model.add_variable(binary().named("on"))?;

    // Indicator: when `on` is 1, x must be >= 2.
    let _indicator = model.add_indicator(
        on,
        IndicatorDirection::WhenOne,
        (x).ge(2.0),
        Some(FormulationPreference::Portable),
    )?;

    // Binary-times-linear product: `take = on * x` (bounded bilinear, exact).
    let (_product, take) = model.add_binary_times_linear(
        on,
        LinExpr::from(x),
        Some(FormulationPreference::Portable),
    )?;

    // Absolute value: `w >= |x|` (zero-binary epigraph).
    let (_abs, w) = model.add_absolute_value(
        LinExpr::from(x),
        AbsoluteValueVariant::Absolute,
        Some(FormulationPreference::Portable),
    )?;
    model.add_constraint(w.le(6.0))?;

    // Min/max: `m >= max(x, 0)` (zero-binary epigraph of the max).
    let (_mm, m) = model.add_minmax(
        vec![LinExpr::from(x), LinExpr::from_constant(0.0)],
        MinMaxSense::Max,
        MinMaxRelation::Epigraph,
        Some(FormulationPreference::Portable),
    )?;

    // Objective: reward taking x when it is on; the |x| and max(x,0)
    // epigraphs bind exactly at the optimum (penalized lightly so they stay
    // tight rather than slack).
    model.maximize(take - 0.1 * w - 0.1 * m)?;

    let mut highs = Highs::new()?;
    let solution = highs.solve(&mut model)?;

    println!(
        "x = {:.2}, on = {:.0}, take = {:.2}, |x| = {:.2}, max(x,0) = {:.2}",
        solution.value(x).unwrap_or(f64::NAN),
        solution.value(on).unwrap_or(f64::NAN),
        solution.value(take).unwrap_or(f64::NAN),
        solution.value(w).unwrap_or(f64::NAN),
        solution.value(m).unwrap_or(f64::NAN),
    );
    let xv = solution.value(x).unwrap_or(0.0);
    let onv = solution.value(on).unwrap_or(0.0);
    assert!(
        onv < 0.5 || xv >= 2.0 - 1e-6,
        "the indicator must enforce x >= 2 when on = 1"
    );
    assert!(
        (solution.value(take).unwrap_or(f64::NAN) - onv * xv).abs() < 1e-6,
        "the binary product must equal on * x exactly"
    );
    assert!(
        (solution.value(w).unwrap_or(f64::NAN) - xv.abs()).abs() < 1e-6,
        "the absolute-value epigraph must bind exactly"
    );
    assert_eq!(solution.status(), SolveStatus::Optimal);
    Ok(())
}
