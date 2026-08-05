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

use std::collections::HashMap;

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
    PwlCurvature, PwlEvalError, PwlPoint, PwlRelation,
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

/// Generated variable ids tracing to a construct role (deterministic order).
fn generated_vars_for_role(
    compiled: &BackendSnapshot,
    construct: roml::Construct,
    role: GeneratedRole,
) -> Vec<CompiledVariableId> {
    compiled
        .origin_map
        .variables_for_origin(&EntityOrigin::Construct { construct, role })
}

/// All compiled linear rows hold for the given fixed compiled-variable values
/// (unprovided variables are treated as 0).
fn rows_hold(compiled: &BackendSnapshot, values: &HashMap<CompiledVariableId, f64>) -> bool {
    compiled.linear_rows.iter().all(|row| {
        let sum: f64 = row
            .coefficients
            .iter()
            .map(|(vid, c)| values.get(vid).copied().unwrap_or(0.0) * c)
            .sum();
        row.bounds.lower - 1e-9 <= sum && sum <= row.bounds.upper + 1e-9
    })
}

/// Fixed context for exact-graph feasibility checks over one compiled PWL.
struct ExactGraphCtx<'a> {
    compiled: &'a BackendSnapshot,
    x_var: CompiledVariableId,
    y_var: CompiledVariableId,
    points: &'a [(f64, f64)],
    z_binaries: &'a [CompiledVariableId],
    lambda_weights: &'a [CompiledVariableId],
}

impl ExactGraphCtx<'_> {
    /// Whether the exact-graph formulation admits `(argument = x, output = y)`.
    ///
    /// Algebraic existential check over the segment binaries (P32 Task 17
    /// pattern): for each segment `k`, place `z_k = 1`, compute the two weight
    /// values from `x`'s position in the segment, and verify every compiled row
    /// holds.
    fn feasible(&self, x: f64, y: f64) -> bool {
        for k in 0..self.z_binaries.len() {
            let (xk, _) = self.points[k];
            let (xk1, _) = self.points[k + 1];
            if xk <= x && x <= xk1 {
                let t = (x - xk) / (xk1 - xk);
                let mut values: HashMap<CompiledVariableId, f64> = HashMap::new();
                values.insert(self.x_var, x);
                values.insert(self.y_var, y);
                for (i, z) in self.z_binaries.iter().enumerate() {
                    values.insert(*z, if i == k { 1.0 } else { 0.0 });
                }
                for (i, l) in self.lambda_weights.iter().enumerate() {
                    let value = if i == k {
                        1.0 - t
                    } else if i == k + 1 {
                        t
                    } else {
                        0.0
                    };
                    values.insert(*l, value);
                }
                if rows_hold(self.compiled, &values) {
                    return true;
                }
            }
        }
        false
    }
}

/// A tiny deterministic LCG for fixed-seed "random" tests (no external dep).
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }

    /// Uniform `f64` in `[0, 1)`.
    fn next_f64(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
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
    assert_eq!(payload.classify_curvature().unwrap(), PwlCurvature::Affine);
}

#[test]
fn pwl_curvature_classifies_convex_from_non_decreasing_slopes() {
    let payload = pwl_payload_direct(
        convex_pwl(),
        PwlRelation::Epigraph,
        ExtrapolationPolicy::Constant,
    );
    assert_eq!(payload.classify_curvature().unwrap(), PwlCurvature::Convex);
}

#[test]
fn pwl_curvature_classifies_concave_from_non_increasing_slopes() {
    let payload = pwl_payload_direct(
        concave_pwl(),
        PwlRelation::Hypograph,
        ExtrapolationPolicy::Constant,
    );
    assert_eq!(payload.classify_curvature().unwrap(), PwlCurvature::Concave);
}

