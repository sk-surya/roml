//! Exact portable bridge for persistent soft constraints (P30, D-02/D-03).
//!
//! Each finite side of the original row receives its own nonnegative
//! violation variable.  The lower and upper equations are intentionally
//! signed in opposite directions:
//!
//! ```text
//! f(x) + v_lo >= l
//! f(x) - v_up <= u
//! ```
//!
//! This module emits no native solver relaxation primitive. The generated
//! variables and rows are ordinary compiled-row entities with stable origin
//! roles, making the formulation portable and auditable.

use std::collections::BTreeMap;

use crate::compiler::backend_ir::CompiledVariableId;
use crate::compiler::bridge::{
    select_path, BridgeContext, BridgeDependency, BridgeFinalizer, BridgeOutput,
};
use crate::compiler::capability::BackendFeature;
use crate::compiler::origin::GeneratedRole;
use crate::compiler::report::FormulationDecision;
use crate::compiler::CompileError;
use crate::construct::SoftConstraintConstraint;
use crate::model::coefficient::CoefficientTarget;
use crate::model::{Bounds, ConstraintBounds, VarType};

pub(crate) fn compile(
    payload: &SoftConstraintConstraint,
    ctx: &BridgeContext,
    next_variable_index: u32,
    next_row_index: u32,
) -> Result<BridgeOutput, CompileError> {
    select_path(
        ctx.capabilities,
        ctx.policy,
        BackendFeature::SoftConstraint,
        "persistent soft constraint",
    )?;

    let original = ctx
        .snapshot
        .constraints
        .iter()
        .find(|constraint| constraint.id == payload.original_constraint)
        .ok_or_else(|| {
            CompileError::UnsupportedFeature(format!(
                "soft constraint {:?} references an absent original constraint {:?}",
                ctx.construct, payload.original_constraint
            ))
        })?;
    if !original.active {
        return Err(CompileError::UnsupportedFeature(format!(
            "soft constraint {:?} references an inactive original constraint {:?}",
            ctx.construct, payload.original_constraint
        )));
    }

    let cap = payload.violation.max_violation.unwrap_or(f64::INFINITY);
    if !cap.is_finite() && cap != f64::INFINITY {
        return Err(CompileError::InvalidBigM {
            construct: ctx.construct,
            expression: format!(
                "soft constraint {:?} violation cap",
                payload.original_constraint
            ),
            reason: format!("non-finite cap {cap}"),
        });
    }
    if cap < 0.0 {
        return Err(CompileError::InvalidBigM {
            construct: ctx.construct,
            expression: format!(
                "soft constraint {:?} violation cap",
                payload.original_constraint
            ),
            reason: format!("negative cap {cap}"),
        });
    }

    let coefficients = original_coefficients(ctx, payload.original_constraint)?;
    let mut finalizer = BridgeFinalizer::new(ctx.construct, next_variable_index, next_row_index);
    finalizer.add_dependency(BridgeDependency::Construct(ctx.construct));
    finalizer.add_dependency(BridgeDependency::Constraint(payload.original_constraint));
    for cell in &ctx.snapshot.cells {
        if let CoefficientTarget::Constraint(constraint) = cell.cell_key.0 {
            if constraint == payload.original_constraint {
                finalizer.add_dependency(BridgeDependency::Variable(cell.cell_key.1));
                for parameter in &cell.dependencies {
                    finalizer.add_dependency(BridgeDependency::Parameter(*parameter));
                }
            }
        }
    }
    finalizer.add_decision(FormulationDecision {
        decision: "soft_constraint.path".to_string(),
        selection: "exact portable weighted-violation bridge".to_string(),
        reason: "P30 has no qualified native relaxation representation; compiled rows are the normative path".to_string(),
    });
    let weight = payload
        .penalty
        .weight
        .eval_checked(|parameter| {
            ctx.parameter_values
                .get(&parameter)
                .copied()
                .ok_or(parameter)
        })
        .map_err(|parameter| CompileError::MissingConstructParameter {
            construct: ctx.construct,
            parameter,
        })?;
    if !weight.is_finite() || weight < 0.0 {
        return Err(CompileError::UnsupportedFeature(format!(
            "soft-constraint penalty weight must be finite and non-negative for {:?}",
            ctx.construct
        )));
    }
    finalizer.add_decision(FormulationDecision {
        decision: "soft_constraint.penalty".to_string(),
        selection: format!("weight={weight}; target={:?}", payload.penalty.target),
        reason: "evaluated from the exact canonical snapshot before backend mutation".to_string(),
    });

    // Stable role order is lower then upper. Equality deliberately emits two
    // distinct variables/rows, preserving the identity of both sides.
    if original.bounds.lower.is_finite() {
        let variable = finalizer.add_variable(
            GeneratedRole::SoftConstraintLowerViolationVariable,
            VarType::Continuous,
            Bounds::new(0.0, cap),
            None,
        );
        let mut row_coefficients = coefficients.clone();
        add_coefficient(&mut row_coefficients, variable, 1.0);
        finalizer.add_row(
            GeneratedRole::SoftConstraintLowerViolationRow,
            ConstraintBounds::ge(original.bounds.lower),
            row_coefficients,
            None,
        );
    }
    if original.bounds.upper.is_finite() {
        let variable = finalizer.add_variable(
            GeneratedRole::SoftConstraintUpperViolationVariable,
            VarType::Continuous,
            Bounds::new(0.0, cap),
            None,
        );
        let mut row_coefficients = coefficients;
        add_coefficient(&mut row_coefficients, variable, -1.0);
        finalizer.add_row(
            GeneratedRole::SoftConstraintUpperViolationRow,
            ConstraintBounds::le(original.bounds.upper),
            row_coefficients,
            None,
        );
    }

    Ok(finalizer.finish())
}

