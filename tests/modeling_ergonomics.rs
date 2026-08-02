//! P22 — Modeling ergonomics: golden-path behavioral tests.
//!
//! Covers the canonical modeling surface established by M2 decisions
//! (D1/D7/D10/D11): validated entity definitions, the canonical constraint and
//! objective paths, and the sparse cell-coordinate operations. Each section
//! pins the accepted M2 semantics and guards API-04/API-05/API-06.

use roml::prelude::*;

// ────────────────────────────────────────────────────────────────────────────
// Task 1 — Validated definitions
// ────────────────────────────────────────────────────────────────────────────

/// Default `continuous()` is `[0, +inf)`.
#[test]
fn continuous_default_is_non_negative_unbounded() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).expect("valid def");
    assert_eq!(model.variable_bounds(x), Some(Bounds::NON_NEGATIVE));
    assert!(model.pprint().contains("Continuous"));
}

/// Default `integer()` is `[0, +inf)`.
#[test]
fn integer_default_is_non_negative_unbounded() {
    let mut model = Model::new();
    let x = model.add_variable(integer()).expect("valid def");
    assert_eq!(model.variable_bounds(x), Some(Bounds::NON_NEGATIVE));
    assert!(model.pprint().contains("Integer"));
}

/// Default `binary()` is `[0, 1]`.
#[test]
fn binary_default_is_unit_interval() {
    let mut model = Model::new();
    let x = model.add_variable(binary()).expect("valid def");
    assert_eq!(model.variable_bounds(x), Some(Bounds::BINARY));
}

/// Single-side bound builders preserve the other default side.
#[test]
fn bounds_and_single_side_builders_override_defaults() {
    let mut model = Model::new();
    let a = model
        .add_variable(continuous().bounds(2.0, 5.0))
        .expect("a");
    let b = model
        .add_variable(continuous().lower_bound(-1.0))
        .expect("b");
    let c = model
        .add_variable(continuous().upper_bound(7.0))
        .expect("c");
    assert_eq!(model.variable_bounds(a), Some(Bounds::new(2.0, 5.0)));
    // lower_bound keeps the +inf upper default.
    assert_eq!(
        model.variable_bounds(b),
        Some(Bounds::new(-1.0, f64::INFINITY))
    );
    // upper_bound keeps the 0.0 lower default and applies the new upper.
    assert_eq!(model.variable_bounds(c), Some(Bounds::new(0.0, 7.0)));
}

/// Inverted bounds are rejected before mutation (API-06.1).
#[test]
fn rejects_inverted_bounds() {
    let mut model = Model::new();
    assert_eq!(
        model
            .add_variable(continuous().bounds(10.0, 0.0))
            .expect_err("inverted bounds rejected"),
        ModelError::InvalidBounds
    );
    assert_eq!(model.num_variables(), 0);
}

/// NaN bounds are rejected (API-06.2).
#[test]
fn rejects_nan_bounds() {
    let mut model = Model::new();
    assert_eq!(
        model
            .add_variable(continuous().bounds(f64::NAN, 1.0))
            .expect_err("NaN lower rejected"),
        ModelError::InvalidBounds
    );
    assert_eq!(
        model
            .add_variable(continuous().bounds(0.0, f64::NAN))
            .expect_err("NaN upper rejected"),
        ModelError::InvalidBounds
    );
    assert_eq!(model.num_variables(), 0);
}

/// A +inf lower bound (with +inf upper) or -inf upper bound (with -inf lower)
/// is a non-finite value, not a legitimate unbounded side (API-06.2).
#[test]
fn rejects_invalid_infinities() {
    let mut model = Model::new();
    assert_eq!(
        model
            .add_variable(continuous().bounds(f64::INFINITY, f64::INFINITY))
            .expect_err("+inf lower rejected"),
        ModelError::NonFiniteValue("variable lower bound")
    );
    assert_eq!(
        model
            .add_variable(continuous().bounds(f64::NEG_INFINITY, f64::NEG_INFINITY))
            .expect_err("-inf upper rejected"),
        ModelError::NonFiniteValue("variable upper bound")
    );
    assert_eq!(model.num_variables(), 0);
}

