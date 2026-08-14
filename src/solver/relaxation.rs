//! Solve-scoped feasibility relaxation contract (P30, D-05 through D-08).
//!
//! Persistent softening and feasibility repair are deliberately separate. The
//! types in this module describe one weighted-L1 repair attempt; they do not
//! add objective-priority or lexicographic semantics owned by P31.

use std::collections::BTreeMap;

use crate::compiler::backend_ir::CompilationId;
use crate::compiler::backend_ir::{
    CompiledConstraintId, CompiledLinearRow, CompiledObjective, CompiledObjectiveId,
    CompiledVariable, CompiledVariableId,
};
use crate::compiler::origin::{EntityOrigin, GeneratedRole, OriginMap, OverlayId};
use crate::compiler::session::CompilationSession;
use crate::id::{ConId, VarId};
use crate::identity::{ModelInstanceId, ModelLineageId};
use crate::model::{Constraint, Objective, Variable};
use crate::revision::ModelRevision;
use crate::solution::Solution;
use crate::solver::backend::TerminationStatus;
use crate::solver::infeasibility::BoundSide;
use crate::solver::infeasibility::{ConflictOrigin, InfeasibilityReport};
use crate::solver::overlay::{CompiledOverlay, OverlayOp};
use crate::solver::SolveStatus;

/// Which canonical restriction sides may be relaxed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RelaxationRestriction {
    /// One side of an original primitive constraint.
    ConstraintSide {
        /// Original constraint.
        constraint: Constraint,
        /// Side to relax.
        side: BoundSide,
    },
    /// One declared variable-bound side.
    VariableBound {
        /// Variable whose declared bound is relaxed.
        variable: Variable,
        /// Side to relax.
        side: BoundSide,
    },
    /// A persistent fixing atom.
    PersistentFixing {
        /// Fixed variable.
        variable: Variable,
    },
}

/// Scope of eligible relaxation restrictions.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum RelaxationScope {
    /// Include every eligible finite constraint side and variable bound.
    #[default]
    AllEligible,
    /// Include exactly the listed restrictions.
    Explicit(Vec<RelaxationRestriction>),
}

/// Objective used by the portable repair formulation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum RelaxationObjective {
    /// Minimize the sum of evaluated weights times violation magnitudes.
    #[default]
    WeightedL1,
}

/// Provider selection policy for repair.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum RelaxationProviderPolicy {
    /// Force the portable compiled-row formulation.
    #[default]
    PortableOnly,
    /// Use a qualified native provider, otherwise record a portable fallback.
    PreferNative,
    /// Reject before synchronization when qualified native support is absent.
    NativeRequired,
}

/// Whether an unproven feasible incumbent may be accepted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum RelaxationAcceptance {
    /// Require a proof of repair optimality.
    #[default]
    RequireOptimal,
    /// Accept any valid feasible repair when optimality is not proven.
    AcceptFeasible,
}

/// Provider that actually executed a repair.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RelaxationExecutionProvider {
    /// Solver-neutral compiled-row provider.
    PortableRoml,
    /// Qualified native provider (reserved for a later qualified backend).
    Native {
        /// Backend family.
        backend: String,
        /// Qualified backend version.
        version: String,
    },
}

/// Why a repair result is not accepted as an optimal/feasible outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelaxationUnknownReason {
    /// A time limit interrupted proof.
    TimeLimit,
    /// An iteration/node limit interrupted proof.
    IterationLimit,
    /// Numerical evidence was insufficient.
    Numerical,
    /// The provider was interrupted.
    Interrupted,
    /// No more specific classification is honest.
    Unclassified,
}

/// Mathematical classification of a repair attempt.
#[derive(Clone, Debug, PartialEq)]
pub enum RelaxationOutcome {
    /// A feasible repair with a proof of minimum weighted-L1 objective.
    OptimalRepair,
    /// A feasible repair accepted under `AcceptFeasible` without proof of optimality.
    FeasibleRepair,
    /// The permitted finite relaxation was proven infeasible.
    NoRepairFound,
    /// No accepted repair and no proof of infeasibility.
    Unknown(RelaxationUnknownReason),
}

/// Numeric evidence reported by the provider.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RelaxationNumerics {
    /// Objective value, if reported.
    pub objective_value: Option<f64>,
    /// Best bound, if reported.
    pub best_bound: Option<f64>,
    /// Absolute gap, if reported.
    pub absolute_gap: Option<f64>,
    /// Relative gap, if reported.
    pub relative_gap: Option<f64>,
    /// Feasibility tolerance, if reported.
    pub feasibility_tolerance: Option<f64>,
    /// Integrality tolerance, if reported.
    pub integrality_tolerance: Option<f64>,
}

/// One restriction's evaluated repair contribution.
#[derive(Clone, Debug, PartialEq)]
pub struct RelaxedRestriction {
    /// Original semantic restriction.
    pub restriction: RelaxationRestriction,
    /// Raw violation magnitude.
    pub violation: f64,
    /// Evaluated nonnegative weight.
    pub evaluated_weight: f64,
    /// Weight times violation.
    pub weighted_violation: f64,
    /// Imported/source provenance, when the restriction came from P29/P35.
    pub source_provenance: Option<String>,
}

