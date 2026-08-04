//! P33 Task 1 — piecewise-linear semantics (packet Task 18; design §17;
//! SM-14.1, SM-14.2, SM-12.8).
//!
//! Covers the PWL payload (`PiecewiseLinearConstraint`, `PwlRelation`,
//! `ExtrapolationPolicy`, `PwlCurvature`, `PwlPoint`), typed validation
//! rejections (non-finite / duplicate / out-of-order / underspecified points —
//! SM-14.1), deterministic curvature classification from segment slopes
//! (SM-14.2), the direct interpolation/extrapolation evaluator, and the
//! `Model::add_piecewise_linear` builder returning a stable `Construct` handle
//! plus the output-variable handle (SM-12.8).

use roml::construct::{
    ConstructKind, ExtrapolationPolicy, FormulationPreference, PiecewiseLinearConstraint,
    PwlCurvature, PwlPoint, PwlRelation,
};
use roml::expr::LinExpr;
use roml::id::{Generation, ParamId, VarId};
use roml::model::ModelError;
use roml::prelude::*;
use roml::value_expr::ValueExpr;

// ===========================================================================
// Shared helpers (grow across Tasks 1–3)
// ===========================================================================

/// Build `Vec<PwlPoint>` from `(breakpoint, constant value)` pairs.
fn pwl_points(values: &[(f64, f64)]) -> Vec<PwlPoint> {
    values.iter().copied().map(PwlPoint::from).collect()
}

/// A two-segment convex PWL: `[0, 1, 4]` at breakpoints `[0, 1, 2]`
/// (slopes `1, 3` — non-decreasing → convex).
fn convex_pwl() -> Vec<PwlPoint> {
    pwl_points(&[(0.0, 0.0), (1.0, 1.0), (2.0, 4.0)])
}

/// A two-segment concave PWL: slopes `3, 1` — non-increasing → concave.
fn concave_pwl() -> Vec<PwlPoint> {
    pwl_points(&[(0.0, 0.0), (1.0, 3.0), (2.0, 4.0)])
}

/// A two-segment affine PWL: slopes equal (`2, 2`).
fn affine_pwl() -> Vec<PwlPoint> {
    pwl_points(&[(0.0, 0.0), (1.0, 2.0), (2.0, 4.0)])
}

/// A three-segment nonconvex PWL: slopes `1, 2, 1` — a slope rise then fall
/// (neither non-decreasing nor non-increasing → nonconvex).
fn nonconvex_pwl() -> Vec<PwlPoint> {
    pwl_points(&[(0.0, 0.0), (1.0, 1.0), (2.0, 3.0), (3.0, 4.0)])
}

/// Construct a PWL payload directly (pure semantics — no builder validation).
fn pwl_payload_direct(
    points: Vec<PwlPoint>,
    relation: PwlRelation,
    extrapolation: ExtrapolationPolicy,
) -> PiecewiseLinearConstraint {
    PiecewiseLinearConstraint {
        points,
        relation,
        extrapolation,
        argument: LinExpr::new(),
        output: VarId::new(0, Generation::new()),
    }
}

/// Pull the PWL payload out of a model snapshot by construct id.
fn pwl_payload(model: &Model, k: roml::Construct) -> PiecewiseLinearConstraint {
    let snapshot = model.take_snapshot().unwrap();
    let entry = snapshot
        .constructs
        .iter()
        .find(|e| e.id == k)
        .expect("construct present in snapshot");
    match &entry.kind {
        ConstructKind::PiecewiseLinear(payload) => payload.clone(),
        other => panic!("expected PiecewiseLinear payload, got {other:?}"),
    }
}

// ===========================================================================
// Task 1 — Validation (SM-14.1)
// ===========================================================================

#[test]
fn pwl_rejects_fewer_than_two_points() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 2.0)).unwrap();
    let err = model
        .add_piecewise_linear(
            LinExpr::from(x),
            pwl_points(&[(0.0, 0.0)]),
            PwlRelation::ExactGraph,
            ExtrapolationPolicy::Constant,
            None,
        )
        .unwrap_err();
    assert_eq!(err, ModelError::PwlTooFewPoints);
}

