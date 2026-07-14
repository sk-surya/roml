//! Parameter update example — solver-free.
//!
//! Demonstrates parameter-dependent coefficients, transactions,
//! and canonical cell combining.
//!
//! Run with: cargo run --example parameter_update

use roml::prelude::*;
use roml::ValueExpr;

fn main() {
    let mut model = Model::with_name("parameter_update");

    let x = model.add_var();
    let y = model.add_var();

    // Parameters: unit costs
    let cost_x = model.add_parameter(10.0);
    let cost_y = model.add_parameter(5.0);

    // Constraint: x + y <= 100
    model.constrain((x + y).le(100.0)).expect("constraint");

    // Objective: minimize cost_x * x + cost_y * y
    let obj = model.add_objective(Sense::Minimize);
    model.set_active_objective(obj).unwrap();
    model
        .add_objective_coefficient(obj, x, ValueExpr::param(cost_x))
        .unwrap();
    model
        .add_objective_coefficient(obj, y, ValueExpr::param(cost_y))
        .unwrap();

    println!("Initial costs: x={}, y={}", 10.0, 5.0);

    // Update parameter and commit
    model.set_parameter(cost_x, 12.0);
    model.set_parameter(cost_y, 3.0);
    model.commit();

    println!("Updated costs: x=12.0, y=3.0");
    println!("Uncommitted changes: {}", model.has_uncommitted());
    println!("Parameters: {}", model.num_parameters());

    // Invariants still hold after parameter update
    model
        .validate_invariants()
        .expect("invariants should hold after parameter update");

    // Canonical cell check: adding another term for same (obj, x) combines
    let existing = model
        .add_objective_coefficient(obj, x, ValueExpr::constant(1.0))
        .unwrap();
    println!(
        "Canonical cell value after combine: {:.1}",
        model.coefficient(existing).unwrap().cached_value
    );
    // Should be 12.0 + 1.0 = 13.0

    println!("\nDone. Attach a solver adapter to solve.");
}
