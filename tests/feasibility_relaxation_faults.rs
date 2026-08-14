//! Typed cleanup/rebuild fault evidence for the P30 lifecycle.

use roml::advanced::{
    BackendCapabilitySet, BackendFeature, FeatureSupport, OverlayApplyReceipt, OverlaySession,
    Synchronization,
};
use roml::solver::backend::{
    BackendCapabilities, BackendError, ErrorCategory, HealthEffect, TerminationStatus,
};
use roml::solver::reference::ReferenceBackend;
use roml::solver::request::{EffectiveConfig, SolveRequest, SolveResult, SolveSolution};
use roml::solver::session::{BackendMetadata, BackendSession, SessionHealth, SyncReceipt};
use roml::solver::{
    FeasibilityRelaxationError, FeasibilityRelaxationPlan, RelaxationRestriction, SolverSession,
};
use roml::sync::{AdapterCursor, AdapterHealth};
use roml::{continuous, ConstraintExprExt, Model};

#[derive(Clone, Copy)]
enum FailurePoint {
    Apply,
    PartialApplyRollback,
    OutOfDomainCandidate,
    Solve,
    Rollback,
    Verify,
}

struct FaultBackend {
    inner: ReferenceBackend,
    caps: BackendCapabilitySet,
    revision: roml::ModelRevision,
    failure: Option<FailurePoint>,
    candidate_variable: Option<roml::VarId>,
    rebuilds: usize,
}

impl FaultBackend {
    fn new(failure: Option<FailurePoint>) -> Self {
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
        Self {
            inner: ReferenceBackend::new(),
            caps,
            revision: roml::ModelRevision::ZERO,
            failure,
            candidate_variable: None,
            rebuilds: 0,
        }
    }
}

impl BackendMetadata for FaultBackend {
    fn name(&self) -> &str {
        "P30FaultBackend"
    }
    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities::all()
    }
    fn typed_capabilities(&self) -> &BackendCapabilitySet {
        &self.caps
    }
}

impl SessionHealth for FaultBackend {
    fn health(&self) -> AdapterHealth {
        AdapterHealth::Ready
    }
    fn revision(&self) -> roml::ModelRevision {
        self.revision
    }
}

impl BackendSession for FaultBackend {
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
                self.revision = snapshot.source_revision;
                self.rebuilds += 1;
            }
            Synchronization::CompiledDeltaBatch(batch) => {
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
                    "compiled synchronization required",
                    ErrorCategory::Unsupported,
                    HealthEffect::RequiresRebuild,
                ));
            }
        }
        Ok(SyncReceipt {
            cursor: AdapterCursor {
                applied_revision: self.revision,
                health: AdapterHealth::Ready,
            },
            health: AdapterHealth::Ready,
        })
    }

    fn solve(&mut self, _request: &SolveRequest) -> Result<SolveResult, BackendError> {
        if matches!(self.failure, Some(FailurePoint::OutOfDomainCandidate)) {
            return Ok(SolveResult {
                effective_configuration: EffectiveConfig::default(),
                termination: TerminationStatus::Optimal,
                solution: Some(SolveSolution {
                    variable_values: vec![(
                        self.candidate_variable
                            .expect("out-of-domain candidate test supplies a variable"),
                        -100.0,
                    )],
                    objective_value: Some(105.0),
                    dual_values: None,
                    reduced_costs: None,
                }),
                compilation_id: self.inner.current_compilation,
                overlay_id: None,
            });
        }
        if matches!(self.failure, Some(FailurePoint::Solve)) {
            return Err(BackendError::new(
                "injected solve failure",
                ErrorCategory::Internal,
                HealthEffect::RequiresRebuild,
            ));
        }
        Ok(SolveResult {
            effective_configuration: EffectiveConfig::default(),
            termination: TerminationStatus::Optimal,
            solution: None,
            compilation_id: self.inner.current_compilation,
            overlay_id: None,
        })
    }

    fn close(self) -> Result<(), BackendError> {
        Ok(())
    }
}

impl OverlaySession for FaultBackend {
    fn apply_overlay(
        &mut self,
        overlay: &roml::advanced::CompiledOverlay,
    ) -> Result<OverlayApplyReceipt, BackendError> {
        if matches!(self.failure, Some(FailurePoint::Apply)) {
            return Err(BackendError::new(
                "injected apply failure",
                ErrorCategory::Internal,
                HealthEffect::RequiresRebuild,
            ));
        }
        if matches!(self.failure, Some(FailurePoint::PartialApplyRollback)) {
            let _receipt = self.inner.apply_overlay(overlay)?;
            return Err(BackendError::new(
                "injected apply failure after mutation",
                ErrorCategory::Internal,
                HealthEffect::RequiresRebuild,
            ));
        }
        self.inner.apply_overlay(overlay)
    }

    fn rollback_overlay(
        &mut self,
        _receipt: &OverlayApplyReceipt,
    ) -> Result<roml::advanced::OverlayRollbackOutcome, BackendError> {
        if matches!(
            self.failure,
            Some(FailurePoint::Rollback | FailurePoint::PartialApplyRollback)
        ) {
            return Err(BackendError::new(
                "injected rollback failure",
                ErrorCategory::Internal,
                HealthEffect::RequiresRebuild,
            ));
        }
        self.inner.rollback_overlay(_receipt)
    }