#[test]
fn pwl_curvature_classifies_nonconvex_from_slope_sign_change() {
    let payload = pwl_payload_direct(
        nonconvex_pwl(),
        PwlRelation::ExactGraph,
        ExtrapolationPolicy::Constant,
    );
    assert_eq!(
        payload.classify_curvature().unwrap(),
        PwlCurvature::NonConvex
    );
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
    assert_eq!(payload.evaluate(0.0).unwrap(), 0.0);
    assert_eq!(payload.evaluate(0.5).unwrap(), 1.0);
    assert_eq!(payload.evaluate(1.0).unwrap(), 2.0);
    assert_eq!(payload.evaluate(1.5).unwrap(), 3.0);
    assert_eq!(payload.evaluate(2.0).unwrap(), 4.0);
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
        payload.evaluate(-3.0).unwrap(),
        0.0,
        "left constant extrapolation clamps to v0"
    );
    assert_eq!(
        payload.evaluate(7.0).unwrap(),
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
        payload.evaluate(-1.0).unwrap(),
        -2.0,
        "left linear extrapolation: v0 + s0*(x - x0)"
    );
    assert_eq!(
        payload.evaluate(3.0).unwrap(),
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
        arg_interval.selection.contains("[0, 2]"),
        "argument interval must be recorded exactly, got {}",
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

// ===========================================================================
// Task 3 — Exact graph via deterministic exact segment binaries (SM-14.4/14.5)
// ===========================================================================

/// The exact-graph feasible set equals the graph for every curvature class:
/// `y = f(x)` is feasible and `y ± delta` is infeasible (SM-14.4).
#[test]
fn pwl_exact_graph_feasible_set_equals_graph_for_all_curvatures() {
    let curves: [(&str, Vec<PwlPoint>); 4] = [
        ("affine", affine_pwl()),
        ("convex", convex_pwl()),
        ("concave", concave_pwl()),
        ("nonconvex", nonconvex_pwl()),
    ];
    for (name, points) in curves {
        let pts: Vec<(f64, f64)> = points
            .iter()
            .map(|p| (p.x, p.value.as_constant().unwrap()))
            .collect();
        let x0 = points[0].x;
        let xn = points[points.len() - 1].x;
        let payload = pwl_payload_direct(
            points.clone(),
            PwlRelation::ExactGraph,
            ExtrapolationPolicy::Constant,
        );
        let mut model = Model::new();
        let x = model.add_variable(continuous().bounds(x0, xn)).unwrap();
        let (k, output) = model
            .add_piecewise_linear(
                LinExpr::from(x),
                points,
                PwlRelation::ExactGraph,
                ExtrapolationPolicy::Constant,
                None,
            )
            .unwrap();
        let compiled = compile(&model, CompilationPolicy::Portable, &pwl_bridge_caps());
        let ctx = ExactGraphCtx {
            compiled: &compiled,
            x_var: compiled_user_var(&compiled, x),
            y_var: compiled_user_var(&compiled, output),
            points: &pts,
            z_binaries: &generated_vars_for_role(&compiled, k, GeneratedRole::PwlSegmentBinary),
            lambda_weights: &generated_vars_for_role(
                &compiled,
                k,
                GeneratedRole::PwlWeightVariable,
            ),
        };

        // Sample each breakpoint and each segment midpoint.
        let mut samples: Vec<f64> = pts.iter().map(|p| p.0).collect();
        for w in pts.windows(2) {
            samples.push(0.5 * (w[0].0 + w[1].0));
        }
        for xv in samples {
            let yv = payload.evaluate(xv).unwrap();
            assert!(
                ctx.feasible(xv, yv),
                "{name}: on-graph y={yv} at x={xv} must be feasible (SM-14.4)"
            );
            assert!(
                !ctx.feasible(xv, yv + 0.05),
                "{name}: y+delta must be infeasible — the exact graph, not a relaxation"
            );
            assert!(
                !ctx.feasible(xv, yv - 0.05),
                "{name}: y-delta must be infeasible — the exact graph, not a relaxation"
            );
        }
    }
}

/// A three-segment zigzag PWL: slopes `1, -1, 1` — NonConvex, with a convex-hull
/// point (1, 0.5) that is NOT on the graph.
fn nonconvex_zigzag() -> Vec<PwlPoint> {
    pwl_points(&[(0.0, 0.0), (1.0, 1.0), (2.0, 0.0), (3.0, 1.0)])
}

/// Nonconvex exact graphs never fall back to a convex relaxation (SM-14.5):
/// a point admitted by the convex hull of the graph but not on the graph is
/// infeasible in the compiled formulation. The phase-gate proof test.
#[test]
fn pwl_nonconvex_exact_graph_excludes_convex_relaxation() {
    let points = nonconvex_zigzag();
    let pts: Vec<(f64, f64)> = points
        .iter()
        .map(|p| (p.x, p.value.as_constant().unwrap()))
        .collect();
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 3.0)).unwrap();
    let (k, output) = model
        .add_piecewise_linear(
            LinExpr::from(x),
            points,
            PwlRelation::ExactGraph,
            ExtrapolationPolicy::Constant,
            None,
        )
        .unwrap();
    let compiled = compile(&model, CompilationPolicy::Portable, &pwl_bridge_caps());
    let ctx = ExactGraphCtx {
        compiled: &compiled,
        x_var: compiled_user_var(&compiled, x),
        y_var: compiled_user_var(&compiled, output),
        points: &pts,
        z_binaries: &generated_vars_for_role(&compiled, k, GeneratedRole::PwlSegmentBinary),
        lambda_weights: &generated_vars_for_role(&compiled, k, GeneratedRole::PwlWeightVariable),
    };
    assert_eq!(
        ctx.z_binaries.len(),
        3,
        "three segments → three adjacency binaries"
    );

    // On-graph: (x=1, y=1) is feasible.
    assert!(ctx.feasible(1.0, 1.0));
    // Convex-hull point: (x=1, y=0.5) lies on the chord (0,0)-(2,0) — admitted by
    // the convex hull of the graph but NOT on the graph (f(1)=1). It must be
    // infeasible: no convex relaxation (SM-14.5).
    assert!(
        !ctx.feasible(1.0, 0.5),
        "the convex-hull point (1, 0.5) must be infeasible in the exact formulation \
         (SM-14.5: never a convex relaxation)"
    );
}

