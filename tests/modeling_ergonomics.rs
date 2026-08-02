//! P22 — Modeling ergonomics: golden-path behavioral tests.
//!
//! Covers the canonical modeling surface established by M2 decisions
//! (D1/D7/D10/D11): validated entity definitions, the canonical constraint and
//! objective paths, and the sparse cell-coordinate operations. Each section
//! pins the accepted M2 semantics and guards API-04/API-05/API-06.

use roml::id::{ConId, Generation, VarId};
use roml::model::{CoefficientTarget, ConstraintBounds};
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

// ────────────────────────────────────────────────────────────────────────────
// Task 4 — Canonical objective path
// ────────────────────────────────────────────────────────────────────────────

/// `minimize` creates and activates exactly one objective (API-04.2).
#[test]
fn minimize_activates_exactly_once() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).expect("x");
    let y = model.add_variable(continuous()).expect("y");
    let obj = model.minimize(x + 2.0 * y).expect("obj");
    assert_eq!(model.active_objective(), Some(obj));
    assert_eq!(model.num_objectives(), 1);
}

/// `maximize` creates and activates exactly one objective (API-04.2).
#[test]
fn maximize_activates_exactly_once() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).expect("x");
    let obj = model.maximize(x + 1.0).expect("obj");
    assert_eq!(model.active_objective(), Some(obj));
    assert_eq!(model.num_objectives(), 1);
}

/// Objective constants are stored and reported (API-03.5).
#[test]
fn objective_constant_is_retained() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).expect("x");
    let obj = model.minimize(x + 5.0).expect("obj");
    assert_eq!(model.objective_constant(obj), Some(5.0));
    assert_eq!(model.active_objective_constant(), Some(5.0));
    assert_eq!(
        model
            .objective_expression(obj)
            .expect("expr")
            .get_constant(),
        5.0
    );
}

/// A later objective replaces the active one; the ordinary path stays
/// single-objective from the caller's perspective.
#[test]
fn subsequent_objective_replaces_active() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).expect("x");
    let first = model.maximize(x).expect("first");
    assert_eq!(model.active_objective(), Some(first));

    let second = model.minimize(2.0 * x).expect("second");
    assert_eq!(model.active_objective(), Some(second));
    assert_ne!(first, second);
    assert_eq!(model.num_objectives(), 2);
}

/// Objective parameter coefficients compile to one canonical cell whose cached
/// value tracks parameter updates.
#[test]
fn objective_parameter_coefficients_are_canonical() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).expect("x");
    let p = model.add_parameter(parameter(3.0)).expect("p");
    let obj = model.maximize(p * x + 2.0).expect("obj");
    assert_eq!(model.objective_constant(obj), Some(2.0));
    assert_eq!(model.num_coefficients(), 1);
    assert_eq!(
        model.objective_expression(obj).expect("expr").terms()[0]
            .coeff
            .as_constant(),
        Some(3.0)
    );

    model.set_parameter(p, 4.0).expect("set");
    model.commit().expect("commit");
    assert_eq!(
        model.objective_expression(obj).expect("expr").terms()[0]
            .coeff
            .as_constant(),
        Some(4.0)
    );
}

/// Named objective variant through the spec path — does not complicate the
/// ordinary `minimize`/`maximize` path (API-05.4).
#[test]
fn named_objective_via_spec_path() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).expect("x");
    let obj = model
        .set_objective((3.0 * x).maximize().named("profit"))
        .expect("obj");
    assert_eq!(model.active_objective(), Some(obj));
    assert_eq!(model.objective_name(obj).expect("name"), Some("profit"));
}

/// Advanced multiple-objective creation and switching under explicit names is
/// preserved alongside the canonical single-objective path.
#[test]
fn advanced_named_objective_creation_and_switching() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).expect("x");
    let profit = model.add_objective_named(Sense::Maximize, "profit");
    let cost = model.add_objective_named(Sense::Minimize, "cost");
    assert_eq!(model.num_objectives(), 2);
    assert_eq!(
        model.active_objective(),
        None,
        "new objectives are inactive"
    );
    assert_eq!(model.objective_name(profit).expect("name"), Some("profit"));
    assert_eq!(model.objective_name(cost).expect("name"), Some("cost"));

    model
        .set_objective_expr(profit, x + 2.0)
        .expect("set profit expr");
    assert_eq!(model.objective_constant(profit), Some(2.0));
    model.set_active_objective(profit).expect("activate profit");
    assert_eq!(model.active_objective(), Some(profit));

    model.set_active_objective(cost).expect("switch to cost");
    assert_eq!(model.active_objective(), Some(cost));
    assert_eq!(
        model.objective_constant(profit),
        Some(2.0),
        "profit retained"
    );
}

// ────────────────────────────────────────────────────────────────────────────
// Task 5 — Sparse cell-coordinate operations (D11)
// ────────────────────────────────────────────────────────────────────────────

