//! Conformance integration tests for HighsSession.
//!
//! Runs the shared synchronization conformance suite against
//! [`HighsFixture`], verifying that HiGHS correctly implements all
//! [`BackendSession`] lifecycle semantics alongside ReferenceBackend.

use roml::compiler::capability::BackendFeature;
use roml::solver::conformance::run_sync_suite;
use roml::solver::session::{BackendFixture, BackendMetadata};
use roml_highs::{highs_capability_set, HighsFixture};

#[test]
fn conformance_highs_session() {
    let fixture = HighsFixture;
    run_sync_suite(&fixture);
}

/// The fixture must expose its backend name and construct fresh sessions, per
/// the [`BackendFixture`] contract the shared conformance suite relies on.
#[test]
fn highs_fixture_contract() {
    let fixture = HighsFixture;
    assert_eq!(fixture.backend_name(), "HiGHS");
    assert!(fixture.new_session().is_ok());
}

/// Characterize the legacy HiGHS flat capability declaration before the typed
/// migration (P26 Task 6). HiGHS declares LP/MIP/solution/duals/reduced-costs
/// native and rejects semi-continuous/semi-integer (H7).
#[test]
fn characterize_highs_flat_capabilities() {
    let session = roml_highs::HighsSession::try_new().expect("HiGHS should be available");
    let caps = session.capabilities();

    assert!(caps.lp, "HiGHS should support LP");
    assert!(caps.mip, "HiGHS should support MIP");
    assert!(caps.solution, "HiGHS should support solution extraction");
    assert!(caps.duals, "HiGHS should support dual values");
    assert!(caps.reduced_costs, "HiGHS should support reduced costs");
    assert!(
        !caps.semicontinuous,
        "HiGHS should NOT support semi-continuous (H7)"
    );
    assert!(!caps.semiinteger, "HiGHS should NOT support semi-integer");
}

/// The HiGHS typed capability set declares the M2-native features `Native`,
/// the P28-qualified MIP start features `Native` (SM-08.7), and every
/// remaining unqualified M3 feature `Unsupported` (SM-04.2/SM-04.4).
#[test]
fn highs_typed_capability_set_native_and_unsupported() {
    let set = highs_capability_set(1, 15, 0);

    let m2_native = [
        BackendFeature::Lp,
        BackendFeature::Mip,
        BackendFeature::IncrementalBounds,
        BackendFeature::IncrementalRows,
        BackendFeature::IncrementalCoefficients,
    ];
    for feature in m2_native {
        assert!(
            set.supports(feature),
            "M2-native feature {:?} must be Native",
            feature
        );
    }

    // P28 (SM-08.7): the pinned-header audit qualifies Highs_setSparseSolution
    // as the native MIP start primitive.
    for feature in [BackendFeature::MipStart, BackendFeature::PartialMipStart] {
        assert!(
            set.supports(feature),
            "P28-qualified MIP start feature {:?} must be Native (SM-08.7)",
            feature
        );
    }

    let unqualified_m3 = [
        BackendFeature::MultipleMipStarts,
        BackendFeature::VariableHints,
        BackendFeature::InitialBasis,
        BackendFeature::FeasibilityRelaxation,
        BackendFeature::Indicator,
        BackendFeature::Sos1,
        BackendFeature::Sos2,
        BackendFeature::NativePiecewiseLinear,
        BackendFeature::NativeMultiObjective,
    ];
    for feature in unqualified_m3 {
        assert!(
            !set.supports(feature),
            "M3 feature {:?} must be Unsupported",
            feature
        );
    }
    assert_eq!(set.supports(BackendFeature::Iis), cfg!(feature = "bundled"));
}
