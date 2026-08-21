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
use crate::solver::SolveStatus;

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
    /// The priority of the stage that produced this lock.
    pub priority: ObjectivePriority,
    /// The full stage scalar (canonical combination + priority penalties).
    pub stage: StageScalar,
    /// The exact lock computed from that stage's `z*`.
    pub lock: ObjectiveLockReport,
}

/// One priority-targeted P30 penalty attached to a lexicographic stage.
///
/// The penalty contributes `weight * v` to the stage scalar, where `v` is the
/// generated violation variable for each finite side of the softened
/// constraint. The weight is the numerically evaluated parameterized penalty
/// weight from the current parameter snapshot.
pub(crate) struct PriorityPenalty {
    /// The evaluated finite nonnegative weight.
    pub weight: f64,
    /// Optional cap on each generated violation variable.
    pub max_violation: Option<f64>,
    /// The softened constraint's bounds (drives which sides are finite).
    pub bounds: ConstraintBounds,
    /// The constraint's row coefficients in compiled-variable order.
    pub row: Vec<(CompiledVariableId, f64)>,
}

/// The full normalized stage scalar: the canonical weighted combination plus
/// every priority-targeted penalty folded into this priority level.
///
/// This single value is what the backend minimizes, what is reported as
/// `scalar_stage_value`, and what later stages degrade against. Prior stages
/// re-materialize their penalties into each later overlay so every
/// degradation lock always includes them.
pub(crate) struct StageScalar {
    /// The canonical weighted objective combination for this level.
    pub canonical: CombinedStage,
    /// Priority penalties folded into this level.
    pub penalties: Vec<PriorityPenalty>,
}

