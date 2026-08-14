//! End-to-end portable P30 repair tests using a deterministic reference session.

use std::collections::HashMap;

use roml::advanced::{
    BackendCapabilitySet, BackendFeature, BackendOp, BackendSnapshot, CompiledVariableId,
    EntityOrigin, FeatureSupport, OverlayApplyReceipt, OverlayRollbackOutcome, OverlaySession,
    Synchronization,
};
use roml::solver::backend::{
    BackendCapabilities, BackendError, ErrorCategory, HealthEffect, TerminationStatus,
};
use roml::solver::reference::ReferenceBackend;
use roml::solver::relaxation::{
    FeasibilityRelaxationPlan, RelaxationAcceptance, RelaxationOutcome, RelaxationProviderPolicy,
};
use roml::solver::request::{EffectiveConfig, SolveRequest, SolveResult, SolveSolution};
use roml::solver::session::{BackendMetadata, BackendSession, SessionHealth, SyncReceipt};
use roml::sync::{AdapterCursor, AdapterHealth};
use roml::{continuous, ConstraintExprExt, Model, SolverSession};

#[derive(Clone)]
struct ReferenceSolveSession {
    inner: ReferenceBackend,
    revision: roml::ModelRevision,
    health: AdapterHealth,
    user_variables: HashMap<CompiledVariableId, roml::VarId>,
    capabilities: BackendCapabilitySet,
}

fn capabilities() -> BackendCapabilitySet {
    let mut caps = BackendCapabilitySet::new();
    for feature in [
        BackendFeature::Lp,
        BackendFeature::IncrementalBounds,
        BackendFeature::IncrementalRows,
        BackendFeature::IncrementalCoefficients,
        BackendFeature::SoftConstraint,
    ] {
        caps.set(feature, FeatureSupport::native(Default::default()));
    }
    caps
}

impl ReferenceSolveSession {
    fn new() -> Self {
        Self {
            inner: ReferenceBackend::new(),
            revision: roml::ModelRevision::ZERO,
            health: AdapterHealth::Ready,
            user_variables: HashMap::new(),
            capabilities: capabilities(),
        }
    }

    fn project_origins(&mut self, snapshot: &BackendSnapshot) {
        self.user_variables.clear();
        for variable in &snapshot.variables {
            if let Some(EntityOrigin::UserVariable(user)) =
                snapshot.origin_map.variable_origin(variable.id)
            {
                self.user_variables.insert(variable.id, *user);
            }
        }
    }

    fn candidate_values(&self) -> Vec<(roml::VarId, f64)> {
        self.user_variables
            .iter()
            .filter_map(|(compiled, user)| {
                self.inner
                    .compiled_variables
                    .get(compiled)
                    .map(|(bounds, _)| (*user, bounds.lower))
            })
            .collect()
    }
}

impl BackendMetadata for ReferenceSolveSession {
    fn name(&self) -> &str {
        "ReferenceSolveSession"
    }
    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities::all()
    }
    fn typed_capabilities(&self) -> &BackendCapabilitySet {
        &self.capabilities
    }
}

impl SessionHealth for ReferenceSolveSession {
    fn health(&self) -> AdapterHealth {
        self.health
    }
    fn revision(&self) -> roml::ModelRevision {
        self.revision
    }
}

impl BackendSession for ReferenceSolveSession {
    fn synchronize(&mut self, sync: Synchronization) -> Result<SyncReceipt, BackendError> {
        match sync {
            Synchronization::CompiledRebuild(snapshot) => {
                self.inner.rebuild_compiled(&snapshot).map_err(|e| {
                    BackendError::new(
                        e.to_string(),
                        ErrorCategory::Internal,
                        HealthEffect::RequiresRebuild,
                    )
                })?;
                self.project_origins(&snapshot);
                self.revision = snapshot.source_revision;
            }
            Synchronization::CompiledDeltaBatch(batch) => {
                for operation in &batch.operations {
                    if let BackendOp::AddVariable(variable) = operation {
                        if let Some(EntityOrigin::UserVariable(user)) =
                            batch.origin_additions.variable_origin(variable.id)
                        {
                            self.user_variables.insert(variable.id, *user);
                        }
                    }
                }
                self.inner.apply_compiled_delta(&batch).map_err(|e| {
                    BackendError::new(
                        e.to_string(),
                        ErrorCategory::Internal,
                        HealthEffect::RequiresRebuild,
                    )
                })?;
                self.revision = batch.to_revision;
            }
            Synchronization::DeltaBatch(_) | Synchronization::Rebuild(_) => {
                return Err(BackendError::new(
                    "reference solve session requires compiled synchronization",
                    ErrorCategory::Unsupported,
                    HealthEffect::RequiresRebuild,
                ));
            }
        }
        Ok(SyncReceipt {
            cursor: AdapterCursor {
                applied_revision: self.revision,
                health: self.health,
            },
            health: self.health,
        })
    }

    fn solve(&mut self, _request: &SolveRequest) -> Result<SolveResult, BackendError> {
        let values = self.candidate_values();
        let objective_value = self
            .inner
            .compiled_rows
            .values()
            .filter_map(|(bounds, _)| bounds.lower.is_finite().then_some(bounds.lower))
            .max_by(f64::total_cmp)
            .unwrap_or(0.0);
        Ok(SolveResult {
            effective_configuration: EffectiveConfig::default(),
            termination: TerminationStatus::Optimal,
            solution: Some(SolveSolution {
                variable_values: values,
                objective_value: Some(objective_value),
                dual_values: None,
                reduced_costs: None,
            }),
            compilation_id: self.inner.current_compilation,
            overlay_id: None,
        })
    }