#[test]
fn pwl_rejects_non_finite_breakpoint() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 2.0)).unwrap();
    let err = model
        .add_piecewise_linear(
            LinExpr::from(x),
            pwl_points(&[(f64::NAN, 0.0), (1.0, 1.0)]),
            PwlRelation::ExactGraph,
            ExtrapolationPolicy::Constant,
            None,
        )
        .unwrap_err();
    assert!(
        matches!(
            &err,
            ModelError::PwlNonFiniteBreakpoint(v) if v.is_nan()
        ),
        "NaN breakpoint must be a typed PwlNonFiniteBreakpoint, got {err:?}"
    );

    let err = model
        .add_piecewise_linear(
            LinExpr::from(x),
            pwl_points(&[(0.0, 0.0), (f64::INFINITY, 1.0)]),
            PwlRelation::ExactGraph,
            ExtrapolationPolicy::Constant,
            None,
        )
        .unwrap_err();
    assert_eq!(err, ModelError::PwlNonFiniteBreakpoint(f64::INFINITY));
}

#[test]
fn pwl_rejects_duplicate_breakpoint() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 2.0)).unwrap();
    let err = model
        .add_piecewise_linear(
            LinExpr::from(x),
            pwl_points(&[(0.0, 0.0), (1.0, 1.0), (1.0, 2.0)]),
            PwlRelation::ExactGraph,
            ExtrapolationPolicy::Constant,
            None,
        )
        .unwrap_err();
    assert_eq!(
        err,
        ModelError::PwlDuplicateBreakpoint { value: 1.0 },
        "duplicate breakpoints must be a typed rejection (SM-14.1)"
    );
}

#[test]
fn pwl_rejects_out_of_order_breakpoint() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 2.0)).unwrap();
    let err = model
        .add_piecewise_linear(
            LinExpr::from(x),
            pwl_points(&[(0.0, 0.0), (2.0, 2.0), (1.0, 1.0)]),
            PwlRelation::ExactGraph,
            ExtrapolationPolicy::Constant,
            None,
        )
        .unwrap_err();
    assert_eq!(
        err,
        ModelError::PwlOutOfOrderBreakpoint {
            value: 1.0,
            previous: 2.0,
        },
        "out-of-order breakpoints must be a typed rejection (SM-14.1)"
    );
}

#[test]
fn pwl_rejects_non_finite_point_value() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 2.0)).unwrap();
    let err = model
        .add_piecewise_linear(
            LinExpr::from(x),
            vec![
                PwlPoint {
                    x: 0.0,
                    value: ValueExpr::constant(f64::NAN),
                },
                PwlPoint {
                    x: 1.0,
                    value: ValueExpr::constant(1.0),
                },
            ],
            PwlRelation::ExactGraph,
            ExtrapolationPolicy::Constant,
            None,
        )
        .unwrap_err();
    assert_eq!(
        err,
        ModelError::PwlNonFinitePointValue,
        "non-finite point values must be a typed rejection (SM-14.1)"
    );
}

// ===========================================================================
// Task 1 — Curvature classification (SM-14.2)
// ===========================================================================

#[test]
fn pwl_curvature_classifies_affine_from_equal_slopes() {
    let payload = pwl_payload_direct(
        affine_pwl(),
        PwlRelation::ExactGraph,
        ExtrapolationPolicy::Constant,
    );
    assert_eq!(payload.classify_curvature(), PwlCurvature::Affine);
}

#[test]
fn pwl_curvature_classifies_convex_from_non_decreasing_slopes() {
    let payload = pwl_payload_direct(
        convex_pwl(),
        PwlRelation::Epigraph,
        ExtrapolationPolicy::Constant,
    );
    assert_eq!(payload.classify_curvature(), PwlCurvature::Convex);
}

#[test]
fn pwl_curvature_classifies_concave_from_non_increasing_slopes() {
    let payload = pwl_payload_direct(
        concave_pwl(),
        PwlRelation::Hypograph,
        ExtrapolationPolicy::Constant,
    );
    assert_eq!(payload.classify_curvature(), PwlCurvature::Concave);
}

#[test]
fn pwl_curvature_classifies_nonconvex_from_slope_sign_change() {
    let payload = pwl_payload_direct(
        nonconvex_pwl(),
        PwlRelation::ExactGraph,
        ExtrapolationPolicy::Constant,
    );
    assert_eq!(payload.classify_curvature(), PwlCurvature::NonConvex);
}

// ===========================================================================
// Task 1 — Direct evaluator (interpolation/extrapolation)
// ===========================================================================

/// f(x) on points (0,0), (1,2), (2,4) — affine slope 2.
#[test]
fn pwl_evaluate_interpolates_inside_breakpoint_range() {
    let payload = pwl_payload_direct(
        affine_pwl(),
        PwlRelation::ExactGraph,
        ExtrapolationPolicy::Constant,
    );
    assert_eq!(payload.evaluate(0.0), 0.0);
    assert_eq!(payload.evaluate(0.5), 1.0);
    assert_eq!(payload.evaluate(1.0), 2.0);
    assert_eq!(payload.evaluate(1.5), 3.0);
    assert_eq!(payload.evaluate(2.0), 4.0);
}

