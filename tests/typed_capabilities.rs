//! Typed capability characterization and migration tests (P26 Task 6).
//!
//! Characterizes the legacy flat `BackendCapabilities` declarations and the
//! current `validate_request` rejections **before** the typed
//! `BackendCapabilitySet` replaces them (D10, SM-04). The flat
//! characterization tests pass on the untouched tree and pin the intended
//! mapping; the typed tests drive the migration.

use roml::compiler::capability::{
    BackendCapabilitySet, BackendFeature, FeatureLimitations, FeatureSupport, SupportLevel,
};
use roml::solver::backend::BackendCapabilities;
use roml::solver::request::{validate_request, SolveRequest};

// ── Legacy flat characterization ───────────────────────────────────────────────

/// Characterize the legacy full-capability backend: every flat field is true.
///
/// This is the mapping source for the typed `Lp`, `Mip`, `IncrementalBounds`,
/// `IncrementalRows`, and `IncrementalCoefficients` features. Flat-only fields
/// (`solution`, `duals`, `reduced_costs`, `callbacks`, `delete`,
/// `set_objective`, `parameter_update`, `semicontinuous`, `semiinteger`) have
/// no typed `BackendFeature` equivalent and stay flat-only.
#[test]
fn characterize_legacy_all_flat_capabilities() {
    let caps = BackendCapabilities::all();
    assert!(caps.lp);
    assert!(caps.mip);
    assert!(caps.add_variable);
    assert!(caps.add_constraint);
    assert!(caps.set_coefficient);
    assert!(caps.set_bounds);
    assert!(caps.set_objective);
    assert!(caps.delete);
    assert!(caps.callbacks);
    assert!(caps.solution);
    assert!(caps.duals);
    assert!(caps.reduced_costs);
    assert!(caps.semicontinuous);
    assert!(caps.semiinteger);
    assert!(caps.parameter_update);
}

/// Characterize the migrated `validate_request` rejection rule against the
/// typed set: a MIP option is rejected exactly when `BackendFeature::Mip` is
/// not declared native. This corresponds to the legacy flat `mip == false`
/// rejection recorded on the untouched tree.
#[test]
fn characterize_validate_request_rejects_mip_when_typed_mip_unsupported() {
    let mut caps = BackendCapabilitySet::new();
    caps.set(
        BackendFeature::Lp,
        FeatureSupport {
            level: SupportLevel::Native,
            limitations: FeatureLimitations::default(),
        },
    );

    let req = SolveRequest::new().with_mip_rel_gap(0.01);
    let rejections = validate_request(&req, &caps);
    assert_eq!(rejections.len(), 1);
    assert_eq!(rejections[0].key, "mip_rel_gap");
    assert!(rejections[0].reason.contains("MIP"));
}

#[test]
fn characterize_validate_request_accepts_mip_when_typed_mip_native() {
    let mut caps = BackendCapabilitySet::new();
    caps.set(
        BackendFeature::Lp,
        FeatureSupport {
            level: SupportLevel::Native,
            limitations: FeatureLimitations::default(),
        },
    );
    caps.set(
        BackendFeature::Mip,
        FeatureSupport {
            level: SupportLevel::Native,
            limitations: FeatureLimitations::default(),
        },
    );

    let req = SolveRequest::new().with_mip_rel_gap(0.01);
    let rejections = validate_request(&req, &caps);
    assert!(rejections.is_empty());
}

// ── Typed capability mapping (drives the migration) ───────────────────────────

/// The typed set reports `Native` support only for features declared native.
#[test]
fn typed_set_supports_only_native_features() {
    let mut set = BackendCapabilitySet::new();
    set.set(
        BackendFeature::Lp,
        FeatureSupport {
            level: SupportLevel::Native,
            limitations: FeatureLimitations::default(),
        },
    );

    assert!(set.supports(BackendFeature::Lp));
    assert!(!set.supports(BackendFeature::Mip));
}

/// `FeatureSupport` carries the version/model-class limitation surface
/// (SM-04.3): `minimum_version`, `model_classes`, `maximum_count`, `notes`.
#[test]
fn typed_feature_support_carries_limitations() {
    let support = FeatureSupport {
        level: SupportLevel::Unsupported,
        limitations: FeatureLimitations {
            minimum_version: Some("1.15.0".into()),
            model_classes: vec!["lp".into(), "mip".into()],
            maximum_count: Some(4),
            notes: vec!["not qualified in P26".into()],
        },
    };

    assert_eq!(support.level, SupportLevel::Unsupported);
    assert_eq!(
        support.limitations.minimum_version.as_deref(),
        Some("1.15.0")
    );
    assert_eq!(support.limitations.model_classes, vec!["lp", "mip"]);
    assert_eq!(support.limitations.maximum_count, Some(4));
    assert_eq!(support.limitations.notes, vec!["not qualified in P26"]);
}

/// `validate_request` rejects MIP options against a typed set lacking
/// `BackendFeature::Mip` (SM-04.4: unsupported features rejected, never
/// silently ignored).
#[test]
fn validate_request_rejects_mip_against_typed_set_without_mip() {
    let mut set = BackendCapabilitySet::new();
    set.set(
        BackendFeature::Lp,
        FeatureSupport {
            level: SupportLevel::Native,
            limitations: FeatureLimitations::default(),
        },
    );

    let req = SolveRequest::new().with_mip_rel_gap(0.01);
    let rejections = validate_request(&req, &set);
    assert_eq!(rejections.len(), 1);
    assert_eq!(rejections[0].key, "mip_rel_gap");
}

/// `validate_request` accepts MIP options against a typed set that declares
/// `BackendFeature::Mip` native.
#[test]
fn validate_request_accepts_mip_against_typed_set_with_mip() {
    let mut set = BackendCapabilitySet::new();
    set.set(
        BackendFeature::Lp,
        FeatureSupport {
            level: SupportLevel::Native,
            limitations: FeatureLimitations::default(),
        },
    );
    set.set(
        BackendFeature::Mip,
        FeatureSupport {
            level: SupportLevel::Native,
            limitations: FeatureLimitations::default(),
        },
    );

    let req = SolveRequest::new().with_mip_rel_gap(0.01);
    let rejections = validate_request(&req, &set);
    assert!(rejections.is_empty());
}