fn original_coefficients(
    ctx: &BridgeContext,
    constraint: crate::id::ConId,
) -> Result<Vec<(CompiledVariableId, f64)>, CompileError> {
    let mut by_variable: BTreeMap<CompiledVariableId, f64> = BTreeMap::new();
    for cell in &ctx.snapshot.cells {
        if let CoefficientTarget::Constraint(candidate) = cell.cell_key.0 {
            if candidate != constraint {
                continue;
            }
            let variable = ctx.variable_ids.get(&cell.cell_key.1).copied().ok_or(
                CompileError::MissingConstructReference {
                    construct: ctx.construct,
                    variable: cell.cell_key.1,
                },
            )?;
            if !cell.evaluated_value.is_finite() {
                return Err(CompileError::InvalidBigM {
                    construct: ctx.construct,
                    expression: format!("coefficient for {:?}", cell.cell_key.1),
                    reason: format!("non-finite evaluated value {}", cell.evaluated_value),
                });
            }
            *by_variable.entry(variable).or_default() += cell.evaluated_value;
        }
    }
    Ok(by_variable
        .into_iter()
        .filter(|(_, value)| *value != 0.0)
        .collect())
}

fn add_coefficient(
    coefficients: &mut Vec<(CompiledVariableId, f64)>,
    variable: CompiledVariableId,
    value: f64,
) {
    if let Some((_, existing)) = coefficients.iter_mut().find(|(id, _)| *id == variable) {
        *existing += value;
    } else {
        coefficients.push((variable, value));
    }
}

