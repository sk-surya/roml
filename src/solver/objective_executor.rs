//! Portable weighted/lexicographic objective execution (P31, design §15).
//!
//! Each weighted stage is reduced to one normalized minimization scalar via
//! [`crate::solver::objective_combine::combine_stage`]. The executor applies a
//! solve-scoped overlay that adds the prior stages' exact degradation-lock
//! rows and sets the current stage's scalar as a single-objective policy,
//! solves, records per-stage metadata, and always rolls back every temporary
//! artifact before a result can escape.

use crate::compiler::backend_ir::{
    CompilationId, CompiledConstraintId, CompiledLinearRow, CompiledObjective, CompiledObjectiveId,
    CompiledVariable, CompiledVariableId,
};
use crate::compiler::origin::{EntityOrigin, GeneratedRole, OriginMap, OverlayId};
use crate::compiler::session::CompilationSession;
use crate::construct::soft_constraint::PenaltyTarget;
use crate::construct::ConstructKind;
use crate::id::{ObjId, VarId};
use crate::model::{ConstraintBounds, Model, Sense};
use crate::objective_policy::{
    ObjectiveLockReport, ObjectivePriority, ObjectiveValue, WeightedObjective,
};
use crate::solver::infeasibility::BoundSide;
use crate::solver::objective_combine::{combine_stage, CombinedStage};
use crate::solver::overlay::OverlayOp;

/// Immutable model/compiler context shared across one stage's overlay build.
pub(crate) struct StageContext<'a> {
    model: &'a Model,
    compiler: &'a CompilationSession,
}

impl<'a> StageContext<'a> {
    pub(crate) fn new(model: &'a Model, compiler: &'a CompilationSession) -> Self {
        Self { model, compiler }
    }
}

/// Mutable overlay-assembly state (operations, origins, and allocation
/// cursors) used while building one stage's overlay.
pub(crate) struct OverlayAssembly {
    operations: Vec<OverlayOp>,
    origins: OriginMap,
    next_row: u32,
    next_variable: u32,
    next_objective: u32,
}

impl OverlayAssembly {
    pub(crate) fn new(compiler: &CompilationSession) -> Result<Self, ObjectiveExecutionError> {
        Ok(Self {
            operations: Vec::new(),
            origins: OriginMap::new(),
            next_row: compiler
                .next_row_index()
                .ok_or_else(|| ObjectiveExecutionError::Preflight("no row cursor".into()))?,
            next_variable: compiler
                .next_variable_index()
                .ok_or_else(|| ObjectiveExecutionError::Preflight("no variable cursor".into()))?,
            next_objective: compiler
                .next_objective_index()
                .ok_or_else(|| ObjectiveExecutionError::Preflight("no objective cursor".into()))?,
        })
    }
}

/// Error raised while executing an objective policy (design §19).
#[derive(Clone, Debug, PartialEq)]
pub enum ObjectiveExecutionError {
    /// Invalid policy or stale canonical identity before mutation.
    Preflight(String),
    /// No qualified native provider exists under `NativeRequired`.
    NativeProviderRequired,
    /// Combining a weighted level failed.
    Combine(String),
    /// Compiler/overlay construction failed before backend mutation.
    Compile(String),
    /// Backend apply/solve/extraction failure.
    Backend(String),
    /// Primary failure plus cleanup/rebuild failure; the backend must be
    /// rebuilt.
    Cleanup {
        /// Original lifecycle failure.
        primary: String,
        /// Cleanup failure retained rather than overwritten.
        cleanup: String,
        /// Restoration could not be proven.
        requires_rebuild: bool,
    },
    /// The backend returned malformed/non-finite numeric evidence.
    Numerical(String),
}

/// A prior stage's degradation lock: the normalized scalar function whose
/// upper bound is fixed for all later stages.
pub(crate) struct PriorLock {
    /// The normalized scalar stage function.
    pub stage: CombinedStage,
    /// The exact lock computed from that stage's `z*`.
    pub lock: ObjectiveLockReport,
}

