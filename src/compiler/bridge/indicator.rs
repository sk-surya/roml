//! Indicator bridge (design §16.1, §8.1; SM-12.1, SM-13.2/13.4).
//!
//! A one-way implication over a binary activator compiles to exact rows. Until
//! the backend IR carries a real native payload, `select_path` selects ONLY the
//! exact finite-bound Big-M bridge (F4): a backend's native `Indicator`
//! declaration is NOT reported as a native selection — the emitted row always
//! carries the `IndicatorImplicationRow` role and an honest "exact bridge"
//! formulation decision, and `NativeRequired` rejects the bridge-only path as a
//! typed error. `Portable` forces the bridge.
//!
//! The Big-M is always derived from the construct's declared bounds via
//! [`BoundAnalyzer`](crate::compiler::bounds::BoundAnalyzer) — never a default
//! constant (D12). A missing finite M is the construct-aware
//! `CompileError::UnboundedBigM` (SM-13.4).

use std::collections::HashMap;

use crate::compiler::bounds::{BigMImplication, BoundAnalyzer};
use crate::compiler::bridge::{
    combine_coefficients, function_coefficients, resolve_variable, select_path, BridgeContext,
    BridgeFinalizer, BridgeOutput,
};
use crate::compiler::capability::BackendFeature;
use crate::compiler::origin::GeneratedRole;
use crate::compiler::report::FormulationDecision;
use crate::compiler::CompileError;
use crate::construct::{IndicatorConstraint, IndicatorDirection};
use crate::function::ScalarSet;
use crate::id::ParamId;
use crate::model::ConstraintBounds;
use crate::value_expr::ValueExpr;

/// One one-sided implication `function op rhs` the indicator decomposes into.
#[derive(Clone, Copy, Debug)]
struct OneSided {
    /// The Big-M implication direction (Upper for `<=`, Lower for `>=`).
    side: BigMImplication,
    /// The evaluated rhs of the one-sided relation.
    rhs: f64,
    /// Human-readable operator for evidence/error naming.
    op: &'static str,
}

/// Decompose a set into its one-sided implications (le/ge; eq and interval
/// become two one-sided implications each). `rhs` values are evaluated against
/// the current parameter values, surfacing a missing parameter as a typed
/// error (F5) — never a silent default of zero.
fn one_sided_implications(
    set: &ScalarSet,
    construct: crate::construct::Construct,
    parameter_values: &HashMap<ParamId, f64>,
) -> Result<Vec<OneSided>, CompileError> {
    let eval = |e: &ValueExpr| -> Result<f64, CompileError> {
        e.eval_checked(|p| parameter_values.get(&p).copied().ok_or(p))
            .map_err(|parameter| CompileError::MissingConstructParameter {
                construct,
                parameter,
            })
    };
    Ok(match set {
        ScalarSet::LessEqual(upper) => vec![OneSided {
            side: BigMImplication::Upper,
            rhs: eval(upper)?,
            op: "<=",
        }],
        ScalarSet::GreaterEqual(lower) => vec![OneSided {
            side: BigMImplication::Lower,
            rhs: eval(lower)?,
            op: ">=",
        }],
        ScalarSet::EqualTo(value) => {
            let rhs = eval(value)?;
            vec![
                OneSided {
                    side: BigMImplication::Upper,
                    rhs,
                    op: "<=",
                },
                OneSided {
                    side: BigMImplication::Lower,
                    rhs,
                    op: ">=",
                },
            ]
        }
        ScalarSet::Interval { lower, upper } => vec![
            OneSided {
                side: BigMImplication::Lower,
                rhs: eval(lower)?,
                op: ">=",
            },
            OneSided {
                side: BigMImplication::Upper,
                rhs: eval(upper)?,
                op: "<=",
            },
        ],
    })
}