/// `set_coefficient` replaces the canonical cell's value (D11): repeating a set
/// overwrites rather than accumulates.
#[test]
fn set_coefficient_replaces_cell_value() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).expect("x");
    let con = model
        .add_constraint(ConstraintBounds::le(100.0))
        .expect("con");
    model
        .set_coefficient(CoefficientTarget::Constraint(con), x, 2.0)
        .expect("set");
    model
        .set_coefficient(CoefficientTarget::Constraint(con), x, 5.0)
        .expect("replace");
    assert_eq!(model.num_coefficients(), 1, "canonical cell preserved");
    assert_eq!(
        model.constraint_expression(con).expect("expr").terms()[0]
            .coeff
            .as_constant(),
        Some(5.0)
    );
}

/// `set_coefficient` creates the cell when the coordinate is empty.
#[test]
fn set_coefficient_creates_cell_if_absent() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).expect("x");
    let con = model
        .add_constraint(ConstraintBounds::le(100.0))
        .expect("con");
    assert_eq!(model.num_coefficients(), 0);
    model
        .set_coefficient(CoefficientTarget::Constraint(con), x, 3.0)
        .expect("set");
    assert_eq!(model.num_coefficients(), 1);
    assert_eq!(
        model.constraint_expression(con).expect("expr").terms()[0]
            .coeff
            .as_constant(),
        Some(3.0)
    );
}

/// `add_to_coefficient` algebraically accumulates; repeated additions keep one
/// canonical cell whose value is the running sum (D11 canonical-cell invariant).
#[test]
fn add_to_coefficient_accumulates_and_keeps_one_cell() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).expect("x");
    let con = model
        .add_constraint(ConstraintBounds::le(100.0))
        .expect("con");
    model
        .add_to_coefficient(CoefficientTarget::Constraint(con), x, 2.0)
        .expect("add");
    model
        .add_to_coefficient(CoefficientTarget::Constraint(con), x, 3.0)
        .expect("add");
    model
        .add_to_coefficient(CoefficientTarget::Constraint(con), x, 4.0)
        .expect("add");
    assert_eq!(
        model.num_coefficients(),
        1,
        "canonical cell count stays one"
    );
    assert_eq!(
        model.constraint_expression(con).expect("expr").terms()[0]
            .coeff
            .as_constant(),
        Some(9.0)
    );
    assert!(model.validate_invariants().is_ok());
}

/// `remove_coefficient_at` removes the cell by coordinate and is idempotent.
#[test]
fn remove_coefficient_at_removes_by_coordinate() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).expect("x");
    let con = model
        .add_constraint(ConstraintBounds::le(100.0))
        .expect("con");
    model
        .set_coefficient(CoefficientTarget::Constraint(con), x, 2.0)
        .expect("set");
    assert_eq!(model.num_coefficients(), 1);

    model
        .remove_coefficient_at(CoefficientTarget::Constraint(con), x)
        .expect("remove");
    assert_eq!(model.num_coefficients(), 0);

    // Removing a missing cell is a no-op, not an error.
    model
        .remove_coefficient_at(CoefficientTarget::Constraint(con), x)
        .expect("idempotent remove");
    assert_eq!(model.num_coefficients(), 0);
}

/// The sparse trio works symmetrically on objective targets.
#[test]
fn sparse_cells_work_on_objective_targets() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).expect("x");
    let obj = model.add_objective_named(Sense::Maximize, "profit");
    model
        .set_coefficient(CoefficientTarget::Objective(obj), x, 3.0)
        .expect("set");
    model
        .add_to_coefficient(CoefficientTarget::Objective(obj), x, 1.5)
        .expect("add");
    assert_eq!(model.num_coefficients(), 1);
    assert_eq!(
        model.objective_expression(obj).expect("expr").terms()[0]
            .coeff
            .as_constant(),
        Some(4.5)
    );
    model
        .remove_coefficient_at(CoefficientTarget::Objective(obj), x)
        .expect("remove");
    assert_eq!(model.num_coefficients(), 0);
}

/// The sparse trio rejects stale entities and non-finite values (D10/API-06.2).
#[test]
fn sparse_ops_reject_stale_entities_and_non_finite_values() {
    let mut model = Model::new();
    let x = model.add_variable(continuous()).expect("x");
    let con = model
        .add_constraint(ConstraintBounds::le(100.0))
        .expect("con");
    let fake_con = ConId::new(999, Generation::new());
    let fake_var = VarId::new(999, Generation::new());

    assert_eq!(
        model
            .set_coefficient(CoefficientTarget::Constraint(fake_con), x, 1.0)
            .expect_err("stale constraint rejected"),
        ModelError::ConstraintNotFound(fake_con)
    );
    assert_eq!(
        model
            .set_coefficient(CoefficientTarget::Constraint(con), fake_var, 1.0)
            .expect_err("stale variable rejected"),
        ModelError::VariableNotFound(fake_var)
    );
    assert_eq!(
        model
            .set_coefficient(CoefficientTarget::Constraint(con), x, f64::NAN)
            .expect_err("NaN rejected"),
        ModelError::NonFiniteValue("coefficient value")
    );
    assert_eq!(
        model
            .add_to_coefficient(CoefficientTarget::Constraint(con), x, f64::INFINITY)
            .expect_err("infinite rejected"),
        ModelError::NonFiniteValue("coefficient value")
    );
    assert_eq!(model.num_coefficients(), 0, "no mutation on rejection");
}

