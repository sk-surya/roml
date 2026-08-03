//! Typed backend capabilities (D10, SM-04).
//!
//! Feature support is queried by typed [`BackendFeature`] with limitations
//! and version-aware declarations (SM-04.3). Native backend support and ROML
//! bridge support are reported separately (D10, SM-04.2). This module replaces
//! the flat, undifferentiated Boolean capability record with a typed registry
//! keyed by [`BackendFeature`] (SM-04.1).
//!
//! [`CompilationPolicy`] (design §8.1) is co-located here per the packet's
//! "Capabilities and compilation" grouping: the policy governs how the
//! compiler selects between native primitives and portable bridges when a
//! [`BackendCapabilitySet`] gates compilation.

use std::collections::BTreeMap;

/// A typed backend feature (SM-04.1).
///
/// The 17 variants are the interface-contract enumeration from the packet
/// ("Capabilities and compilation"). [`BackendCapabilitySet`] is keyed by this
/// type; support is queried per feature instead of through a growing flat
/// Boolean record (D10).
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BackendFeature {
    /// Linear programming solves.
    Lp,
    /// Mixed-integer programming solves.
    Mip,
    /// Incremental bound changes on existing variables/rows.
    IncrementalBounds,
    /// Incremental addition/removal of constraints (rows).
    IncrementalRows,
    /// Incremental coefficient changes (including objective coefficients).
    IncrementalCoefficients,
    /// Full warm-start solution for a MIP model.
    MipStart,
    /// Partial warm-start solution covering a subset of integer variables.
    PartialMipStart,
    /// Multiple warm-start solutions supplied for one solve.
    MultipleMipStarts,
    /// Variable hint values that guide the MIP search.
    VariableHints,
    /// A user-supplied initial basis.
    InitialBasis,
    /// Irreducible infeasible subsystem analysis.
    Iis,
    /// Feasibility relaxation / repair of an infeasible model.
    FeasibilityRelaxation,
    /// Indicator constraints (binary activation of a row).
    Indicator,
    /// Special ordered set of type 1.
    Sos1,
    /// Special ordered set of type 2.
    Sos2,
    /// Native piecewise-linear constraints.
    NativePiecewiseLinear,
    /// Native multi-objective solve support.
    NativeMultiObjective,
}

/// Whether a backend provides a feature natively.
///
/// Absent features are `Unsupported` by default; the ROML bridge surface is
/// reported separately from this native declaration (D10, SM-04.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SupportLevel {
    /// The backend lacks exact native support for the feature.
    #[default]
    Unsupported,
    /// The backend provides exact native support for the feature.
    Native,
}

/// Version/model-class limitations attached to a feature's support (SM-04.3).
///
/// Capability declarations may vary by backend version and model class; these
/// fields record the bounds of a [`FeatureSupport`] declaration.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FeatureLimitations {
    /// Minimum backend version for which the support declaration holds.
    pub minimum_version: Option<String>,
    /// Model classes the declaration applies to (e.g. `"lp"`, `"mip"`).
    pub model_classes: Vec<String>,
    /// Maximum supported count for countable features (e.g. starts).
    pub maximum_count: Option<usize>,
    /// Free-form notes documenting the declaration's evidence or caveats.
    pub notes: Vec<String>,
}

/// Support declaration for one typed feature.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeatureSupport {
    /// The native support level.
    pub level: SupportLevel,
    /// Version/model-class limitations on the declaration.
    pub limitations: FeatureLimitations,
}

impl FeatureSupport {
    /// Declare the feature natively supported with the given limitations.
    pub fn native(limitations: FeatureLimitations) -> Self {
        Self {
            level: SupportLevel::Native,
            limitations,
        }
    }

    /// Declare the feature unsupported (or unqualified) with the given
    /// limitations.
    pub fn unsupported(limitations: FeatureLimitations) -> Self {
        Self {
            level: SupportLevel::Unsupported,
            limitations,
        }
    }

    /// Whether this declaration reports native support.
    pub fn is_native(&self) -> bool {
        self.level == SupportLevel::Native
    }
}

/// Compilation policy (design §8.1).
///
/// `Auto` prefers a qualified exact native primitive, otherwise an exact
/// portable bridge; `Portable` forces deterministic ROML formulations suitable
/// for solver comparison; `NativeRequired` rejects when the backend lacks exact
/// native support. Per-construct preferences may narrow the global policy but
/// cannot weaken exactness.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompilationPolicy {
    /// Prefer a qualified exact native primitive, otherwise an exact portable
    /// bridge.
    Auto,
    /// Force deterministic ROML formulations suitable for solver comparison.
    Portable,
    /// Reject when the backend lacks exact native support.
    NativeRequired,
}

