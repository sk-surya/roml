//! Min/max bridge (design §16.3, §8.1, §8.5; D12, D13; SM-12.3, SM-13.4/13.5).
//!
//! Exact equality and the one-sided epigraph/hypograph relations are distinct
//! semantics (D13). This bridge compiles:
//!
//! - **max epigraph** (`sense=Max`, `relation=Epigraph`): zero-binary rows
//!   `x_i <= y` for all operands.
//! - **min hypograph** (`sense=Min`, `relation=Hypograph`): zero-binary rows
//!   `x_i >= y` for all operands.
//! - **exact max**: the bounded selector formulation — `y >= x_i` per operand,
//!   one binary `z_i` per operand with `sum z = 1`, and `y <= x_i + M_i(1-z_i)`
//!   with the finite derived `M_i = u_max - l_i` (`u_max = max_j u_j`).
//! - **exact min**: the mirror selector — `y <= x_i`, binary `z_i` with
//!   `sum z = 1`, and `y >= x_i - M_i(1-z_i)` with `M_i = u_i - l_min`
//!   (`l_min = min_j l_j`).
//!
//! The selector M values are finite derived from the operands' `BoundAnalyzer`
//! intervals (never an arbitrary default constant — D12) and recorded as
//! bound-evidence report entries with their sources (SM-13.5). An unbounded
//! operand in an exact relation is the construct-aware
//! `CompileError::UnboundedBigM` naming the construct and expression
//! (SM-13.4). The `exact_max_selector` / `exact_min_selector` helpers are
//! shared with the absolute-value clamp bridge (P32 Task 17b).

use crate::compiler::bounds::{BoundAnalyzer, BoundSource, Interval};
use crate::compiler::bridge::{
    combine_coefficients, function_coefficients, resolve_variable, select_path, BridgeContext,
    BridgeFinalizer, BridgeOutput, ConstructPath,
};
use crate::compiler::capability::BackendFeature;
use crate::compiler::origin::GeneratedRole;
use crate::compiler::report::FormulationDecision;
use crate::compiler::CompileError;
use crate::construct::{MinMaxConstraint, MinMaxRelation, MinMaxSense};
use crate::function::ScalarFunction;
use crate::model::{Bounds, ConstraintBounds, VarType};

/// A resolved selector operand: compiled coefficients, constant term, the
/// finite interval, and its bound sources.
#[derive(Clone, Debug)]
pub(crate) struct Operand {
    /// Compiled coefficients of the operand expression.
    pub coefficients: Vec<(crate::compiler::backend_ir::CompiledVariableId, f64)>,
    /// Constant term of the operand expression (folded into row bounds).
    pub constant: f64,
    /// The operand's interval over the construct's declared bounds.
    pub interval: Interval,
    /// Bound-source provenance of the interval (SM-13.5).
    pub sources: Vec<BoundSource>,
}