/// Constant extrapolation: values clamp to the end breakpoint values.
#[test]
fn pwl_evaluate_extrapolates_constant_policy() {
    let payload = pwl_payload_direct(
        affine_pwl(),
        PwlRelation::ExactGraph,
        ExtrapolationPolicy::Constant,
    );
    assert_eq!(
        payload.evaluate(-3.0),
        0.0,
        "left constant extrapolation clamps to v0"
    );
    assert_eq!(
        payload.evaluate(7.0),
        4.0,
        "right constant extrapolation clamps to vn"
    );
}

/// Linear extrapolation: the end segment slope continues outside the range.
#[test]
fn pwl_evaluate_extrapolates_linear_policy() {
    let payload = pwl_payload_direct(
        affine_pwl(),
        PwlRelation::ExactGraph,
        ExtrapolationPolicy::Linear,
    );
    assert_eq!(
        payload.evaluate(-1.0),
        -2.0,
        "left linear extrapolation: v0 + s0*(x - x0)"
    );
    assert_eq!(
        payload.evaluate(3.0),
        6.0,
        "right linear extrapolation: vn + s_last*(x - xn)"
    );
}

// ===========================================================================
// Task 1 — Builder returns stable handle + output variable (SM-12.8)
// ===========================================================================

#[test]
fn pwl_builder_returns_construct_and_output_variable() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 2.0)).unwrap();
    let (k, output) = model
        .add_piecewise_linear(
            LinExpr::from(x),
            convex_pwl(),
            PwlRelation::Epigraph,
            ExtrapolationPolicy::Constant,
            Some(FormulationPreference::Portable),
        )
        .unwrap();

    let snapshot = model.take_snapshot().unwrap();
    let entry = snapshot.constructs.iter().find(|e| e.id == k).unwrap();
    assert_eq!(entry.preference, FormulationPreference::Portable);
    match &entry.kind {
        ConstructKind::PiecewiseLinear(payload) => {
            assert_eq!(
                payload.output, output,
                "builder stores the output variable in the payload"
            );
            assert_eq!(payload.relation, PwlRelation::Epigraph);
            assert_eq!(payload.extrapolation, ExtrapolationPolicy::Constant);
            assert_eq!(payload.points.len(), 3);
            assert_eq!(payload.argument, LinExpr::from(x));
            // The output variable is created by the builder (continuous).
            let out_entry = snapshot
                .variables
                .iter()
                .find(|v| v.id == output)
                .expect("output variable exists in snapshot");
            assert!(out_entry.active);
        }
        other => panic!("expected PiecewiseLinear payload, got {other:?}"),
    }
}

#[test]
fn pwl_parameter_dependencies_include_point_values_and_argument() {
    let mut model = Model::new();
    let p = model.add_parameter(parameter(1.0).named("price")).unwrap();
    let x = model.add_variable(continuous().bounds(0.0, 2.0)).unwrap();
    let (k, _) = model
        .add_piecewise_linear(
            LinExpr::from(x),
            vec![
                PwlPoint {
                    x: 0.0,
                    value: ValueExpr::constant(0.0),
                },
                PwlPoint {
                    x: 1.0,
                    value: ValueExpr::param(p),
                },
            ],
            PwlRelation::ExactGraph,
            ExtrapolationPolicy::Constant,
            None,
        )
        .unwrap();
    let payload = pwl_payload(&model, k);
    assert!(
        payload.parameter_dependencies().contains(&p),
        "point-value parameter dependency must be derived (F1)"
    );
}

#[test]
fn pwl_builder_rejects_missing_parameter_in_point_value() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 2.0)).unwrap();
    // A parameter id that was never added to the model.
    let ghost_param = ParamId::new(999, Generation::new());
    let err = model
        .add_piecewise_linear(
            LinExpr::from(x),
            vec![
                PwlPoint {
                    x: 0.0,
                    value: ValueExpr::constant(0.0),
                },
                PwlPoint {
                    x: 1.0,
                    value: ValueExpr::param(ghost_param),
                },
            ],
            PwlRelation::ExactGraph,
            ExtrapolationPolicy::Constant,
            None,
        )
        .unwrap_err();
    assert_eq!(err, ModelError::ParameterNotFound(ghost_param));
}