/// Build the solve-scoped overlay for one stage.
///
/// `locks` carries every prior stage's normalized scalar and its exact
/// degradation bound (all remain active). `current` is the stage to optimize.
pub(crate) fn build_stage_overlay(
    ctx: &StageContext,
    base_compilation: CompilationId,
    overlay_id: OverlayId,
    assembly: &mut OverlayAssembly,
    locks: &[PriorLock],
    current: &CombinedStage,
    priority: ObjectivePriority,
) -> Result<Box<crate::solver::overlay::CompiledOverlay>, ObjectiveExecutionError> {
    let mut objective_coefficients = current.coefficients.clone();
    fold_priority_penalties(
        ctx,
        priority,
        overlay_id,
        assembly,
        &mut objective_coefficients,
    )?;

    for prior in locks {
        let row_id = CompiledConstraintId(assembly.next_row);
        assembly.next_row += 1;
        assembly.origins.insert_constraint(
            row_id,
            EntityOrigin::SolveOverlay {
                overlay: overlay_id,
                role: GeneratedRole::ObjectiveLockRow,
            },
        );
        let rhs = prior.lock.normalized_upper_bound - prior.stage.constant;
        assembly.operations.push(OverlayOp::AddTemporaryRow {
            row: CompiledLinearRow {
                id: row_id,
                bounds: ConstraintBounds::le(rhs),
                coefficients: prior.stage.coefficients.clone(),
                name: Some(format!("p31 degradation lock priority {:?}", priority)),
            },
        });
    }

    let objective_id = CompiledObjectiveId(assembly.next_objective);
    assembly.next_objective += 1;
    assembly.origins.insert_objective(
        objective_id,
        EntityOrigin::SolveOverlay {
            overlay: overlay_id,
            role: GeneratedRole::Bridge,
        },
    );
    assembly.operations.push(OverlayOp::AddTemporaryObjective {
        objective: CompiledObjective {
            id: objective_id,
            sense: Sense::Minimize,
            coefficients: objective_coefficients,
            constant: current.constant,
            name: Some(format!("p31 combined objective priority {:?}", priority)),
        },
    });
    let policy = crate::compiler::backend_ir::CompiledObjectivePolicy::Single(objective_id);
    assembly
        .operations
        .push(OverlayOp::SetObjectivePolicy(policy));

    let compiled_id = CompilationId::allocate()
        .map_err(|_| ObjectiveExecutionError::Compile("compilation identity exhausted".into()))?;
    Ok(Box::new(crate::solver::overlay::CompiledOverlay {
        base_compilation,
        compilation_id: compiled_id,
        overlay_id,
        operations: std::mem::take(&mut assembly.operations),
        origin_additions: std::mem::take(&mut assembly.origins),
        objective_policy_override: None,
    }))
}

fn fold_priority_penalties(
    ctx: &StageContext,
    priority: ObjectivePriority,
    overlay_id: OverlayId,
    assembly: &mut OverlayAssembly,
    objective_coefficients: &mut Vec<(CompiledVariableId, f64)>,
) -> Result<(), ObjectiveExecutionError> {
    let snapshot = ctx
        .model
        .take_snapshot()
        .map_err(|e| ObjectiveExecutionError::Compile(format!("snapshot failed: {e}")))?;
    for construct in &snapshot.constructs {
        let (payload, target) = match &construct.kind {
            ConstructKind::SoftConstraint(payload) => (payload, payload.penalty.target),
            _ => continue,
        };
        let target_priority = match target {
            PenaltyTarget::Priority(p) => p,
            _ => continue,
        };
        if target_priority != priority {
            continue;
        }
        let weight = payload
            .penalty
            .weight
            .eval_checked(|parameter| {
                snapshot
                    .parameters
                    .iter()
                    .find(|p| p.id == parameter)
                    .map(|p| p.value)
                    .ok_or(parameter)
            })
            .map_err(|parameter| {
                ObjectiveExecutionError::Preflight(format!(
                    "priority-targeted penalty weight references missing parameter {parameter:?}"
                ))
            })?;
        if !weight.is_finite() || weight < 0.0 {
            return Err(ObjectiveExecutionError::Preflight(format!(
                "priority-targeted penalty weight must be finite and nonnegative, got {weight}"
            )));
        }
        let constraint = snapshot
            .constraints
            .iter()
            .find(|entry| entry.id == payload.original_constraint)
            .ok_or_else(|| {
                ObjectiveExecutionError::Preflight(format!(
                    "priority-targeted penalty references unknown constraint {:?}",
                    payload.original_constraint
                ))
            })?;
        let row_coefficients =
            constraint_coefficients(&snapshot, ctx.compiler, payload.original_constraint)?;
        for side in violation_sides(constraint.bounds) {
            let violation =
                add_violation_variable(assembly, overlay_id, payload.violation.max_violation);
            objective_coefficients.push((violation, weight));
            let row_id = CompiledConstraintId(assembly.next_row);
            assembly.next_row += 1;
            assembly.origins.insert_constraint(
                row_id,
                EntityOrigin::SolveOverlay {
                    overlay: overlay_id,
                    role: GeneratedRole::FeasibilityRelaxationViolationRow,
                },
            );
            let mut coefficients = row_coefficients.clone();
            coefficients.push((violation, side_sign(side)));
            coefficients.sort_by_key(|(id, _)| *id);
            assembly.operations.push(OverlayOp::AddTemporaryRow {
                row: CompiledLinearRow {
                    id: row_id,
                    bounds: side_bounds(constraint.bounds, side),
                    coefficients,
                    name: Some(format!("p31 priority penalty row {:?}", priority)),
                },
            });
        }
    }
    Ok(())
}

pub(crate) fn validate_priority_targets(
    model: &Model,
    priorities: &[ObjectivePriority],
) -> Result<(), ObjectiveExecutionError> {
    let snapshot = model
        .take_snapshot()
        .map_err(|e| ObjectiveExecutionError::Preflight(format!("snapshot failed: {e}")))?;
    for construct in &snapshot.constructs {
        let target = match &construct.kind {
            ConstructKind::SoftConstraint(payload) => payload.penalty.target,
            _ => continue,
        };
        if let PenaltyTarget::Priority(p) = target {
            if !priorities.contains(&p) {
                return Err(ObjectiveExecutionError::Preflight(format!(
                    "soft constraint penalty targets priority {p:?} which is not present in the \
                     objective policy levels"
                )));
            }
        }
    }
    Ok(())
}