/// Binary bounds must lie within `[0, 1]` (D10 / API-06.4).
#[test]
fn rejects_binary_bounds_outside_unit_interval() {
    let mut model = Model::new();
    assert_eq!(
        model
            .add_variable(binary().bounds(0.0, 2.0))
            .expect_err("upper above 1 rejected"),
        ModelError::InvalidBinaryBounds
    );
    assert_eq!(
        model
            .add_variable(binary().bounds(-1.0, 1.0))
            .expect_err("lower below 0 rejected"),
        ModelError::InvalidBinaryBounds
    );
    assert_eq!(
        model
            .add_variable(binary().bounds(-0.5, 0.5))
            .expect_err("both sides outside rejected"),
        ModelError::InvalidBinaryBounds
    );
    assert_eq!(model.num_variables(), 0);

    // Subsets of [0, 1] are accepted.
    assert!(model.add_variable(binary().bounds(0.0, 1.0)).is_ok());
    assert!(model.add_variable(binary().bounds(0.25, 0.75)).is_ok());
    assert_eq!(model.num_variables(), 2);
}

/// Failed creation must not change counts, the changelog, or the revision
/// (API-06.5 — atomic from the caller's perspective).
#[test]
fn failed_creation_is_atomic() {
    let mut model = Model::new();
    let vars_before = model.num_variables();
    let params_before = model.num_parameters();
    let seq_before = model.changelog_sequence();
    let rev_before = model.current_revision();

    assert!(model.add_variable(continuous().bounds(10.0, 0.0)).is_err());
    assert!(model.add_variable(binary().bounds(0.0, 5.0)).is_err());
    assert!(model.add_parameter(f64::NAN).is_err());

    assert_eq!(model.num_variables(), vars_before);
    assert_eq!(model.num_parameters(), params_before);
    assert_eq!(model.changelog_sequence(), seq_before);
    assert_eq!(model.current_revision(), rev_before);
}

/// `parameter(value)` carries the finite initial value; non-finite values are
/// rejected before mutation (API-06.2).
#[test]
fn parameter_definition_defaults_and_validation() {
    let mut model = Model::new();
    let p = model
        .add_parameter(parameter(2.5))
        .expect("valid parameter");
    assert_eq!(model.parameter_value(p), Some(2.5));

    assert!(model.add_parameter(parameter(f64::NAN)).is_err());
    assert!(model.add_parameter(parameter(f64::INFINITY)).is_err());
    assert_eq!(model.num_parameters(), 1);
}

// ────────────────────────────────────────────────────────────────────────────
// Task 3 — Canonical constraint path
// ────────────────────────────────────────────────────────────────────────────

/// `add_constraint((x + y).le(4.0))` is the canonical constraint mutation
/// (API-04.1/04.3). Each variable compiles to one canonical coefficient cell.
#[test]
fn canonical_add_constraint_le() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).expect("x");
    let y = model.add_variable(continuous()).expect("y");
    let con = model.add_constraint((x + y).le(4.0)).expect("con");
    assert_eq!(model.num_constraints(), 1);
    assert_eq!(
        model.constraint_bounds(con),
        Some(ConstraintBounds::le(4.0))
    );
    assert_eq!(
        model.num_coefficients(),
        2,
        "one canonical cell per variable"
    );
}

/// Equality builder routes through the same canonical path.
#[test]
fn canonical_add_constraint_eq() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).expect("x");
    let y = model.add_variable(continuous()).expect("y");
    let con = model.add_constraint((2.0 * x - y).eq(2.0)).expect("con");
    assert_eq!(
        model.constraint_bounds(con),
        Some(ConstraintBounds::eq(2.0))
    );
    assert_eq!(model.num_coefficients(), 2);
}

