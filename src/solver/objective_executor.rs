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
};
use crate::compiler::origin::{EntityOrigin, GeneratedRole, OriginMap, OverlayId};
use crate::compiler::session::CompilationSession;
use crate::id::{ObjId, VarId};
use crate::model::{Model, Sense};
use crate::objective_policy::{
    ObjectiveLockReport, ObjectivePriority, ObjectiveValue, WeightedObjective,
};
use crate::solver::objective_combine::{combine_stage, CombinedStage};

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
    base_compilation: CompilationId,
    overlay_id: OverlayId,
    next_row: &mut u32,
    next_objective: &mut u32,
    locks: &[PriorLock],
    current: &CombinedStage,
    priority: ObjectivePriority,
) -> Result<Box<crate::solver::overlay::CompiledOverlay>, ObjectiveExecutionError> {
    let mut operations = Vec::new();
    let mut origins = OriginMap::new();

    // Prior-stage degradation rows: prior_g(x) <= bound.
    for prior in locks {
        let row_id = CompiledConstraintId(*next_row);
        *next_row += 1;
        origins.insert_constraint(
            row_id,
            EntityOrigin::SolveOverlay {
                overlay: overlay_id,
                role: GeneratedRole::ObjectiveLockRow,
            },
        );
        let rhs = prior.lock.normalized_upper_bound - prior.stage.constant;
        operations.push(crate::solver::overlay::OverlayOp::AddTemporaryRow {
            row: CompiledLinearRow {
                id: row_id,
                bounds: crate::model::ConstraintBounds::le(rhs),
                coefficients: prior.stage.coefficients.clone(),
                name: Some(format!("p31 degradation lock priority {:?}", priority)),
            },
        });
    }

    // Current stage objective: the normalized scalar, minimized.
    let objective_id = CompiledObjectiveId(*next_objective);
    *next_objective += 1;
    origins.insert_objective(
        objective_id,
        EntityOrigin::SolveOverlay {
            overlay: overlay_id,
            role: GeneratedRole::Bridge,
        },
    );
    operations.push(crate::solver::overlay::OverlayOp::AddTemporaryObjective {
        objective: CompiledObjective {
            id: objective_id,
            sense: Sense::Minimize,
            coefficients: current.coefficients.clone(),
            constant: current.constant,
            name: Some(format!("p31 combined objective priority {:?}", priority)),
        },
    });
    let policy = crate::compiler::backend_ir::CompiledObjectivePolicy::Single(objective_id);
    operations.push(crate::solver::overlay::OverlayOp::SetObjectivePolicy(
        policy,
    ));

    let compiled_id = CompilationId::allocate()
        .map_err(|_| ObjectiveExecutionError::Compile("compilation identity exhausted".into()))?;
    Ok(Box::new(crate::solver::overlay::CompiledOverlay {
        base_compilation,
        compilation_id: compiled_id,
        overlay_id,
        operations,
        origin_additions: origins,
        objective_policy_override: None,
    }))
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
