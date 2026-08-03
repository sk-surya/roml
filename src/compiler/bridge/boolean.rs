//! Boolean bridge (design §16.4, §8.1; SM-12.5).
//!
//! Implication, equivalence, any (at-least-one), and all (all-ones) over binary
//! variables compile to exact linear rows (no auxiliary variables, no Big-M).

use crate::compiler::bridge::{
    resolve_variable, select_path, BridgeContext, BridgeFinalizer, BridgeOutput,
};
use crate::compiler::capability::BackendFeature;
use crate::compiler::origin::GeneratedRole;
use crate::compiler::report::FormulationDecision;
use crate::compiler::CompileError;
use crate::construct::{BooleanConstraint, BooleanKind};
use crate::model::ConstraintBounds;

/// Compile one Boolean construct into its exact linear rows (design §16.4).
pub(crate) fn compile(
    payload: &BooleanConstraint,
    ctx: &BridgeContext,
    next_variable_index: u32,
    next_row_index: u32,
) -> Result<BridgeOutput, CompileError> {
    // SM-04.4 (WR-01): an unqualified `Boolean` feature is rejected, never
    // silently compiled. No P32 backend declares a qualified native Boolean,
    // so the exact linear rows are the bridge representation; a native path
    // would land here and select it.
    select_path(
        ctx.capabilities,
        ctx.policy,
        BackendFeature::Boolean,
        "boolean construct",
    )?;
    let mut finalizer = BridgeFinalizer::new(ctx.construct, next_variable_index, next_row_index);

    match &payload.kind {
        BooleanKind::Implication {
            antecedent,
            consequent,
        } => {
            let a = resolve_variable(ctx.variable_ids, *antecedent, ctx.construct)?;
            let b = resolve_variable(ctx.variable_ids, *consequent, ctx.construct)?;
            // a ⇒ b  ⇔  a - b <= 0.
            finalizer.add_row(
                GeneratedRole::BooleanImplicationRow,
                ConstraintBounds::le(0.0),
                vec![(a, 1.0), (b, -1.0)],
                None,
            );
        }
        BooleanKind::Equivalence { left, right } => {
            let a = resolve_variable(ctx.variable_ids, *left, ctx.construct)?;
            let b = resolve_variable(ctx.variable_ids, *right, ctx.construct)?;
            // a ⟺ b  ⇔  a - b <= 0 AND b - a <= 0.
            finalizer.add_row(
                GeneratedRole::BooleanEquivalenceRow,
                ConstraintBounds::le(0.0),
                vec![(a, 1.0), (b, -1.0)],
                None,
            );
            finalizer.add_row(
                GeneratedRole::BooleanEquivalenceRow,
                ConstraintBounds::le(0.0),
                vec![(b, 1.0), (a, -1.0)],
                None,
            );
        }
        BooleanKind::Any { variables } => {
            let ids: Vec<_> = variables
                .iter()
                .map(|v| resolve_variable(ctx.variable_ids, *v, ctx.construct))
                .collect::<Result<_, _>>()?;
            let coeffs = ids.into_iter().map(|id| (id, 1.0)).collect();
            // Any = at-least-one: Σ v_i >= 1.
            finalizer.add_row(
                GeneratedRole::BooleanAnyRow,
                ConstraintBounds::ge(1.0),
                coeffs,
                None,
            );
        }
        BooleanKind::All { variables } => {
            let n = variables.len();
            let ids: Vec<_> = variables
                .iter()
                .map(|v| resolve_variable(ctx.variable_ids, *v, ctx.construct))
                .collect::<Result<_, _>>()?;
            let coeffs = ids.into_iter().map(|id| (id, 1.0)).collect();
            // All = all-ones: Σ v_i >= n (each v_i ≤ 1, so equality at n).
            finalizer.add_row(
                GeneratedRole::BooleanAllRow,
                ConstraintBounds::ge(n as f64),
                coeffs,
                None,
            );
        }
    }

    finalizer.add_decision(FormulationDecision {
        decision: "boolean.representation".to_string(),
        selection: "exact linear rows".to_string(),
        reason: "exact Boolean rows over binary variables (design §16.4, SM-12.5)".to_string(),
    });

    Ok(finalizer.finish())
}
