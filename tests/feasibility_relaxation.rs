//! End-to-end portable P30 repair tests using a deterministic reference session.

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
    fn new() -> Self {
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

    fn observing_overlay(mut self, observed_overlay: Rc<RefCell<Option<Vec<OverlayOp>>>>) -> Self {
        self.observed_overlay = Some(observed_overlay);
        self
    }

    fn with_termination(mut self, termination: TerminationStatus) -> Self {
        self.termination = termination;
        self
    }

    fn inject_candidate(&mut self, variable: roml::VarId, value: f64) {
        self.candidate_overrides.insert(variable, value);
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
fn portable_repair_ignores_inactive_compiled_candidate() {
    let mut model = Model::new();
    let inactive = model.add_variable(continuous().bounds(0.0, 1.0)).unwrap();
    model.set_variable_active(inactive, false).unwrap();
    let active = model.add_variable(continuous().bounds(0.0, 1.0)).unwrap();
    let constraint = model.add_constraint(active.ge(2.0)).unwrap();

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
                ..Default::default()
            },
        )
        .expect("inactive compiled candidates must not invalidate repair");

    assert_eq!(report.outcome, RelaxationOutcome::OptimalRepair);
    assert_eq!(report.members.len(), 1);
    assert!((report.total_weighted_violation - 2.0).abs() < 1e-7);
    assert_eq!(report.solution.values().get(&active), Some(&0.0));
}

#[test]
fn portable_repair_treats_inactive_constraint_terms_as_zero() {
    let mut model = Model::new();
    let inactive = model.add_variable(continuous().bounds(0.0, 1.0)).unwrap();
    let active = model.add_variable(continuous().bounds(0.0, 1.0)).unwrap();
    let constraint = model.add_constraint((inactive + active).ge(2.0)).unwrap();
    model.set_variable_active(inactive, false).unwrap();

    let mut backend = ReferenceSolveSession::new();
    backend.inject_candidate(inactive, 2.0);
    let report = SolverSession::new(backend)
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
        .expect("inactive terms must use their canonical zero value");

    assert_eq!(report.outcome, RelaxationOutcome::OptimalRepair);
    assert_eq!(report.members.len(), 1);
    assert!((report.members[0].violation - 2.0).abs() < 1e-7);
    assert!((report.total_weighted_violation - 2.0).abs() < 1e-7);
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
fn feasible_acceptance_accepts_an_actual_feasible_termination() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 1.0)).unwrap();
    let constraint = model.add_constraint((x).ge(0.0)).unwrap();
    let plan = FeasibilityRelaxationPlan {
        scope: roml::solver::RelaxationScope::Explicit(vec![
            roml::solver::RelaxationRestriction::ConstraintSide {
                constraint,
                side: roml::solver::infeasibility::BoundSide::Lower,
            },
        ]),
        acceptance: RelaxationAcceptance::AcceptFeasible,
        ..Default::default()
    };
    let report = SolverSession::new(
        ReferenceSolveSession::new().with_termination(TerminationStatus::Feasible),
    )
    .solve_feasibility_relaxation(&mut model, plan)
    .unwrap();

    assert_eq!(report.outcome, RelaxationOutcome::FeasibleRepair);
    assert_eq!(report.metadata.termination, roml::SolveStatus::Feasible);
}

#[test]
fn two_sided_ranged_and_equality_rows_keep_both_declared_sides_widened() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    // The reference seam supplies the declared lower bound as its candidate;
    // choose row sides that are already satisfied so this test isolates the
    // temporary-bound replacement shape rather than objective calculation.
    let ranged = model.add_constraint(x.between(0.0, 8.0)).unwrap();
    let equality = model.add_constraint(x.eq(0.0)).unwrap();
    let observed = Rc::new(RefCell::new(None));
    let plan = FeasibilityRelaxationPlan {
        scope: roml::solver::RelaxationScope::Explicit(vec![
            roml::solver::RelaxationRestriction::ConstraintSide {
                constraint: ranged,
                side: roml::solver::infeasibility::BoundSide::Lower,
            },
            roml::solver::RelaxationRestriction::ConstraintSide {
                constraint: ranged,
                side: roml::solver::infeasibility::BoundSide::Upper,
            },
            roml::solver::RelaxationRestriction::ConstraintSide {
                constraint: equality,
                side: roml::solver::infeasibility::BoundSide::Lower,
            },
            roml::solver::RelaxationRestriction::ConstraintSide {
                constraint: equality,
                side: roml::solver::infeasibility::BoundSide::Upper,
            },
        ]),
        ..Default::default()
    };

    SolverSession::new(ReferenceSolveSession::new().observing_overlay(observed.clone()))
        .solve_feasibility_relaxation(&mut model, plan)
        .unwrap();

    let bounds = observed
        .borrow()
        .as_ref()
        .unwrap()
        .iter()
        .filter_map(|operation| match operation {
            OverlayOp::SetTemporaryRowBounds { constraint, bounds } => Some((*constraint, *bounds)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(bounds.len(), 2, "one final replacement per original row");
    assert!(bounds
        .iter()
        .all(|(_, bounds)| bounds.lower == f64::NEG_INFINITY && bounds.upper == f64::INFINITY));
}

#[test]
fn two_sided_variable_bound_relaxation_keeps_both_declared_sides_widened() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 3.0)).unwrap();
    let observed = Rc::new(RefCell::new(None));
    let plan = FeasibilityRelaxationPlan {
        scope: roml::solver::RelaxationScope::Explicit(vec![
            roml::solver::RelaxationRestriction::VariableBound {
                variable: x,
                side: roml::solver::infeasibility::BoundSide::Lower,
            },
            roml::solver::RelaxationRestriction::VariableBound {
                variable: x,
                side: roml::solver::infeasibility::BoundSide::Upper,
            },
        ]),
        ..Default::default()
    };

    SolverSession::new(ReferenceSolveSession::new().observing_overlay(observed.clone()))
        .solve_feasibility_relaxation(&mut model, plan)
        .unwrap();

    let bounds = observed
        .borrow()
        .as_ref()
        .unwrap()
        .iter()
        .filter_map(|operation| match operation {
            OverlayOp::SetTemporaryVariableBounds { bounds, .. } => Some(*bounds),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        bounds.len(),
        1,
        "one final replacement per original variable"
    );
    assert_eq!(bounds[0].lower, f64::NEG_INFINITY);
    assert_eq!(bounds[0].upper, f64::INFINITY);
}