/// Under `Auto`/`Portable` the exact graph selects the deterministic exact
/// segment-binary representation; the report records representation, generated
/// counts, the binary-introduction reason, and a scaling diagnostic (SM-14.4,
/// SM-14.6, ROADMAP P33).
#[test]
fn pwl_exact_graph_selects_segment_binaries_and_reports() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 2.0)).unwrap();
    let (k, _) = model
        .add_piecewise_linear(
            LinExpr::from(x),
            convex_pwl(),
            PwlRelation::ExactGraph,
            ExtrapolationPolicy::Constant,
            None,
        )
        .unwrap();
    let compiled = compile(&model, CompilationPolicy::Auto, &pwl_bridge_caps());

    let decisions = &compiled.report.formulation_decisions;
    let find = |key: &str| {
        decisions
            .iter()
            .find(|d| d.decision == key)
            .unwrap_or_else(|| panic!("missing report decision {key}"))
    };
    let rep = find("pwl.representation");
    assert!(
        rep.selection.contains("exact segment binaries"),
        "representation must name exact segment binaries, got {}",
        rep.selection
    );
    assert_eq!(
        find("pwl.generated_binaries").selection,
        "2",
        "two segments → two adjacency binaries"
    );
    assert_eq!(
        find("pwl.generated_auxiliary_variables").selection,
        "3",
        "three points → three weights"
    );
    assert!(
        find("pwl.binary_introduction_reason")
            .reason
            .contains("no convex relaxation"),
        "the report must explain the binary introduction, got {}",
        find("pwl.binary_introduction_reason").reason
    );
    assert!(
        find("pwl.scaling").selection.contains("x_span"),
        "a scaling diagnostic must be recorded"
    );

    // Role inventory: n-1 segment binaries, n weight variables.
    assert_eq!(
        generated_vars_for_role(&compiled, k, GeneratedRole::PwlSegmentBinary).len(),
        2
    );
    assert_eq!(
        generated_vars_for_role(&compiled, k, GeneratedRole::PwlWeightVariable).len(),
        3
    );
}

