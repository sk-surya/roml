//! P32 Task 15 — interval bound analysis and bridge-framework tests
//! (SM-13.1, SM-13.6).
//!
//! This suite pins the PUBLIC surface of the deterministic interval bound
//! analyzer over the five coefficient/bound classes the plan names —
//! coefficient signs, constants, fixed/equal-bound variables, infinite
//! bounds, and evaluated parameters — and asserts NaN rejection with a typed
//! [`BoundError`]. Determinism (equal inputs -> equal traces) is asserted
//! directly.
//!
//! The construct-dependent surfaces (one-sided Big-M helpers returning the
//! construct-aware [`CompileError::UnboundedBigM`] marker, and the
//! [`BridgeFinalizer`](roml::compiler::bridge::BridgeFinalizer) origin/
//! dependency/report behavior) are exercised by the in-crate `#[cfg(test)]`
//! suites in `src/compiler/bounds.rs` and `src/compiler/bridge/mod.rs`,
//! because there is NO public way to obtain a canonical `Construct` handle
//! until Task 16's public builders land (`add_construct_fixture` is
//! `#[cfg(test)]`-gated test-only scaffolding per A30). This mirrors the F3
//! precedent documented in `tests/semantic_ir.rs` ("construct-lifecycle tests
//! moved IN-CRATE").

use roml::compiler::bounds::{BoundAnalyzer, BoundError, BoundSource, BoundTrace, Interval};
use roml::expr::{LinExpr, TermCoeff};
use roml::function::ScalarFunction;
use roml::id::{Generation, ParamId, VarId};
use roml::model::Bounds;
use roml::value_expr::ValueExpr;

fn var(index: u32) -> VarId {
    VarId::new(index, Generation::new())
}

fn param(index: u32) -> ParamId {
    ParamId::new(index, Generation::new())
}

fn bounds_of(vars: &[(VarId, Bounds)]) -> impl Fn(VarId) -> Bounds + '_ {
    move |v| {
        vars.iter()
            .find(|(id, _)| *id == v)
            .map(|(_, b)| *b)
            .unwrap_or(Bounds::UNBOUNDED)
    }
}

fn params_of(values: &[(ParamId, f64)]) -> impl Fn(ParamId) -> f64 + '_ {
    move |p| {
        values
            .iter()
            .find(|(id, _)| *id == p)
            .map(|(_, v)| *v)
            .unwrap_or(0.0)
    }
}

fn linear(expr: LinExpr) -> ScalarFunction {
    ScalarFunction::Linear(expr)
}

// ── Interval analysis over coefficient signs ──────────────────────────────

/// Positive coefficient with a constant: `f = 2x + 3`, `x in [0, 10]` ->
/// `[3, 23]`.
#[test]
fn interval_positive_coefficient_with_constant() {
    let x = var(0);
    let f = linear(LinExpr::new().term(2.0, x).constant(3.0));
    let trace = BoundAnalyzer::new()
        .interval_of(
            &f,
            bounds_of(&[(x, Bounds::new(0.0, 10.0))]),
            params_of(&[]),
        )
        .expect("bounded input must analyze");
    assert_eq!(
        trace.result,
        Interval {
            lower: 3.0,
            upper: 23.0
        }
    );
}

/// Negative coefficient flips the contribution: `f = -2x + 3`, `x in [0, 10]`
/// -> `[-17, 3]`.
#[test]
fn interval_negative_coefficient_flips_endpoints() {
    let x = var(0);
    let f = linear(LinExpr::new().term(-2.0, x).constant(3.0));
    let trace = BoundAnalyzer::new()
        .interval_of(
            &f,
            bounds_of(&[(x, Bounds::new(0.0, 10.0))]),
            params_of(&[]),
        )
        .expect("bounded input must analyze");
    assert_eq!(
        trace.result,
        Interval {
            lower: -17.0,
            upper: 3.0
        }
    );
}

/// Mixed-sign coefficients: `f = 2x - 3y`, `x in [0, 10]`, `y in [1, 5]` ->
/// `[-15, 17]`.
#[test]
fn interval_mixed_sign_coefficients() {
    let x = var(0);
    let y = var(1);
    let f = linear(LinExpr::new().term(2.0, x).term(-3.0, y));
    let trace = BoundAnalyzer::new()
        .interval_of(
            &f,
            bounds_of(&[(x, Bounds::new(0.0, 10.0)), (y, Bounds::new(1.0, 5.0))]),
            params_of(&[]),
        )
        .expect("bounded input must analyze");
    assert_eq!(
        trace.result,
        Interval {
            lower: -15.0,
            upper: 17.0
        }
    );
}