// ────────────────────────────────────────────────────────────────────────────
// Review round 1 — expression-semantics replacement + atomic creation (API-06.5)
// ────────────────────────────────────────────────────────────────────────────

/// `set_coefficient` compares EXPRESSION semantics, not cached evaluated
/// values: a parameter-dependent cell whose current value coincides with the
/// requested constant must still be replaced (dependency dropped), so a later
/// parameter update cannot change the supposedly replaced coefficient
/// (PR #22 review round 1).
#[test]
fn set_coefficient_replaces_parameter_dependent_expression(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut model = Model::new();
    let x = model.add_variable(continuous())?;
    let con = model.add_constraint(ConstraintBounds::le(100.0))?;
    let price = model.add_parameter(parameter(2.0))?;

    // Parameter-dependent cell: price * x (evaluated value 2.0).
    model.add_constraint_coefficient(con, x, roml::ValueExpr::Param(price))?;

    // Replace with constant 2 — the evaluated value coincides (2.0), but the
    // semantics differ: the dependency must be dropped.
    model.set_coefficient(CoefficientTarget::Constraint(con), x, 2.0)?;

    // A later parameter update must NOT change the replaced coefficient. If
    // the parameter dependency survived (the bug), the cell would track the
    // parameter and report 5.0 after the update.
    model.set_parameter(price, 5.0)?;
    model.commit()?;
    assert_eq!(
        model.constraint_expression(con)?.terms()[0]
            .coeff
            .as_constant(),
        Some(2.0),
        "replacement must drop the parameter dependency and survive updates"
    );
    Ok(())
}

/// A stale variable in a constraint spec fails atomically: no dangling row,
/// no changelog event, no coefficients (API-06.5).
#[test]
fn add_constraint_with_stale_variable_is_atomic() {
    let mut model = Model::new();
    let y = model.add_variable(continuous()).expect("y");
    let stale_x = VarId::new(999, Generation::new());

    let err = model.add_constraint((stale_x + y).le(4.0));
    assert!(matches!(err, Err(ModelError::VariableNotFound(_))));
    assert_eq!(model.num_constraints(), 0, "no dangling row");
    assert_eq!(model.num_coefficients(), 0, "no coefficients written");
    assert!(!model.has_uncommitted(), "no changelog event");
}

/// A stale parameter in a constraint spec fails atomically (API-06.5).
#[test]
fn add_constraint_with_stale_parameter_is_atomic() {
    let mut model_a = Model::new();
    let x = model_a.add_variable(continuous()).expect("x");
    let mut model_b = Model::new();
    let stale_price = model_b
        .add_parameter(parameter(2.0))
        .expect("price in other model");

    let err = model_a.add_constraint((stale_price * x).le(4.0));
    assert!(matches!(err, Err(ModelError::ParameterNotFound(_))));
    assert_eq!(model_a.num_constraints(), 0, "no dangling row");
    assert_eq!(model_a.num_coefficients(), 0);
    assert!(!model_a.has_uncommitted());
}

/// A stale variable in an objective spec fails atomically: no dangling
/// objective, no activation, no changelog event (API-06.5).
#[test]
fn add_objective_with_stale_variable_is_atomic() {
    let mut model = Model::new();
    let stale_x = VarId::new(7, Generation::new());
    let objs_before = model.num_objectives();

    let err = model.maximize(stale_x);
    assert!(matches!(err, Err(ModelError::VariableNotFound(_))));
    assert_eq!(model.num_objectives(), objs_before, "no dangling objective");
    assert!(model.active_objective().is_none(), "nothing activated");
    assert!(!model.has_uncommitted());
}

/// A stale parameter in an objective spec fails atomically (API-06.5).
#[test]
fn add_objective_with_stale_parameter_is_atomic() {
    let mut model_a = Model::new();
    let x = model_a.add_variable(continuous()).expect("x");
    let mut model_b = Model::new();
    let stale_price = model_b.add_parameter(parameter(2.0)).expect("price");

    let err = model_a.maximize(stale_price * x);
    assert!(matches!(err, Err(ModelError::ParameterNotFound(_))));
    assert_eq!(model_a.num_objectives(), 0, "no dangling objective");
    assert!(!model_a.has_uncommitted());
}

/// The low-level `add_constraint_expr` path is atomic too: a stale variable
/// must not leave a dangling row behind (API-06.5).
#[test]
fn add_constraint_expr_with_stale_variable_is_atomic() {
    let mut model = Model::new();
    let stale_x = VarId::new(3, Generation::new());

    let err = model.add_constraint_expr(stale_x, ConstraintBounds::le(4.0));
    assert!(matches!(err, Err(ModelError::VariableNotFound(_))));
    assert_eq!(model.num_constraints(), 0, "no dangling row");
    assert!(!model.has_uncommitted());
}
