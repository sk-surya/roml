//! Binary product bridge (design §16.5, §8.1, §8.5; D12, D23; SM-12.6,
//! SM-12.7, SM-13.2/13.4/13.5).
//!
//! Exact product support is limited to binary-binary and binary-times-bounded-
//! linear (D23). The builder rejects continuous-times-continuous requests
//! (SM-12.7); this bridge has no path for them.
//!
//! - **binary-binary** (`w = a·b`): the four exact rows `w <= a`, `w <= b`,
//!   `w >= a + b - 1`, plus the output bounds `0 <= w <= 1` — no generated
//!   binaries, no Big-M.
//! - **binary-times-bounded-linear** (`w = b·f`, `f` with finite interval
//!   `[L, U]`): the four exact product rows `w >= L·b`, `w <= U·b`,
//!   `w >= f - U·(1-b)`, `w <= f - L·(1-b)`; each of the four M values is the
//!   finite derived interval endpoint (SM-13.2) and is recorded as a
//!   bound-evidence report entry with its sources (SM-13.5). An unbounded `f`
//!   is the construct-aware `CompileError::UnboundedBigM` naming the construct
//!   and expression (SM-13.4, D12).

use crate::compiler::bounds::BoundAnalyzer;
use crate::compiler::bridge::{
    function_coefficients, resolve_variable, select_path, BridgeContext, BridgeFinalizer,
    BridgeOutput, ConstructPath,
};
use crate::compiler::capability::BackendFeature;
use crate::compiler::origin::GeneratedRole;
use crate::compiler::report::FormulationDecision;
use crate::compiler::CompileError;
use crate::construct::{BinaryProductConstraint, ProductOperand};
use crate::function::ScalarFunction;
use crate::model::ConstraintBounds;