// ── Constant terms ─────────────────────────────────────────────────────────

/// A pure constant function: `f = 7` -> `[7, 7]`.
#[test]
fn interval_constant_only() {
    let f = linear(LinExpr::from_constant(7.0));
    let trace = BoundAnalyzer::new()
        .interval_of(&f, bounds_of(&[]), params_of(&[]))
        .expect("constant input must analyze");
    assert_eq!(
        trace.result,
        Interval {
            lower: 7.0,
            upper: 7.0
        }
    );
    assert!(trace.sources.contains(&BoundSource::Constant));
}

// ── Fixed variables (equal lower/upper bounds) ─────────────────────────────

/// A fixed variable (the fixing representation: equal lower/upper bounds)
/// contributes an exact value: `x fixed at 4`, `f = 2x` -> `[8, 8]`, and the
/// trace records the fixed-value source.
#[test]
fn interval_fixed_variable_contributes_exact_value() {
    let x = var(0);
    let f = linear(LinExpr::new().term(2.0, x));
    let trace = BoundAnalyzer::new()
        .interval_of(
            &f,
            bounds_of(&[(x, Bounds::fixed(4.0, None))]),
            params_of(&[]),
        )
        .expect("fixed input must analyze");
    assert_eq!(
        trace.result,
        Interval {
            lower: 8.0,
            upper: 8.0
        }
    );
    assert!(trace.sources.contains(&BoundSource::FixedValue(x)));
}

// ── Infinite bounds (free variables) ───────────────────────────────────────

/// A fully free variable propagates to an unbounded interval: `f = x`,
/// `x free` -> `[-inf, inf]`.
#[test]
fn interval_free_variable_is_unbounded() {
    let x = var(0);
    let f = linear(LinExpr::new().term(1.0, x));
    let trace = BoundAnalyzer::new()
        .interval_of(&f, bounds_of(&[(x, Bounds::UNBOUNDED)]), params_of(&[]))
        .expect("free input must analyze");
    assert_eq!(
        trace.result,
        Interval {
            lower: f64::NEG_INFINITY,
            upper: f64::INFINITY,
        }
    );
}

/// A one-sided bound propagates on the matching side: `f = 2x`, `x in
/// [0, +inf)` -> `[0, +inf]`.
#[test]
fn interval_one_sided_infinite_bound_propagates() {
    let x = var(0);
    let f = linear(LinExpr::new().term(2.0, x));
    let trace = BoundAnalyzer::new()
        .interval_of(
            &f,
            bounds_of(&[(x, Bounds::new(0.0, f64::INFINITY))]),
            params_of(&[]),
        )
        .expect("one-sided input must analyze");
    assert_eq!(
        trace.result,
        Interval {
            lower: 0.0,
            upper: f64::INFINITY,
        }
    );
}

// ── Evaluated parameters ───────────────────────────────────────────────────

/// A parameterized coefficient evaluates against the supplied parameter
/// values: `f = p*x`, `p = 3`, `x in [0, 10]` -> `[0, 30]`, and the trace
/// records the parameter-value source.
#[test]
fn interval_parameter_coefficient_evaluates() {
    let p = param(0);
    let x = var(0);
    let f = linear(LinExpr::new().term(p, x));
    let trace = BoundAnalyzer::new()
        .interval_of(
            &f,
            bounds_of(&[(x, Bounds::new(0.0, 10.0))]),
            params_of(&[(p, 3.0)]),
        )
        .expect("parameterized input must analyze");
    assert_eq!(
        trace.result,
        Interval {
            lower: 0.0,
            upper: 30.0
        }
    );
    assert!(trace.sources.contains(&BoundSource::ParameterValue(p)));
}

// ── Determinism ────────────────────────────────────────────────────────────

/// Equal inputs produce equal traces (deterministic propagation).
#[test]
fn interval_analysis_is_deterministic() {
    let x = var(0);
    let y = var(1);
    let f = linear(LinExpr::new().term(2.0, x).term(-3.0, y).constant(1.0));
    let var_bounds = [(x, Bounds::new(0.0, 10.0)), (y, Bounds::new(1.0, 5.0))];
    let empty_params: [(ParamId, f64); 0] = [];
    // `move` closures over owned (Copy) arrays, so the closures are Copy and
    // can be reused across both calls.
    let bounds = move |v: VarId| {
        var_bounds
            .iter()
            .find(|(id, _)| *id == v)
            .map(|(_, b)| *b)
            .unwrap_or(Bounds::UNBOUNDED)
    };
    let params = move |p: ParamId| {
        empty_params
            .iter()
            .find(|(id, _)| *id == p)
            .map(|(_, v)| *v)
            .unwrap_or(0.0)
    };
    let analyzer = BoundAnalyzer::new();
    let a = analyzer.interval_of(&f, bounds, params).unwrap();
    let b = analyzer.interval_of(&f, bounds, params).unwrap();
    assert_eq!(a, b);
    assert_eq!(
        a,
        BoundTrace {
            sources: vec![
                BoundSource::Constant,
                BoundSource::DeclaredVariableBounds(x),
                BoundSource::DeclaredVariableBounds(y)
            ],
            result: Interval {
                lower: 1.0 + 2.0 * 0.0 - 3.0 * 5.0,
                upper: 1.0 + 2.0 * 10.0 - 3.0 * 1.0
            },
        }
    );
}

