//! MIP warm starts and effective-plan reporting (P28).
//!
//! A small scheduling MIP is solved cold, then re-solved with a [`SolvePlan`]
//! carrying a [`MipStart`] seeded from the cold solution. A warm start is a
//! search hint — it can never change the proven optimum (SM-08.3) — and the
//! returned metadata records which features were applied and the exact
//! compilation identity of the solve (SM-04.5, SM-07.7).
//!
//! Run with: `cargo run -p roml-highs --example warm_start_mip`

use std::collections::BTreeMap;

use roml::prelude::*;
use roml::{MipStart, PrimalAssignment, RepairPolicy, SolvePlan};
use roml_highs::Highs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut model = Model::named("warm_start_mip");

    let x = model.add_variable(integer().bounds(0.0, 10.0).named("x"))?;
    let y = model.add_variable(integer().bounds(0.0, 10.0).named("y"))?;
    model.add_constraint((x + y).le(12.0))?;
    model.maximize(3.0 * x + y)?;

    let mut highs = Highs::new()?;

    // Cold solve: the baseline optimum.
    let cold = highs.solve(&mut model)?;
    println!(
        "cold optimum: {} (x = {})",
        cold.objective_value().unwrap_or(f64::NAN),
        cold.value(x).unwrap_or(f64::NAN)
    );

    // Seed a warm start from the cold solution. The assignment carries the
    // model's lineage/instance/revision as provenance (D4) and is validated
    // against the model before any backend call.
    let assignment = PrimalAssignment {
        lineage: model.lineage(),
        source_instance: Some(model.instance()),
        source_revision: Some(model.current_revision()),
        values: BTreeMap::from([
            (x, cold.value(x).unwrap_or(0.0)),
            (y, cold.value(y).unwrap_or(0.0)),
        ]),
    };
    let plan = SolvePlan {
        mip_starts: vec![MipStart::new(assignment, RepairPolicy::BackendDefault)],
        ..SolvePlan::new(SolveOptions::default())
            .expect("solve plan identity allocation cannot overflow")
    };

    let warm = highs.solve_plan(&mut model, plan)?;

    // A warm start is a hint: the proven optimum is unchanged, and the
    // metadata records what was applied and under which compilation identity.
    assert_eq!(
        warm.objective_value(),
        cold.objective_value(),
        "a warm start can never change the proven optimum (SM-08.3)"
    );
    println!(
        "warm start applied: {:?}",
        warm.metadata().effective_plan.applied_features
    );
    println!("compilation id: {:?}", warm.metadata().compilation_id);
    assert_eq!(warm.status(), SolveStatus::Optimal);
    Ok(())
}
