//! Shared deterministic backend session for P30 integration tests.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use roml::advanced::{
    BackendCapabilitySet, BackendFeature, BackendOp, BackendSnapshot, CompiledVariableId,
    EntityOrigin, FeatureSupport, OverlayApplyReceipt, OverlayOp, OverlayRollbackOutcome,
    OverlaySession, Synchronization,
};
use roml::solver::backend::{
    BackendCapabilities, BackendError, ErrorCategory, HealthEffect, TerminationStatus,
};
use roml::solver::reference::ReferenceBackend;
use roml::solver::request::{EffectiveConfig, SolveRequest, SolveResult, SolveSolution};
use roml::solver::session::{BackendMetadata, BackendSession, SessionHealth, SyncReceipt};
use roml::sync::{AdapterCursor, AdapterHealth};

#[derive(Clone)]
pub struct ReferenceSolveSession {
    inner: ReferenceBackend,
    revision: roml::ModelRevision,
    health: AdapterHealth,
    user_variables: HashMap<CompiledVariableId, roml::VarId>,
    candidate_overrides: HashMap<roml::VarId, f64>,
    capabilities: BackendCapabilitySet,
    observed_overlay: Option<Rc<RefCell<Option<Vec<OverlayOp>>>>>,
    termination: TerminationStatus,
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
    pub fn new() -> Self {
        Self {
            inner: ReferenceBackend::new(),
            revision: roml::ModelRevision::ZERO,
            health: AdapterHealth::Ready,
            user_variables: HashMap::new(),
            candidate_overrides: HashMap::new(),
            capabilities: capabilities(),
            observed_overlay: None,
            termination: TerminationStatus::Optimal,
        }
    }

    #[allow(dead_code)]
    pub fn observing_overlay(
        mut self,
        observed_overlay: Rc<RefCell<Option<Vec<OverlayOp>>>>,
    ) -> Self {
        self.observed_overlay = Some(observed_overlay);
        self
    }

    #[allow(dead_code)]
    pub fn with_termination(mut self, termination: TerminationStatus) -> Self {
        self.termination = termination;
        self
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
                    .map(|(bounds, _)| {
                        (
                            *user,
                            self.candidate_overrides
                                .get(user)
                                .copied()
                                .unwrap_or_else(|| {
                                    if bounds.lower.is_finite() {
                                        bounds.lower
                                    } else if bounds.upper.is_finite() {
                                        bounds.upper
                                    } else {
                                        0.0
                                    }
                                }),
                        )
                    })
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
            termination: self.termination,
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
        if let Some(observed_overlay) = &self.observed_overlay {
            *observed_overlay.borrow_mut() = Some(overlay.operations.clone());
        }
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