/// `NativeRequired` on a PWL construct rejects with `CompileError::UnsupportedFeature`
/// — no native PWL/SOS2 payload exists (P32 F4 rule).
#[test]
fn pwl_native_required_rejects_exact_graph() {
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
    let err = compile_err(
        &model,
        CompilationPolicy::NativeRequired,
        &pwl_bridge_caps(),
    );
    assert!(
        matches!(err, CompileError::UnsupportedFeature(_)),
        "NativeRequired on a Bridge-only PWL must reject (P32 F4), got {err:?}"
    );
}

/// Randomized fixed-input agreement (SM-14.7): fixed-seed random arguments over
/// the tested domain; the direct `evaluate` value is feasible in the compiled
/// exact-graph formulation, and `y ± delta` is infeasible — for all four
/// curvature classes.
#[test]
fn pwl_exact_graph_randomized_fixed_input_agreement() {
    let curves: [(&str, Vec<PwlPoint>); 4] = [
        ("affine", affine_pwl()),
        ("convex", convex_pwl()),
        ("concave", concave_pwl()),
        ("nonconvex", nonconvex_pwl()),
    ];
    let mut rng = Lcg(0x9E37_79B9_7F4A_7C15);
    for (name, points) in curves {
        let pts: Vec<(f64, f64)> = points
            .iter()
            .map(|p| (p.x, p.value.as_constant().unwrap()))
            .collect();
        let x0 = points[0].x;
        let xn = points[points.len() - 1].x;
        let payload = pwl_payload_direct(
            points.clone(),
            PwlRelation::ExactGraph,
            ExtrapolationPolicy::Constant,
        );
        let mut model = Model::new();
        let x = model.add_variable(continuous().bounds(x0, xn)).unwrap();
        let (k, output) = model
            .add_piecewise_linear(
                LinExpr::from(x),
                points,
                PwlRelation::ExactGraph,
                ExtrapolationPolicy::Constant,
                None,
            )
            .unwrap();
        let compiled = compile(&model, CompilationPolicy::Portable, &pwl_bridge_caps());
        let ctx = ExactGraphCtx {
            compiled: &compiled,
            x_var: compiled_user_var(&compiled, x),
            y_var: compiled_user_var(&compiled, output),
            points: &pts,
            z_binaries: &generated_vars_for_role(&compiled, k, GeneratedRole::PwlSegmentBinary),
            lambda_weights: &generated_vars_for_role(
                &compiled,
                k,
                GeneratedRole::PwlWeightVariable,
            ),
        };

        for _ in 0..64 {
            let xv = x0 + rng.next_f64() * (xn - x0);
            let yv = payload.evaluate(xv).unwrap();
            assert!(
                ctx.feasible(xv, yv),
                "{name}: direct evaluate {yv} at x={xv} must be feasible in the compiled \
                 formulation (SM-14.7)"
            );
            assert!(
                !ctx.feasible(xv, yv + 0.05),
                "{name}: y+delta must be infeasible (exact graph, SM-14.7)"
            );
        }
    }
}