/// Compile one binary product construct into its exact rows (design §16.5).
pub(crate) fn compile(
    payload: &BinaryProductConstraint,
    ctx: &BridgeContext,
    next_variable_index: u32,
    next_row_index: u32,
) -> Result<BridgeOutput, CompileError> {
    let path = select_path(
        ctx.capabilities,
        ctx.policy,
        BackendFeature::BinaryProduct,
        "binary product construct",
    )?;
    let mut finalizer = BridgeFinalizer::new(ctx.construct, next_variable_index, next_row_index);
    // IN-01: record the selected representation path so a native selection is
    // observable in the formulation decisions (the indicator bridge precedent).
    let (path_selection, path_reason) = match path {
        ConstructPath::Native => (
            "native binary product",
            "qualified native BackendFeature::BinaryProduct selected (Auto)",
        ),
        ConstructPath::Bridge => (
            "exact bridge",
            "no qualified native binary product; exact ROML bridge (design §8.1)",
        ),
    };
    finalizer.add_decision(FormulationDecision {
        decision: "product.path".to_string(),
        selection: path_selection.to_string(),
        reason: path_reason.to_string(),
    });
    let w = resolve_variable(ctx.variable_ids, payload.output, ctx.construct)?;

    match (&payload.left, &payload.right) {
        // ── Binary × binary: w = a·b exactly ────────────────────────────────
        (ProductOperand::Binary(a), ProductOperand::Binary(b)) => {
            let a = resolve_variable(ctx.variable_ids, *a, ctx.construct)?;
            let b = resolve_variable(ctx.variable_ids, *b, ctx.construct)?;
            // w <= a  ⇔  w - a <= 0.
            finalizer.add_row(
                GeneratedRole::BinaryProductRow,
                ConstraintBounds::le(0.0),
                vec![(w, 1.0), (a, -1.0)],
                None,
            );
            // w <= b  ⇔  w - b <= 0.
            finalizer.add_row(
                GeneratedRole::BinaryProductRow,
                ConstraintBounds::le(0.0),
                vec![(w, 1.0), (b, -1.0)],
                None,
            );
            // w >= a + b - 1  ⇔  w - a - b >= -1.
            finalizer.add_row(
                GeneratedRole::BinaryProductRow,
                ConstraintBounds::ge(-1.0),
                vec![(w, 1.0), (a, -1.0), (b, -1.0)],
                None,
            );
            finalizer.add_decision(FormulationDecision {
                decision: "product.representation".to_string(),
                selection: "exact bridge (binary-binary)".to_string(),
                reason: "binary-binary product compiles to the four exact rows w <= a, w <= b, \
                         w >= a + b - 1, 0 <= w <= 1 (design §16.5, SM-12.6)"
                    .to_string(),
            });
        }

        // ── Binary × bounded-linear: w = b·f exactly ────────────────────────
        (ProductOperand::Binary(b), ProductOperand::Linear(expr))
        | (ProductOperand::Linear(expr), ProductOperand::Binary(b)) => {
            let b = resolve_variable(ctx.variable_ids, *b, ctx.construct)?;
            let analyzer = BoundAnalyzer::new();
            let function = ScalarFunction::Linear(expr.clone());
            let trace = analyzer
                .interval_of_snapshot(&function, ctx.snapshot)
                .map_err(|e| CompileError::InvalidBigM {
                    construct: ctx.construct,
                    expression: format!("{:?}", expr),
                    reason: e.to_string(),
                })?;
            let interval = trace.result;
            if !interval.is_bounded() {
                return Err(CompileError::UnboundedBigM {
                    construct: ctx.construct,
                    expression: format!("{:?}", expr),
                });
            }
            let (f_coeffs, f_constant) = function_coefficients(
                &function,
                ctx.construct,
                ctx.variable_ids,
                ctx.parameter_values,
            )?;
            let l = interval.lower;
            let u = interval.upper;

            // w >= L·b  ⇔  w - L·b >= 0.
            finalizer.add_row(
                GeneratedRole::BinaryProductBoundRow,
                ConstraintBounds::ge(0.0),
                vec![(w, 1.0), (b, -l)],
                None,
            );
            // w <= U·b  ⇔  w - U·b <= 0.
            finalizer.add_row(
                GeneratedRole::BinaryProductBoundRow,
                ConstraintBounds::le(0.0),
                vec![(w, 1.0), (b, -u)],
                None,
            );
            // w >= f - U·(1-b)  ⇔  w - f - U·b >= -U.
            let mut lower_coeffs = vec![(w, 1.0), (b, -u)];
            lower_coeffs.extend(f_coeffs.iter().map(|&(vid, c)| (vid, -c)));
            finalizer.add_row(
                GeneratedRole::BinaryProductLinearRow,
                ConstraintBounds::ge(-u + f_constant),
                lower_coeffs,
                None,
            );
            // w <= f - L·(1-b)  ⇔  w - f - L·b <= -L.
            let mut upper_coeffs = vec![(w, 1.0), (b, -l)];
            upper_coeffs.extend(f_coeffs.iter().map(|&(vid, c)| (vid, -c)));
            finalizer.add_row(
                GeneratedRole::BinaryProductLinearRow,
                ConstraintBounds::le(-l + f_constant),
                upper_coeffs,
                None,
            );

            // Record the finite derived M values (the interval endpoints) with
            // their bound sources (SM-13.5).
            finalizer.record_bound_evidence(
                "product.m_lower",
                Some(l),
                &format!("L = {l} (interval lower endpoint for w >= L·b / w <= f - L·(1-b))"),
                &trace.sources,
            );
            finalizer.record_bound_evidence(
                "product.m_upper",
                Some(u),
                &format!("U = {u} (interval upper endpoint for w <= U·b / w >= f - U·(1-b))"),
                &trace.sources,
            );
            finalizer.add_decision(FormulationDecision {
                decision: "product.representation".to_string(),
                selection: "exact bridge (binary-times-bounded-linear)".to_string(),
                reason: "binary-times-bounded-linear product compiles to the four exact rows \
                         w >= L·b, w <= U·b, w >= f - U·(1-b), w <= f - L·(1-b) with finite derived \
                         M = interval endpoints (design §16.5, SM-12.6)"
                    .to_string(),
            });
        }

        // The builder rejects continuous×continuous — no exact MILP path and
        // no relaxation is emitted (SM-12.7, D23).
        (ProductOperand::Linear(_), ProductOperand::Linear(_)) => {
            return Err(CompileError::UnsupportedFeature(
                "continuous-times-continuous product (rejected by the builder; SM-12.7)".into(),
            ));
        }
    }

    Ok(finalizer.finish())
}
