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
use crate::model::ConstraintBounds;

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
            // Task 3 lands the deterministic exact segment-binary bridge. Until
            // then the exact graph is a typed error — never a relaxation.
            return Err(CompileError::UnsupportedFeature(
                "exact PWL graph bridge lands in P33 Task 3".to_string(),
            ));
        }
    }

    Ok(finalizer.finish())
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
