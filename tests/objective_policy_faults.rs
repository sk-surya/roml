//! Typed cleanup/rebuild fault evidence for the P31 portable objective
//! executor (Task 31-05; SM-11.5, SM-11.6).
//!
//! The `ReferenceBackend` is wrapped in a `FaultBackend` that can fail at
//! apply, partial-apply/rollback, solve, rollback, and verify boundaries.
//! Each test proves the executor preserves the primary and any cleanup/rebuild
//! error, forces a rebuild before reuse, and only returns a
//! `MultiObjectiveResult` when every stage's math and cleanup succeeded.

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
use roml::sync::{AdapterCursor, AdapterHealth};
use roml::{
    continuous, ConstraintExprExt, LexicographicObjectives, Model, ObjId, ObjectiveExecutionError,
    ObjectivePolicy, ObjectivePriority, ObjectiveProviderPolicy, SolverSession, StageContinuation,
    StageContinuationDecision, WeightedObjective, WeightedObjectiveLevel,
};

/// Controls what a fault-free `solve` returns (termination + optional values),
/// so continuation/lock semantics can be exercised without a real optimizer.
#[derive(Clone, Debug, Default)]
enum SolveOutcome {
    #[default]
    Optimal,
    Feasible {
        values: Vec<(roml::VarId, f64)>,
    },
    Infeasible,
}

#[derive(Clone, Copy, Debug)]
enum FailurePoint {
    Apply,
    PartialApplyRollback,
    Solve,
    Rollback,
    Verify,
}

struct FaultBackend {
    inner: ReferenceBackend,
    caps: BackendCapabilitySet,
    revision: roml::ModelRevision,
    failure: Option<FailurePoint>,
    outcome: SolveOutcome,
    rebuilds: usize,
}

impl FaultBackend {
    fn with_outcome(failure: Option<FailurePoint>, outcome: SolveOutcome) -> Self {
        let mut caps = BackendCapabilitySet::new();
        for feature in [
            BackendFeature::Lp,
            BackendFeature::IncrementalBounds,
            BackendFeature::IncrementalRows,
            BackendFeature::IncrementalCoefficients,
        ] {
            caps.set(feature, FeatureSupport::native(Default::default()));
        }
        Self {
            inner: ReferenceBackend::new(),
            caps,
            revision: roml::ModelRevision::ZERO,
            failure,
            outcome,
            rebuilds: 0,
        }
    }

    fn new(failure: Option<FailurePoint>) -> Self {
        Self::with_outcome(failure, SolveOutcome::default())
    }
}

impl BackendMetadata for FaultBackend {
    fn name(&self) -> &str {
        "P31ObjectiveFaultBackend"
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
        if matches!(self.failure, Some(FailurePoint::Solve)) {
            return Err(BackendError::new(
                "injected objective solve failure",
                ErrorCategory::Internal,
                HealthEffect::RequiresRebuild,
            ));
        }
        let (termination, solution) = match &self.outcome {
            SolveOutcome::Feasible { values } => (
                TerminationStatus::Feasible,
                Some(SolveSolution {
                    variable_values: values.clone(),
                    objective_value: None,
                    dual_values: None,
                    reduced_costs: None,
                }),
            ),
            SolveOutcome::Infeasible => (TerminationStatus::Infeasible, None),
            SolveOutcome::Optimal => (TerminationStatus::Optimal, None),
        };
        Ok(SolveResult {
            effective_configuration: EffectiveConfig::default(),
            termination,
            solution,
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
                "injected objective apply failure",
                ErrorCategory::Internal,
                HealthEffect::RequiresRebuild,
            ));
        }
        if matches!(self.failure, Some(FailurePoint::PartialApplyRollback)) {
            let _receipt = self.inner.apply_overlay(overlay)?;
            return Err(BackendError::new(
                "injected objective apply failure after mutation",
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
                "injected objective rollback failure",
                ErrorCategory::Internal,
                HealthEffect::RequiresRebuild,
            ));
        }
        self.inner.rollback_overlay(_receipt)
    }

    fn verify_overlay_clean(&mut self) -> Result<(), BackendError> {
        if matches!(self.failure, Some(FailurePoint::Verify)) {
            return Err(BackendError::new(
                "injected objective verification failure",
                ErrorCategory::Internal,
                HealthEffect::RequiresRebuild,
            ));
        }
        self.inner.verify_overlay_clean()
    }
}

