//! Absolute value / positive part / clamp bridge (design §16.3, §8.1, §8.5;
//! D12, D13, D14; SM-12.4, SM-13.2/13.4/13.5).
//!
//! Each variant is a **bounded exact** bridge over the expression's finite
//! `BoundAnalyzer` interval `[L, U]`:
//!
//! - **absolute** (`z = |x|`): the exact decomposition `z = p + n`, `p - n = x`,
//!   `p, n >= 0`, with a selector binary `b`, `p <= M_p·b` and
//!   `n <= M_n·(1-b)` where `M_p = max(U, 0)` and `M_n = max(-L, 0)` are finite
//!   derived bounds;
//! - **positive part** (`z = max(x, 0)`): the same decomposition with `z` wired
//!   to the positive part (`z - n = x`, `z <= M_p·b`, `n <= M_n·(1-b)`);
//! - **clamp** (`z = clamp(x, lo, hi)`): the composed exact formulation — inner
//!   exact max `w = max(x, lo)` (selector on `{x, lo}`), then outer exact min
//!   `z = min(w, hi)` (selector on `{w, hi}`) — reusing the min/max selector
//!   helpers; all M finite derived from `[L, U]` and the constants.
//!
//! The M values are finite derived (never an arbitrary default constant — D12)
//! and recorded as bound-evidence report entries with their sources (SM-13.5).
//! An unbounded expression is the construct-aware `CompileError::UnboundedBigM`
//! naming the construct and expression (SM-13.4). No one-sided relaxation
//! (`z >= x, z >= -x` alone is NOT exact — D13) and no reification /
//! strict-inequality row is ever emitted (D14).

use crate::compiler::bounds::{BoundAnalyzer, BoundSource, Interval};
use crate::compiler::bridge::minmax::{exact_max_selector, exact_min_selector, Operand};
use crate::compiler::bridge::{
    function_coefficients, resolve_variable, select_path, BridgeContext, BridgeFinalizer,
    BridgeOutput, ConstructPath,
};
use crate::compiler::capability::BackendFeature;
use crate::compiler::origin::GeneratedRole;
use crate::compiler::report::FormulationDecision;
use crate::compiler::CompileError;
use crate::construct::{AbsoluteValueConstraint, AbsoluteValueVariant};
use crate::function::ScalarFunction;
use crate::model::{Bounds, ConstraintBounds, VarType};

