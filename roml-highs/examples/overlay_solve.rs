//! Reversible solve overlays: temporary fixings and solution locks (P27).
//!
//! A production mix problem is solved, then re-solved under a
//! [`SolveOverlay`] that temporarily fixes one variable and locks another to
//! its previous value. The overlay applies only to that solve attempt: the
//! canonical model is never mutated, the overlay is rolled back and verified
//! afterward, and the metadata reports the overlay identity (SM-07.3–07.5).
//!
//! Run with: `cargo run -p roml-highs --example overlay_solve`

use std::collections::BTreeMap;

use roml::prelude::*;
use roml::{ContinuousLock, LockSelector, PrimalAssignment, SolutionLock, SolveOverlay};
use roml_highs::Highs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut model = Model::named("overlay_solve");

    let x = model.add_variable(continuous().bounds(0.0, 10.0).named("x"))?;
    let y = model.add_variable(continuous().bounds(0.0, 10.0).named("y"))?;
    model.add_constraint((x + y).le(10.0))?;
    model.maximize(x + 2.0 * y)?;

    let mut highs = Highs::new()?;

    // Baseline: the unconstrained optimum pushes all capacity into y.
    let base = highs.solve(&mut model)?;
    println!(
        "base: x = {:.2}, y = {:.2}, obj = {:.2}",
        base.value(x).unwrap_or(f64::NAN),
        base.value(y).unwrap_or(f64::NAN),
        base.objective_value().unwrap_or(f64::NAN)
    );

    // Overlay: temporarily fix y = 4 AND lock x to its baseline value.
    // Both apply to THIS solve attempt only.
    let lock_assignment = PrimalAssignment {
        lineage: model.lineage(),
        source_instance: Some(model.instance()),
        source_revision: Some(model.current_revision()),
        values: BTreeMap::from([(x, base.value(x).unwrap_or(0.0))]),
    };
    let overlay = SolveOverlay::new(
        BTreeMap::from([(y, 4.0)]), // temporary fixings
        vec![SolutionLock {
            assignment: lock_assignment,
            selector: LockSelector::AllAssigned,
            continuous: ContinuousLock::Exact,
        }],
        vec![], // objective-lock rows (P31 supplies stage optima)
        vec![], // cutoffs
    )
    .expect("overlay identity allocation cannot overflow");

    let expected_id = overlay.id;
    let fixed = highs.solve_with_overlay(&mut model, SolveOptions::default(), &overlay, None)?;
    assert_eq!(fixed.value(y), Some(4.0), "the temporary fixing binds y");
    assert_eq!(
        fixed.value(x),
        base.value(x),
        "the solution lock pins x to its baseline value"
    );
    println!(
        "overlay solve: y fixed at 4, x locked -> x = {:.2}, obj = {:.2}",
        fixed.value(x).unwrap_or(f64::NAN),
        fixed.objective_value().unwrap_or(f64::NAN)
    );
    // The metadata reports EXACTLY the overlay that was applied (review
    // closure: assert the identity, don't just print it).
    assert_eq!(
        fixed.metadata().overlay_id,
        Some(expected_id),
        "the solve must report the exact applied overlay identity"
    );

    // A plain re-solve is unaffected: the overlay was fully rolled back and
    // the canonical model was never changed.
    let plain = highs.solve(&mut model)?;
    assert_eq!(plain.value(x), base.value(x));
    assert_eq!(plain.value(y), base.value(y));
    assert_eq!(plain.metadata().overlay_id, None);
    println!(
        "post-overlay solve: x = {:.2}, y = {:.2} (overlay fully rolled back)",
        plain.value(x).unwrap_or(f64::NAN),
        plain.value(y).unwrap_or(f64::NAN)
    );
    Ok(())
}
