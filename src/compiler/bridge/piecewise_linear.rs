//! Piecewise-linear bridge (design §17, §8.1, §8.5; D13, D24; SM-14.3,
//! SM-14.4, SM-14.5, SM-14.6, SM-13.2, SM-13.5).
//!
//! The PWL relation determines the formulation (D24):
//!
//! - **convex epigraph** (`relation = Epigraph`, curvature Convex/Affine):
//!   zero-binary supporting-inequality rows
//!   `output >= v_i + s_i * (argument - x_i)` for every breakpoint `i`
//!   (SM-14.3).
//! - **concave hypograph** (`relation = Hypograph`, curvature Concave/Affine):
//!   zero-binary mirror rows `output <= v_i + s_i * (argument - x_i)`
//!   (SM-14.3).
//! - **exact graph** (`relation = ExactGraph`): the deterministic exact
//!   segment-binary convex-combination formulation (P33 Task 3) — never a
//!   convex relaxation (SM-14.4/SM-14.5).
//!
//! A relation/curvature mismatch (`Epigraph` on a non-convex PWL, `Hypograph`
//! on a non-concave PWL) is a typed [`CompileError`] — never a silent
//! relaxation (D13). No Big-M is introduced anywhere (SM-13.2, D12): the
//! supporting rows and the exact segment-binary formulation need no M, and the
//! report records the argument interval from [`BoundAnalyzer`] plus the
//! breakpoint/value ranges as bound evidence (SM-13.5).

use crate::compiler::backend_ir::CompiledVariableId;
use crate::compiler::bounds::BoundAnalyzer;
use crate::compiler::bridge::{
    combine_coefficients, function_coefficients, resolve_variable, select_path, BridgeContext,
    BridgeFinalizer, BridgeOutput,
};
use crate::compiler::capability::BackendFeature;
use crate::compiler::origin::GeneratedRole;
use crate::compiler::report::FormulationDecision;
use crate::compiler::CompileError;
use crate::construct::{
    classify_curvature_from_slopes, PiecewiseLinearConstraint, PwlCurvature, PwlPoint, PwlRelation,
};
use crate::function::ScalarFunction;
use crate::model::{Bounds, ConstraintBounds, VarType};