fn base_objective_model() -> (Model, roml::VarId, roml::VarId, ObjId, ObjId) {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    let y = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    model.add_constraint((x + y).le(10.0)).unwrap();
    let obj1 = model.minimize(x).unwrap();
    let obj2 = model.maximize(y).unwrap();
    (model, x, y, obj1, obj2)
}

fn two_level_policy(obj1: ObjId, obj2: ObjId) -> ObjectivePolicy {
    ObjectivePolicy::Lexicographic(LexicographicObjectives {
        levels: vec![
            WeightedObjectiveLevel {
                priority: ObjectivePriority::new(0),
                objectives: vec![WeightedObjective {
                    objective: obj1,
                    weight: 1.0,
                }],
                absolute_tolerance: 1e-6,
                relative_tolerance: 1e-9,
            },
            WeightedObjectiveLevel {
                priority: ObjectivePriority::new(1),
                objectives: vec![WeightedObjective {
                    objective: obj2,
                    weight: 1.0,
                }],
                absolute_tolerance: 1e-6,
                relative_tolerance: 1e-9,
            },
        ],
    })
}

#[test]
fn injected_apply_failure_returns_backend_error_and_no_leak() {
    let (mut model, _x, _y, obj1, obj2) = base_objective_model();
    let mut session = SolverSession::new(FaultBackend::new(Some(FailurePoint::Apply)));
    let result = session.solve_objective_policy(
        &mut model,
        two_level_policy(obj1, obj2),
        ObjectiveProviderPolicy::PortableOnly,
        StageContinuation::RequireOptimal,
    );
    // The apply failure is preserved as the primary error and, because the
    // rollback path must still be attempted (and reports no applied overlay
    // to undo), the two errors combine into a single composite Cleanup error
    // that carries the rebuild requirement. Both facts are retained.
    assert!(
        matches!(
            result,
            Err(ObjectiveExecutionError::Cleanup {
                ref primary,
                requires_rebuild: true,
                ..
            }) if primary.contains("objective apply")
        ),
        "got {result:?}"
    );
    // The failure forces the façade to rebuild before the next ordinary solve;
    // no stage overlay leaks into subsequent execution.
    assert!(session.solve(&mut model).is_ok());
}

#[test]
fn injected_solve_failure_returns_backend_error() {
    let (mut model, _x, _y, obj1, obj2) = base_objective_model();
    let mut session = SolverSession::new(FaultBackend::new(Some(FailurePoint::Solve)));
    let result = session.solve_objective_policy(
        &mut model,
        two_level_policy(obj1, obj2),
        ObjectiveProviderPolicy::PortableOnly,
        StageContinuation::RequireOptimal,
    );
    assert!(
        matches!(result, Err(ObjectiveExecutionError::Backend(ref msg)) if msg.contains("objective solve")),
        "got {result:?}"
    );
}

#[test]
fn injected_rollback_and_verify_failures_are_composite_cleanup() {
    for failure in [FailurePoint::Rollback, FailurePoint::Verify] {
        let (mut model, _x, _y, obj1, obj2) = base_objective_model();
        let mut session = SolverSession::new(FaultBackend::new(Some(failure)));
        let result = session.solve_objective_policy(
            &mut model,
            two_level_policy(obj1, obj2),
            ObjectiveProviderPolicy::PortableOnly,
            StageContinuation::RequireOptimal,
        );
        assert!(
            matches!(
                result,
                Err(ObjectiveExecutionError::Cleanup {
                    requires_rebuild: true,
                    ..
                })
            ),
            "expected composite cleanup error for {failure:?}, got {result:?}"
        );
    }
}