/// Every generated exact-graph entity carries `EntityOrigin::Construct { construct,
/// role }` (SM-02.5) — origin completeness across the generated binaries, weights,
/// and rows.
#[test]
fn pwl_exact_graph_entities_are_origin_complete() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 2.0)).unwrap();
    let (k, _) = model
        .add_piecewise_linear(
            LinExpr::from(x),
            convex_pwl(),
            PwlRelation::ExactGraph,
            ExtrapolationPolicy::Constant,
            None,
        )
        .unwrap();
    let compiled = compile(&model, CompilationPolicy::Portable, &pwl_bridge_caps());
    assert_eq!(
        generated_vars_for_role(&compiled, k, GeneratedRole::PwlSegmentBinary).len(),
        2
    );
    assert_eq!(
        generated_vars_for_role(&compiled, k, GeneratedRole::PwlWeightVariable).len(),
        3
    );
    assert_eq!(
        compiled_rows_for_role(&compiled, k, GeneratedRole::PwlExactGraphRow).len(),
        7,
        "sum-lambda, argument, output, sum-z, and three adjacency rows"
    );
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

// ===========================================================================
// P1-01 review fix — extrapolation policy is enforced at compile time
// ===========================================================================
//
// The bridge previously never read `ExtrapolationPolicy`: supporting rows are
// exact only for the LINEARLY-extrapolated function, and the exact segment-
// binary formulation pins the argument to the breakpoint range. When the
// bound-derived argument interval can leave the breakpoint range, the compiled
// model silently diverges from the declared function. The fix: reject with
// `CompileError::ExtrapolationConflict` (Constant one-sided; exact graph under
// either policy) and compile exactly only the Linear one-sided case.

/// Constant extrapolation + an argument that can leave the breakpoint range:
/// the clamped function diverges from the supporting rows (relaxation when the
/// first slope is positive, restriction when negative) — a typed rejection,
/// never a silent change of the feasible set.
#[test]
fn pwl_constant_extrapolation_rejects_when_argument_leaves_breakpoint_range() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(-1.0, 3.0)).unwrap();
    let (_, _) = model
        .add_piecewise_linear(
            LinExpr::from(x),
            convex_pwl(),
            PwlRelation::Epigraph,
            ExtrapolationPolicy::Constant,
            Some(FormulationPreference::Portable),
        )
        .unwrap();
    let err = compile_err(&model, CompilationPolicy::Portable, &pwl_bridge_caps());
    assert!(
        matches!(err, CompileError::ExtrapolationConflict { .. }),
        "Constant extrapolation with an argument that can leave the breakpoint \
         range must reject with ExtrapolationConflict, got {err:?}"
    );
}

/// The hypograph mirror of the same rule.
#[test]
fn pwl_constant_hypograph_rejects_when_argument_leaves_breakpoint_range() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(-1.0, 3.0)).unwrap();
    let (_, _) = model
        .add_piecewise_linear(
            LinExpr::from(x),
            concave_pwl(),
            PwlRelation::Hypograph,
            ExtrapolationPolicy::Constant,
            Some(FormulationPreference::Portable),
        )
        .unwrap();
    let err = compile_err(&model, CompilationPolicy::Portable, &pwl_bridge_caps());
    assert!(
        matches!(err, CompileError::ExtrapolationConflict { .. }),
        "Constant hypograph with an out-of-range argument must reject, got {err:?}"
    );
}