fn violation_sides(bounds: ConstraintBounds) -> Vec<BoundSide> {
    let mut sides = Vec::new();
    if bounds.lower.is_finite() {
        sides.push(BoundSide::Lower);
    }
    if bounds.upper.is_finite() {
        sides.push(BoundSide::Upper);
    }
    sides
}

fn side_sign(side: BoundSide) -> f64 {
    match side {
        BoundSide::Lower => 1.0,
        BoundSide::Upper => -1.0,
    }
}

fn side_bounds(bounds: ConstraintBounds, side: BoundSide) -> ConstraintBounds {
    match side {
        BoundSide::Lower => ConstraintBounds::ge(bounds.lower),
        BoundSide::Upper => ConstraintBounds::le(bounds.upper),
    }
}

fn add_violation_variable(
    assembly: &mut OverlayAssembly,
    overlay_id: OverlayId,
    max_violation: Option<f64>,
) -> CompiledVariableId {
    let variable = CompiledVariableId(assembly.next_variable);
    assembly.next_variable += 1;
    assembly.origins.insert_variable(
        variable,
        EntityOrigin::SolveOverlay {
            overlay: overlay_id,
            role: GeneratedRole::FeasibilityRelaxationViolationVariable,
        },
    );
    assembly.operations.push(OverlayOp::AddTemporaryVariable {
        variable: CompiledVariable {
            id: variable,
            bounds: crate::model::Bounds::new(0.0, max_violation.unwrap_or(f64::INFINITY)),
            var_type: crate::model::VarType::Continuous,
            name: Some("p31 priority penalty violation".into()),
        },
    });
    variable
}
fn constraint_coefficients(
    snapshot: &crate::snapshot::ModelSnapshot,
    compiler: &CompilationSession,
    constraint: crate::id::ConId,
) -> Result<Vec<(CompiledVariableId, f64)>, ObjectiveExecutionError> {
    let mut coefficients = Vec::new();
    for cell in &snapshot.cells {
        if let crate::model::coefficient::CoefficientTarget::Constraint(candidate) = cell.cell_key.0
        {
            if candidate == constraint {
                let variable = compiler
                    .compiled_variable_id(cell.cell_key.1)
                    .ok_or_else(|| {
                        ObjectiveExecutionError::Compile(format!(
                            "constraint references missing compiled variable {:?}",
                            cell.cell_key.1
                        ))
                    })?;
                coefficients.push((variable, cell.evaluated_value));
            }
        }
    }
    coefficients.sort_by_key(|(id, _)| *id);
    Ok(coefficients)
}

/// Evaluate a combined scalar stage at a solution's variable values.
pub(crate) fn evaluate_combined(
    compiler: &CompilationSession,
    combined: &CombinedStage,
    values: &[(VarId, f64)],
) -> f64 {
    let mut value = combined.constant;
    for (cid, coef) in &combined.coefficients {
        if let Some(var) = compiler.user_variable(*cid) {
            if let Some(v) = lookup(values, var) {
                value += coef * v;
            }
        }
    }
    value
}

/// Evaluate one raw (un-normalized) objective at a solution.
pub(crate) fn evaluate_objective(
    compiler: &CompilationSession,
    model: &Model,
    objective: ObjId,
    values: &[(VarId, f64)],
) -> Result<f64, ObjectiveExecutionError> {
    let (terms, constant) = compiler
        .compiled_objective_terms(objective)
        .ok_or_else(|| {
            ObjectiveExecutionError::Combine(format!("stale objective {objective:?}"))
        })?;
    let mut value = constant;
    for (cid, coef) in terms {
        if let Some(var) = compiler.user_variable(cid) {
            if let Some(v) = lookup(values, var) {
                value += coef * v;
            }
        }
    }
    let _ = model.objective_sense(objective);
    Ok(value)
}

/// Compute every referenced objective's value at a solution for a level.
pub(crate) fn objective_values(
    compiler: &CompilationSession,
    model: &Model,
    objectives: &[WeightedObjective],
    values: &[(VarId, f64)],
) -> Result<Vec<ObjectiveValue>, ObjectiveExecutionError> {
    let mut out = Vec::new();
    for wo in objectives {
        out.push(ObjectiveValue {
            objective: wo.objective,
            value: evaluate_objective(compiler, model, wo.objective, values)?,
        });
    }
    Ok(out)
}

fn lookup(values: &[(VarId, f64)], var: VarId) -> Option<f64> {
    values.iter().find(|(v, _)| *v == var).map(|(_, val)| *val)
}

/// Resolve the current stage's combined scalar.
pub(crate) fn first_combined(
    compiler: &CompilationSession,
    model: &Model,
    priority: ObjectivePriority,
    objectives: &[WeightedObjective],
) -> Result<CombinedStage, ObjectiveExecutionError> {
    combine_stage(compiler, model, priority, objectives)
        .map_err(|e| ObjectiveExecutionError::Combine(format!("{e:?}")))
}