#[test]
fn fixing_and_declared_bound_restrictions_are_independent_and_compose() {
    let cases = [
        // Declared-bound-only relaxation without a fixing.
        (
            true,
            false,
            false,
            roml::model::Bounds::new(f64::NEG_INFINITY, 10.0),
            1,
            1,
        ),
        // Fixing-only relaxation leaves the declared bounds hard.
        (false, true, true, roml::model::Bounds::new(0.0, 10.0), 2, 1),
        // Declared-bound-only relaxation leaves the fixing hard.
        (true, false, true, roml::model::Bounds::new(5.0, 5.0), 1, 1),
        // Selecting both restrictions removes neither semantic contribution.
        (
            true,
            true,
            true,
            roml::model::Bounds::new(f64::NEG_INFINITY, 10.0),
            3,
            2,
        ),
    ];

    for (
        relax_bound,
        relax_fixing,
        with_fixing,
        expected_bounds,
        expected_rows,
        expected_members,
    ) in cases
    {
        let mut model = Model::new();
        let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
        if with_fixing {
            model.fix(x, 5.0).unwrap();
        }
        let observed = Rc::new(RefCell::new(None));
        let mut restrictions = Vec::new();
        if relax_bound {
            restrictions.push(roml::solver::RelaxationRestriction::VariableBound {
                variable: x,
                side: roml::solver::infeasibility::BoundSide::Lower,
            });
        }
        if relax_fixing {
            restrictions
                .push(roml::solver::RelaxationRestriction::PersistentFixing { variable: x });
        }
        let report =
            SolverSession::new(ReferenceSolveSession::new().observing_overlay(observed.clone()))
                .solve_feasibility_relaxation(
                    &mut model,
                    FeasibilityRelaxationPlan {
                        scope: roml::solver::RelaxationScope::Explicit(restrictions),
                        ..Default::default()
                    },
                );

        let report = report.unwrap();
        let operations = observed.borrow();
        let bounds = operations
            .as_ref()
            .unwrap()
            .iter()
            .find_map(|operation| match operation {
                OverlayOp::SetTemporaryVariableBounds { bounds, .. } => Some(*bounds),
                _ => None,
            })
            .unwrap();
        let rows = operations
            .as_ref()
            .unwrap()
            .iter()
            .filter(|operation| matches!(operation, OverlayOp::AddTemporaryRow { .. }))
            .count();
        assert_eq!(bounds, expected_bounds);
        assert_eq!(rows, expected_rows);
        assert_eq!(report.members.len(), expected_members);
    }
}

#[test]
fn all_eligible_includes_declared_bounds_alongside_persistent_fixing() {
    let mut model = Model::new();
    let x = model.add_variable(continuous().bounds(0.0, 10.0)).unwrap();
    model.fix(x, 5.0).unwrap();
    let observed = Rc::new(RefCell::new(None));

    SolverSession::new(ReferenceSolveSession::new().observing_overlay(observed.clone()))
        .solve_feasibility_relaxation(&mut model, FeasibilityRelaxationPlan::default())
        .unwrap();

    let operations = observed.borrow();
    let bounds = operations
        .as_ref()
        .unwrap()
        .iter()
        .filter_map(|operation| match operation {
            OverlayOp::SetTemporaryVariableBounds { bounds, .. } => Some(*bounds),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(bounds, vec![roml::model::Bounds::UNBOUNDED]);
    assert_eq!(
        operations
            .as_ref()
            .unwrap()
            .iter()
            .filter(|operation| matches!(operation, OverlayOp::AddTemporaryRow { .. }))
            .count(),
        4
    );
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