/// Linear extrapolation + one-sided: the supporting rows are EXACT for the
/// linearly-extrapolated convex function everywhere (a convex PL function is
/// the max of its supporting lines), so compilation succeeds and the rows
/// imply exactly the linear extension outside the breakpoint range.
#[test]
fn pwl_linear_extrapolation_one_sided_compiles_exactly_outside_range() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(-1.0, 3.0)).unwrap();
    let (k, output) = model
        .add_piecewise_linear(
            LinExpr::from(x),
            convex_pwl(),
            PwlRelation::Epigraph,
            ExtrapolationPolicy::Linear,
            Some(FormulationPreference::Portable),
        )
        .unwrap();
    let compiled = compile(&model, CompilationPolicy::Portable, &pwl_bridge_caps());

    let xid = compiled_user_var(&compiled, x);
    let yid = compiled_user_var(&compiled, output);
    let rows = compiled_rows_for_role(&compiled, k, GeneratedRole::PwlEpigraphRow);
    assert_eq!(rows.len(), 3, "one supporting row per breakpoint");

    // Evaluate the row implication at points outside the breakpoint range:
    // the binding row must equal the linear extension f_lin.
    // convex_pwl = [(0,0),(1,1),(2,4)], slopes 1, 3 → f_lin(-1) = -1, f_lin(3) = 7.
    for (x_val, expected_lb) in [(-1.0, -1.0), (3.0, 7.0)] {
        let mut max_lb = f64::NEG_INFINITY;
        for rid in &rows {
            let row = compiled_row(&compiled, *rid);
            let x_coeff = row_coefficient(row, xid);
            let y_coeff = row_coefficient(row, yid);
            assert_eq!(y_coeff, 1.0, "row must be output >= ...");
            let implied = (row.bounds.lower - x_coeff * x_val) / y_coeff;
            max_lb = max_lb.max(implied);
        }
        assert!(
            (max_lb - expected_lb).abs() < 1e-9,
            "at x={x_val} the supporting rows must imply output >= {expected_lb} \
             (the linear extension), got {max_lb}"
        );
    }
}

/// The exact segment-binary formulation pins the argument to the breakpoint
/// range (`argument = sum x_i * lambda_i`), so an argument that can leave the
/// range silently narrows the user's declared bounds under EITHER policy —
/// a typed rejection.
#[test]
fn pwl_exact_graph_rejects_when_argument_leaves_breakpoint_range() {
    for extrapolation in [ExtrapolationPolicy::Constant, ExtrapolationPolicy::Linear] {
        let mut model = Model::new();
        let x = model.add_variable(continuous().bounds(-1.0, 3.0)).unwrap();
        let _ = model
            .add_piecewise_linear(
                LinExpr::from(x),
                convex_pwl(),
                PwlRelation::ExactGraph,
                extrapolation,
                None,
            )
            .unwrap();
        let err = compile_err(&model, CompilationPolicy::Auto, &pwl_bridge_caps());
        assert!(
            matches!(err, CompileError::ExtrapolationConflict { .. }),
            "exact graph with an argument that can leave the breakpoint range \
             must reject under {extrapolation:?}, got {err:?}"
        );
    }
}

/// The compilation report records the extrapolation policy and its
/// disposition, and the one-sided path records the same schema keys as the
/// exact path (review P2-02: generated auxiliaries + scaling).
#[test]
fn pwl_report_records_extrapolation_decision_and_full_schema() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 2.0)).unwrap();
    let _ = model
        .add_piecewise_linear(
            LinExpr::from(x),
            convex_pwl(),
            PwlRelation::Epigraph,
            ExtrapolationPolicy::Constant,
            None,
        )
        .unwrap();
    let compiled = compile(&model, CompilationPolicy::Auto, &pwl_bridge_caps());

    let decisions = &compiled.report.formulation_decisions;
    let find = |key: &str| {
        decisions
            .iter()
            .find(|d| d.decision == key)
            .unwrap_or_else(|| panic!("missing report decision {key}"))
    };
    let ext = find("pwl.extrapolation");
    assert!(
        ext.selection.contains("Constant"),
        "extrapolation policy must be recorded, got {}",
        ext.selection
    );
    assert!(
        ext.reason.contains("breakpoint range"),
        "the disposition must reference the breakpoint range, got {}",
        ext.reason
    );
    // P2-02: one-sided path records the same schema keys as the exact path.
    assert_eq!(
        find("pwl.generated_auxiliary_variables").selection,
        "0",
        "one-sided rows generate no auxiliaries"
    );
    assert!(
        find("pwl.scaling").selection.contains("x_span"),
        "a scaling diagnostic must be recorded on the one-sided path too"
    );
}

// ===========================================================================
// Review P1 — parameterized point values: typed errors, never panics
// ===========================================================================