/// Compile one indicator construct into its exact rows (design §16.1).
pub(crate) fn compile(
    payload: &IndicatorConstraint,
    ctx: &BridgeContext,
    next_variable_index: u32,
    next_row_index: u32,
) -> Result<BridgeOutput, CompileError> {
    // SM-04.4 / F4 gating: an unqualified feature and a bridge-only feature
    // under `NativeRequired` are typed errors. Under `Auto` a native declaration
    // falls back to the exact bridge (the backend IR has no native payload in
    // P32), so the emitted rows are ALWAYS the exact bridge rows — never a
    // "native indicator" label for a bridge formulation.
    select_path(
        ctx.capabilities,
        ctx.policy,
        BackendFeature::Indicator,
        "indicator construct",
    )?;
    // The exact bridge row role is used regardless of the native declaration
    // (F4: honest FormulationDecision — no native label for a bridge row).
    let role = GeneratedRole::IndicatorImplicationRow;
    let (selection, decision_reason) =
        if *ctx.policy == crate::compiler::capability::CompilationPolicy::Portable {
            (
                "exact bridge (portable forced)",
                "Portable policy forces the deterministic ROML bridge (design §8.1)",
            )
        } else {
            (
                "exact bridge (finite bound)",
                "no qualified native indicator; exact finite-bound one-way Big-M bridge \
                 (design §8.1)",
            )
        };

    let mut finalizer = BridgeFinalizer::new(ctx.construct, next_variable_index, next_row_index);
    let analyzer = BoundAnalyzer::new();
    let z = resolve_variable(ctx.variable_ids, payload.activator, ctx.construct)?;
    let (function_coeffs, constant) = function_coefficients(
        &payload.function,
        ctx.construct,
        ctx.variable_ids,
        ctx.parameter_values,
    )?;

    for one_sided in one_sided_implications(&payload.set, ctx.construct, ctx.parameter_values)? {
        let expression = format!("{:?} {} {}", payload.function, one_sided.op, one_sided.rhs);
        let (m, sources) = crate::compiler::bounds::bound_big_m_implied_snapshot(
            &analyzer,
            ctx.construct,
            &expression,
            &payload.function,
            one_sided.side,
            one_sided.rhs,
            ctx.snapshot,
        )?;
        finalizer.record_bound_evidence(
            "indicator.big_m",
            Some(m),
            &format!("derived M for the one-sided implication {expression}"),
            &sources,
        );

        let (bounds, m_sign) = indicator_bounds(payload.direction, one_sided, m, constant);
        let coefficients = combine_coefficients(function_coeffs.clone(), (z, m_sign * m));
        finalizer.add_row(role, bounds, coefficients, None);
        finalizer.add_dependency(crate::compiler::bridge::BridgeDependency::Construct(
            ctx.construct,
        ));
        finalizer.add_dependency(crate::compiler::bridge::BridgeDependency::Variable(
            payload.activator,
        ));
    }

    finalizer.add_decision(FormulationDecision {
        decision: "indicator.representation".to_string(),
        selection: selection.to_string(),
        reason: decision_reason.to_string(),
    });

    Ok(finalizer.finish())
}

/// The compiled row bounds and the M sign on the activator for one one-sided
/// implication, given the function constant `c` (moved into the bounds).
fn indicator_bounds(
    direction: IndicatorDirection,
    one_sided: OneSided,
    m: f64,
    constant: f64,
) -> (ConstraintBounds, f64) {
    match (direction, one_sided.side) {
        // WhenOne: f + M z <= rhs + M  |  f - M z >= rhs - M
        (IndicatorDirection::WhenOne, BigMImplication::Upper) => {
            (ConstraintBounds::le(one_sided.rhs + m - constant), 1.0)
        }
        (IndicatorDirection::WhenOne, BigMImplication::Lower) => {
            (ConstraintBounds::ge(one_sided.rhs - m - constant), -1.0)
        }
        // WhenZero: f - M z <= rhs  |  f + M z >= rhs
        (IndicatorDirection::WhenZero, BigMImplication::Upper) => {
            (ConstraintBounds::le(one_sided.rhs - constant), -1.0)
        }
        (IndicatorDirection::WhenZero, BigMImplication::Lower) => {
            (ConstraintBounds::ge(one_sided.rhs - constant), 1.0)
        }
    }
}