/// A P29 restriction selected for repair, retaining imported/source-aware
/// declaration metadata before it enters the solve-scoped overlay.
#[derive(Clone, Debug, PartialEq)]
pub struct RelaxationMappedRestriction {
    /// Supported portable restriction.
    pub restriction: RelaxationRestriction,
    /// Source row/bound identity from the conflict declaration, when present.
    pub source_provenance: Option<String>,
}

/// Identity and provider metadata for one repair report.
#[derive(Clone, Debug, PartialEq)]
pub struct RelaxationMetadata {
    /// Model lineage.
    pub model_lineage: ModelLineageId,
    /// Exact model instance.
    pub model_instance: ModelInstanceId,
    /// Canonical model revision.
    pub model_revision: ModelRevision,
    /// Base compiled identity.
    pub base_compilation_id: CompilationId,
    /// Solve-scoped relaxation compiled identity.
    pub relaxation_compilation_id: CompilationId,
    /// Provider selected.
    pub provider: RelaxationExecutionProvider,
    /// Provider termination classification.
    pub termination: SolveStatus,
    /// Numeric evidence.
    pub numerics: RelaxationNumerics,
    /// Explicit portable fallback/provider decision reason.
    pub provider_reason: Option<String>,
}

/// Report returned after a successful mathematical repair attempt.
#[derive(Clone, Debug, PartialEq)]
pub struct FeasibilityRelaxationReport {
    /// Frozen mathematical outcome.
    pub outcome: RelaxationOutcome,
    /// The solve-scoped candidate, retained only as immutable report data.
    pub solution: Solution,
    /// Per-restriction values.
    pub members: Vec<RelaxedRestriction>,
    /// Sum of weighted violations.
    pub total_weighted_violation: f64,
    /// Identity/provider/numerical evidence.
    pub metadata: RelaxationMetadata,
}

/// Public plan for one solve-scoped weighted-L1 repair.
#[derive(Clone, Debug, PartialEq)]
pub struct FeasibilityRelaxationPlan {
    /// Eligible restriction scope.
    pub scope: RelaxationScope,
    /// Repair objective.
    pub objective: RelaxationObjective,
    /// Provider selection.
    pub provider_policy: RelaxationProviderPolicy,
    /// Acceptance rule.
    pub acceptance: RelaxationAcceptance,
}

impl Default for FeasibilityRelaxationPlan {
    fn default() -> Self {
        Self {
            scope: RelaxationScope::AllEligible,
            objective: RelaxationObjective::WeightedL1,
            provider_policy: RelaxationProviderPolicy::PortableOnly,
            acceptance: RelaxationAcceptance::RequireOptimal,
        }
    }
}