/// Compile one min/max construct into its exact rows (design §16.3).
pub(crate) fn compile(
    payload: &MinMaxConstraint,
    ctx: &BridgeContext,
    next_variable_index: u32,
    next_row_index: u32,
) -> Result<BridgeOutput, CompileError> {
    let path = select_path(
        ctx.capabilities,
        ctx.policy,
        BackendFeature::MinMax,
        "minmax construct",
    )?;
    match path {
        ConstructPath::Native => {
            // No P32 backend declares a qualified native min/max; the enum is
            // `#[non_exhaustive]` and a future native path lands here. The exact
            // portable bridge is the P32 representation.
        }
        ConstructPath::Bridge => {}
    }

    let mut finalizer = BridgeFinalizer::new(ctx.construct, next_variable_index, next_row_index);
    let y = resolve_variable(ctx.variable_ids, payload.output, ctx.construct)?;

    // Resolve every operand: compiled coefficients, constant, interval, and
    // bound sources (deterministic interval analysis over declared bounds).
    let mut operands = Vec::with_capacity(payload.operands.len());
    for (i, expr) in payload.operands.iter().enumerate() {
        let (coefficients, constant) = function_coefficients(
            &ScalarFunction::Linear(expr.clone()),
            ctx.construct,
            ctx.variable_ids,
            ctx.parameter_values,
        )?;
        let trace = BoundAnalyzer::new()
            .interval_of_snapshot(&ScalarFunction::Linear(expr.clone()), ctx.snapshot)
            .map_err(|e| CompileError::InvalidBigM {
                construct: ctx.construct,
                expression: format!("minmax operand {i}: {expr:?}"),
                reason: e.to_string(),
            })?;
        operands.push(Operand {
            coefficients,
            constant,
            interval: trace.result,
            sources: trace.sources,
        });
    }

    match (payload.sense, payload.relation) {
        // ── Max epigraph: x_i <= y for all i (zero binaries) ────────────────
        (MinMaxSense::Max, MinMaxRelation::Epigraph) => {
            for op in &operands {
                let coeffs = combine_coefficients(op.coefficients.clone(), (y, -1.0));
                finalizer.add_row(
                    GeneratedRole::MinMaxEpigraphRow,
                    ConstraintBounds::le(-op.constant),
                    coeffs,
                    None,
                );
            }
            finalizer.add_decision(FormulationDecision {
                decision: "minmax.representation".to_string(),
                selection: "exact bridge (max epigraph, zero binaries)".to_string(),
                reason: "max epigraph compiles to one-sided rows x_i <= y with zero generated \
                         binaries (design §16.3, SM-12.3)"
                    .to_string(),
            });
        }

        // ── Min hypograph: x_i >= y for all i (zero binaries) ───────────────
        (MinMaxSense::Min, MinMaxRelation::Hypograph) => {
            for op in &operands {
                let coeffs = combine_coefficients(op.coefficients.clone(), (y, -1.0));
                finalizer.add_row(
                    GeneratedRole::MinMaxHypographRow,
                    ConstraintBounds::ge(-op.constant),
                    coeffs,
                    None,
                );
            }
            finalizer.add_decision(FormulationDecision {
                decision: "minmax.representation".to_string(),
                selection: "exact bridge (min hypograph, zero binaries)".to_string(),
                reason: "min hypograph compiles to one-sided rows x_i >= y with zero generated \
                         binaries (design §16.3, SM-12.3)"
                    .to_string(),
            });
        }

        // ── Exact max: bounded selector formulation ─────────────────────────
        (MinMaxSense::Max, MinMaxRelation::Exact) => {
            require_bounded_operands(&operands, ctx, "max")?;
            let u_max = operands
                .iter()
                .map(|op| op.interval.upper)
                .fold(f64::NEG_INFINITY, f64::max);
            // y >= x_i for all i  ⇔  y - x_i >= 0.
            for op in &operands {
                let coeffs =
                    combine_coefficients(negate_coefficients(op.coefficients.clone()), (y, 1.0));
                finalizer.add_row(
                    GeneratedRole::MinMaxSelectorRow,
                    ConstraintBounds::ge(op.constant),
                    coeffs,
                    None,
                );
            }
            let binaries = exact_max_selector(
                &mut finalizer,
                y,
                &operands,
                u_max,
                GeneratedRole::MinMaxSelectorBinary,
                GeneratedRole::MinMaxSelectorRow,
                "minmax.selector_m",
            )?;
            finalizer.add_decision(FormulationDecision {
                decision: "minmax.representation".to_string(),
                selection: "exact bridge (max selector)".to_string(),
                reason: format!(
                    "exact max compiles to a bounded selector formulation with {} selector \
                     binaries and finite derived M values (design §16.3, SM-12.3)",
                    binaries.len()
                ),
            });
        }

        // ── Exact min: bounded selector formulation ─────────────────────────
        (MinMaxSense::Min, MinMaxRelation::Exact) => {
            require_bounded_operands(&operands, ctx, "min")?;
            let l_min = operands
                .iter()
                .map(|op| op.interval.lower)
                .fold(f64::INFINITY, f64::min);
            // y <= x_i for all i  ⇔  y - x_i <= 0.
            for op in &operands {
                let coeffs =
                    combine_coefficients(negate_coefficients(op.coefficients.clone()), (y, 1.0));
                finalizer.add_row(
                    GeneratedRole::MinMaxSelectorRow,
                    ConstraintBounds::le(op.constant),
                    coeffs,
                    None,
                );
            }
            let binaries = exact_min_selector(
                &mut finalizer,
                y,
                &operands,
                l_min,
                GeneratedRole::MinMaxSelectorBinary,
                GeneratedRole::MinMaxSelectorRow,
                "minmax.selector_m",
            )?;
            finalizer.add_decision(FormulationDecision {
                decision: "minmax.representation".to_string(),
                selection: "exact bridge (min selector)".to_string(),
                reason: format!(
                    "exact min compiles to a bounded selector formulation with {} selector \
                     binaries and finite derived M values (design §16.3, SM-12.3)",
                    binaries.len()
                ),
            });
        }

        // The builder rejects these; defensive guard — never silently emitted.
        (MinMaxSense::Min, MinMaxRelation::Epigraph)
        | (MinMaxSense::Max, MinMaxRelation::Hypograph) => {
            return Err(CompileError::UnsupportedFeature(
                "trivially satisfiable min-epigraph / max-hypograph minmax relation".into(),
            ));
        }
    }

    Ok(finalizer.finish())
}

