//! Reification bridge (design §16.2, §8.1; D14; SM-12.2).
//!
//! `b = 1 ⟺ relation` compiles to exactly **two implications**: the forward
//! implication (`b = 1 ⇒ relation`) and the complement (`b = 0 ⇒ ¬relation`,
//! separated by the tolerance). The unit gap is used only when the expression
//! is proven integer-valued (D14); an explicit separation tolerance is honored
//! otherwise. Both rows use Big-Ms derived from the construct's declared bounds
//! (never a default constant — D12); a missing finite M is the construct-aware
//! `CompileError::UnboundedBigM` (SM-13.4).

use crate::compiler::bounds::{BigMImplication, BoundAnalyzer};
use crate::compiler::bridge::{
    combine_coefficients, eval_bound, function_coefficients, resolve_variable, select_path,
    BridgeContext, BridgeFinalizer, BridgeOutput,
};
use crate::compiler::capability::BackendFeature;
use crate::compiler::origin::GeneratedRole;
use crate::compiler::report::FormulationDecision;
use crate::compiler::CompileError;
use crate::construct::ReificationConstraint;
use crate::function::ScalarSet;
use crate::model::ConstraintBounds;

/// Compile one reification construct into its two implication rows
/// (design §16.2).
pub(crate) fn compile(
    payload: &ReificationConstraint,
    ctx: &BridgeContext,
    next_variable_index: u32,
    next_row_index: u32,
) -> Result<BridgeOutput, CompileError> {
    // F4: reification is centralized under the same policy gating as every
    // other construct family — an unqualified `Reification` feature and a
    // bridge-only path under `NativeRequired` are typed errors (never silently
    // compiled); under `Auto`/`Portable` with bridge support the exact
    // two-implication bridge is selected.
    select_path(
        ctx.capabilities,
        ctx.policy,
        BackendFeature::Reification,
        "reification construct",
    )?;

    // The build-time validation guarantees a threshold set and a valid
    // separation contract; the unit gap is `1.0` when the expression is proven
    // integral and no explicit tolerance was given (D14).
    let separation = payload.separation_tolerance.unwrap_or(1.0);

    // F3: the inferred unit gap `f > rhs ⟺ f >= rhs + 1` is exact only when the
    // threshold is integral. The builder validates once at build time, but a
    // parameter-dependent threshold can change to fractional before a LATER
    // compilation (F1 forces the check after parameter changes) — revalidate
    // threshold integrality at EVERY compilation and return a typed error
    // BEFORE any backend mutation.
    if payload.separation_tolerance.is_none() && payload.proven_integrality {
        let integral = |v: f64| v.is_finite() && (v - v.round()).abs() < 1e-9;
        match &payload.set {
            ScalarSet::LessEqual(upper) => {
                let rhs = eval_bound(upper, ctx.construct, ctx.parameter_values)?;
                if !integral(rhs) {
                    return Err(CompileError::NonIntegralReificationThreshold {
                        construct: ctx.construct,
                        threshold: rhs,
                    });
                }
            }
            ScalarSet::GreaterEqual(lower) => {
                let rhs = eval_bound(lower, ctx.construct, ctx.parameter_values)?;
                if !integral(rhs) {
                    return Err(CompileError::NonIntegralReificationThreshold {
                        construct: ctx.construct,
                        threshold: rhs,
                    });
                }
            }
            ScalarSet::EqualTo(_) | ScalarSet::Interval { .. } => {
                return Err(CompileError::UnsupportedFeature(
                    "reification of equality/interval relations (deferred beyond P32)".into(),
                ));
            }
        }
    }

    let mut finalizer = BridgeFinalizer::new(ctx.construct, next_variable_index, next_row_index);
    let analyzer = BoundAnalyzer::new();
    let b = resolve_variable(ctx.variable_ids, payload.activator, ctx.construct)?;
    let (function_coeffs, constant) = function_coefficients(
        &payload.function,
        ctx.construct,
        ctx.variable_ids,
        ctx.parameter_values,
    )?;

    match &payload.set {
        ScalarSet::LessEqual(upper) => {
            let rhs = eval_bound(upper, ctx.construct, ctx.parameter_values)?;

            // Forward: b=1 ⇒ f <= rhs  →  f + M1·b <= rhs + M1.
            let expression = format!("{:?} <= {rhs}", payload.function);
            let (m1, sources1) = crate::compiler::bounds::bound_big_m_implied_snapshot(
                &analyzer,
                ctx.construct,
                &expression,
                &payload.function,
                BigMImplication::Upper,
                rhs,
                ctx.snapshot,
            )?;
            finalizer.record_bound_evidence(
                "reification.implication",
                Some(m1),
                &expression,
                &sources1,
            );
            let coeffs = combine_coefficients(function_coeffs.clone(), (b, m1));
            finalizer.add_row(
                GeneratedRole::ReificationImplicationRow,
                ConstraintBounds::le(rhs + m1 - constant),
                coeffs,
                None,
            );

            // Complement: b=0 ⇒ f >= rhs + sep  →  f + M2·b >= rhs + sep.
            let rhs_c = rhs + separation;
            let expression = format!("{:?} >= {rhs_c}", payload.function);
            let (m2, sources2) = crate::compiler::bounds::bound_big_m_implied_snapshot(
                &analyzer,
                ctx.construct,
                &expression,
                &payload.function,
                BigMImplication::Lower,
                rhs_c,
                ctx.snapshot,
            )?;
            finalizer.record_bound_evidence(
                "reification.complement",
                Some(m2),
                &expression,
                &sources2,
            );
            let coeffs = combine_coefficients(function_coeffs.clone(), (b, m2));
            finalizer.add_row(
                GeneratedRole::ReificationComplement,
                ConstraintBounds::ge(rhs_c - constant),
                coeffs,
                None,
            );
        }
        ScalarSet::GreaterEqual(lower) => {
            let rhs = eval_bound(lower, ctx.construct, ctx.parameter_values)?;

            // Forward: b=1 ⇒ f >= rhs  →  f - M1·b >= rhs - M1.
            let expression = format!("{:?} >= {rhs}", payload.function);
            let (m1, sources1) = crate::compiler::bounds::bound_big_m_implied_snapshot(
                &analyzer,
                ctx.construct,
                &expression,
                &payload.function,
                BigMImplication::Lower,
                rhs,
                ctx.snapshot,
            )?;
            finalizer.record_bound_evidence(
                "reification.implication",
                Some(m1),
                &expression,
                &sources1,
            );
            let coeffs = combine_coefficients(function_coeffs.clone(), (b, -m1));
            finalizer.add_row(
                GeneratedRole::ReificationImplicationRow,
                ConstraintBounds::ge(rhs - m1 - constant),
                coeffs,
                None,
            );

            // Complement: b=0 ⇒ f <= rhs - sep  →  f - M2·b <= rhs - sep.
            let rhs_c = rhs - separation;
            let expression = format!("{:?} <= {rhs_c}", payload.function);
            let (m2, sources2) = crate::compiler::bounds::bound_big_m_implied_snapshot(
                &analyzer,
                ctx.construct,
                &expression,
                &payload.function,
                BigMImplication::Upper,
                rhs_c,
                ctx.snapshot,
            )?;
            finalizer.record_bound_evidence(
                "reification.complement",
                Some(m2),
                &expression,
                &sources2,
            );
            let coeffs = combine_coefficients(function_coeffs.clone(), (b, -m2));
            finalizer.add_row(
                GeneratedRole::ReificationComplement,
                ConstraintBounds::le(rhs_c - constant),
                coeffs,
                None,
            );
        }
        // Build-time validated to threshold sets only — unreachable.
        ScalarSet::EqualTo(_) | ScalarSet::Interval { .. } => {
            return Err(CompileError::UnsupportedFeature(
                "reification of equality/interval relations (deferred beyond P32)".into(),
            ));
        }
    }

    finalizer.add_decision(FormulationDecision {
        decision: "reification.representation".to_string(),
        selection: "exact bridge (two implications)".to_string(),
        reason: if payload.proven_integrality {
            "unit gap inferred from proven integrality (D14)".to_string()
        } else {
            format!("explicit separation tolerance {separation} honored (D14)")
        },
    });

    Ok(finalizer.finish())
}
