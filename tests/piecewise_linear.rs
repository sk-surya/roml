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

use roml::compiler::backend_ir::{
    BackendSnapshot, CompiledConstraintId, CompiledEntityRef, CompiledLinearRow, CompiledVariableId,
};
use roml::compiler::capability::{
    BackendCapabilitySet, BackendFeature, CompilationPolicy, FeatureSupport,
};
use roml::compiler::origin::{EntityOrigin, GeneratedRole};
use roml::compiler::session::CompilationSession;
use roml::compiler::CompileError;
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

/// Capability set with the PWL bridge declared (Tasks 2/3).
fn pwl_bridge_caps() -> BackendCapabilitySet {
    let mut set = BackendCapabilitySet::new();
    for f in [
        BackendFeature::Lp,
        BackendFeature::Mip,
        BackendFeature::IncrementalBounds,
        BackendFeature::IncrementalRows,
        BackendFeature::IncrementalCoefficients,
    ] {
        set.set(f, FeatureSupport::native(Default::default()));
    }
    set.set(
        BackendFeature::PiecewiseLinear,
        FeatureSupport::bridge(Default::default()),
    );
    set
}

/// Compile a model snapshot, panicking on failure.
fn compile(
    model: &Model,
    policy: CompilationPolicy,
    caps: &BackendCapabilitySet,
) -> BackendSnapshot {
    let snapshot = model.take_snapshot().unwrap();
    let mut session = CompilationSession::new();
    session
        .compile_snapshot(model.instance(), &snapshot, &policy, caps)
        .expect("snapshot must compile")
}

/// Compile a model snapshot, expecting a typed error.
fn compile_err(
    model: &Model,
    policy: CompilationPolicy,
    caps: &BackendCapabilitySet,
) -> CompileError {
    let snapshot = model.take_snapshot().unwrap();
    let mut session = CompilationSession::new();
    session
        .compile_snapshot(model.instance(), &snapshot, &policy, caps)
        .expect_err("snapshot compilation must fail")
}

/// The compiled id of a user variable (exactly one compiled projection).
fn compiled_user_var(compiled: &BackendSnapshot, var: VarId) -> CompiledVariableId {
    let ids = compiled
        .origin_map
        .variables_for_origin(&EntityOrigin::UserVariable(var));
    assert_eq!(ids.len(), 1, "each user variable has one compiled id");
    ids[0]
}

/// Compiled constraint ids tracing to a construct role.
fn compiled_rows_for_role(
    compiled: &BackendSnapshot,
    construct: roml::Construct,
    role: GeneratedRole,
) -> Vec<CompiledConstraintId> {
    compiled
        .origin_map
        .constraints_for_origin(&EntityOrigin::Construct { construct, role })
}

/// The compiled row for a compiled constraint id.
fn compiled_row(compiled: &BackendSnapshot, id: CompiledConstraintId) -> &CompiledLinearRow {
    compiled
        .linear_rows
        .iter()
        .find(|r| r.id == id)
        .expect("row present in compiled snapshot")
}

/// Generated binary variable ids tracing to a construct (any role).
fn generated_binaries_for_construct(
    compiled: &BackendSnapshot,
    construct: roml::Construct,
) -> Vec<CompiledVariableId> {
    compiled
        .variables
        .iter()
        .filter(|v| v.var_type == roml::model::VarType::Binary)
        .filter(|v| {
            matches!(
                compiled
                    .origin_map
                    .origin_for(CompiledEntityRef::Variable(v.id)),
                Some(EntityOrigin::Construct { construct: c, .. }) if *c == construct
            )
        })
        .map(|v| v.id)
        .collect()
}