/// Typed operational failures. Mathematical outcomes are never used for
/// preflight, backend, extraction, rollback, or rebuild failures.
#[derive(Clone, Debug, PartialEq)]
pub enum FeasibilityRelaxationError {
    /// Invalid request or stale canonical identity before mutation.
    Preflight(String),
    /// No qualified native provider exists under `NativeRequired`.
    NativeProviderRequired,
    /// A P29 member belongs to a semantic origin P30 does not relax.
    UnsupportedOrigin(String),
    /// A P29 report was not produced for the exact model/base requested.
    StaleIdentity(String),
    /// Compiler/overlay construction failed before backend mutation.
    Compile(String),
    /// Backend apply/solve/extraction failure.
    Backend(String),
    /// Primary failure plus cleanup/rebuild failure; backend must be rebuilt.
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

impl std::fmt::Display for FeasibilityRelaxationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for FeasibilityRelaxationError {}

/// Convert a complete P29 report into P30 restrictions without filtering.
///
/// The identity tuple is checked before any member is converted. If one
/// member has an unsupported semantic origin, the entire conversion fails;
/// callers never receive a silently narrowed repair scope.
pub fn map_p29_members(
    report: &InfeasibilityReport,
    model_instance: crate::identity::ModelInstanceId,
    model_revision: ModelRevision,
    compilation_id: CompilationId,
) -> Result<Vec<RelaxationMappedRestriction>, FeasibilityRelaxationError> {
    if report.model_instance != model_instance
        || report.model_revision != model_revision
        || report.compilation_id != compilation_id
    {
        return Err(FeasibilityRelaxationError::StaleIdentity(format!(
            "P29 report identity ({:?}, {}, {:?}) does not match requested ({:?}, {}, {:?})",
            report.model_instance,
            report.model_revision,
            report.compilation_id,
            model_instance,
            model_revision,
            compilation_id
        )));
    }
    report
        .members
        .iter()
        .map(|member| {
            let restriction = match &member.declaration.origin {
                ConflictOrigin::ConstraintSide { constraint, side } => {
                    RelaxationRestriction::ConstraintSide {
                        constraint: *constraint,
                        side: *side,
                    }
                }
                ConflictOrigin::VariableBound { variable, side } => {
                    RelaxationRestriction::VariableBound {
                        variable: *variable,
                        side: *side,
                    }
                }
                ConflictOrigin::PersistentFixing { variable } => {
                    RelaxationRestriction::PersistentFixing {
                        variable: *variable,
                    }
                }
                ref unsupported => {
                    return Err(FeasibilityRelaxationError::UnsupportedOrigin(format!(
                        "P29 conflict member {:?} is not a P30 portable restriction",
                        unsupported
                    )))
                }
            };
            Ok(RelaxationMappedRestriction {
                restriction,
                source_provenance: member.declaration.name.clone(),
            })
        })
        .collect()
}

/// Convert a regular solve status into the frozen unknown reason.
pub(crate) fn unknown_reason(status: SolveStatus) -> RelaxationUnknownReason {
    match status {
        SolveStatus::TimeLimit => RelaxationUnknownReason::TimeLimit,
        SolveStatus::IterationLimit | SolveStatus::NodeLimit => {
            RelaxationUnknownReason::IterationLimit
        }
        SolveStatus::Numerical => RelaxationUnknownReason::Numerical,
        SolveStatus::Interrupted => RelaxationUnknownReason::Interrupted,
        _ => RelaxationUnknownReason::Unclassified,
    }
}

/// Compile the portable weighted-L1 overlay against the exact current base.
/// This is read-only over canonical model state and allocates only solve-scoped
/// compiled identities.
pub(crate) fn compile_portable_overlay(
    model: &crate::model::Model,
    compiler: &CompilationSession,
    plan: &FeasibilityRelaxationPlan,
) -> Result<(CompiledOverlay, Vec<(RelaxationRestriction, f64)>), FeasibilityRelaxationError> {
    if plan.objective != RelaxationObjective::WeightedL1 {
        return Err(FeasibilityRelaxationError::Preflight(
            "P30 supports only WeightedL1".into(),
        ));
    }
    let base = compiler.current_compilation().ok_or_else(|| {
        FeasibilityRelaxationError::Preflight("no exact base compilation exists".into())
    })?;
    let snapshot = model
        .take_snapshot()
        .map_err(|error| FeasibilityRelaxationError::Preflight(error.to_string()))?;
    let mut restrictions = match &plan.scope {
        RelaxationScope::Explicit(items) => items.clone(),
        RelaxationScope::AllEligible => {
            let mut items = Vec::new();
            for constraint in &snapshot.constraints {
                if !constraint.active {
                    continue;
                }
                if constraint.bounds.lower.is_finite() {
                    items.push(RelaxationRestriction::ConstraintSide {
                        constraint: constraint.id,
                        side: BoundSide::Lower,
                    });
                }
                if constraint.bounds.upper.is_finite() {
                    items.push(RelaxationRestriction::ConstraintSide {
                        constraint: constraint.id,
                        side: BoundSide::Upper,
                    });
                }
            }
            for variable in &snapshot.variables {
                if !variable.active {
                    continue;
                }
                if variable.fixing.is_some() {
                    items.push(RelaxationRestriction::PersistentFixing {
                        variable: variable.id,
                    });
                } else {
                    if variable.bounds.lower.is_finite() {
                        items.push(RelaxationRestriction::VariableBound {
                            variable: variable.id,
                            side: BoundSide::Lower,
                        });
                    }
                    if variable.bounds.upper.is_finite() {
                        items.push(RelaxationRestriction::VariableBound {
                            variable: variable.id,
                            side: BoundSide::Upper,
                        });
                    }
                }
            }
            items
        }
    };
    restrictions.sort_by_key(restriction_key);
    restrictions.dedup();
    if restrictions.is_empty() {
        return Err(FeasibilityRelaxationError::Preflight(
            "relaxation scope contains no finite eligible restrictions".into(),
        ));
    }

    let overlay_id = OverlayId::allocate()
        .map_err(|_| FeasibilityRelaxationError::Preflight("overlay identity exhausted".into()))?;
    let relaxation_compilation_id = CompilationId::allocate().map_err(|_| {
        FeasibilityRelaxationError::Preflight("compilation identity exhausted".into())
    })?;
    let mut next_variable = compiler.next_variable_index().ok_or_else(|| {
        FeasibilityRelaxationError::Preflight("base has no variable allocation cursor".into())
    })?;
    let mut next_row = compiler
        .next_row_index()
        .ok_or_else(|| FeasibilityRelaxationError::Preflight("base has no row cursor".into()))?;
    let next_objective = compiler.next_objective_index().ok_or_else(|| {
        FeasibilityRelaxationError::Preflight("base has no objective cursor".into())
    })?;
    let mut operations = Vec::new();
    let mut origins = OriginMap::new();
    let mut objective_coefficients = Vec::new();
    let mut evaluated_weights = Vec::new();

    for restriction in &restrictions {
        let weight = restriction_weight(model, restriction)?;
        evaluated_weights.push((restriction.clone(), weight));
        match restriction {
            RelaxationRestriction::ConstraintSide { constraint, side } => {
                let entry = snapshot
                    .constraints
                    .iter()
                    .find(|entry| entry.id == *constraint)
                    .ok_or_else(|| {
                        FeasibilityRelaxationError::Preflight(format!(
                            "unknown constraint {constraint:?}"
                        ))
                    })?;
                let row = compiler.compiled_row_id(*constraint).ok_or_else(|| {
                    FeasibilityRelaxationError::Preflight(format!(
                        "constraint {constraint:?} has no compiled row"
                    ))
                })?;
                let variable = add_violation_variable(
                    &mut operations,
                    &mut origins,
                    &mut next_variable,
                    overlay_id,
                    restriction_cap(model, restriction)?,
                );
                objective_coefficients.push((variable, weight));
                operations.push(OverlayOp::SetTemporaryRowBounds {
                    constraint: row,
                    bounds: widened_row_bounds(entry.bounds, *side),
                });
                let mut coefficients = constraint_coefficients(&snapshot, compiler, *constraint)?;
                coefficients.push((variable, side_sign(*side)));
                coefficients.sort_by_key(|(id, _)| *id);
                let generated_row = CompiledConstraintId(next_row);
                next_row += 1;
                origins.insert_constraint(
                    generated_row,
                    EntityOrigin::SolveOverlay {
                        overlay: overlay_id,
                        role: GeneratedRole::FeasibilityRelaxationViolationRow,
                    },
                );
                operations.push(OverlayOp::AddTemporaryRow {
                    row: CompiledLinearRow {
                        id: generated_row,
                        bounds: side_bounds(entry.bounds, *side),
                        coefficients,
                        name: None,
                    },
                });
            }
            RelaxationRestriction::VariableBound { variable, side } => {
                let entry = snapshot
                    .variables
                    .iter()
                    .find(|entry| entry.id == *variable)
                    .ok_or_else(|| {
                        FeasibilityRelaxationError::Preflight(format!(
                            "unknown variable {variable:?}"
                        ))
                    })?;
                let compiled_variable =
                    compiler.compiled_variable_id(*variable).ok_or_else(|| {
                        FeasibilityRelaxationError::Preflight(format!(
                            "variable {variable:?} has no compiled id"
                        ))
                    })?;
                let violation = add_violation_variable(
                    &mut operations,
                    &mut origins,
                    &mut next_variable,
                    overlay_id,
                    restriction_cap(model, restriction)?,
                );
                objective_coefficients.push((violation, weight));
                operations.push(OverlayOp::SetTemporaryVariableBounds {
                    variable: compiled_variable,
                    bounds: widened_variable_bounds(entry.bounds, *side),
                });
                let row = CompiledConstraintId(next_row);
                next_row += 1;
                origins.insert_constraint(
                    row,
                    EntityOrigin::SolveOverlay {
                        overlay: overlay_id,
                        role: GeneratedRole::FeasibilityRelaxationViolationRow,
                    },
                );
                operations.push(OverlayOp::AddTemporaryRow {
                    row: CompiledLinearRow {
                        id: row,
                        bounds: variable_side_bounds(entry.bounds, *side),
                        coefficients: vec![
                            (compiled_variable, 1.0),
                            (violation, variable_violation_sign(*side)),
                        ],
                        name: None,
                    },
                });
            }
            RelaxationRestriction::PersistentFixing { variable } => {
                let entry = snapshot
                    .variables
                    .iter()
                    .find(|entry| entry.id == *variable)
                    .ok_or_else(|| {
                        FeasibilityRelaxationError::Preflight(format!(
                            "unknown variable {variable:?}"
                        ))
                    })?;
                let fixing = entry.fixing.as_ref().ok_or_else(|| {
                    FeasibilityRelaxationError::Preflight(format!(
                        "variable {variable:?} has no persistent fixing"
                    ))
                })?;
                let compiled_variable =
                    compiler.compiled_variable_id(*variable).ok_or_else(|| {
                        FeasibilityRelaxationError::Preflight(format!(
                            "variable {variable:?} has no compiled id"
                        ))
                    })?;
                operations.push(OverlayOp::SetTemporaryVariableBounds {
                    variable: compiled_variable,
                    bounds: entry.bounds,
                });
                for side in [BoundSide::Lower, BoundSide::Upper] {
                    let violation = add_violation_variable(
                        &mut operations,
                        &mut origins,
                        &mut next_variable,
                        overlay_id,
                        restriction_cap(model, restriction)?,
                    );
                    objective_coefficients.push((violation, weight));
                    let row = CompiledConstraintId(next_row);
                    next_row += 1;
                    origins.insert_constraint(
                        row,
                        EntityOrigin::SolveOverlay {
                            overlay: overlay_id,
                            role: GeneratedRole::FeasibilityRelaxationViolationRow,
                        },
                    );
                    operations.push(OverlayOp::AddTemporaryRow {
                        row: CompiledLinearRow {
                            id: row,
                            bounds: if side == BoundSide::Lower {
                                crate::model::ConstraintBounds::ge(fixing.value)
                            } else {
                                crate::model::ConstraintBounds::le(fixing.value)
                            },
                            coefficients: vec![
                                (compiled_variable, 1.0),
                                (violation, variable_violation_sign(side)),
                            ],
                            name: None,
                        },
                    });
                }
            }
        }
    }

    let objective_id = CompiledObjectiveId(next_objective);
    origins.insert_objective(
        objective_id,
        EntityOrigin::SolveOverlay {
            overlay: overlay_id,
            // `Bridge` is the explicit solver-local role for the generated
            // weighted-L1 objective; the core compiler role set is owned by
            // another phase and cannot be extended here.
            role: GeneratedRole::Bridge,
        },
    );
    operations.push(OverlayOp::AddTemporaryObjective {
        objective: CompiledObjective {
            id: objective_id,
            sense: crate::model::Sense::Minimize,
            coefficients: objective_coefficients,
            constant: 0.0,
            name: Some("portable weighted L1 relaxation".into()),
        },
    });
    operations.push(OverlayOp::SetObjectivePolicy(
        crate::compiler::backend_ir::CompiledObjectivePolicy::Single(objective_id),
    ));
    Ok((
        CompiledOverlay {
            base_compilation: base,
            compilation_id: relaxation_compilation_id,
            overlay_id,
            operations,
            origin_additions: origins,
            objective_policy_override: None,
        },
        evaluated_weights,
    ))
}

/// Validate user-supplied relaxation scope and policy before synchronization.
pub(crate) fn validate_plan_preflight(
    model: &crate::model::Model,
    plan: &FeasibilityRelaxationPlan,
) -> Result<(), FeasibilityRelaxationError> {
    if plan.objective != RelaxationObjective::WeightedL1 {
        return Err(FeasibilityRelaxationError::Preflight(
            "P30 supports only WeightedL1".into(),
        ));
    }
    let snapshot = model
        .take_snapshot()
        .map_err(|error| FeasibilityRelaxationError::Preflight(error.to_string()))?;
    if let RelaxationScope::Explicit(restrictions) = &plan.scope {
        if restrictions.is_empty() {
            return Err(FeasibilityRelaxationError::Preflight(
                "explicit relaxation scope is empty".into(),
            ));
        }
        for restriction in restrictions {
            match restriction {
                RelaxationRestriction::ConstraintSide { constraint, side } => {
                    let entry = snapshot
                        .constraints
                        .iter()
                        .find(|entry| entry.id == *constraint)
                        .ok_or_else(|| {
                            FeasibilityRelaxationError::Preflight(format!(
                                "unknown constraint {constraint:?}"
                            ))
                        })?;
                    if !entry.active
                        || match side {
                            BoundSide::Lower => !entry.bounds.lower.is_finite(),
                            BoundSide::Upper => !entry.bounds.upper.is_finite(),
                        }
                    {
                        return Err(FeasibilityRelaxationError::Preflight(format!(
                            "constraint side {constraint:?}/{side:?} is not an active finite restriction"
                        )));
                    }
                }
                RelaxationRestriction::VariableBound { variable, side } => {
                    let entry = snapshot
                        .variables
                        .iter()
                        .find(|entry| entry.id == *variable)
                        .ok_or_else(|| {
                            FeasibilityRelaxationError::Preflight(format!(
                                "unknown variable {variable:?}"
                            ))
                        })?;
                    if !entry.active
                        || entry.fixing.is_some()
                        || match side {
                            BoundSide::Lower => !entry.bounds.lower.is_finite(),
                            BoundSide::Upper => !entry.bounds.upper.is_finite(),
                        }
                    {
                        return Err(FeasibilityRelaxationError::Preflight(format!(
                            "variable bound {variable:?}/{side:?} is not an active finite declared restriction"
                        )));
                    }
                }
                RelaxationRestriction::PersistentFixing { variable } => {
                    let entry = snapshot
                        .variables
                        .iter()
                        .find(|entry| entry.id == *variable)
                        .ok_or_else(|| {
                            FeasibilityRelaxationError::Preflight(format!(
                                "unknown variable {variable:?}"
                            ))
                        })?;
                    if !entry.active || entry.fixing.is_none() {
                        return Err(FeasibilityRelaxationError::Preflight(format!(
                            "variable {variable:?} has no active persistent fixing"
                        )));
                    }
                }
            }
            restriction_weight(model, restriction)?;
            restriction_cap(model, restriction)?;
        }
    }
    Ok(())
}

/// Extract portable repair evidence from the provider's candidate values.
pub(crate) fn report_members(
    model: &crate::model::Model,
    weighted: &[(RelaxationRestriction, f64)],
    values: &[(VarId, f64)],
) -> Result<(Vec<RelaxedRestriction>, f64), FeasibilityRelaxationError> {
    let snapshot = model
        .take_snapshot()
        .map_err(|error| FeasibilityRelaxationError::Numerical(error.to_string()))?;
    let mut candidate_values = BTreeMap::new();
    for (variable, candidate) in values {
        if !candidate.is_finite() {
            return Err(FeasibilityRelaxationError::Numerical(format!(
                "provider returned non-finite candidate value {candidate} for {variable:?}"
            )));
        }
        let entry = snapshot
            .variables
            .iter()
            .find(|entry| entry.id == *variable)
            .ok_or_else(|| {
                FeasibilityRelaxationError::Numerical(format!(
                    "provider returned a candidate for unknown variable {variable:?}"
                ))
            })?;
        if candidate_values.insert(*variable, *candidate).is_some() {
            return Err(FeasibilityRelaxationError::Numerical(format!(
                "provider returned duplicate candidate value for {variable:?}"
            )));
        }
        if !entry.active {
            continue;
        }

        let domain = model.variable_domain(*variable).ok_or_else(|| {
            FeasibilityRelaxationError::Numerical(format!(
                "candidate variable {variable:?} is stale"
            ))
        })?;
        let relax_lower = weighted.iter().any(|(restriction, _)| match restriction {
            RelaxationRestriction::VariableBound {
                variable: candidate,
                side,
            } if candidate == variable => *side == BoundSide::Lower,
            _ => false,
        });
        let relax_upper = weighted.iter().any(|(restriction, _)| match restriction {
            RelaxationRestriction::VariableBound {
                variable: candidate,
                side,
            } if candidate == variable => *side == BoundSide::Upper,
            _ => false,
        });
        let bounds = if weighted.iter().any(|(restriction, _)| {
            matches!(
                restriction,
                RelaxationRestriction::PersistentFixing { variable: candidate }
                    if candidate == variable
            )
        }) {
            domain.bounds
        } else {
            model.effective_bounds(*variable).ok_or_else(|| {
                FeasibilityRelaxationError::Numerical(format!(
                    "candidate variable {variable:?} is stale"
                ))
            })?
        };
        if (!relax_lower && *candidate < bounds.lower)
            || (!relax_upper && *candidate > bounds.upper)
        {
            return Err(FeasibilityRelaxationError::Numerical(format!(
                "candidate value {candidate} violates domain of {variable:?}"
            )));
        }
        if matches!(
            domain.var_type,
            crate::model::VarType::Integer | crate::model::VarType::Binary
        ) && (candidate - candidate.round()).abs() > model.integrality_tolerance()
        {
            return Err(FeasibilityRelaxationError::Numerical(format!(
                "candidate value {candidate} is non-integral for {variable:?}"
            )));
        }
        if let Some(nonzero_lower) = snapshot
            .variables
            .iter()
            .find(|entry| entry.id == *variable)
            .and_then(|entry| entry.semicontinuous_lower)
        {
            if *candidate != 0.0 && *candidate < nonzero_lower {
                return Err(FeasibilityRelaxationError::Numerical(format!(
                    "candidate value {candidate} violates semi-continuous domain of {variable:?}"
                )));
            }
        }
    }
    for entry in snapshot.variables.iter().filter(|entry| entry.active) {
        if !candidate_values.contains_key(&entry.id) {
            return Err(FeasibilityRelaxationError::Numerical(format!(
                "provider omitted required candidate value for {:?}",
                entry.id
            )));
        }
    }

    let tolerance = model.constants.feasibility_tolerance;
    if !tolerance.is_finite() || tolerance < 0.0 {
        return Err(FeasibilityRelaxationError::Numerical(
            "model feasibility tolerance is non-finite or negative".into(),
        ));
    }
    for entry in snapshot.constraints.iter().filter(|entry| entry.active) {
        let mut expression = 0.0;
        for cell in snapshot.cells.iter().filter(|cell| {
            matches!(
                cell.cell_key.0,
                crate::model::coefficient::CoefficientTarget::Constraint(candidate)
                    if candidate == entry.id
            )
        }) {
            let coefficient = cell.evaluated_value;
            if !coefficient.is_finite() {
                return Err(FeasibilityRelaxationError::Numerical(format!(
                    "non-finite coefficient while validating constraint {:?}",
                    entry.id
                )));
            }
            let variable = cell.cell_key.1;
            let candidate = candidate_values.get(&variable).ok_or_else(|| {
                FeasibilityRelaxationError::Numerical(format!(
                    "missing candidate value for constraint {:?} variable {:?}",
                    entry.id, variable
                ))
            })?;
            expression += coefficient * candidate;
        }
        if !expression.is_finite() {
            return Err(FeasibilityRelaxationError::Numerical(format!(
                "non-finite candidate expression for constraint {:?}",
                entry.id
            )));
        }
        let lower_relaxed = weighted.iter().any(|(restriction, _)| {
            matches!(
                restriction,
                RelaxationRestriction::ConstraintSide { constraint, side: BoundSide::Lower }
                    if *constraint == entry.id
            )
        });
        let upper_relaxed = weighted.iter().any(|(restriction, _)| {
            matches!(
                restriction,
                RelaxationRestriction::ConstraintSide { constraint, side: BoundSide::Upper }
                    if *constraint == entry.id
            )
        });
        if (!lower_relaxed
            && entry.bounds.lower.is_finite()
            && expression < entry.bounds.lower - tolerance)
            || (!upper_relaxed
                && entry.bounds.upper.is_finite()
                && expression > entry.bounds.upper + tolerance)
        {
            return Err(FeasibilityRelaxationError::Numerical(format!(
                "candidate violates non-relaxed base constraint {:?}",
                entry.id
            )));
        }
    }

    let value = |variable: VarId| {
        candidate_values.get(&variable).copied().ok_or_else(|| {
            FeasibilityRelaxationError::Numerical(format!(
                "missing candidate value for {variable:?}"
            ))
        })
    };
    let mut members = Vec::with_capacity(weighted.len());
    let mut total = 0.0;
    for (restriction, weight) in weighted {
        let violation = match restriction {
            RelaxationRestriction::ConstraintSide { constraint, side } => {
                let entry = snapshot
                    .constraints
                    .iter()
                    .find(|entry| entry.id == *constraint)
                    .ok_or_else(|| {
                        FeasibilityRelaxationError::Numerical(format!(
                            "missing constraint {constraint:?}"
                        ))
                    })?;
                let expression = snapshot
                    .cells
                    .iter()
                    .filter_map(|cell| match cell.cell_key.0 {
                        crate::model::coefficient::CoefficientTarget::Constraint(candidate)
                            if candidate == *constraint =>
                        {
                            Some((cell, cell.cell_key.1))
                        }
                        _ => None,
                    })
                    .try_fold(0.0, |sum, (cell, variable)| {
                        let candidate = value(variable)?;
                        let term = cell.evaluated_value * candidate;
                        if !term.is_finite() {
                            return Err(FeasibilityRelaxationError::Numerical(
                                "non-finite candidate constraint term".into(),
                            ));
                        }
                        Ok(sum + term)
                    })?;
                match side {
                    BoundSide::Lower => (entry.bounds.lower - expression).max(0.0),
                    BoundSide::Upper => (expression - entry.bounds.upper).max(0.0),
                }
            }
            RelaxationRestriction::VariableBound { variable, side } => {
                let entry = snapshot
                    .variables
                    .iter()
                    .find(|entry| entry.id == *variable)
                    .ok_or_else(|| {
                        FeasibilityRelaxationError::Numerical(format!(
                            "missing variable {variable:?}"
                        ))
                    })?;
                match side {
                    BoundSide::Lower => (entry.bounds.lower - value(*variable)?).max(0.0),
                    BoundSide::Upper => (value(*variable)? - entry.bounds.upper).max(0.0),
                }
            }
            RelaxationRestriction::PersistentFixing { variable } => {
                let entry = snapshot
                    .variables
                    .iter()
                    .find(|entry| entry.id == *variable)
                    .ok_or_else(|| {
                        FeasibilityRelaxationError::Numerical(format!(
                            "missing variable {variable:?}"
                        ))
                    })?;
                let fixed = entry
                    .fixing
                    .as_ref()
                    .ok_or_else(|| {
                        FeasibilityRelaxationError::Numerical(format!(
                            "missing fixing for {variable:?}"
                        ))
                    })?
                    .value;
                (value(*variable)? - fixed).abs()
            }
        };
        if !violation.is_finite() || !weight.is_finite() {
            return Err(FeasibilityRelaxationError::Numerical(
                "provider returned non-finite relaxation evidence".into(),
            ));
        }
        if let Some(cap) = restriction_cap(model, restriction).map_err(|error| {
            FeasibilityRelaxationError::Numerical(format!(
                "invalid temporary violation cap: {error}"
            ))
        })? {
            if violation > cap + tolerance {
                return Err(FeasibilityRelaxationError::Numerical(format!(
                    "candidate violation {violation} exceeds temporary row cap {cap}"
                )));
            }
        }
        let weighted_violation = violation * *weight;
        total += weighted_violation;
        members.push(RelaxedRestriction {
            restriction: restriction.clone(),
            violation,
            evaluated_weight: *weight,
            weighted_violation,
            source_provenance: None,
        });
    }
    if !total.is_finite() {
        return Err(FeasibilityRelaxationError::Numerical(
            "weighted relaxation total is non-finite".into(),
        ));
    }
    Ok((members, total))
}

fn restriction_key(restriction: &RelaxationRestriction) -> (u8, u32, u32, u8) {
    match restriction {
        RelaxationRestriction::ConstraintSide { constraint, side } => (
            0,
            constraint.index(),
            constraint.generation().value(),
            side_key(*side),
        ),
        RelaxationRestriction::VariableBound { variable, side } => (
            1,
            variable.index(),
            variable.generation().value(),
            side_key(*side),
        ),
        RelaxationRestriction::PersistentFixing { variable } => {
            (2, variable.index(), variable.generation().value(), 0)
        }
    }
}

fn side_key(side: BoundSide) -> u8 {
    match side {
        BoundSide::Lower => 0,
        BoundSide::Upper => 1,
    }
}

fn side_sign(side: BoundSide) -> f64 {
    match side {
        BoundSide::Lower => 1.0,
        BoundSide::Upper => -1.0,
    }
}

fn variable_violation_sign(side: BoundSide) -> f64 {
    match side {
        BoundSide::Lower => 1.0,
        BoundSide::Upper => -1.0,
    }
}

fn side_bounds(
    bounds: crate::model::ConstraintBounds,
    side: BoundSide,
) -> crate::model::ConstraintBounds {
    match side {
        BoundSide::Lower => crate::model::ConstraintBounds::ge(bounds.lower),
        BoundSide::Upper => crate::model::ConstraintBounds::le(bounds.upper),
    }
}

fn variable_side_bounds(
    bounds: crate::model::Bounds,
    side: BoundSide,
) -> crate::model::ConstraintBounds {
    match side {
        BoundSide::Lower => crate::model::ConstraintBounds::ge(bounds.lower),
        BoundSide::Upper => crate::model::ConstraintBounds::le(bounds.upper),
    }
}

fn widened_row_bounds(
    bounds: crate::model::ConstraintBounds,
    side: BoundSide,
) -> crate::model::ConstraintBounds {
    match side {
        BoundSide::Lower => crate::model::ConstraintBounds::range(f64::NEG_INFINITY, bounds.upper),
        BoundSide::Upper => crate::model::ConstraintBounds::range(bounds.lower, f64::INFINITY),
    }
}

fn widened_variable_bounds(bounds: crate::model::Bounds, side: BoundSide) -> crate::model::Bounds {
    match side {
        BoundSide::Lower => crate::model::Bounds::new(f64::NEG_INFINITY, bounds.upper),
        BoundSide::Upper => crate::model::Bounds::new(bounds.lower, f64::INFINITY),
    }
}

fn add_violation_variable(
    operations: &mut Vec<OverlayOp>,
    origins: &mut OriginMap,
    next_variable: &mut u32,
    overlay: OverlayId,
    max_violation: Option<f64>,
) -> CompiledVariableId {
    let variable = CompiledVariableId(*next_variable);
    *next_variable += 1;
    origins.insert_variable(
        variable,
        EntityOrigin::SolveOverlay {
            overlay,
            role: GeneratedRole::FeasibilityRelaxationViolationVariable,
        },
    );
    operations.push(OverlayOp::AddTemporaryVariable {
        variable: CompiledVariable {
            id: variable,
            bounds: crate::model::Bounds::new(0.0, max_violation.unwrap_or(f64::INFINITY)),
            var_type: crate::model::VarType::Continuous,
            name: Some("portable relaxation violation".into()),
        },
    });
    variable
}

fn restriction_cap(
    model: &crate::model::Model,
    restriction: &RelaxationRestriction,
) -> Result<Option<f64>, FeasibilityRelaxationError> {
    let cap = match restriction {
        RelaxationRestriction::ConstraintSide { constraint, .. } => model
            .take_snapshot()
            .map_err(|e| FeasibilityRelaxationError::Preflight(e.to_string()))?
            .constructs
            .iter()
            .find_map(|entry| match &entry.kind {
                crate::construct::ConstructKind::SoftConstraint(payload)
                    if payload.original_constraint == *constraint =>
                {
                    Some(payload.violation.max_violation)
                }
                _ => None,
            })
            .flatten(),
        _ => None,
    };
    if let Some(cap) = cap {
        if !cap.is_finite() || cap < 0.0 {
            return Err(FeasibilityRelaxationError::Preflight(format!(
                "evaluated relaxation cap must be finite and nonnegative, got {cap}"
            )));
        }
    }
    Ok(cap)
}

fn constraint_coefficients(
    snapshot: &crate::snapshot::ModelSnapshot,
    compiler: &CompilationSession,
    constraint: ConId,
) -> Result<Vec<(CompiledVariableId, f64)>, FeasibilityRelaxationError> {
    let mut coefficients = Vec::new();
    for cell in &snapshot.cells {
        if let crate::model::coefficient::CoefficientTarget::Constraint(candidate) = cell.cell_key.0
        {
            if candidate == constraint {
                let variable = compiler
                    .compiled_variable_id(cell.cell_key.1)
                    .ok_or_else(|| {
                        FeasibilityRelaxationError::Preflight(format!(
                            "constraint references missing variable {:?}",
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

fn restriction_weight(
    model: &crate::model::Model,
    restriction: &RelaxationRestriction,
) -> Result<f64, FeasibilityRelaxationError> {
    let weight = match restriction {
        RelaxationRestriction::ConstraintSide { constraint, .. } => model
            .take_snapshot()
            .map_err(|e| FeasibilityRelaxationError::Preflight(e.to_string()))?
            .constructs
            .iter()
            .find_map(|entry| match &entry.kind {
                crate::construct::ConstructKind::SoftConstraint(payload)
                    if payload.original_constraint == *constraint =>
                {
                    Some(
                        payload
                            .penalty
                            .weight
                            .eval_checked(|parameter| {
                                model.parameter_value(parameter).ok_or(parameter)
                            })
                            .map_err(|parameter| {
                                FeasibilityRelaxationError::Preflight(format!(
                                    "missing weight parameter {parameter:?}"
                                ))
                            }),
                    )
                }
                _ => None,
            })
            .transpose()?
            .unwrap_or(1.0),
        _ => 1.0,
    };
    if !weight.is_finite() || weight < 0.0 {
        return Err(FeasibilityRelaxationError::Preflight(format!(
            "evaluated relaxation weight must be finite and nonnegative, got {weight}"
        )));
    }
    Ok(weight)
}

#[allow(dead_code)]
fn _termination_status(status: TerminationStatus) -> SolveStatus {
    match status {
        TerminationStatus::Optimal => SolveStatus::Optimal,
        TerminationStatus::Feasible => SolveStatus::Feasible,
        TerminationStatus::Infeasible => SolveStatus::Infeasible,
        TerminationStatus::Unbounded => SolveStatus::Unbounded,
        TerminationStatus::InfeasibleOrUnbounded => SolveStatus::InfeasibleOrUnbounded,
        TerminationStatus::TimeLimit => SolveStatus::TimeLimit,
        TerminationStatus::IterationLimit => SolveStatus::IterationLimit,
        TerminationStatus::NodeLimit => SolveStatus::NodeLimit,
        TerminationStatus::Interrupted => SolveStatus::Interrupted,
        TerminationStatus::NumericalIssue => SolveStatus::Numerical,
        TerminationStatus::Unknown => SolveStatus::Unknown,
        TerminationStatus::Error => SolveStatus::Error,
    }
}

// Keep these aliases in the module for implementation code and external
// callers that prefer the semantic names while retaining the existing id
// aliases in the core model.
#[allow(dead_code)]
fn _typed_ids(_: ConId, _: VarId, _: Objective) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr::ConstraintExprExt;
    use crate::model::{continuous, Model};

    #[test]
    fn report_members_rejects_missing_candidate_values() {
        let mut model = Model::new();
        let x = model.add_variable(continuous().bounds(0.0, 1.0)).unwrap();
        let constraint = model.add_constraint(x.ge(1.0)).unwrap();
        let weighted = vec![(
            RelaxationRestriction::ConstraintSide {
                constraint,
                side: BoundSide::Lower,
            },
            1.0,
        )];

        let error = report_members(&model, &weighted, &[]).unwrap_err();
        assert!(matches!(
            error,
            FeasibilityRelaxationError::Numerical(message)
                if message.contains("candidate")
        ));
    }

    #[test]
    fn report_members_rejects_nonfinite_candidate_values_even_without_evidence() {
        let mut model = Model::new();
        let x = model.add_variable(continuous().bounds(0.0, 1.0)).unwrap();

        let error = report_members(&model, &[], &[(x, f64::NAN)]).unwrap_err();
        assert!(matches!(
            error,
            FeasibilityRelaxationError::Numerical(message)
                if message.contains("non-finite candidate value")
        ));
    }
}
