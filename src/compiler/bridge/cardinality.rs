//! Cardinality bridge (design §16.4, §8.1; SM-12.5).
//!
//! Exactly/at-most/at-least-k over binary variables compiles to one exact
//! linear row (no auxiliary variables, no Big-M).

use crate::compiler::bridge::{
    resolve_variable, select_path, BridgeContext, BridgeFinalizer, BridgeOutput,
};
use crate::compiler::capability::BackendFeature;
use crate::compiler::origin::GeneratedRole;
use crate::compiler::report::FormulationDecision;
use crate::compiler::CompileError;
use crate::construct::{CardinalityConstraint, CardinalityKind};
use crate::model::ConstraintBounds;

/// Compile one cardinality construct into its exact linear row (design §16.4).
pub(crate) fn compile(
    payload: &CardinalityConstraint,
    ctx: &BridgeContext,
    next_variable_index: u32,
    next_row_index: u32,
) -> Result<BridgeOutput, CompileError> {
    // SM-04.4 (WR-01): an unqualified `Cardinality` feature is rejected, never
    // silently compiled. No P32 backend declares a qualified native
    // cardinality, so the exact linear row is the bridge representation.
    select_path(
        ctx.capabilities,
        ctx.policy,
        BackendFeature::Cardinality,
        "cardinality construct",
    )?;
    let mut finalizer = BridgeFinalizer::new(ctx.construct, next_variable_index, next_row_index);

    let ids: Vec<_> = payload
        .variables
        .iter()
        .map(|v| resolve_variable(ctx.variable_ids, *v, ctx.construct))
        .collect::<Result<_, _>>()?;
    let coeffs = ids.into_iter().map(|id| (id, 1.0)).collect();

    let k = payload.k as f64;
    let bounds = match payload.kind {
        CardinalityKind::Exactly => ConstraintBounds::eq(k),
        CardinalityKind::AtMost => ConstraintBounds::le(k),
        CardinalityKind::AtLeast => ConstraintBounds::ge(k),
    };
    finalizer.add_row(GeneratedRole::CardinalityRow, bounds, coeffs, None);

    finalizer.add_decision(FormulationDecision {
        decision: "cardinality.representation".to_string(),
        selection: "exact linear row".to_string(),
        reason: "exact cardinality row over binary variables (design §16.4, SM-12.5)".to_string(),
    });

    Ok(finalizer.finish())
}