/// The coefficient on `var` in a compiled row (0.0 when absent).
fn row_coefficient(row: &CompiledLinearRow, var: CompiledVariableId) -> f64 {
    row.coefficients
        .iter()
        .find(|(id, _)| *id == var)
        .map(|(_, c)| *c)
        .unwrap_or(0.0)
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

// ===========================================================================
// Task 2 — Zero-binary one-sided PWL bridges (SM-14.3) and reports
// ===========================================================================

/// Convex PWL `[(0,0),(1,1),(2,4)]` — slopes `1, 3` → Convex. Epigraph rows
/// `output >= v_i + s_i*(x - x_i)`: `output - x >= 0`; `output - 3x >= -2`
/// twice.
#[test]
fn pwl_convex_epigraph_compiles_with_zero_binaries_and_supporting_rows() {
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
    let compiled = compile(&model, CompilationPolicy::Portable, &pwl_bridge_caps());

    // SM-14.3: zero generated binaries.
    assert!(
        generated_binaries_for_construct(&compiled, k).is_empty(),
        "convex epigraph must introduce zero generated binaries (SM-14.3)"
    );

    let xid = compiled_user_var(&compiled, x);
    let yid = compiled_user_var(&compiled, output);
    let rows = compiled_rows_for_role(&compiled, k, GeneratedRole::PwlEpigraphRow);
    assert_eq!(rows.len(), 3, "one supporting row per breakpoint");
    // Expected: (coeff on y, coeff on x, lower bound) per breakpoint i.
    let expected: [(f64, f64, f64); 3] = [
        (1.0, -1.0, 0.0),  // output - x >= 0
        (1.0, -3.0, -2.0), // output - 3x >= -2
        (1.0, -3.0, -2.0), // output - 3x >= -2 (last segment slope)
    ];
    for (i, rid) in rows.iter().enumerate() {
        let row = compiled_row(&compiled, *rid);
        assert!(
            (row.bounds.lower - expected[i].2).abs() < 1e-9,
            "row {i} lower bound: got {}, expected {}",
            row.bounds.lower,
            expected[i].2
        );
        assert_eq!(
            row_coefficient(row, yid),
            expected[i].0,
            "row {i} output coeff"
        );
        assert_eq!(
            row_coefficient(row, xid),
            expected[i].1,
            "row {i} argument coeff"
        );
        // SM-02.5: every generated row carries a construct origin.
        assert!(
            matches!(
                compiled.origin_map.constraint_origin(*rid),
                Some(EntityOrigin::Construct { construct: c, .. }) if *c == k
            ),
            "row {i} must carry a Construct origin"
        );
    }
}

/// Concave PWL `[(0,0),(1,3),(2,4)]` — slopes `3, 1` → Concave. Hypograph rows
/// `output <= v_i + s_i*(x - x_i)`: `output - 3x <= 0`; `output - x <= 2`
/// twice.
#[test]
fn pwl_concave_hypograph_compiles_with_zero_binaries_and_supporting_rows() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 2.0)).unwrap();
    let (k, output) = model
        .add_piecewise_linear(
            LinExpr::from(x),
            concave_pwl(),
            PwlRelation::Hypograph,
            ExtrapolationPolicy::Constant,
            Some(FormulationPreference::Portable),
        )
        .unwrap();
    let compiled = compile(&model, CompilationPolicy::Portable, &pwl_bridge_caps());

    assert!(
        generated_binaries_for_construct(&compiled, k).is_empty(),
        "concave hypograph must introduce zero generated binaries (SM-14.3)"
    );

    let xid = compiled_user_var(&compiled, x);
    let yid = compiled_user_var(&compiled, output);
    let rows = compiled_rows_for_role(&compiled, k, GeneratedRole::PwlHypographRow);
    assert_eq!(rows.len(), 3, "one supporting row per breakpoint");
    let expected: [(f64, f64, f64); 3] = [
        (1.0, -3.0, 0.0), // output - 3x <= 0
        (1.0, -1.0, 2.0), // output - x <= 2
        (1.0, -1.0, 2.0), // output - x <= 2
    ];
    for (i, rid) in rows.iter().enumerate() {
        let row = compiled_row(&compiled, *rid);
        assert!(
            (row.bounds.upper - expected[i].2).abs() < 1e-9,
            "row {i} upper bound: got {}, expected {}",
            row.bounds.upper,
            expected[i].2
        );
        assert_eq!(
            row_coefficient(row, yid),
            expected[i].0,
            "row {i} output coeff"
        );
        assert_eq!(
            row_coefficient(row, xid),
            expected[i].1,
            "row {i} argument coeff"
        );
    }
}

/// Epigraph on a non-convex PWL is a typed compile error — never a silent
/// relaxation (D13).
#[test]
fn pwl_epigraph_on_nonconvex_pwl_is_typed_error() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 3.0)).unwrap();
    model
        .add_piecewise_linear(
            LinExpr::from(x),
            nonconvex_pwl(),
            PwlRelation::Epigraph,
            ExtrapolationPolicy::Constant,
            None,
        )
        .unwrap();
    let err = compile_err(&model, CompilationPolicy::Portable, &pwl_bridge_caps());
    assert!(
        matches!(err, CompileError::UnsupportedFeature(_)),
        "Epigraph on a non-convex PWL must be a typed CompileError, got {err:?}"
    );
}