/// A payload with a parameter-dependent point value is VALID (the compiler
/// bridge resolves it at compile time) — the public constant-only operations
/// must return typed errors for it, never panic (review P1).
fn parameterized_pwl() -> PiecewiseLinearConstraint {
    let p = ParamId::new(0, roml::id::Generation::new());
    PiecewiseLinearConstraint {
        points: vec![
            PwlPoint {
                x: 0.0,
                value: ValueExpr::constant(0.0),
            },
            PwlPoint {
                x: 1.0,
                value: ValueExpr::param(p),
            },
            PwlPoint {
                x: 2.0,
                value: ValueExpr::constant(4.0),
            },
        ],
        relation: PwlRelation::Epigraph,
        extrapolation: ExtrapolationPolicy::Constant,
        argument: LinExpr::from(VarId::new(0, roml::id::Generation::new())),
        output: VarId::new(1, roml::id::Generation::new()),
    }
}

/// The constant-only operations return a typed `ParameterizedPointValue`
/// error for a valid parameterized payload — the `expect` panic path is
/// removed (review P1).
#[test]
fn pwl_parameterized_points_return_typed_errors_from_constant_only_ops() {
    let pwl = parameterized_pwl();
    assert!(
        matches!(
            pwl.evaluate(0.5),
            Err(PwlEvalError::ParameterizedPointValue { index: 1, .. })
        ),
        "evaluate must return a typed error naming the parameterized point"
    );
    assert!(
        matches!(
            pwl.classify_curvature(),
            Err(PwlEvalError::ParameterizedPointValue { .. })
        ),
        "classify_curvature must return a typed error"
    );
    assert!(
        matches!(
            pwl.segment_slopes(),
            Err(PwlEvalError::ParameterizedPointValue { .. })
        ),
        "segment_slopes must return a typed error"
    );
}

/// The `_with` resolver variants evaluate parameterized point values for
/// slopes, curvature, interpolation, and extrapolation (review P1).
#[test]
fn pwl_resolver_variants_evaluate_parameterized_points() {
    let pwl = parameterized_pwl(); // points (0,0),(1,p),(2,4); p resolves to 1.0
    let resolve = |_p: ParamId| Some(1.0);

    // Slopes: (1-0)/1 = 1, (4-1)/1 = 3 → non-decreasing → convex.
    assert_eq!(pwl.segment_slopes_with(&resolve).unwrap(), vec![1.0, 3.0]);
    assert_eq!(
        pwl.classify_curvature_with(&resolve).unwrap(),
        PwlCurvature::Convex
    );
    // Interpolation at x = 0.5: f = 0 + 1 * 0.5 = 0.5.
    assert_eq!(pwl.evaluate_with(0.5, &resolve).unwrap(), 0.5);
    // Extrapolation under Constant: clamps to the end values.
    assert_eq!(pwl.evaluate_with(-1.0, &resolve).unwrap(), 0.0);
    assert_eq!(pwl.evaluate_with(3.0, &resolve).unwrap(), 4.0);
    // Extrapolation under Linear: continues the end segment slope (3.0).
    let pwl_lin = PiecewiseLinearConstraint {
        extrapolation: ExtrapolationPolicy::Linear,
        ..parameterized_pwl()
    };
    assert_eq!(
        pwl_lin.evaluate_with(3.0, &resolve).unwrap(),
        4.0 + 3.0 * 1.0
    );
}

/// A resolver that cannot supply a parameter is a typed `MissingParameter`
/// error (F5) — never a panic and never a silent default (review P1).
#[test]
fn pwl_missing_parameter_is_typed_error() {
    let pwl = parameterized_pwl();
    let err = pwl
        .evaluate_with(0.5, &|_: ParamId| None)
        .expect_err("a resolver that cannot supply the parameter must fail");
    assert!(
        matches!(err, PwlEvalError::MissingParameter { .. }),
        "missing parameter must be a typed error, got {err:?}"
    );
}