/// Compile one absolute-value-family construct into its bounded exact bridge
/// (design §16.3).
pub(crate) fn compile(
    payload: &AbsoluteValueConstraint,
    ctx: &BridgeContext,
    next_variable_index: u32,
    next_row_index: u32,
) -> Result<BridgeOutput, CompileError> {
    let path = select_path(
        ctx.capabilities,
        ctx.policy,
        BackendFeature::AbsoluteValue,
        "absolute value construct",
    )?;
    match path {
        ConstructPath::Native => {
            // No P32 backend declares a qualified native absolute value; the
            // exact portable bridge is the P32 representation.
        }
        ConstructPath::Bridge => {}
    }

    let mut finalizer = BridgeFinalizer::new(ctx.construct, next_variable_index, next_row_index);
    let analyzer = BoundAnalyzer::new();
    let function = ScalarFunction::Linear(payload.expression.clone());
    let trace = analyzer
        .interval_of_snapshot(&function, ctx.snapshot)
        .map_err(|e| CompileError::InvalidBigM {
            construct: ctx.construct,
            expression: format!("{:?}", payload.expression),
            reason: e.to_string(),
        })?;
    let interval = trace.result;
    if !interval.is_bounded() {
        return Err(CompileError::UnboundedBigM {
            construct: ctx.construct,
            expression: format!("{:?}", payload.expression),
        });
    }
    let (coefficients, constant) = function_coefficients(
        &function,
        ctx.construct,
        ctx.variable_ids,
        ctx.parameter_values,
    )?;
    let z = resolve_variable(ctx.variable_ids, payload.output, ctx.construct)?;

    let l = interval.lower;
    let u = interval.upper;
    let m_p = u.max(0.0);
    let m_n = (-l).max(0.0);
    finalizer.record_bound_evidence(
        "absolute.m_p",
        Some(m_p),
        &format!("M_p = max(U, 0) = max({u}, 0)"),
        &trace.sources,
    );
    finalizer.record_bound_evidence(
        "absolute.m_n",
        Some(m_n),
        &format!("M_n = max(-L, 0) = max({}, 0)", -l),
        &trace.sources,
    );

    match &payload.variant {
        AbsoluteValueVariant::Absolute => {
            // z = p + n, p - n = x, p,n >= 0, b, p <= M_p·b, n <= M_n·(1-b).
            let p = finalizer.add_variable(
                GeneratedRole::AbsoluteValuePositivePartRow,
                VarType::Continuous,
                Bounds::new(0.0, m_p),
                None,
            );
            let n = finalizer.add_variable(
                GeneratedRole::AbsoluteValueNegativePartRow,
                VarType::Continuous,
                Bounds::new(0.0, m_n),
                None,
            );
            let b = finalizer.add_variable(
                GeneratedRole::AbsoluteValueSelectorBinary,
                VarType::Binary,
                Bounds::BINARY,
                None,
            );
            // z - p - n = 0.
            finalizer.add_row(
                GeneratedRole::AbsoluteValueDecompositionRow,
                ConstraintBounds::eq(0.0),
                vec![(z, 1.0), (p, -1.0), (n, -1.0)],
                None,
            );
            // p - n - x = 0  ⇔  p - n - Σ coeff·var = constant.
            let mut coeffs = vec![(p, 1.0), (n, -1.0)];
            coeffs.extend(coefficients.iter().map(|&(vid, c)| (vid, -c)));
            finalizer.add_row(
                GeneratedRole::AbsoluteValueDecompositionRow,
                ConstraintBounds::eq(constant),
                coeffs,
                None,
            );
            // p - M_p·b <= 0.
            finalizer.add_row(
                GeneratedRole::AbsoluteValuePositivePartRow,
                ConstraintBounds::le(0.0),
                vec![(p, 1.0), (b, -m_p)],
                None,
            );
            // n - M_n·(1-b) <= 0  ⇔  n + M_n·b <= M_n.
            finalizer.add_row(
                GeneratedRole::AbsoluteValueNegativePartRow,
                ConstraintBounds::le(m_n),
                vec![(n, 1.0), (b, m_n)],
                None,
            );
        }
        AbsoluteValueVariant::PositivePart => {
            // z = max(x, 0): z - n = x, z >= 0, n >= 0, b, z <= M_p·b,
            // n <= M_n·(1-b).
            let n = finalizer.add_variable(
                GeneratedRole::AbsoluteValueNegativePartRow,
                VarType::Continuous,
                Bounds::new(0.0, m_n),
                None,
            );
            let b = finalizer.add_variable(
                GeneratedRole::AbsoluteValueSelectorBinary,
                VarType::Binary,
                Bounds::BINARY,
                None,
            );
            // z - n - x = 0  ⇔  z - n - Σ coeff·var = constant.
            let mut coeffs = vec![(z, 1.0), (n, -1.0)];
            coeffs.extend(coefficients.iter().map(|&(vid, c)| (vid, -c)));
            finalizer.add_row(
                GeneratedRole::AbsoluteValueDecompositionRow,
                ConstraintBounds::eq(constant),
                coeffs,
                None,
            );
            // z - M_p·b <= 0.
            finalizer.add_row(
                GeneratedRole::AbsoluteValuePositivePartRow,
                ConstraintBounds::le(0.0),
                vec![(z, 1.0), (b, -m_p)],
                None,
            );
            // n + M_n·b <= M_n.
            finalizer.add_row(
                GeneratedRole::AbsoluteValueNegativePartRow,
                ConstraintBounds::le(m_n),
                vec![(n, 1.0), (b, m_n)],
                None,
            );
        }
        AbsoluteValueVariant::Clamp { lower, upper } => {
            // Composed exact: inner max w = max(x, lo), outer min z = min(w, hi).
            let lo = *lower;
            let hi = *upper;

            // Inner max operands: {x, lo}.
            let x_operand = Operand {
                coefficients: coefficients.clone(),
                constant,
                interval,
                sources: trace.sources.clone(),
            };
            let lo_operand = Operand {
                coefficients: Vec::new(),
                constant: lo,
                interval: Interval::exact(lo),
                sources: vec![BoundSource::Constant],
            };
            let inner_operands = [x_operand, lo_operand];
            let w_lower = interval.lower.max(lo);
            let w_upper = interval.upper.max(lo);
            let w = finalizer.add_variable(
                GeneratedRole::ClampInnerSelectorRow,
                VarType::Continuous,
                Bounds::new(w_lower, w_upper),
                None,
            );
            exact_max_selector(
                &mut finalizer,
                w,
                &inner_operands,
                w_upper,
                GeneratedRole::ClampInnerSelectorBinary,
                GeneratedRole::ClampInnerSelectorRow,
                "clamp.inner_m",
            )?;

            // Outer min operands: {w, hi}.
            let w_operand = Operand {
                coefficients: vec![(w, 1.0)],
                constant: 0.0,
                interval: Interval {
                    lower: w_lower,
                    upper: w_upper,
                },
                sources: vec![],
            };
            let hi_operand = Operand {
                coefficients: Vec::new(),
                constant: hi,
                interval: Interval::exact(hi),
                sources: vec![BoundSource::Constant],
            };
            let outer_operands = [w_operand, hi_operand];
            let l_min_outer = w_lower.min(hi);
            exact_min_selector(
                &mut finalizer,
                z,
                &outer_operands,
                l_min_outer,
                GeneratedRole::ClampOuterSelectorBinary,
                GeneratedRole::ClampOuterSelectorRow,
                "clamp.outer_m",
            )?;
        }
    }

    let selection = match &payload.variant {
        AbsoluteValueVariant::Absolute => "exact bridge (abs decomposition)",
        AbsoluteValueVariant::PositivePart => "exact bridge (positive-part decomposition)",
        AbsoluteValueVariant::Clamp { .. } => "exact bridge (composed clamp selectors)",
    };
    finalizer.add_decision(FormulationDecision {
        decision: "absolute.representation".to_string(),
        selection: selection.to_string(),
        reason: "bounded exact bridge with finite derived M from the expression interval \
                 (design §16.3, SM-12.4; no one-sided relaxation, no reification row — D13/D14)"
            .to_string(),
    });

    Ok(finalizer.finish())
}
