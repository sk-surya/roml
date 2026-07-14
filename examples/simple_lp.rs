//! Simple LP example — solver-free model construction.
//!
//! Demonstrates the core modeling API without any solver dependency.
//! Builds: maximize 3x + y subject to x + y ≤ 4, x ≤ 3, x ≥ 0, y ≥ 0.
//!
//! Run with: cargo run --example simple_lp

use roml::prelude::*;

fn main() {
    let mut model = Model::with_name("simple_lp");

    // Variables
    let x = model.add_var(); // defaults to x >= 0, continuous
    let y = model.add_var();

    // Constraints
    model
        .constrain((x + y).le(4.0))
        .expect("constraint should be valid");
    model
        .constrain(x.le(3.0))
        .expect("constraint should be valid");

    // Objective: maximize 3x + y
    let obj = model
        .maximize(3.0 * x + y + 0.0)
        .expect("objective should be valid");

    println!("Model: {}", model.name.as_deref().unwrap_or("unnamed"));
    println!("  Variables: {}", model.num_variables());
    println!("  Constraints: {}", model.num_constraints());
    println!("  Objective constant: {:?}", model.objective_constant(obj));

    // Verify invariants
    model
        .validate_invariants()
        .expect("model invariants should hold");

    println!("  Invariants: OK");
    println!("\nTo solve, attach a solver adapter (e.g., roml-highs).");
}
