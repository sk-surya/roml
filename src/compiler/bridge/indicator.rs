//! Indicator bridge (design §16.1, §8.1; SM-12.1, SM-13.2/13.4).
//!
//! A one-way implication over a binary activator compiles to exact rows. Under
//! `Auto`, a qualified native `BackendFeature::Indicator` is selected when the
//! backend declares it (the emitted exact row carries the `IndicatorNative`
//! role — the P32 backend IR has no native-constraint representation, so the
//! exact finite-bound row is emitted and the selection is observable via the
//! role + a formulation decision); otherwise the exact finite-bound Big-M
//! bridge emits the same one-way implication row with the `IndicatorImplicationRow`
//! role. `Portable` forces the bridge; `NativeRequired` rejects a non-native
//! feature.
//!
//! The Big-M is always derived from the construct's declared bounds via
//! [`BoundAnalyzer`](crate::compiler::bounds::BoundAnalyzer) — never a default
//! constant (D12). A missing finite M is the construct-aware
//! `CompileError::UnboundedBigM` (SM-13.4).

use std::collections::HashMap;

use crate::compiler::bounds::{BigMImplication, BoundAnalyzer};
use crate::compiler::bridge::{
    combine_coefficients, function_coefficients, resolve_variable, select_path, BridgeContext,
    BridgeFinalizer, BridgeOutput, ConstructPath,
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
/// the current parameter values.
fn one_sided_implications(
    set: &ScalarSet,
    parameter_values: &HashMap<ParamId, f64>,
) -> Vec<OneSided> {
    let eval = |e: &ValueExpr| e.eval(|p| parameter_values.get(&p).copied().unwrap_or(0.0));
    match set {
        ScalarSet::LessEqual(upper) => vec![OneSided {
            side: BigMImplication::Upper,
            rhs: eval(upper),
            op: "<=",
        }],
        ScalarSet::GreaterEqual(lower) => vec![OneSided {
            side: BigMImplication::Lower,
            rhs: eval(lower),
            op: ">=",
        }],
        ScalarSet::EqualTo(value) => {
            let rhs = eval(value);
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
                rhs: eval(lower),
                op: ">=",
            },
            OneSided {
                side: BigMImplication::Upper,
                rhs: eval(upper),
                op: "<=",
            },
        ],
    }
}

/// Compile one indicator construct into its exact rows (design §16.1).
pub(crate) fn compile(
    payload: &IndicatorConstraint,
    ctx: &BridgeContext,
    next_variable_index: u32,
    next_row_index: u32,
) -> Result<BridgeOutput, CompileError> {
    let path = select_path(
        ctx.capabilities,
        ctx.policy,
        BackendFeature::Indicator,
        "indicator construct",
    )?;
    let (role, selection, decision_reason) = match path {
        ConstructPath::Native => (
            GeneratedRole::IndicatorNative,
            "native indicator",
            "qualified native BackendFeature::Indicator selected (Auto)",
        ),
        ConstructPath::Bridge
            if *ctx.policy == crate::compiler::capability::CompilationPolicy::Portable =>
        {
            (
                GeneratedRole::IndicatorImplicationRow,
                "exact bridge (portable forced)",
                "Portable policy forces the deterministic ROML bridge (design §8.1)",
            )
        }
        ConstructPath::Bridge => (
            GeneratedRole::IndicatorImplicationRow,
            "exact bridge (finite bound)",
            "no qualified native indicator; exact finite-bound one-way Big-M bridge (design §8.1)",
        ),
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

    for one_sided in one_sided_implications(&payload.set, ctx.parameter_values) {
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