/// Compile one PWL construct into its rows (design §17).
pub(crate) fn compile(
    payload: &PiecewiseLinearConstraint,
    ctx: &BridgeContext,
    next_variable_index: u32,
    next_row_index: u32,
) -> Result<BridgeOutput, CompileError> {
    // SM-04.4 / F4 gating: an unqualified feature and a bridge-only feature
    // under `NativeRequired` are typed errors. Under `Auto` a native declaration
    // falls back to the exact bridge (no native PWL/SOS2 payload exists in M3),
    // so the recorded path is always the honest exact bridge.
    select_path(
        ctx.capabilities,
        ctx.policy,
        BackendFeature::PiecewiseLinear,
        "pwl construct",
    )?;
    let mut finalizer = BridgeFinalizer::new(ctx.construct, next_variable_index, next_row_index);
    // IN-01: record the selected representation path.
    finalizer.add_decision(FormulationDecision {
        decision: "pwl.path".to_string(),
        selection: "exact bridge".to_string(),
        reason: "no qualified native PWL/SOS2; exact ROML bridge (design §8.1; F4)".to_string(),
    });

    // Deterministic curvature classification from the EVALUATED point values
    // (parameter-dependent values resolve against the snapshot's parameter
    // map; shared logic with the payload's constant classification — SM-14.2).
    let curvature = classify_evaluated_curvature(payload, ctx)?;
    finalizer.add_decision(FormulationDecision {
        decision: "pwl.curvature".to_string(),
        selection: format!("{curvature:?}"),
        reason: "deterministic classification from segment slopes (SM-14.2)".to_string(),
    });
    finalizer.add_decision(FormulationDecision {
        decision: "pwl.relation".to_string(),
        selection: format!("{:?}", payload.relation),
        reason: "the declared PWL relation determines the formulation (D24)".to_string(),
    });

    // Argument interval + bound sources (SM-13.1/SM-13.5).
    let trace = BoundAnalyzer::new()
        .interval_of_snapshot(
            &ScalarFunction::Linear(payload.argument.clone()),
            ctx.snapshot,
        )
        .map_err(|e| CompileError::InvalidBigM {
            construct: ctx.construct,
            expression: format!("pwl argument: {:?}", payload.argument),
            reason: e.to_string(),
        })?;
    finalizer.add_decision(FormulationDecision {
        decision: "pwl.argument_interval".to_string(),
        selection: format!("[{}, {}]", trace.result.lower, trace.result.upper),
        reason: format!(
            "derivation: BoundAnalyzer over the PWL argument expression; bound sources: [{}]",
            trace
                .sources
                .iter()
                .map(|s| format!("{s:?}"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    });
    let x_min = payload.points.first().map(|p| p.x).unwrap_or(f64::NAN);
    let x_max = payload.points.last().map(|p| p.x).unwrap_or(f64::NAN);
    finalizer.add_decision(FormulationDecision {
        decision: "pwl.breakpoint_range".to_string(),
        selection: format!("x in [{x_min}, {x_max}]"),
        reason: "the finite breakpoint range of the PWL graph (SM-13.5)".to_string(),
    });

    match payload.relation {
        PwlRelation::Epigraph => {
            if !matches!(curvature, PwlCurvature::Convex | PwlCurvature::Affine) {
                return Err(CompileError::UnsupportedFeature(format!(
                    "PWL relation/curvature mismatch: Epigraph on {curvature:?} (D13, SM-14.3) — \
                     never a silent relaxation"
                )));
            }
            emit_supporting_rows(
                &mut finalizer,
                payload,
                ctx,
                GeneratedRole::PwlEpigraphRow,
                true,
            )?;
            finalizer.add_decision(FormulationDecision {
                decision: "pwl.representation".to_string(),
                selection: "supporting inequalities (convex epigraph, zero binaries)".to_string(),
                reason: format!(
                    "epigraph on {curvature:?} compiles to supporting rows \
                     output >= v_i + s_i*(argument - x_i) with zero generated binaries (SM-14.3)"
                ),
            });
            record_zero_binary_decisions(&mut finalizer, "convex epigraph");
        }
        PwlRelation::Hypograph => {
            if !matches!(curvature, PwlCurvature::Concave | PwlCurvature::Affine) {
                return Err(CompileError::UnsupportedFeature(format!(
                    "PWL relation/curvature mismatch: Hypograph on {curvature:?} (D13, SM-14.3) — \
                     never a silent relaxation"
                )));
            }
            emit_supporting_rows(
                &mut finalizer,
                payload,
                ctx,
                GeneratedRole::PwlHypographRow,
                false,
            )?;
            finalizer.add_decision(FormulationDecision {
                decision: "pwl.representation".to_string(),
                selection: "supporting inequalities (concave hypograph, zero binaries)".to_string(),
                reason: format!(
                    "hypograph on {curvature:?} compiles to supporting rows \
                     output <= v_i + s_i*(argument - x_i) with zero generated binaries (SM-14.3)"
                ),
            });
            record_zero_binary_decisions(&mut finalizer, "concave hypograph");
        }
        PwlRelation::ExactGraph => {
            emit_exact_graph(&mut finalizer, payload, ctx)?;
            let segment_count = payload.points.len() - 1;
            finalizer.add_decision(FormulationDecision {
                decision: "pwl.representation".to_string(),
                selection: "exact segment binaries".to_string(),
                reason: format!(
                    "the exact graph of a {curvature:?} PWL compiles to the deterministic exact \
                     segment-binary convex-combination formulation (adjacency binaries, SM-14.4); \
                     SOS2/native PWL are never selected because native_payloads_available() is \
                     false and HiGHS declares no native SOS2/PWL (P32 F4)"
                ),
            });
            finalizer.add_decision(FormulationDecision {
                decision: "pwl.generated_binaries".to_string(),
                selection: format!("{segment_count}"),
                reason: format!(
                    "one adjacency binary per segment ({segment_count} segments) — SM-14.4"
                ),
            });
            finalizer.add_decision(FormulationDecision {
                decision: "pwl.generated_auxiliary_variables".to_string(),
                selection: format!("{}", payload.points.len()),
                reason: "one convex-combination weight per point (SM-14.4)".to_string(),
            });
            finalizer.add_decision(FormulationDecision {
                decision: "pwl.binary_introduction_reason".to_string(),
                selection: "exactness of the (possibly nonconvex) graph".to_string(),
                reason: "the exact graph requires adjacency binaries to enforce the \
                         at-most-two-adjacent-weights condition; no convex relaxation is emitted \
                         (SM-14.4/SM-14.5)"
                    .to_string(),
            });
            record_scaling_diagnostic(&mut finalizer, payload, ctx)?;
        }
    }

    Ok(finalizer.finish())
}

/// Record the numerical scaling diagnostic (ROADMAP P33): the value span over
/// the breakpoint span (average segment-slope magnitude).
fn record_scaling_diagnostic(
    finalizer: &mut BridgeFinalizer,
    payload: &PiecewiseLinearConstraint,
    ctx: &BridgeContext,
) -> Result<(), CompileError> {
    let x_span = payload.points.last().map(|p| p.x).unwrap_or(0.0)
        - payload.points.first().map(|p| p.x).unwrap_or(0.0);
    let mut values = Vec::with_capacity(payload.points.len());
    for i in 0..payload.points.len() {
        values.push(eval_point_value(&payload.points, i, ctx)?);
    }
    let v_min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let v_max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let v_span = v_max - v_min;
    let avg_slope = if x_span > 0.0 { v_span / x_span } else { 0.0 };
    finalizer.add_decision(FormulationDecision {
        decision: "pwl.scaling".to_string(),
        selection: format!("value_span {v_span:.6} over x_span {x_span:.6}"),
        reason: format!(
            "numerical scaling diagnostic: average segment-slope magnitude {avg_slope:.6} \
             (value span {v_span:.6} / breakpoint span {x_span:.6})"
        ),
    });
    Ok(())
}

/// Emit the deterministic exact segment-binary convex-combination formulation
/// (design §17, SM-14.4/SM-14.5). With points `(x_0, v_0)..(x_m, v_m)`:
///
/// - weights `lambda_i >= 0` (`i in 0..m`) with `sum lambda = 1`;
/// - `argument = sum_i x_i * lambda_i`;
/// - `output = sum_i v_i * lambda_i`;
/// - adjacency binaries `z_k` (`k in 0..m-1`) with `sum z = 1`;
/// - `lambda_0 <= z_0`, `lambda_i <= z_{i-1} + z_i` for interior `i`,
///   `lambda_m <= z_{m-1}`.
///
/// No Big-M is introduced anywhere (SM-13.2, D12). Entities are emitted in
/// deterministic order: weight variables, segment binaries, then rows.
fn emit_exact_graph(
    finalizer: &mut BridgeFinalizer,
    payload: &PiecewiseLinearConstraint,
    ctx: &BridgeContext,
) -> Result<(), CompileError> {
    let y = resolve_variable(ctx.variable_ids, payload.output, ctx.construct)?;
    let (arg_coefficients, arg_constant) = function_coefficients(
        &ScalarFunction::Linear(payload.argument.clone()),
        ctx.construct,
        ctx.variable_ids,
        ctx.parameter_values,
    )?;
    let m = payload.points.len();
    debug_assert!(m >= 2, "builder validates at least two points");

    // Weight variables lambda_0..lambda_{m-1} (continuous, nonnegative).
    let lambdas: Vec<CompiledVariableId> = (0..m)
        .map(|_| {
            finalizer.add_variable(
                GeneratedRole::PwlWeightVariable,
                VarType::Continuous,
                Bounds::new(0.0, f64::INFINITY),
                None,
            )
        })
        .collect();
    // Segment binaries z_0..z_{m-2} (one per segment).
    let zs: Vec<CompiledVariableId> = (0..m - 1)
        .map(|_| {
            finalizer.add_variable(
                GeneratedRole::PwlSegmentBinary,
                VarType::Binary,
                Bounds::BINARY,
                None,
            )
        })
        .collect();

    // sum_i lambda_i = 1.
    finalizer.add_row(
        GeneratedRole::PwlExactGraphRow,
        ConstraintBounds::eq(1.0),
        lambdas.iter().map(|&l| (l, 1.0)).collect(),
        None,
    );
    // argument = sum_i x_i * lambda_i  ⇔  argument - sum x_i lambda_i = 0.
    let mut arg_row: Vec<(CompiledVariableId, f64)> = arg_coefficients.clone();
    for (i, &l) in lambdas.iter().enumerate() {
        arg_row.push((l, -payload.points[i].x));
    }
    finalizer.add_row(
        GeneratedRole::PwlExactGraphRow,
        ConstraintBounds::eq(-arg_constant),
        arg_row,
        None,
    );
    // output = sum_i v_i * lambda_i  ⇔  output - sum v_i lambda_i = 0.
    let mut out_row: Vec<(CompiledVariableId, f64)> = vec![(y, 1.0)];
    for (i, &l) in lambdas.iter().enumerate() {
        let v_i = eval_point_value(&payload.points, i, ctx)?;
        out_row.push((l, -v_i));
    }
    finalizer.add_row(
        GeneratedRole::PwlExactGraphRow,
        ConstraintBounds::eq(0.0),
        out_row,
        None,
    );
    // sum_k z_k = 1.
    finalizer.add_row(
        GeneratedRole::PwlExactGraphRow,
        ConstraintBounds::eq(1.0),
        zs.iter().map(|&z| (z, 1.0)).collect(),
        None,
    );
    // lambda_0 <= z_0.
    finalizer.add_row(
        GeneratedRole::PwlExactGraphRow,
        ConstraintBounds::le(0.0),
        vec![(lambdas[0], 1.0), (zs[0], -1.0)],
        None,
    );
    // lambda_i <= z_{i-1} + z_i for interior i in 1..m-1.
    for i in 1..m - 1 {
        finalizer.add_row(
            GeneratedRole::PwlExactGraphRow,
            ConstraintBounds::le(0.0),
            vec![(lambdas[i], 1.0), (zs[i - 1], -1.0), (zs[i], -1.0)],
            None,
        );
    }
    // lambda_{m-1} <= z_{m-2}.
    finalizer.add_row(
        GeneratedRole::PwlExactGraphRow,
        ConstraintBounds::le(0.0),
        vec![(lambdas[m - 1], 1.0), (zs[m - 2], -1.0)],
        None,
    );
    Ok(())
}

/// Record the shared zero-binary representation/count/reason report entries
/// (SM-14.6).
fn record_zero_binary_decisions(finalizer: &mut BridgeFinalizer, selection: &str) {
    finalizer.add_decision(FormulationDecision {
        decision: "pwl.generated_binaries".to_string(),
        selection: "0".to_string(),
        reason: "one-sided supporting-inequality rows need no binaries (SM-14.3)".to_string(),
    });
    finalizer.add_decision(FormulationDecision {
        decision: "pwl.binary_avoidance_reason".to_string(),
        selection: selection.to_string(),
        reason: "convex epigraph / concave hypograph compile to supporting-inequality rows with \
                 zero generated binaries (design §17, SM-14.3)"
            .to_string(),
    });
}

/// The evaluated numeric value at breakpoint `i` (F5: a missing parameter is a
/// typed error, never a silent default of zero).
fn eval_point_value(
    points: &[PwlPoint],
    i: usize,
    ctx: &BridgeContext,
) -> Result<f64, CompileError> {
    points[i]
        .value
        .eval_checked(|p| ctx.parameter_values.get(&p).copied().ok_or(p))
        .map_err(|parameter| CompileError::MissingConstructParameter {
            construct: ctx.construct,
            parameter,
        })
}

/// The segment slope at breakpoint `i`: the right-adjacent segment slope for
/// `i < n-1`, and the last segment slope for the final breakpoint.
fn segment_slope_at(
    points: &[PwlPoint],
    i: usize,
    ctx: &BridgeContext,
) -> Result<f64, CompileError> {
    let n = points.len();
    let j = if i < n - 1 { i } else { n - 2 };
    let v_j = eval_point_value(points, j, ctx)?;
    let v_j1 = eval_point_value(points, j + 1, ctx)?;
    Ok((v_j1 - v_j) / (points[j + 1].x - points[j].x))
}

/// Classify the PWL curvature from the EVALUATED segment slopes (SM-14.2),
/// sharing the payload's deterministic classification logic.
fn classify_evaluated_curvature(
    payload: &PiecewiseLinearConstraint,
    ctx: &BridgeContext,
) -> Result<PwlCurvature, CompileError> {
    let mut slopes = Vec::with_capacity(payload.points.len().saturating_sub(1));
    for i in 0..payload.points.len().saturating_sub(1) {
        let v_i = eval_point_value(&payload.points, i, ctx)?;
        let v_i1 = eval_point_value(&payload.points, i + 1, ctx)?;
        slopes.push((v_i1 - v_i) / (payload.points[i + 1].x - payload.points[i].x));
    }
    Ok(classify_curvature_from_slopes(&slopes))
}

/// Emit the one-sided supporting-inequality rows
/// `output >= v_i + s_i * (argument - x_i)` (epigraph) or the mirror
/// `output <= v_i + s_i * (argument - x_i)` (hypograph) for every breakpoint
/// `i`, with zero generated binaries (SM-14.3).
fn emit_supporting_rows(
    finalizer: &mut BridgeFinalizer,
    payload: &PiecewiseLinearConstraint,
    ctx: &BridgeContext,
    role: GeneratedRole,
    epigraph: bool,
) -> Result<(), CompileError> {
    let y = resolve_variable(ctx.variable_ids, payload.output, ctx.construct)?;
    let (arg_coefficients, arg_constant) = function_coefficients(
        &ScalarFunction::Linear(payload.argument.clone()),
        ctx.construct,
        ctx.variable_ids,
        ctx.parameter_values,
    )?;
    let n = payload.points.len();
    for i in 0..n {
        let v_i = eval_point_value(&payload.points, i, ctx)?;
        let x_i = payload.points[i].x;
        let s_i = segment_slope_at(&payload.points, i, ctx)?;
        // Row: output >= v_i + s_i*(argument - x_i)
        //   ⇔ output - s_i*argument >= v_i - s_i*x_i + s_i*arg_constant.
        let negated: Vec<(CompiledVariableId, f64)> = arg_coefficients
            .iter()
            .map(|(id, c)| (*id, -s_i * c))
            .collect();
        let coeffs = combine_coefficients(negated, (y, 1.0));
        let rhs = v_i - s_i * x_i + s_i * arg_constant;
        if epigraph {
            finalizer.add_row(role, ConstraintBounds::ge(rhs), coeffs, None);
        } else {
            finalizer.add_row(role, ConstraintBounds::le(rhs), coeffs, None);
        }
    }
    Ok(())
}