// ── NaN rejection (SM-13.1) ────────────────────────────────────────────────

/// A NaN coefficient is a typed error — no NaN propagates silently.
#[test]
fn interval_rejects_nan_coefficient() {
    let x = var(0);
    let f = linear(LinExpr::new().term(f64::NAN, x));
    let err = BoundAnalyzer::new()
        .interval_of(&f, bounds_of(&[(x, Bounds::new(0.0, 1.0))]), params_of(&[]))
        .unwrap_err();
    assert!(matches!(err, BoundError::NonFiniteCoefficient { variable } if variable == x));
}

/// A NaN variable bound is a typed error.
#[test]
fn interval_rejects_nan_bound() {
    let x = var(0);
    let f = linear(LinExpr::new().term(1.0, x));
    let err = BoundAnalyzer::new()
        .interval_of(
            &f,
            bounds_of(&[(x, Bounds::new(f64::NAN, 1.0))]),
            params_of(&[]),
        )
        .unwrap_err();
    assert!(matches!(err, BoundError::NonFiniteBound { variable } if variable == x));
}

/// A NaN constant term is a typed error.
#[test]
fn interval_rejects_nan_constant() {
    let f = linear(LinExpr::from_constant(f64::NAN));
    let err = BoundAnalyzer::new()
        .interval_of(&f, bounds_of(&[]), params_of(&[]))
        .unwrap_err();
    assert!(matches!(err, BoundError::NonFiniteConstant));
}

/// A NaN parameter value used by a bare-parameter coefficient is a typed
/// error naming the parameter.
#[test]
fn interval_rejects_nan_parameter_value() {
    let p = param(0);
    let x = var(0);
    let f = linear(LinExpr::new().term(p, x));
    let err = BoundAnalyzer::new()
        .interval_of(
            &f,
            bounds_of(&[(x, Bounds::new(0.0, 1.0))]),
            params_of(&[(p, f64::NAN)]),
        )
        .unwrap_err();
    assert!(matches!(err, BoundError::NonFiniteParameterValue { parameter } if parameter == p));
}

/// A complex coefficient expression evaluating to NaN is a typed error.
#[test]
fn interval_rejects_nan_evaluated_coefficient_expression() {
    let x = var(0);
    let nan_expr = ValueExpr::div(ValueExpr::constant(0.0), ValueExpr::constant(0.0));
    let f = linear(LinExpr::new().term(TermCoeff::Expr(nan_expr), x));
    let err = BoundAnalyzer::new()
        .interval_of(&f, bounds_of(&[(x, Bounds::new(0.0, 1.0))]), params_of(&[]))
        .unwrap_err();
    assert!(matches!(err, BoundError::NonFiniteCoefficient { variable } if variable == x));
}

// ── Snapshot convenience ───────────────────────────────────────────────────

/// `interval_of_snapshot` reads declared variable bounds and evaluated
/// parameter values from a canonical snapshot.
#[test]
fn interval_of_snapshot_uses_declared_bounds_and_parameters() {
    let x = var(0);
    let p = param(0);
    let f = linear(LinExpr::new().term(p, x).constant(1.0));
    let snapshot = roml::ModelSnapshot {
        revision: roml::ModelRevision::ZERO,
        variables: vec![roml::snapshot::VariableEntry {
            id: x,
            bounds: Bounds::new(0.0, 10.0),
            var_type: roml::model::VarType::Continuous,
            active: true,
            semicontinuous_lower: None,
            fixing: None,
        }],
        constraints: vec![],
        objectives: vec![],
        parameters: vec![roml::snapshot::ParameterEntry { id: p, value: 3.0 }],
        cells: vec![],
        functions: vec![],
        constructs: vec![],
    };
    let trace = BoundAnalyzer::new()
        .interval_of_snapshot(&f, &snapshot)
        .expect("snapshot input must analyze");
    assert_eq!(
        trace.result,
        Interval {
            lower: 1.0,
            upper: 31.0
        }
    );
}
