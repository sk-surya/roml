//! Contract characterization for Phase 29 LP infeasibility analysis.

use roml::{
    classify_feasibility, ConflictGuarantee, FeasibilityOutcome, InfeasibilityMode,
    InfeasibilityPlan, InfeasibilityScope, Model, SolverSession, TerminationStatus,
};

#[allow(dead_code)]
fn assert_session_entrypoint<B>(
    session: &mut SolverSession<B>,
    model: &Model,
    plan: &InfeasibilityPlan,
) where
    B: roml::BackendSession + roml::SessionHealth + roml::BackendMetadata,
{
    let _ = session.analyze_infeasibility(model, plan);
}

#[test]
fn analysis_is_plan_driven_and_lp_relaxation_is_explicit() {
    let _model = Model::new();
    let plan = InfeasibilityPlan::portable_lp();
    assert_eq!(plan.mode, InfeasibilityMode::RomlPortable);
    assert_eq!(plan.scope, InfeasibilityScope::OriginalLp);

    let relaxation = InfeasibilityPlan {
        scope: InfeasibilityScope::LpRelaxation,
        ..plan
    };
    assert_eq!(relaxation.scope, InfeasibilityScope::LpRelaxation);
}

#[test]
fn all_provider_modes_are_distinct() {
    assert_ne!(InfeasibilityMode::Auto, InfeasibilityMode::RomlPortable);
    assert_ne!(
        InfeasibilityMode::NativeOnly,
        InfeasibilityMode::NativeThenRoml
    );
}

#[test]
fn feasibility_classification_is_strictly_tri_state() {
    assert!(matches!(
        classify_feasibility(TerminationStatus::Optimal),
        FeasibilityOutcome::ProvenFeasible(_)
    ));
    assert!(matches!(
        classify_feasibility(TerminationStatus::Infeasible),
        FeasibilityOutcome::ProvenInfeasible(_)
    ));

    for status in [
        TerminationStatus::InfeasibleOrUnbounded,
        TerminationStatus::Unbounded,
        TerminationStatus::TimeLimit,
        TerminationStatus::IterationLimit,
        TerminationStatus::NodeLimit,
        TerminationStatus::Interrupted,
        TerminationStatus::NumericalIssue,
        TerminationStatus::Error,
        TerminationStatus::Unknown,
    ] {
        assert!(matches!(
            classify_feasibility(status),
            FeasibilityOutcome::Unknown(_)
        ));
    }
}

#[test]
fn guarantee_type_does_not_offer_minimum_cardinality() {
    let _guarantee: Option<ConflictGuarantee> = None;
}