/// Negate every coefficient of an operand expression (for rows `y - x_i`).
fn negate_coefficients(
    coefficients: Vec<(crate::compiler::backend_ir::CompiledVariableId, f64)>,
) -> Vec<(crate::compiler::backend_ir::CompiledVariableId, f64)> {
    coefficients.into_iter().map(|(id, c)| (id, -c)).collect()
}

/// Require every operand interval to be finite for the exact selector; the
/// first unbounded operand surfaces the construct-aware `UnboundedBigM` naming
/// the construct and expression (SM-13.4, D12) — never a silent default
/// constant.
fn require_bounded_operands(
    operands: &[Operand],
    ctx: &BridgeContext,
    sense: &str,
) -> Result<(), CompileError> {
    for (i, op) in operands.iter().enumerate() {
        if !op.interval.is_bounded() {
            return Err(CompileError::UnboundedBigM {
                construct: ctx.construct,
                expression: format!("{sense} operand {i} with interval {:?}", op.interval),
            });
        }
    }
    Ok(())
}

/// Emit the exact max selector rows for `output` over `operands` (all intervals
/// finite): `output >= x_i`, a binary per operand with `sum = 1`, and
/// `output <= x_i + M_i(1-z_i)` with `M_i = u_max - l_i`. Records the M evidence
/// (SM-13.5) and returns the selector binary ids.
pub(crate) fn exact_max_selector(
    finalizer: &mut BridgeFinalizer,
    output: crate::compiler::backend_ir::CompiledVariableId,
    operands: &[Operand],
    u_max: f64,
    binary_role: GeneratedRole,
    row_role: GeneratedRole,
    m_key_prefix: &str,
) -> Result<Vec<crate::compiler::backend_ir::CompiledVariableId>, CompileError> {
    let mut binaries = Vec::with_capacity(operands.len());
    for _ in operands {
        binaries.push(finalizer.add_variable(binary_role, VarType::Binary, Bounds::BINARY, None));
    }
    // sum_i z_i = 1.
    finalizer.add_row(
        row_role,
        ConstraintBounds::eq(1.0),
        binaries.iter().map(|&z| (z, 1.0)).collect(),
        None,
    );
    // output <= x_i + M_i(1-z_i)  ⇔  output - x_i + M_i·z_i <= M_i.
    for (i, op) in operands.iter().enumerate() {
        let m = u_max - op.interval.lower;
        let coeffs =
            combine_coefficients(negate_coefficients(op.coefficients.clone()), (output, 1.0));
        let coeffs = combine_coefficients(coeffs, (binaries[i], m));
        finalizer.add_row(
            row_role,
            ConstraintBounds::le(m + op.constant),
            coeffs,
            None,
        );
        finalizer.record_bound_evidence(
            &format!("{m_key_prefix}.{i}"),
            Some(m),
            &format!("M_{i} = u_max ({u_max}) - l_{i} ({})", op.interval.lower),
            &op.sources,
        );
    }
    Ok(binaries)
}

/// Emit the exact min selector rows for `output` over `operands` (all intervals
/// finite): `output <= x_i`, a binary per operand with `sum = 1`, and
/// `output >= x_i - M_i(1-z_i)` with `M_i = u_i - l_min`. Records the M evidence
/// (SM-13.5) and returns the selector binary ids.
pub(crate) fn exact_min_selector(
    finalizer: &mut BridgeFinalizer,
    output: crate::compiler::backend_ir::CompiledVariableId,
    operands: &[Operand],
    l_min: f64,
    binary_role: GeneratedRole,
    row_role: GeneratedRole,
    m_key_prefix: &str,
) -> Result<Vec<crate::compiler::backend_ir::CompiledVariableId>, CompileError> {
    let mut binaries = Vec::with_capacity(operands.len());
    for _ in operands {
        binaries.push(finalizer.add_variable(binary_role, VarType::Binary, Bounds::BINARY, None));
    }
    // sum_i z_i = 1.
    finalizer.add_row(
        row_role,
        ConstraintBounds::eq(1.0),
        binaries.iter().map(|&z| (z, 1.0)).collect(),
        None,
    );
    // output >= x_i - M_i(1-z_i)  ⇔  output - x_i - M_i·z_i >= -M_i.
    for (i, op) in operands.iter().enumerate() {
        let m = op.interval.upper - l_min;
        let coeffs =
            combine_coefficients(negate_coefficients(op.coefficients.clone()), (output, 1.0));
        let coeffs = combine_coefficients(coeffs, (binaries[i], -m));
        finalizer.add_row(
            row_role,
            ConstraintBounds::ge(-m + op.constant),
            coeffs,
            None,
        );
        finalizer.record_bound_evidence(
            &format!("{m_key_prefix}.{i}"),
            Some(m),
            &format!("M_{i} = u_{i} ({}) - l_min ({l_min})", op.interval.upper),
            &op.sources,
        );
    }
    Ok(binaries)
}