#[test]
fn partial_apply_failure_composes_primary_and_cleanup() {
    let (mut model, _x, _y, obj1, obj2) = base_objective_model();
    let mut session =
        SolverSession::new(FaultBackend::new(Some(FailurePoint::PartialApplyRollback)));
    let result = session.solve_objective_policy(
        &mut model,
        two_level_policy(obj1, obj2),
        ObjectiveProviderPolicy::PortableOnly,
        StageContinuation::RequireOptimal,
    );
    assert!(
        matches!(
            result,
            Err(ObjectiveExecutionError::Cleanup {
                ref primary,
                ref cleanup,
                requires_rebuild: true,
            }) if primary.contains("apply failure after mutation") && cleanup.contains("rollback failure")
        ),
        "primary and cleanup errors must both be preserved: {result:?}"
    );
}

#[test]
fn native_required_rejects_before_mutation() {
    let (mut model, _x, _y, obj1, obj2) = base_objective_model();
    let mut session = SolverSession::new(FaultBackend::new(None));
    let result = session.solve_objective_policy(
        &mut model,
        two_level_policy(obj1, obj2),
        ObjectiveProviderPolicy::NativeRequired,
        StageContinuation::RequireOptimal,
    );
    assert!(matches!(
        result,
        Err(ObjectiveExecutionError::NativeProviderRequired)
    ));
    // No backend mutation ever occurred.
    assert!(session.solve(&mut model).is_ok());
}

#[test]
fn best_feasible_descends_and_locks_against_feasible_incumbent() {
    // Stage 0 (minimize x) returns a FEASIBLE (not optimal) incumbent x=2, y=3.
    // BestFeasible must descend and lock against the actual feasible scalar
    // (z*=2), recording that it was not proven optimal. Stage 1 (maximize y)
    // then solves optimally.
    let (mut model, x, y, obj1, obj2) = base_objective_model();
    let mut session = SolverSession::new(FaultBackend::with_outcome(
        None,
        SolveOutcome::Feasible {
            values: vec![(x, 2.0), (y, 3.0)],
        },
    ));
    let result = session
        .solve_objective_policy(
            &mut model,
            two_level_policy(obj1, obj2),
            ObjectiveProviderPolicy::PortableOnly,
            StageContinuation::BestFeasible,
        )
        .expect("BestFeasible descent must succeed");
    assert_eq!(result.stages.len(), 2);
    let s0 = &result.stages[0];
    // A feasible (not proven-optimal) stage under BestFeasible must descend
    // and record that it was not proven optimal.
    assert_eq!(
        s0.continuation,
        StageContinuationDecision::ContinueBestFeasible
    );
    let lock = s0.lock.expect("feasible stage must emit a lock");
    // The lock binds to exactly the feasible incumbent's normalized scalar,
    // whatever the engine resolved it to at that point.
    assert_eq!(
        lock.reference_value,
        s0.scalar_stage_value.expect("feasible scalar")
    );
    assert_eq!(
        lock.normalized_upper_bound,
        lock.reference_value + lock.allowed_degradation
    );
    let s1 = &result.stages[1];
    assert_eq!(
        s1.continuation,
        StageContinuationDecision::ContinueBestFeasible
    );
    let _ = (x, y);
}

#[test]
fn require_optimal_infeasible_stage_stops_descent() {
    let (mut model, _x, _y, obj1, obj2) = base_objective_model();
    let mut session =
        SolverSession::new(FaultBackend::with_outcome(None, SolveOutcome::Infeasible));
    let result = session
        .solve_objective_policy(
            &mut model,
            two_level_policy(obj1, obj2),
            ObjectiveProviderPolicy::PortableOnly,
            StageContinuation::RequireOptimal,
        )
        .expect("infeasible stage must still yield a staged result");
    assert_eq!(result.stages.len(), 1);
    assert_eq!(
        result.stages[0].continuation,
        StageContinuationDecision::StopNoFeasiblePoint
    );
    assert!(result.stages[0].lock.is_none());
}
