//! P20 Task 3 — Compile-pass characterization of the CURRENT `roml` public
//! API surface.
//!
//! The frozen target contracts in `tests/ui/target_*.rs` reference the P21/P22
//! API and intentionally do not compile today. This file is the executable
//! counterpart: it proves which surface DOES compile and run on current main,
//! and it guards API-10.2 ("compile-pass tests cover the canonical API") and
//! API-10.1 ("existing core and HiGHS test suites remain green") while the
//! ergonomic work lands.
//!
//! It uses only the current, documented `roml::prelude` surface and the
//! method-first style that already compiles (see README's method-based
//! example). Keep this file green.

use roml::prelude::*;

/// The README's documented method-based workflow compiles and runs today.
///
/// ```text
/// let mut model = Model::new();
/// let x = model.add_var();
/// let y = model.add_var();
/// model.constrain((x + y).le(4.0))?;
/// let obj = model.maximize(x + 2.0 * y + 5.0)?;
/// assert_eq!(model.objective_constant(obj), Some(5.0));
/// ```
#[test]
fn readme_method_style_compiles_and_runs() {
    let mut model = Model::new();
    let x = model.add_var();
    let y = model.add_var();

    model
        .constrain((x + y).le(4.0))
        .expect("constrain should succeed");
    let obj = model
        .maximize(x + 2.0 * y + 5.0)
        .expect("maximize should succeed");

    assert_eq!(model.objective_constant(obj), Some(5.0));
}

/// The current prelude exposes model builders, fluent expression traits, and
/// the pure `constraint!` builder (imported explicitly — not in the prelude).
#[test]
fn current_prelude_surface_compiles() {
    use roml::constraint;

    let mut model = Model::new();
    let x = model.add_var();
    let y = model.add_binary();
    let z = model.add_integer(Bounds::new(0.0, 10.0));
    let price = model.add_parameter(1.0);
    assert_eq!(model.parameter_value(price), Some(1.0));

    let cap = constraint!(2.0 * x + y <= 10.0);
    model
        .constrain(cap)
        .expect("fluent constraint should succeed");
    model
        .constrain((y + z).ge(1.0))
        .expect("ge constraint should succeed");
    model
        .minimize(price * x + 3.0 * z)
        .expect("minimize should succeed");

    // Parameter updates are queued and take effect on commit (current
    // documented semantics).
    model.set_parameter(price, 3.0);
    model.commit().expect("commit should succeed");
    assert_eq!(model.parameter_value(price), Some(3.0));
}

/// `add_constraint` (low-level bounds form) and the ranged `.between` builder
/// are part of the current surface and must keep compiling.
#[test]
fn low_level_and_between_forms_compile() {
    let mut model = Model::new();
    let x = model.add_var();
    let y = model.add_var();

    model.add_constraint(ConstraintBounds::le(4.0));
    model
        .constrain((x).between(0.0, 10.0))
        .expect("between should succeed");
    model.constrain((y).ge(0.0)).expect("ge should succeed");

    let obj = model.maximize(x + y).expect("objective should be set");
    assert_eq!(model.active_objective(), Some(obj));
}
