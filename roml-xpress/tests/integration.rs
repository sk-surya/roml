//! Integration tests for XpressAdapter.
//!
//! Each test builds a roml Model, drains changes to the adapter, solves,
//! and verifies results.

use roml::{Bounds, ConstraintBounds, Model, Sense, VarType};
use roml::solver::{SolverAdapter, SolverStatus};
use roml_xpress::{XpressAdapter, XpressOptions};

fn init_test_logging() {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {
        if let Err(e) = roml::init_logging() {
            eprintln!("warning: failed to initialise logging: {}", e);
        }
    });
}

fn new_adapter() -> XpressAdapter {
    XpressAdapter::with_options(XpressOptions::default().log_level(10).max_time(60.0))
}

fn approx_eq(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

fn sync(model: &mut Model, adapter: &mut XpressAdapter) {
    let changes = model.drain_changes();
    adapter.apply_changes(&changes).unwrap();
}

#[test]
fn bulk_incremental_columns_update_existing_rows_and_objective() {
    let mut model = Model::new();
    let base = model.add_variable(Bounds::new(0.0, 10.0), VarType::Continuous);
    let capacity = model.add_constraint(ConstraintBounds::le(10.0));
    model.add_coeff(capacity, base, 1.0).unwrap();
    let objective = model.add_objective(Sense::Maximize);
    model.add_objective_coeff(objective, base, 1.0).unwrap();
    model.set_active_objective(objective).unwrap();

    let mut adapter = new_adapter();
    sync(&mut model, &mut adapter);
    assert_eq!(adapter.solve().unwrap(), SolverStatus::Optimal);

    for _ in 0..256 {
        let var = model.add_variable(Bounds::new(0.0, 10.0), VarType::Continuous);
        model.add_coeff(capacity, var, 1.0).unwrap();
        model.add_objective_coeff(objective, var, 2.0).unwrap();
    }
    sync(&mut model, &mut adapter);

    assert_eq!(adapter.solve().unwrap(), SolverStatus::Optimal);
    assert!(approx_eq(adapter.objective_value_raw().unwrap(), 20.0));
}

// ── Test 1: simple LP ─────────────────────────────────────────────────────

/// maximize  x + y
/// subject to  x + y <= 4
///             x     <= 3
///             y     <= 3
///             x, y >= 0
///
/// Optimal: x=1, y=3 or x=3, y=1 (obj=4)
#[test]
fn simple_lp_solve() {
    init_test_logging();
    let mut model = Model::new();

    let x = model.add_variable(Bounds::NON_NEGATIVE, VarType::Continuous);
    let y = model.add_variable(Bounds::NON_NEGATIVE, VarType::Continuous);

    let c1 = model.add_constraint(ConstraintBounds::le(4.0));
    model.add_coeff(c1, x, 1.0).unwrap();
    model.add_coeff(c1, y, 1.0).unwrap();

    let c2 = model.add_constraint(ConstraintBounds::le(3.0));
    model.add_coeff(c2, x, 1.0).unwrap();

    let c3 = model.add_constraint(ConstraintBounds::le(3.0));
    model.add_coeff(c3, y, 1.0).unwrap();

    use roml::value_expr::ValueExpr;
    let obj = model.add_objective(Sense::Maximize);
    model.set_active_objective(obj).unwrap();
    model.add_objective_coefficient(obj, x, ValueExpr::constant(1.0)).unwrap();
    model.add_objective_coefficient(obj, y, ValueExpr::constant(1.0)).unwrap();

    let mut adapter = new_adapter();
    sync(&mut model, &mut adapter);

    let status = adapter.solve().unwrap();
    assert_eq!(status, SolverStatus::Optimal);

    let sol = adapter.solution_values().expect("should have solution");
    let xv = sol[&x];
    let yv = sol[&y];

    assert!(approx_eq(xv + yv, 4.0), "objective = {} (expected 4)", xv + yv);
    assert!(xv >= -1e-6);
    assert!(yv >= -1e-6);
}

// ── Test 2: incremental bound change ──────────────────────────────────────

/// minimize x  s.t. x >= lb
/// First solve with lb=1, then tighten to lb=5, re-solve.
#[test]
fn incremental_bound_change() {
    init_test_logging();
    let mut model = Model::new();

    let x = model.add_variable(Bounds::new(1.0, f64::INFINITY), VarType::Continuous);

    let obj = model.add_objective(Sense::Minimize);
    model.set_active_objective(obj).unwrap();
    use roml::value_expr::ValueExpr;
    model.add_objective_coefficient(obj, x, ValueExpr::constant(1.0)).unwrap();

    let mut adapter = new_adapter();
    sync(&mut model, &mut adapter);

    let status = adapter.solve().unwrap();
    assert_eq!(status, SolverStatus::Optimal);
    let sol = adapter.solution_values().unwrap();
    assert!(approx_eq(sol[&x], 1.0), "expected x=1, got {}", sol[&x]);

    model.set_variable_bounds(x, Bounds::new(5.0, f64::INFINITY)).unwrap();
    sync(&mut model, &mut adapter);

    let status = adapter.solve().unwrap();
    assert_eq!(status, SolverStatus::Optimal);
    let sol = adapter.solution_values().unwrap();
    assert!(approx_eq(sol[&x], 5.0), "expected x=5, got {}", sol[&x]);
}

// ── Test 3: constraint add / remove ───────────────────────────────────────

/// maximize x  s.t. x <= 10 (then add x <= 3, then remove it)
#[test]
fn constraint_add_remove() {
    init_test_logging();
    let mut model = Model::new();

    let x = model.add_variable(Bounds::new(0.0, f64::INFINITY), VarType::Continuous);

    let _c1 = model.add_constraint_expr(x, ConstraintBounds::le(10.0));

    let (obj, _obj_constant) = model.add_objective_expr(x, Sense::Maximize).unwrap();
    model.set_active_objective(obj).unwrap();

    let mut adapter = new_adapter();
    sync(&mut model, &mut adapter);

    // First solve: x = 10
    let status = adapter.solve().unwrap();
    assert_eq!(status, SolverStatus::Optimal);
    let sol = adapter.solution_values().unwrap();
    assert!(approx_eq(sol[&x], 10.0), "expected x=10, got {}", sol[&x]);

    // Add tighter constraint: x <= 3
    let c2 = model.add_constraint(ConstraintBounds::le(3.0));
    model.add_coeff(c2, x, 1.0).unwrap();
    sync(&mut model, &mut adapter);

    let status = adapter.solve().unwrap();
    assert_eq!(status, SolverStatus::Optimal);
    let sol = adapter.solution_values().unwrap();
    assert!(approx_eq(sol[&x], 3.0), "expected x=3, got {}", sol[&x]);

    // Remove tighter constraint: x should go back to 10
    model.remove_constraint(c2).unwrap();
    sync(&mut model, &mut adapter);

    let status = adapter.solve().unwrap();
    assert_eq!(status, SolverStatus::Optimal);
    let sol = adapter.solution_values().unwrap();
    assert!(approx_eq(sol[&x], 10.0), "expected x=10 again, got {}", sol[&x]);
}

// ── Test 4: parameter-based coefficient update ────────────────────────────

/// minimize p*x  s.t. x >= 1
/// First p=1 (optimal x=1, obj=1), then p=3 (optimal x=1, obj=3).
#[test]
fn parameter_coefficient_update() {
    init_test_logging();
    let mut model = Model::new();

    let x = model.add_variable(Bounds::new(1.0, f64::INFINITY), VarType::Continuous);
    let p = model.add_parameter(1.0);

    let obj = model.add_objective(Sense::Minimize);
    model.set_active_objective(obj).unwrap();
    use roml::value_expr::ValueExpr;
    model.add_objective_coefficient(obj, x, ValueExpr::param(p)).unwrap();

    let mut adapter = new_adapter();
    sync(&mut model, &mut adapter);

    let status = adapter.solve().unwrap();
    assert_eq!(status, SolverStatus::Optimal);
    let sol = adapter.solution_values().unwrap();
    assert!(approx_eq(sol[&x], 1.0), "expected x=1, got {}", sol[&x]);

    model.set_parameter(p, 3.0);
    sync(&mut model, &mut adapter);

    let status = adapter.solve().unwrap();
    assert_eq!(status, SolverStatus::Optimal);
    let sol = adapter.solution_values().unwrap();
    assert!(approx_eq(sol[&x], 1.0), "expected x=1, got {}", sol[&x]);
}

// ── Test 5: binary MIP ────────────────────────────────────────────────────

/// Simple 0-1 knapsack:
///   maximize 5x + 4y + 3z
///   s.t.     2x + 3y + 2z <= 5
///            x, y, z ∈ {0, 1}
///
/// Optimal: x=1, y=1, z=0 → obj=9
#[test]
fn binary_mip() {
    let mut model = Model::new();

    let x = model.add_binary();
    let y = model.add_binary();
    let z = model.add_binary();

    let c = model.add_constraint(ConstraintBounds::le(5.0));
    model.add_coeff(c, x, 2.0).unwrap();
    model.add_coeff(c, y, 3.0).unwrap();
    model.add_coeff(c, z, 2.0).unwrap();

    use roml::value_expr::ValueExpr;
    let obj = model.add_objective(Sense::Maximize);
    model.set_active_objective(obj).unwrap();
    model.add_objective_coefficient(obj, x, ValueExpr::constant(5.0)).unwrap();
    model.add_objective_coefficient(obj, y, ValueExpr::constant(4.0)).unwrap();
    model.add_objective_coefficient(obj, z, ValueExpr::constant(3.0)).unwrap();

    let mut adapter = new_adapter();
    sync(&mut model, &mut adapter);

    let status = adapter.solve().unwrap();
    assert_eq!(status, SolverStatus::Optimal);

    let sol = adapter.solution_values().unwrap();
    let xv = sol[&x].round();
    let yv = sol[&y].round();
    let zv = sol[&z].round();
    let obj_val = 5.0 * xv + 4.0 * yv + 3.0 * zv;
    assert!(approx_eq(obj_val, 9.0), "expected obj=9, got {}", obj_val);
    assert!(2.0 * xv + 3.0 * yv + 2.0 * zv <= 5.0 + 1e-6);
}

// ── Test 6: objective switch ──────────────────────────────────────────────

/// minimize x (optimal: x=0) vs maximize x (optimal: x=5)
/// s.t. 0 <= x <= 5
#[test]
fn objective_switch() {
    let mut model = Model::new();

    let x = model.add_variable(Bounds::new(0.0, 5.0), VarType::Continuous);

    use roml::value_expr::ValueExpr;

    let obj_min = model.add_objective(Sense::Minimize);
    model.add_objective_coefficient(obj_min, x, ValueExpr::constant(1.0)).unwrap();

    let obj_max = model.add_objective(Sense::Maximize);
    model.add_objective_coefficient(obj_max, x, ValueExpr::constant(1.0)).unwrap();

    model.set_active_objective(obj_min).unwrap();

    let mut adapter = new_adapter();
    sync(&mut model, &mut adapter);

    let status = adapter.solve().unwrap();
    assert_eq!(status, SolverStatus::Optimal);
    let sol = adapter.solution_values().unwrap();
    assert!(approx_eq(sol[&x], 0.0), "expected x=0 (minimize), got {}", sol[&x]);

    model.set_active_objective(obj_max).unwrap();
    sync(&mut model, &mut adapter);

    let status = adapter.solve().unwrap();
    assert_eq!(status, SolverStatus::Optimal);
    let sol = adapter.solution_values().unwrap();
    assert!(approx_eq(sol[&x], 5.0), "expected x=5 (maximize), got {}", sol[&x]);
}

// ── Test 7: infeasible model ──────────────────────────────────────────────

/// x >= 10 AND x <= 5 → infeasible
#[test]
fn infeasible_model() {
    let mut model = Model::new();

    let x = model.add_variable(Bounds::new(10.0, f64::INFINITY), VarType::Continuous);

    let c = model.add_constraint(ConstraintBounds::le(5.0));
    model.add_coeff(c, x, 1.0).unwrap();

    let obj = model.add_objective(Sense::Minimize);
    model.set_active_objective(obj).unwrap();
    use roml::value_expr::ValueExpr;
    model.add_objective_coefficient(obj, x, ValueExpr::constant(1.0)).unwrap();

    let mut adapter = new_adapter();
    sync(&mut model, &mut adapter);

    let status = adapter.solve().unwrap();
    assert_eq!(status, SolverStatus::Infeasible);
    assert!(adapter.solution_values().is_none());
}

// ── Test 8: solution enrichment (obj value + duals + reduced costs) ────────

/// Simple LP:
///   minimize  3x + 2y
///   s.t.      x + y >= 4
///             x, y >= 0
///
/// Optimal: x=0, y=4 → obj=8
#[test]
fn solution_enrichment_lp() {
    init_test_logging();
    let mut model = Model::new();

    let x = model.add_variable(Bounds::NON_NEGATIVE, VarType::Continuous);
    let y = model.add_variable(Bounds::NON_NEGATIVE, VarType::Continuous);

    let c1 = model.add_constraint(ConstraintBounds::ge(4.0));
    model.add_coeff(c1, x, 1.0).unwrap();
    model.add_coeff(c1, y, 1.0).unwrap();

    use roml::value_expr::ValueExpr;
    let obj = model.add_objective(Sense::Minimize);
    model.set_active_objective(obj).unwrap();
    model.add_objective_coefficient(obj, x, ValueExpr::constant(3.0)).unwrap();
    model.add_objective_coefficient(obj, y, ValueExpr::constant(2.0)).unwrap();

    let mut adapter = new_adapter();
    sync(&mut model, &mut adapter);

    let status = adapter.solve().unwrap();
    assert_eq!(status, SolverStatus::Optimal);

    let obj_val = adapter
        .objective_value_raw()
        .expect("objective_value_raw should be Some after optimal solve");
    assert!(approx_eq(obj_val, 8.0), "expected obj=8, got {obj_val}");

    let duals = adapter
        .dual_values()
        .expect("dual_values should be Some after LP solve");
    assert!(duals.contains_key(&c1), "duals should contain entry for c1");

    let rc = adapter
        .reduced_costs_raw()
        .expect("reduced_costs_raw should be Some after LP solve");
    assert!(rc.contains_key(&x), "reduced costs should contain entry for x");
    assert!(rc.contains_key(&y), "reduced costs should contain entry for y");
}