#[allow(dead_code)]
fn side_name(side: crate::construct::ViolationSide) -> &'static str {
    match side {
        crate::construct::ViolationSide::Lower => "lower",
        crate::construct::ViolationSide::Upper => "upper",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::capability::{
        BackendCapabilitySet, CompilationPolicy, FeatureSupport, SupportLevel,
    };
    use crate::construct::soft_constraint::{
        PenaltyPolicy, PenaltyTarget, SoftConstraint, ViolationPolicy,
    };
    use crate::expr::ConstraintExprExt;
    use crate::id::{ConId, Generation, VarId};
    use crate::identity::ConstructId;
    use crate::model::{continuous, Model};
    use crate::snapshot::ModelSnapshot;
    use crate::value_expr::ValueExpr;
    use std::collections::HashMap;

    type VarIds = HashMap<VarId, CompiledVariableId>;

    fn capabilities() -> BackendCapabilitySet {
        let mut set = BackendCapabilitySet::new();
        set.set(
            BackendFeature::SoftConstraint,
            FeatureSupport {
                level: SupportLevel::Bridge,
                limitations: Default::default(),
            },
        );
        set
    }

    fn active_snapshot() -> (ModelSnapshot, ConId, VarIds) {
        let mut model = Model::new();
        let x = model
            .add_variable(continuous().bounds(0.0, 1.0))
            .expect("x");
        let con = model.add_constraint((x).ge(1.0)).expect("row");
        let mut variable_ids = VarIds::new();
        variable_ids.insert(x, CompiledVariableId(0));
        (model.take_snapshot().expect("snapshot"), con, variable_ids)
    }

    fn compile_with(
        construct: ConstructId,
        con: ConId,
        violation: ViolationPolicy,
        weight: ValueExpr,
        snapshot: &ModelSnapshot,
        variable_ids: &VarIds,
    ) -> Result<BridgeOutput, CompileError> {
        let payload = SoftConstraintConstraint {
            handle: SoftConstraint::new(construct, con),
            original_constraint: con,
            violation,
            penalty: PenaltyPolicy {
                weight,
                target: PenaltyTarget::None,
            },
        };
        let caps = capabilities();
        let policy = CompilationPolicy::Auto;
        let parameter_values = HashMap::new();
        let ctx = BridgeContext {
            construct,
            snapshot,
            variable_ids,
            parameter_values: &parameter_values,
            policy: &policy,
            capabilities: &caps,
        };
        compile(&payload, &ctx, 0, 0)
    }

    #[test]
    fn absent_original_constraint_is_rejected() {
        let (snapshot, _, variable_ids) = active_snapshot();
        let missing = ConId::new(9_999, Generation::new());
        let construct = ConstructId::allocate().expect("construct");
        let error = compile_with(
            construct,
            missing,
            ViolationPolicy::default(),
            ValueExpr::constant(1.0),
            &snapshot,
            &variable_ids,
        )
        .expect_err("absent original rejects");
        assert!(matches!(error, CompileError::UnsupportedFeature(ref m) if m.contains("absent")));
    }

    #[test]
    fn inactive_original_constraint_is_rejected() {
        let mut model = Model::new();
        let x = model
            .add_variable(continuous().bounds(0.0, 1.0))
            .expect("x");
        let con = model.add_constraint((x).ge(1.0)).expect("row");
        model
            .set_constraint_active(con, false)
            .expect("deactivate row");
        let snapshot = model.take_snapshot().expect("snapshot");
        let mut variable_ids = VarIds::new();
        variable_ids.insert(x, CompiledVariableId(0));
        let construct = ConstructId::allocate().expect("construct");
        let error = compile_with(
            construct,
            con,
            ViolationPolicy::default(),
            ValueExpr::constant(1.0),
            &snapshot,
            &variable_ids,
        )
        .expect_err("inactive original rejects");
        assert!(matches!(error, CompileError::UnsupportedFeature(ref m) if m.contains("inactive")));
    }

    #[test]
    fn non_finite_violation_cap_is_rejected() {
        let (snapshot, con, variable_ids) = active_snapshot();
        let construct = ConstructId::allocate().expect("construct");
        let error = compile_with(
            construct,
            con,
            ViolationPolicy {
                max_violation: Some(f64::NAN),
            },
            ValueExpr::constant(1.0),
            &snapshot,
            &variable_ids,
        )
        .expect_err("non-finite cap rejects");
        assert!(matches!(error, CompileError::InvalidBigM { .. }));
    }

    #[test]
    fn negative_violation_cap_is_rejected() {
        let (snapshot, con, variable_ids) = active_snapshot();
        let construct = ConstructId::allocate().expect("construct");
        let error = compile_with(
            construct,
            con,
            ViolationPolicy {
                max_violation: Some(-1.0),
            },
            ValueExpr::constant(1.0),
            &snapshot,
            &variable_ids,
        )
        .expect_err("negative cap rejects");
        assert!(matches!(error, CompileError::InvalidBigM { .. }));
    }

    #[test]
    fn non_finite_and_negative_weights_are_rejected() {
        let (snapshot, con, variable_ids) = active_snapshot();
        for weight in [f64::NAN, -1.0] {
            let construct = ConstructId::allocate().expect("construct");
            let error = compile_with(
                construct,
                con,
                ViolationPolicy::default(),
                ValueExpr::constant(weight),
                &snapshot,
                &variable_ids,
            )
            .expect_err("invalid weight rejects");
            match error {
                CompileError::UnsupportedFeature(_) => {}
                other => panic!("expected UnsupportedFeature, got {other:?}"),
            }
        }
    }

    #[test]
    fn missing_weight_parameter_is_a_typed_error() {
        let (snapshot, con, variable_ids) = active_snapshot();
        let construct = ConstructId::allocate().expect("construct");
        let missing = crate::id::ParamId::new(9_999, Generation::new());
        let error = compile_with(
            construct,
            con,
            ViolationPolicy::default(),
            ValueExpr::param(missing),
            &snapshot,
            &variable_ids,
        )
        .expect_err("missing weight parameter rejects");
        match error {
            CompileError::MissingConstructParameter { .. } => {}
            other => panic!("expected MissingConstructParameter, got {other:?}"),
        }
    }
}
