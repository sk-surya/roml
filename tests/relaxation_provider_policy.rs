//! Frozen provider/acceptance policy contract checks.

use roml::solver::{
    FeasibilityRelaxationError, FeasibilityRelaxationPlan, RelaxationAcceptance,
    RelaxationObjective, RelaxationProviderPolicy, RelaxationScope,
};

#[test]
fn defaults_are_portable_weighted_l1_and_require_optimality() {
    let plan = FeasibilityRelaxationPlan::default();
    assert_eq!(plan.scope, RelaxationScope::AllEligible);
    assert_eq!(plan.objective, RelaxationObjective::WeightedL1);
    assert_eq!(plan.provider_policy, RelaxationProviderPolicy::PortableOnly);
    assert_eq!(plan.acceptance, RelaxationAcceptance::RequireOptimal);
}

#[test]
fn native_required_is_a_typed_operational_boundary() {
    assert_eq!(
        FeasibilityRelaxationError::NativeProviderRequired,
        FeasibilityRelaxationError::NativeProviderRequired
    );
    assert_ne!(
        FeasibilityRelaxationError::NativeProviderRequired,
        FeasibilityRelaxationError::Preflight("native provider absent".into())
    );
}