/// Hypograph on a non-concave (convex) PWL is a typed compile error — never a
/// silent relaxation (D13).
#[test]
fn pwl_hypograph_on_nonconcave_pwl_is_typed_error() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 2.0)).unwrap();
    model
        .add_piecewise_linear(
            LinExpr::from(x),
            convex_pwl(), // convex, not concave
            PwlRelation::Hypograph,
            ExtrapolationPolicy::Constant,
            None,
        )
        .unwrap();
    let err = compile_err(&model, CompilationPolicy::Portable, &pwl_bridge_caps());
    assert!(
        matches!(err, CompileError::UnsupportedFeature(_)),
        "Hypograph on a non-concave PWL must be a typed CompileError, got {err:?}"
    );
}

/// The compilation report records curvature, relation, representation, zero
/// generated binaries, the binary-avoidance reason, and the argument interval
/// + bound sources (SM-14.6, SM-13.5).
#[test]
fn pwl_report_records_curvature_relation_representation_and_bound_evidence() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 2.0)).unwrap();
    let (_k, _) = model
        .add_piecewise_linear(
            LinExpr::from(x),
            convex_pwl(),
            PwlRelation::Epigraph,
            ExtrapolationPolicy::Constant,
            None,
        )
        .unwrap();
    let compiled = compile(&model, CompilationPolicy::Portable, &pwl_bridge_caps());

    let decisions = &compiled.report.formulation_decisions;
    let find = |key: &str| {
        decisions
            .iter()
            .find(|d| d.decision == key)
            .unwrap_or_else(|| panic!("missing report decision {key}"))
    };

    assert_eq!(find("pwl.curvature").selection, "Convex");
    assert_eq!(find("pwl.relation").selection, "Epigraph");
    let rep = find("pwl.representation");
    assert!(
        rep.selection.contains("supporting inequalities"),
        "representation must name supporting inequalities, got {}",
        rep.selection
    );
    assert_eq!(find("pwl.generated_binaries").selection, "0");
    assert!(
        find("pwl.binary_avoidance_reason")
            .reason
            .contains("zero generated binaries"),
        "binary-avoidance reason must explain why binaries were avoided"
    );

    // SM-13.5: the argument interval and its bound sources are recorded.
    let arg_interval = find("pwl.argument_interval");
    assert!(
        arg_interval.selection.contains("[0, 2]") || arg_interval.selection.contains("0"),
        "argument interval must be recorded, got {}",
        arg_interval.selection
    );
    assert!(
        arg_interval.reason.contains("DeclaredVariableBounds"),
        "argument interval must carry bound-source provenance, got {}",
        arg_interval.reason
    );

    // The PWL construct is Bridge-only (no native claim).
    assert!(
        compiled
            .report
            .formulation_decisions
            .iter()
            .any(|d| d.decision == "pwl.path" && d.selection.contains("bridge")),
        "pwl.path decision must record the bridge path"
    );
}

/// ExactGraph is not yet compiled by the one-sided bridge (Task 3 adds the
/// exact representation) — it must be a typed error, never a relaxation.
#[test]
fn pwl_exact_graph_is_typed_error_before_task3() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 2.0)).unwrap();
    model
        .add_piecewise_linear(
            LinExpr::from(x),
            convex_pwl(),
            PwlRelation::ExactGraph,
            ExtrapolationPolicy::Constant,
            None,
        )
        .unwrap();
    let err = compile_err(&model, CompilationPolicy::Portable, &pwl_bridge_caps());
    assert!(
        matches!(err, CompileError::UnsupportedFeature(_)),
        "exact-graph compilation must be a typed error before the Task 3 bridge, got {err:?}"
    );
}

/// Every generated PWL row carries `EntityOrigin::Construct { construct, role }`
/// (SM-02.5) — verified per row above; this test asserts origin completeness at
/// the snapshot level.
#[test]
fn pwl_zero_binary_rows_are_origin_complete() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 2.0)).unwrap();
    let (k, _) = model
        .add_piecewise_linear(
            LinExpr::from(x),
            convex_pwl(),
            PwlRelation::Epigraph,
            ExtrapolationPolicy::Constant,
            None,
        )
        .unwrap();
    let compiled = compile(&model, CompilationPolicy::Portable, &pwl_bridge_caps());
    let rows = compiled_rows_for_role(&compiled, k, GeneratedRole::PwlEpigraphRow);
    assert_eq!(rows.len(), 3);
    // No compiled entity is missing an origin.
    assert!(
        compiled
            .origin_map
            .missing_origins(
                &compiled.variables,
                &compiled.linear_rows,
                &compiled.objectives
            )
            .is_empty(),
        "every compiled entity must carry an origin (SM-02.5)"
    );
}