/// Build the solve-scoped overlay for one stage.
///
/// `locks` carries every prior stage's normalized scalar and its exact
/// degradation bound (all remain active). `current` is the stage to optimize.
pub(crate) fn build_stage_overlay(
    base_compilation: CompilationId,
    overlay_id: OverlayId,
    assembly: &mut OverlayAssembly,
    locks: &[PriorLock],
    current: &StageScalar,
    priority: ObjectivePriority,
) -> Result<Box<crate::solver::overlay::CompiledOverlay>, ObjectiveExecutionError> {
    // Backend objective: canonical coefficients + this stage's priority
    // penalties. Every penalty violation variable/row is materialized in this
    // overlay before the objective is emitted.
    let mut objective_coefficients = current.canonical.coefficients.clone();
    materialize_penalties(
        priority,
        overlay_id,
        assembly,
        &current.penalties,
        &mut objective_coefficients,
    )?;

    // Prior-stage degradation locks: each later stage re-materializes the
    // prior stage's penalties into THIS overlay so the lock row references
    // valid violation variables and therefore includes the penalties.
    for prior in locks {
        let mut lock_coefficients = prior.stage.canonical.coefficients.clone();
        materialize_penalties(
            prior.priority,
            overlay_id,
            assembly,
            &prior.stage.penalties,
            &mut lock_coefficients,
        )?;
        let row_id = CompiledConstraintId(assembly.next_row);
        assembly.next_row += 1;
        assembly.origins.insert_constraint(
            row_id,
            EntityOrigin::SolveOverlay {
                overlay: overlay_id,
                role: GeneratedRole::ObjectiveLockRow,
            },
        );
        let rhs = prior.lock.normalized_upper_bound - prior.stage.canonical.constant;
        assembly.operations.push(OverlayOp::AddTemporaryRow {
            row: CompiledLinearRow {
                id: row_id,
                bounds: ConstraintBounds::le(rhs),
                coefficients: lock_coefficients,
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
            constant: current.canonical.constant,
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

/// Fold a stage's priority penalties into an output coefficient list by
/// materializing each penalty's violation variables/rows in the given overlay.
fn materialize_penalties(
    priority: ObjectivePriority,
    overlay_id: OverlayId,
    assembly: &mut OverlayAssembly,
    penalties: &[PriorityPenalty],
    out_coefficients: &mut Vec<(CompiledVariableId, f64)>,
) -> Result<(), ObjectiveExecutionError> {
    for penalty in penalties {
        for side in violation_sides(penalty.bounds) {
            let violation = add_violation_variable(assembly, overlay_id, penalty.max_violation);
            out_coefficients.push((violation, penalty.weight));
            let row_id = CompiledConstraintId(assembly.next_row);
            assembly.next_row += 1;
            assembly.origins.insert_constraint(
                row_id,
                EntityOrigin::SolveOverlay {
                    overlay: overlay_id,
                    role: GeneratedRole::FeasibilityRelaxationViolationRow,
                },
            );
            let mut coefficients = penalty.row.clone();
            coefficients.push((violation, side_sign(side)));
            coefficients.sort_by_key(|(id, _)| *id);
            assembly.operations.push(OverlayOp::AddTemporaryRow {
                row: CompiledLinearRow {
                    id: row_id,
                    bounds: side_bounds(penalty.bounds, side),
                    coefficients,
                    name: Some(format!("p31 priority penalty row {:?}", priority)),
                },
            });
        }
    }
    Ok(())
}

/// Build the full stage scalar (canonical combination + resolved priority
/// penalties) for one level from the current parameter snapshot.
pub(crate) fn stage_scalar(
    compiler: &CompilationSession,
    model: &Model,
    priority: ObjectivePriority,
    objectives: &[WeightedObjective],
) -> Result<StageScalar, ObjectiveExecutionError> {
    let canonical = combine_stage(compiler, model, priority, objectives)
        .map_err(|e| ObjectiveExecutionError::Combine(format!("{e:?}")))?;
    let penalties = resolve_priority_penalties(model, compiler, priority)?;
    Ok(StageScalar {
        canonical,
        penalties,
    })
}

/// Enumerate and numerically resolve every priority-targeted penalty for one
/// level from the current parameter snapshot, before any backend mutation.
fn resolve_priority_penalties(
    model: &Model,
    compiler: &CompilationSession,
    priority: ObjectivePriority,
) -> Result<Vec<PriorityPenalty>, ObjectiveExecutionError> {
    let snapshot = model
        .take_snapshot()
        .map_err(|e| ObjectiveExecutionError::Compile(format!("snapshot failed: {e}")))?;
    let mut out = Vec::new();
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
        let row = constraint_coefficients(&snapshot, compiler, payload.original_constraint)?;
        out.push(PriorityPenalty {
            weight,
            max_violation: payload.violation.max_violation,
            bounds: constraint.bounds,
            row,
        });
    }
    Ok(out)
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

/// Evaluate the full stage scalar (canonical + priority penalties) at a
/// solution's canonical variable values.
///
/// Priority penalties are evaluated as a pure function of the solution:
/// `weight * max(0, lower - expr)` for a lower-side violation and
/// `weight * max(0, expr - upper)` for an upper-side violation, using the
/// softened constraint's row coefficients resolved against the solution.
pub(crate) fn evaluate_stage_scalar(
    compiler: &CompilationSession,
    scalar: &StageScalar,
    values: &[(VarId, f64)],
) -> Result<f64, ObjectiveExecutionError> {
    let mut value = evaluate_combined(compiler, &scalar.canonical, values)?;
    for penalty in &scalar.penalties {
        let expr = evaluate_constraint_row(compiler, &penalty.row, values)?;
        for side in violation_sides(penalty.bounds) {
            let violation = match side {
                BoundSide::Lower => (penalty.bounds.lower - expr).max(0.0),
                BoundSide::Upper => (expr - penalty.bounds.upper).max(0.0),
            };
            value += penalty.weight * violation;
        }
    }
    finite_or_numerical(value)
}

/// Evaluate a constraint row (compiled coefficients) at canonical values.
fn evaluate_constraint_row(
    compiler: &CompilationSession,
    coefficients: &[(CompiledVariableId, f64)],
    values: &[(VarId, f64)],
) -> Result<f64, ObjectiveExecutionError> {
    let mut value = 0.0;
    for (cid, coef) in coefficients {
        if let Some(var) = compiler.user_variable(*cid) {
            let v = lookup_required(values, var, "objective penalty row")?;
            if !v.is_finite() || !coef.is_finite() {
                return Err(ObjectiveExecutionError::Numerical(
                    "solver-derived value or coefficient is not finite in an objective penalty row"
                        .into(),
                ));
            }
            value += coef * v;
        }
    }
    finite_or_numerical(value)
}

/// Evaluate a combined scalar stage at a solution's variable values.
pub(crate) fn evaluate_combined(
    compiler: &CompilationSession,
    combined: &CombinedStage,
    values: &[(VarId, f64)],
) -> Result<f64, ObjectiveExecutionError> {
    let mut value = combined.constant;
    for (cid, coef) in &combined.coefficients {
        if let Some(var) = compiler.user_variable(*cid) {
            let v = lookup_required(values, var, "stage scalar")?;
            if !v.is_finite() || !coef.is_finite() {
                return Err(ObjectiveExecutionError::Numerical(
                    "solver-derived value or coefficient is not finite in a stage scalar".into(),
                ));
            }
            value += coef * v;
        }
    }
    finite_or_numerical(value)
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
            let v = lookup_required(values, var, "objective")?;
            if !v.is_finite() || !coef.is_finite() {
                return Err(ObjectiveExecutionError::Numerical(
                    "solver-derived value or coefficient is not finite in an objective".into(),
                ));
            }
            value += coef * v;
        }
    }
    let _ = model.objective_sense(objective);
    finite_or_numerical(value)
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

/// Look up a required user primal value, rejecting a missing value with a
/// typed [`Numerical`] error rather than silently treating it as zero.
///
/// `SolveSolution.variable_values` is not guaranteed by the backend contract
/// to be complete: a missing required user variable is an extraction defect
/// that must never be silently folded into an objective/scalar as zero.
fn lookup_required(
    values: &[(VarId, f64)],
    var: VarId,
    context: &str,
) -> Result<f64, ObjectiveExecutionError> {
    lookup(values, var).ok_or_else(|| {
        ObjectiveExecutionError::Numerical(format!(
            "required user primal value for variable {var:?} is missing while evaluating {context}"
        ))
    })
}

/// Reject non-finite accumulated numeric evidence with a typed [`Numerical`]
/// error rather than letting a `NaN`/infinity reach a lock assertion.
fn finite_or_numerical(value: f64) -> Result<f64, ObjectiveExecutionError> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(ObjectiveExecutionError::Numerical(
            "stage scalar is not finite (NaN or infinity)".into(),
        ))
    }
}

/// Map a backend termination status into a [`SolveStatus`] for the objective
/// executor, preserving every legitimate mathematical outcome.
///
/// The general [`SolveStatus::from_termination`] treats both `Error` and
/// `Unknown` as uninterpretable (API-03.3). For a staged lexicographic solve,
/// `Unknown` is a real solver outcome that must flow through continuation
/// classification (`StopNotOptimal` under `RequireOptimal`, `StopUnknown`
/// under `BestFeasible`) and be recorded in the stage result — never silently
/// forced into a `Backend` error. Only `Error` is an operational failure.
pub(crate) fn objective_status_from_termination(
    termination: crate::solver::backend::TerminationStatus,
) -> SolveStatus {
    use crate::solver::backend::TerminationStatus::{
        Error, Feasible, Infeasible, InfeasibleOrUnbounded, Interrupted, IterationLimit, NodeLimit,
        NumericalIssue, Optimal, TimeLimit, Unbounded, Unknown,
    };
    match termination {
        Optimal => SolveStatus::Optimal,
        Feasible => SolveStatus::Feasible,
        Infeasible => SolveStatus::Infeasible,
        Unbounded => SolveStatus::Unbounded,
        InfeasibleOrUnbounded => SolveStatus::InfeasibleOrUnbounded,
        TimeLimit => SolveStatus::TimeLimit,
        IterationLimit => SolveStatus::IterationLimit,
        NodeLimit => SolveStatus::NodeLimit,
        Interrupted => SolveStatus::Interrupted,
        NumericalIssue => SolveStatus::Numerical,
        // `Error` is an operational failure; the caller rejects it below.
        Error => SolveStatus::Error,
        Unknown => SolveStatus::Unknown,
    }
}

#[cfg(test)]
mod finiteness_tests {
    use super::*;
    use crate::compiler::capability::{
        BackendCapabilitySet, BackendFeature, CompilationPolicy, FeatureSupport, SupportLevel,
    };
    use crate::model::variable::continuous;
    use crate::objective_policy::{ObjectivePriority, WeightedObjective};

    fn full_capabilities() -> BackendCapabilitySet {
        let mut set = BackendCapabilitySet::new();
        for feature in [
            BackendFeature::Lp,
            BackendFeature::Mip,
            BackendFeature::IncrementalBounds,
            BackendFeature::IncrementalRows,
            BackendFeature::IncrementalCoefficients,
        ] {
            set.set(
                feature,
                FeatureSupport {
                    level: SupportLevel::Native,
                    limitations: Default::default(),
                },
            );
        }
        set
    }

    fn compiled_minimize_x() -> (CompilationSession, Model, VarId, ObjId) {
        let mut model = Model::new();
        let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
        let obj = model.minimize(x).unwrap();
        let mut compiler = CompilationSession::new();
        let snapshot = model.take_snapshot().expect("snapshot of committed model");
        compiler
            .compile_snapshot(
                model.instance(),
                &snapshot,
                &CompilationPolicy::Auto,
                &full_capabilities(),
            )
            .expect("simple minimize-x model compiles");
        (compiler, model, x, obj)
    }

    #[test]
    fn stage_scalar_rejects_non_finite_candidate_values() {
        let (compiler, model, x, obj) = compiled_minimize_x();
        let objectives = vec![WeightedObjective {
            objective: obj,
            weight: 1.0,
        }];
        let scalar = stage_scalar(&compiler, &model, ObjectivePriority::new(0), &objectives)
            .expect("canonical stage scalar resolves");
        // Confirm the scalar actually references x (coefficient 1.0), so the
        // finite check is on the candidate value, not an empty row.
        assert!(!scalar.canonical.coefficients.is_empty());

        // Non-finite solver-derived values must be typed Numerical errors.
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let err = evaluate_stage_scalar(&compiler, &scalar, &[(x, bad)]).unwrap_err();
            assert!(
                matches!(err, ObjectiveExecutionError::Numerical(_)),
                "expected Numerical error for {bad}, got {err:?}"
            );
        }

        // A finite candidate evaluates normally (minimize x => scalar = x).
        assert_eq!(
            evaluate_stage_scalar(&compiler, &scalar, &[(x, 2.0)]).unwrap(),
            2.0
        );
    }

    /// `SolveSolution.variable_values` is not guaranteed by the backend
    /// contract to be complete. A missing required user primal value must be a
    /// typed extraction error, never silently folded in as zero (review
    /// #4989892030, P1 partial-primal).
    #[test]
    fn stage_scalar_rejects_missing_required_user_primal() {
        let (compiler, model, _x, obj) = compiled_minimize_x();
        let objectives = vec![WeightedObjective {
            objective: obj,
            weight: 1.0,
        }];
        let scalar = stage_scalar(&compiler, &model, ObjectivePriority::new(0), &objectives)
            .expect("canonical stage scalar resolves");
        assert!(!scalar.canonical.coefficients.is_empty());

        // The objective references x, which is absent from the provided
        // (empty) primal values: this must error, not evaluate as 0.
        let err = evaluate_stage_scalar(&compiler, &scalar, &[]).unwrap_err();
        assert!(
            matches!(err, ObjectiveExecutionError::Numerical(_)),
            "expected a Numerical missing-primal error, got {err:?}"
        );

        // The same applies to the raw objective evaluator used for per-objective
        // reporting and the final-point vector.
        let err = evaluate_objective(&compiler, &model, obj, &[]).unwrap_err();
        assert!(
            matches!(err, ObjectiveExecutionError::Numerical(_)),
            "expected a Numerical missing-primal error, got {err:?}"
        );
    }
}