/// A backend's typed capability set (D10, SM-04.1).
///
/// Keyed by [`BackendFeature`]; each feature carries a [`FeatureSupport`]
/// declaration. Request validation and compilation gate on this set — an
/// unsupported feature is rejected or forces a rebuild, never silently ignored
/// (SM-04.4).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BackendCapabilitySet {
    support: BTreeMap<BackendFeature, FeatureSupport>,
}

impl BackendCapabilitySet {
    /// Create an empty capability set (no declared features).
    pub fn new() -> Self {
        Self {
            support: BTreeMap::new(),
        }
    }

    /// Whether the feature is declared natively supported.
    pub fn supports(&self, feature: BackendFeature) -> bool {
        self.support
            .get(&feature)
            .is_some_and(|support| support.is_native())
    }

    /// The full support declaration for a feature, if present.
    pub fn support(&self, feature: BackendFeature) -> Option<&FeatureSupport> {
        self.support.get(&feature)
    }

    /// Declare (or override) a feature's support declaration.
    ///
    /// Returns `&mut self` so declarations can be chained.
    pub fn set(&mut self, feature: BackendFeature, support: FeatureSupport) -> &mut Self {
        self.support.insert(feature, support);
        self
    }

    /// Iterate over features declared natively supported.
    pub fn native_features(&self) -> impl Iterator<Item = BackendFeature> + '_ {
        self.support
            .iter()
            .filter(|(_, support)| support.is_native())
            .map(|(feature, _)| *feature)
    }

    /// Iterate over features declared unsupported (or unqualified).
    pub fn unsupported_features(&self) -> impl Iterator<Item = BackendFeature> + '_ {
        self.support
            .iter()
            .filter(|(_, support)| !support.is_native())
            .map(|(feature, _)| *feature)
    }

    /// Number of declared features.
    pub fn len(&self) -> usize {
        self.support.len()
    }

    /// Whether no features are declared.
    pub fn is_empty(&self) -> bool {
        self.support.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_set_reports_native_support() {
        let mut set = BackendCapabilitySet::new();
        set.set(
            BackendFeature::Lp,
            FeatureSupport::native(Default::default()),
        );

        assert!(set.supports(BackendFeature::Lp));
        assert!(!set.supports(BackendFeature::Mip));
    }

    #[test]
    fn capability_set_reports_unsupported_features() {
        let mut set = BackendCapabilitySet::new();
        set.set(
            BackendFeature::MipStart,
            FeatureSupport::unsupported(Default::default()),
        );

        assert!(!set.supports(BackendFeature::MipStart));
        assert!(!set.supports(BackendFeature::Lp));
    }

    #[test]
    fn feature_support_carries_limitation_fields() {
        let support = FeatureSupport {
            level: SupportLevel::Unsupported,
            limitations: FeatureLimitations {
                minimum_version: Some("1.15.0".into()),
                model_classes: vec!["mip".into()],
                maximum_count: Some(3),
                notes: vec!["not qualified in P26".into()],
            },
        };

        assert_eq!(support.level, SupportLevel::Unsupported);
        assert_eq!(
            support.limitations.minimum_version.as_deref(),
            Some("1.15.0")
        );
        assert_eq!(support.limitations.model_classes, vec!["mip"]);
        assert_eq!(support.limitations.maximum_count, Some(3));
        assert_eq!(support.limitations.notes, vec!["not qualified in P26"]);
    }

    #[test]
    fn capability_set_lists_native_and_unsupported_features() {
        let mut set = BackendCapabilitySet::new();
        set.set(
            BackendFeature::Lp,
            FeatureSupport::native(Default::default()),
        );
        set.set(
            BackendFeature::Mip,
            FeatureSupport::native(Default::default()),
        );
        set.set(
            BackendFeature::Sos1,
            FeatureSupport::unsupported(Default::default()),
        );

        let native: Vec<BackendFeature> = set.native_features().collect();
        assert!(native.contains(&BackendFeature::Lp));
        assert!(native.contains(&BackendFeature::Mip));
        assert!(!native.contains(&BackendFeature::Sos1));

        let unsupported: Vec<BackendFeature> = set.unsupported_features().collect();
        assert!(unsupported.contains(&BackendFeature::Sos1));
        assert!(!unsupported.contains(&BackendFeature::Lp));
    }

    #[test]
    fn compilation_policy_has_three_variants() {
        let _auto = CompilationPolicy::Auto;
        let _portable = CompilationPolicy::Portable;
        let _native_required = CompilationPolicy::NativeRequired;
    }
}