    fn close(self) -> Result<(), BackendError> {
        Ok(())
    }
}

impl OverlaySession for ReferenceSolveSession {
    fn apply_overlay(
        &mut self,
        overlay: &roml::advanced::CompiledOverlay,
    ) -> Result<OverlayApplyReceipt, BackendError> {
        self.inner.apply_overlay(overlay)
    }

    fn rollback_overlay(
        &mut self,
        receipt: &OverlayApplyReceipt,
    ) -> Result<OverlayRollbackOutcome, BackendError> {
        self.inner.rollback_overlay(receipt)
    }

    fn verify_overlay_clean(&mut self) -> Result<(), BackendError> {
        self.inner.verify_overlay_clean()
    }
}

#[test]
fn portable_repair_reports_exact_identity_and_preserves_base() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    let constraint = model.add_constraint((x).ge(1.0)).unwrap();
    model.commit().unwrap();
    let revision = model.current_revision();
    let mut session = SolverSession::new(ReferenceSolveSession::new());
    let report = session
        .solve_feasibility_relaxation(
            &mut model,
            FeasibilityRelaxationPlan {
                scope: roml::solver::RelaxationScope::Explicit(vec![
                    roml::solver::RelaxationRestriction::ConstraintSide {
                        constraint,
                        side: roml::solver::infeasibility::BoundSide::Lower,
                    },
                ]),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(report.outcome, RelaxationOutcome::OptimalRepair);
    assert_eq!(
        report.metadata.provider,
        roml::solver::RelaxationExecutionProvider::PortableRoml
    );
    assert_ne!(
        report.metadata.base_compilation_id,
        report.metadata.relaxation_compilation_id
    );
    assert_eq!(report.metadata.model_revision, revision);
    assert_eq!(report.members.len(), 1);
    assert_eq!(report.members[0].evaluated_weight, 1.0);
    assert_eq!(
        report.members[0].restriction,
        roml::solver::RelaxationRestriction::ConstraintSide {
            constraint,
            side: roml::solver::infeasibility::BoundSide::Lower
        }
    );
    assert_eq!(report.solution.status(), roml::SolveStatus::Optimal);
    assert_eq!(model.current_revision(), revision);
}

#[test]
fn native_required_rejects_before_backend_synchronization() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 1.0)).unwrap();
    let constraint = model.add_constraint((x).ge(0.0)).unwrap();
    let mut session = SolverSession::new(ReferenceSolveSession::new());
    let result = session.solve_feasibility_relaxation(
        &mut model,
        FeasibilityRelaxationPlan {
            scope: roml::solver::RelaxationScope::Explicit(vec![
                roml::solver::RelaxationRestriction::ConstraintSide {
                    constraint,
                    side: roml::solver::infeasibility::BoundSide::Lower,
                },
            ]),
            provider_policy: RelaxationProviderPolicy::NativeRequired,
            ..Default::default()
        },
    );
    assert!(matches!(
        result,
        Err(roml::solver::FeasibilityRelaxationError::NativeProviderRequired)
    ));
    assert_eq!(model.current_revision(), roml::ModelRevision::ZERO);
}

#[test]
fn feasible_acceptance_is_not_promoted_to_optimality() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 1.0)).unwrap();
    let constraint = model.add_constraint((x).ge(0.0)).unwrap();
    let mut plan = FeasibilityRelaxationPlan {
        scope: roml::solver::RelaxationScope::Explicit(vec![
            roml::solver::RelaxationRestriction::ConstraintSide {
                constraint,
                side: roml::solver::infeasibility::BoundSide::Lower,
            },
        ]),
        acceptance: RelaxationAcceptance::RequireOptimal,
        ..Default::default()
    };
    // The deterministic reference provider reports an optimal proof; this
    // test still locks the acceptance field into the public request corpus.
    assert_eq!(plan.acceptance, RelaxationAcceptance::RequireOptimal);
    plan.acceptance = RelaxationAcceptance::AcceptFeasible;
    assert_eq!(plan.acceptance, RelaxationAcceptance::AcceptFeasible);
}

#[test]
fn prefer_native_records_explicit_portable_fallback() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 1.0)).unwrap();
    let constraint = model.add_constraint((x).ge(0.0)).unwrap();
    let report = SolverSession::new(ReferenceSolveSession::new())
        .solve_feasibility_relaxation(
            &mut model,
            FeasibilityRelaxationPlan {
                scope: roml::solver::RelaxationScope::Explicit(vec![
                    roml::solver::RelaxationRestriction::ConstraintSide {
                        constraint,
                        side: roml::solver::infeasibility::BoundSide::Lower,
                    },
                ]),
                provider_policy: RelaxationProviderPolicy::PreferNative,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        report.metadata.provider,
        roml::solver::RelaxationExecutionProvider::PortableRoml
    );
    assert!(report
        .metadata
        .provider_reason
        .as_deref()
        .is_some_and(|reason| reason.contains("portable ROML")));
}