    fn verify_overlay_clean(&mut self) -> Result<(), BackendError> {
        if matches!(self.failure, Some(FailurePoint::Verify)) {
            return Err(BackendError::new(
                "injected verification failure",
                ErrorCategory::Internal,
                HealthEffect::RequiresRebuild,
            ));
        }
        self.inner.verify_overlay_clean()
    }
}

fn request_plan(model: &mut Model) -> FeasibilityRelaxationPlan {
    let x = model.add_variable(continuous().bounds(0.0, 1.0)).unwrap();
    let constraint = model.add_constraint((x).ge(0.0)).unwrap();
    FeasibilityRelaxationPlan {
        scope: roml::solver::RelaxationScope::Explicit(vec![
            RelaxationRestriction::ConstraintSide {
                constraint,
                side: roml::solver::infeasibility::BoundSide::Lower,
            },
        ]),
        ..Default::default()
    }
}

#[test]
fn cleanup_error_retains_primary_failure_and_rebuild_requirement() {
    let error = FeasibilityRelaxationError::Cleanup {
        primary: "solve returned malformed candidate".into(),
        cleanup: "rollback verification failed".into(),
        requires_rebuild: true,
    };
    match error {
        FeasibilityRelaxationError::Cleanup {
            primary,
            cleanup,
            requires_rebuild,
        } => {
            assert!(primary.contains("malformed"));
            assert!(cleanup.contains("rollback"));
            assert!(requires_rebuild);
        }
        other => panic!("wrong error classification: {other:?}"),
    }
}

#[test]
fn numerical_and_compile_failures_are_not_mathematical_outcomes() {
    assert!(matches!(
        FeasibilityRelaxationError::Numerical("non-finite candidate".into()),
        FeasibilityRelaxationError::Numerical(_)
    ));
    assert!(matches!(
        FeasibilityRelaxationError::Compile("dangling row".into()),
        FeasibilityRelaxationError::Compile(_)
    ));
}

#[test]
fn injected_solve_failure_preserves_primary_error_and_rolls_back() {
    let mut model = Model::new();
    let plan = request_plan(&mut model);
    let result = SolverSession::new(FaultBackend::new(Some(FailurePoint::Solve)))
        .solve_feasibility_relaxation(&mut model, plan);
    assert!(
        matches!(result, Err(FeasibilityRelaxationError::Backend(message)) if message.contains("solve"))
    );
}

#[test]
fn injected_rollback_and_verification_failures_are_composite() {
    for failure in [FailurePoint::Rollback, FailurePoint::Verify] {
        let mut model = Model::new();
        let plan = request_plan(&mut model);
        let result = SolverSession::new(FaultBackend::new(Some(failure)))
            .solve_feasibility_relaxation(&mut model, plan);
        assert!(matches!(
            result,
            Err(FeasibilityRelaxationError::Cleanup {
                requires_rebuild: true,
                ..
            })
        ));
    }
}

#[test]
fn injected_apply_failure_forces_rebuild_before_next_plain_solve() {
    let mut model = Model::new();
    let plan = request_plan(&mut model);
    let mut session = SolverSession::new(FaultBackend::new(Some(FailurePoint::Apply)));
    let result = session.solve_feasibility_relaxation(&mut model, plan);
    assert!(matches!(
        result,
        Err(FeasibilityRelaxationError::Cleanup {
            primary,
            requires_rebuild: true,
            ..
        }) if primary.contains("apply")
    ));
    // The injected backend keeps the canonical compiled state, but the façade
    // resets its compiler so the next ordinary solve must rebuild it. The
    // solve itself is allowed to return an empty optimal reference solution.
    assert!(session.solve(&mut model).is_ok());
}

#[test]
fn partial_apply_failure_composes_primary_and_cleanup_errors() {
    let mut model = Model::new();
    let plan = request_plan(&mut model);
    let result = SolverSession::new(FaultBackend::new(Some(FailurePoint::PartialApplyRollback)))
        .solve_feasibility_relaxation(&mut model, plan);

    assert!(matches!(
        result,
        Err(FeasibilityRelaxationError::Cleanup {
            primary,
            cleanup,
            requires_rebuild: true,
        }) if primary.contains("apply failure after mutation") && cleanup.contains("rollback failure")
    ));
}

#[test]
fn injected_out_of_domain_persistent_fixing_candidate_is_rejected() {
    let mut model = Model::new();
    let variable = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    model.fix(variable, 5.0).unwrap();
    let plan = FeasibilityRelaxationPlan {
        scope: roml::solver::RelaxationScope::Explicit(vec![
            RelaxationRestriction::PersistentFixing { variable },
        ]),
        ..Default::default()
    };

    let mut backend = FaultBackend::new(Some(FailurePoint::OutOfDomainCandidate));
    backend.candidate_variable = Some(variable);
    let result = SolverSession::new(backend).solve_feasibility_relaxation(&mut model, plan);

    assert!(
        matches!(
            result,
            Err(FeasibilityRelaxationError::Numerical(ref message))
                if message.contains("violates domain")
        ),
        "got {result:?}"
    );
}