/// Lower-bound builder routes through the same canonical path.
#[test]
fn canonical_add_constraint_ge() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).expect("x");
    let y = model.add_variable(continuous()).expect("y");
    let con = model.add_constraint((x + y).ge(1.0)).expect("con");
    assert_eq!(
        model.constraint_bounds(con),
        Some(ConstraintBounds::ge(1.0))
    );
}

/// Ranged `.between` builder routes through the same canonical path.
#[test]
fn canonical_add_constraint_between() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).expect("x");
    let con = model.add_constraint((x).between(0.0, 10.0)).expect("con");
    assert_eq!(
        model.constraint_bounds(con),
        Some(ConstraintBounds::range(0.0, 10.0))
    );
}

/// The expression's constant offset is folded into the bounds, not stored as a
/// coefficient cell (canonical-cell invariant).
#[test]
fn constraint_expression_constant_adjusts_bounds() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).expect("x");
    let con = model.add_constraint((2.0 * x + 3.0).le(10.0)).expect("con");
    // expr constant 3.0 is subtracted from the RHS: 2x + 3 <= 10 -> 2x <= 7
    assert_eq!(
        model.constraint_bounds(con),
        Some(ConstraintBounds::le(7.0))
    );
    assert_eq!(model.num_coefficients(), 1, "constant is not a coefficient");
    let expr = model.constraint_expression(con).expect("expr");
    assert_eq!(expr.get_constant(), 0.0);
    assert_eq!(expr.terms()[0].coeff.as_constant(), Some(2.0));
}

/// Parameter coefficients compile to one canonical cell whose cached value
/// tracks parameter updates.
#[test]
fn constraint_parameter_coefficients_are_canonical() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).expect("x");
    let p = model.add_parameter(parameter(2.0)).expect("p");
    let con = model.add_constraint((p * x).le(10.0)).expect("con");
    assert_eq!(model.num_coefficients(), 1);
    let expr = model.constraint_expression(con).expect("expr");
    assert_eq!(expr.terms()[0].coeff.as_constant(), Some(2.0));

    model.set_parameter(p, 5.0).expect("set");
    model.commit().expect("commit");
    assert_eq!(
        model.constraint_expression(con).expect("expr").terms()[0]
            .coeff
            .as_constant(),
        Some(5.0)
    );
}

/// Named constraint specs are retrievable by name (D6).
#[test]
fn named_constraint_via_spec_is_retrievable() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).expect("x");
    let y = model.add_variable(continuous()).expect("y");
    let con = model
        .add_constraint((x + y).le(4.0).named("capacity"))
        .expect("con");
    assert_eq!(model.constraint_name(con).expect("name"), Some("capacity"));
}

/// Raw `ConstraintBounds` still compiles through the generic spec API with no
/// ambiguity (API-04.1 input-shape compatibility bridge).
#[test]
fn add_constraint_raw_bounds_keeps_working_without_ambiguity() {
    let mut model = Model::new();
    let con = model
        .add_constraint(ConstraintBounds::le(4.0))
        .expect("con");
    assert_eq!(
        model.constraint_bounds(con),
        Some(ConstraintBounds::le(4.0))
    );
    assert_eq!(model.num_coefficients(), 0);
}

/// Raw bounds-only row creation is an explicitly advanced method
/// (`add_empty_constraint`), kept separate from the canonical spec path.
#[test]
fn advanced_add_empty_constraint_creates_bounds_only_row() {
    let mut model = Model::new();
    let con = model.add_empty_constraint(ConstraintBounds::le(5.0));
    assert_eq!(model.num_constraints(), 1);
    assert_eq!(
        model.constraint_bounds(con),
        Some(ConstraintBounds::le(5.0))
    );
    assert_eq!(model.num_coefficients(), 0, "bounds-only row has no cells");
}
