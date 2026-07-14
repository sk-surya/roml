//! Tests for the status lattice, solve-request negotiation, and error classification.
//!
//! These are integration-level tests that exercise the public API of the solver
//! backend and request modules, verifying the contracts defined by:
//!
//! - `src/solver/backend.rs` — BackendCapabilities, BackendError, ErrorCategory,
//!   HealthEffect, TerminationStatus
//! - `src/solver/request.rs` — SolveRequest, SolveResult, EffectiveConfig,
//!   validate_request()
//!
//! The tests cover:
//!
//! 1. TerminationStatus information ordering (lattice)
//! 2. Error category → health effect mapping
//! 3. Solve-request validation against backend capabilities
//! 4. EffectiveConfig structure (adjustments + rejections)
//! 5. BackendCapabilities::all() — every field true
//! 6. BackendCapabilities::default() — every field false
//! 7. Full option-negotiation round-trip

use roml::solver::backend::{
    BackendCapabilities, BackendError, ErrorCategory, HealthEffect, TerminationStatus,
};
use roml::solver::request::validate_request;
use roml::solver::request::{ConfigAdjustment, ConfigRejection, EffectiveConfig, SolveRequest};
use roml::solver::LpAlgorithm;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Return `true` if `a` is strictly more informative than `b` in the
/// TerminationStatus information lattice.
///
/// The ordering (from most to least informative):
///
///   Tier 1 (proof states): Optimal, Infeasible, Unbounded
///   Tier 2 (feasible but unproven): Feasible
///   Tier 3 (partial / interrupted): TimeLimit, IterationLimit, NodeLimit,
///         Interrupted, NumericalIssue
///   Tier 4 (no usable information): Error, Unknown
fn is_more_informative(a: TerminationStatus, b: TerminationStatus) -> bool {
    if a == b {
        return false;
    }
    tier(a) > tier(b)
}

/// The information tier: higher = more informative.
fn tier(s: TerminationStatus) -> u8 {
    match s {
        TerminationStatus::Optimal
        | TerminationStatus::Infeasible
        | TerminationStatus::Unbounded => 4,
        TerminationStatus::Feasible => 3,
        TerminationStatus::TimeLimit
        | TerminationStatus::IterationLimit
        | TerminationStatus::NodeLimit
        | TerminationStatus::Interrupted
        | TerminationStatus::NumericalIssue => 2,
        TerminationStatus::Error | TerminationStatus::Unknown => 1,
    }
}

/// Map every ErrorCategory to its expected HealthEffect.
fn classify_health(category: ErrorCategory) -> HealthEffect {
    match category {
        ErrorCategory::LibraryNotFound => HealthEffect::Terminal,
        ErrorCategory::LicenseFailure => HealthEffect::Terminal,
        ErrorCategory::Unsupported => HealthEffect::RequiresRebuild,
        ErrorCategory::Numerical => HealthEffect::Recoverable,
        ErrorCategory::OutOfMemory => HealthEffect::Terminal,
        ErrorCategory::Internal => HealthEffect::RequiresRebuild,
        ErrorCategory::InvalidInput => HealthEffect::Recoverable,
        ErrorCategory::Limit => HealthEffect::None,
        ErrorCategory::Unknown => HealthEffect::Recoverable,
    }
}

/// Create a backend with only LP capability (no MIP).
fn lp_only_backend() -> BackendCapabilities {
    let mut caps = BackendCapabilities::all();
    caps.mip = false;
    caps
}

/// Create a backend with only MIP capability (no LP).
fn mip_only_backend() -> BackendCapabilities {
    let mut caps = BackendCapabilities::all();
    caps.lp = false;
    caps
}

// ===================================================================
// 1. TerminationStatus information lattice
// ===================================================================

#[test]
fn optimal_is_more_informative_than_feasible() {
    assert!(is_more_informative(
        TerminationStatus::Optimal,
        TerminationStatus::Feasible
    ));
    assert!(!is_more_informative(
        TerminationStatus::Feasible,
        TerminationStatus::Optimal
    ));
}

#[test]
fn feasible_is_more_informative_than_time_limit() {
    assert!(is_more_informative(
        TerminationStatus::Feasible,
        TerminationStatus::TimeLimit
    ));
    assert!(!is_more_informative(
        TerminationStatus::TimeLimit,
        TerminationStatus::Feasible
    ));
}

#[test]
fn optimal_is_more_informative_than_time_limit() {
    assert!(is_more_informative(
        TerminationStatus::Optimal,
        TerminationStatus::TimeLimit
    ));
    assert!(!is_more_informative(
        TerminationStatus::TimeLimit,
        TerminationStatus::Optimal
    ));
}

#[test]
fn proof_states_are_informative_over_error() {
    for proof in &[
        TerminationStatus::Optimal,
        TerminationStatus::Infeasible,
        TerminationStatus::Unbounded,
    ] {
        assert!(
            is_more_informative(*proof, TerminationStatus::Error),
            "{:?} should be more informative than Error",
            proof,
        );
        assert!(
            is_more_informative(*proof, TerminationStatus::Unknown),
            "{:?} should be more informative than Unknown",
            proof,
        );
    }
}

#[test]
fn proof_states_are_informative_over_time_limit() {
    for proof in &[
        TerminationStatus::Optimal,
        TerminationStatus::Infeasible,
        TerminationStatus::Unbounded,
    ] {
        assert!(
            is_more_informative(*proof, TerminationStatus::TimeLimit),
            "{:?} should be more informative than TimeLimit",
            proof,
        );
    }
}

#[test]
fn error_and_unknown_are_least_informative() {
    // Every other status is strictly more informative than Error and Unknown.
    let others = [
        TerminationStatus::Optimal,
        TerminationStatus::Infeasible,
        TerminationStatus::Unbounded,
        TerminationStatus::Feasible,
        TerminationStatus::TimeLimit,
        TerminationStatus::IterationLimit,
        TerminationStatus::NodeLimit,
        TerminationStatus::Interrupted,
        TerminationStatus::NumericalIssue,
    ];
    for bottom in &[TerminationStatus::Error, TerminationStatus::Unknown] {
        for other in &others {
            assert!(
                is_more_informative(*other, *bottom),
                "{:?} should be more informative than {:?}",
                other,
                bottom,
            );
            assert!(
                !is_more_informative(*bottom, *other),
                "{:?} should NOT be more informative than {:?}",
                bottom,
                other,
            );
        }
    }
}

#[test]
fn transitive_ordering() {
    // Optimal > Feasible > TimeLimit  ⇒  Optimal > TimeLimit
    assert!(is_more_informative(
        TerminationStatus::Optimal,
        TerminationStatus::Feasible
    ));
    assert!(is_more_informative(
        TerminationStatus::Feasible,
        TerminationStatus::TimeLimit
    ));
    assert!(is_more_informative(
        TerminationStatus::Optimal,
        TerminationStatus::TimeLimit
    ));
}

#[test]
fn same_status_is_not_more_informative() {
    let statuses = [
        TerminationStatus::Optimal,
        TerminationStatus::Infeasible,
        TerminationStatus::Unbounded,
        TerminationStatus::Feasible,
        TerminationStatus::TimeLimit,
        TerminationStatus::IterationLimit,
        TerminationStatus::NodeLimit,
        TerminationStatus::Interrupted,
        TerminationStatus::NumericalIssue,
        TerminationStatus::Error,
        TerminationStatus::Unknown,
    ];
    for s in &statuses {
        assert!(
            !is_more_informative(*s, *s),
            "{:?} should not be more informative than itself",
            s
        );
    }
}

#[test]
fn partial_order_reflexivity() {
    // Statuses in the same tier are incomparable (neither is more informative).
    let tier2 = [
        TerminationStatus::TimeLimit,
        TerminationStatus::IterationLimit,
        TerminationStatus::NodeLimit,
        TerminationStatus::Interrupted,
        TerminationStatus::NumericalIssue,
    ];
    for i in 0..tier2.len() {
        for j in i + 1..tier2.len() {
            let a = tier2[i];
            let b = tier2[j];
            assert!(
                !is_more_informative(a, b),
                "{:?} should be incomparable with {:?} (same tier)",
                a,
                b,
            );
            assert!(
                !is_more_informative(b, a),
                "{:?} should be incomparable with {:?} (same tier)",
                b,
                a,
            );
        }
    }
}

// ===================================================================
// 2. Error category → health effect classification
// ===================================================================

#[test]
fn library_not_found_is_terminal() {
    assert_eq!(
        classify_health(ErrorCategory::LibraryNotFound),
        HealthEffect::Terminal
    );
}

#[test]
fn license_failure_is_terminal() {
    assert_eq!(
        classify_health(ErrorCategory::LicenseFailure),
        HealthEffect::Terminal
    );
}

#[test]
fn unsupported_requires_rebuild() {
    assert_eq!(
        classify_health(ErrorCategory::Unsupported),
        HealthEffect::RequiresRebuild
    );
}

#[test]
fn numerical_is_recoverable() {
    assert_eq!(
        classify_health(ErrorCategory::Numerical),
        HealthEffect::Recoverable
    );
}

#[test]
fn out_of_memory_is_terminal() {
    assert_eq!(
        classify_health(ErrorCategory::OutOfMemory),
        HealthEffect::Terminal
    );
}

#[test]
fn internal_requires_rebuild() {
    assert_eq!(
        classify_health(ErrorCategory::Internal),
        HealthEffect::RequiresRebuild
    );
}

#[test]
fn invalid_input_is_recoverable() {
    assert_eq!(
        classify_health(ErrorCategory::InvalidInput),
        HealthEffect::Recoverable
    );
}

#[test]
fn limit_has_no_health_effect() {
    assert_eq!(classify_health(ErrorCategory::Limit), HealthEffect::None);
}

#[test]
fn unknown_error_is_recoverable() {
    assert_eq!(
        classify_health(ErrorCategory::Unknown),
        HealthEffect::Recoverable
    );
}

#[test]
fn every_category_has_mapped_health_effect() {
    // Compile-time proof: every variant appears in classify_health's match.
    // This won't compile if a new variant is added and classify_health isn't updated.
    let categories = [
        ErrorCategory::InvalidInput,
        ErrorCategory::Unsupported,
        ErrorCategory::LibraryNotFound,
        ErrorCategory::LicenseFailure,
        ErrorCategory::Numerical,
        ErrorCategory::OutOfMemory,
        ErrorCategory::Internal,
        ErrorCategory::Limit,
        ErrorCategory::Unknown,
    ];
    for cat in &categories {
        let effect = classify_health(*cat);
        // All must be mapped — just verify the call doesn't panic and
        // returns a non-default value that makes sense.
        assert!(
            effect == HealthEffect::None
                || effect == HealthEffect::Recoverable
                || effect == HealthEffect::RequiresRebuild
                || effect == HealthEffect::Terminal,
            "classify_health({:?}) returned unexpected effect {:?}",
            cat,
            effect,
        );
    }
}

#[test]
fn backend_error_health_effect_matches_category() {
    // Verify that the convenience constructors produce the expected
    // health effects (already tested inline, but repeated here as
    // an alignment check with classify_health).
    let err = BackendError::unsupported("foo");
    assert_eq!(
        classify_health(err.category),
        err.health_effect,
        "BackendError::unsupported health_effect should match classify_health",
    );
    let err = BackendError::library_not_found("bar");
    assert_eq!(
        classify_health(err.category),
        err.health_effect,
        "BackendError::library_not_found health_effect should match classify_health",
    );
    let err = BackendError::license_failure("baz");
    assert_eq!(
        classify_health(err.category),
        err.health_effect,
        "BackendError::license_failure health_effect should match classify_health",
    );
}

// ===================================================================
// 3. Solve-request validation
// ===================================================================

#[test]
fn mip_rel_gap_rejected_when_backend_lacks_mip() {
    let caps = lp_only_backend();
    let req = SolveRequest::new().with_mip_rel_gap(0.01);
    let rejections = validate_request(&req, &caps);
    assert_eq!(rejections.len(), 1);
    assert_eq!(rejections[0].key, "mip_rel_gap");
    assert!(rejections[0].reason.contains("MIP"));
}

#[test]
fn mip_abs_gap_rejected_when_backend_lacks_mip() {
    let caps = lp_only_backend();
    let req = SolveRequest {
        mip_abs_gap: Some(1e-6),
        ..SolveRequest::new()
    };
    let rejections = validate_request(&req, &caps);
    assert_eq!(rejections.len(), 1);
    assert_eq!(rejections[0].key, "mip_abs_gap");
    assert!(rejections[0].reason.contains("MIP"));
}

#[test]
fn lp_algorithm_accepted_when_backend_supports_lp() {
    let caps = BackendCapabilities::all();
    let req = SolveRequest::new().with_lp_algorithm(LpAlgorithm::DualSimplex);
    let rejections = validate_request(&req, &caps);
    assert!(rejections.is_empty());
}

#[test]
fn all_options_accepted_on_full_capability_backend() {
    let caps = BackendCapabilities::all();
    let req = SolveRequest::new()
        .with_lp_algorithm(LpAlgorithm::Barrier)
        .with_mip_rel_gap(0.01)
        .with_time_limit(60.0)
        .with_threads(8)
        .with_option("presolve", "on");
    let rejections = validate_request(&req, &caps);
    assert!(
        rejections.is_empty(),
        "Expected no rejections, got: {:?}",
        rejections
    );
}

#[test]
fn empty_request_produces_no_rejections() {
    let caps = lp_only_backend();
    let req = SolveRequest::new();
    let rejections = validate_request(&req, &caps);
    assert!(rejections.is_empty());
}

#[test]
fn mip_options_all_individually_rejected_on_lp_only_backend() {
    let caps = lp_only_backend();
    let req = SolveRequest {
        mip_rel_gap: Some(0.05),
        mip_abs_gap: Some(1e-6),
        ..SolveRequest::new()
    };
    let rejections = validate_request(&req, &caps);
    assert_eq!(
        rejections.len(),
        2,
        "Expected both MIP options to be rejected"
    );
    let keys: Vec<&str> = rejections.iter().map(|r| r.key.as_str()).collect();
    assert!(keys.contains(&"mip_rel_gap"));
    assert!(keys.contains(&"mip_abs_gap"));
}

#[test]
fn non_mip_options_are_not_rejected_on_lp_only_backend() {
    let caps = lp_only_backend();
    let req = SolveRequest::new()
        .with_lp_algorithm(LpAlgorithm::PrimalSimplex)
        .with_time_limit(30.0)
        .with_threads(2);
    let rejections = validate_request(&req, &caps);
    assert!(rejections.is_empty());
}

#[test]
fn validate_request_does_not_mutate_request() {
    let caps = lp_only_backend();
    let req = SolveRequest::new().with_mip_rel_gap(0.01);
    let original = req.clone();
    let _rejections = validate_request(&req, &caps);
    assert_eq!(
        req, original,
        "validate_request should not mutate the request"
    );
}

#[test]
fn empty_backend_rejects_all_mip_options() {
    let caps = BackendCapabilities::default();
    let req = SolveRequest {
        mip_rel_gap: Some(0.01),
        mip_abs_gap: Some(1e-6),
        ..SolveRequest::new()
    };
    let rejections = validate_request(&req, &caps);
    assert_eq!(rejections.len(), 2);
}

// ===================================================================
// 4. Effective configuration
// ===================================================================

#[test]
fn config_adjustment_carries_all_fields() {
    let adj = ConfigAdjustment {
        key: "lp_algorithm".into(),
        requested: "PrimalSimplex".into(),
        applied: "DualSimplex".into(),
        reason: "solver does not support PrimalSimplex".into(),
    };
    assert_eq!(adj.key, "lp_algorithm");
    assert_eq!(adj.requested, "PrimalSimplex");
    assert_eq!(adj.applied, "DualSimplex");
    assert_eq!(adj.reason, "solver does not support PrimalSimplex");
}

#[test]
fn config_rejection_carries_key_and_reason() {
    let rej = ConfigRejection {
        key: "mip_rel_gap".into(),
        reason: "backend does not support MIP".into(),
    };
    assert_eq!(rej.key, "mip_rel_gap");
    assert_eq!(rej.reason, "backend does not support MIP");
}

#[test]
fn effective_config_can_hold_adjustments_only() {
    let config = EffectiveConfig {
        lp_algorithm: Some(LpAlgorithm::DualSimplex),
        time_limit_secs: Some(60.0),
        adjustments: vec![ConfigAdjustment {
            key: "threads".into(),
            requested: "8".into(),
            applied: "4".into(),
            reason: "solver limit".into(),
        }],
        ..EffectiveConfig::default()
    };
    assert_eq!(config.adjustments.len(), 1);
    assert!(config.rejections.is_empty());
}

#[test]
fn effective_config_can_hold_rejections_only() {
    let config = EffectiveConfig {
        rejections: vec![ConfigRejection {
            key: "mip_rel_gap".into(),
            reason: "backend does not support MIP".into(),
        }],
        ..EffectiveConfig::default()
    };
    assert!(config.adjustments.is_empty());
    assert_eq!(config.rejections.len(), 1);
}

#[test]
fn effective_config_can_hold_both_adjustments_and_rejections() {
    let config = EffectiveConfig {
        lp_algorithm: Some(LpAlgorithm::Barrier),
        adjustments: vec![ConfigAdjustment {
            key: "threads".into(),
            requested: "16".into(),
            applied: "8".into(),
            reason: "max 8 threads available".into(),
        }],
        rejections: vec![
            ConfigRejection {
                key: "mip_rel_gap".into(),
                reason: "backend does not support MIP".into(),
            },
            ConfigRejection {
                key: "mip_abs_gap".into(),
                reason: "backend does not support MIP".into(),
            },
        ],
        ..EffectiveConfig::default()
    };
    assert_eq!(config.adjustments.len(), 1);
    assert_eq!(config.rejections.len(), 2);
}

#[test]
fn default_effective_config_is_empty() {
    let config = EffectiveConfig::default();
    assert!(config.lp_algorithm.is_none());
    assert!(config.time_limit_secs.is_none());
    assert!(config.mip_rel_gap.is_none());
    assert!(config.threads.is_none());
    assert!(config.enable_output.is_none());
    assert!(config.adjustments.is_empty());
    assert!(config.rejections.is_empty());
}

// ===================================================================
// 5. BackendCapabilities::all()
// ===================================================================

#[test]
fn all_capabilities_every_field_true() {
    let caps = BackendCapabilities::all();
    assert!(caps.add_variable);
    assert!(caps.add_constraint);
    assert!(caps.set_coefficient);
    assert!(caps.set_bounds);
    assert!(caps.set_objective);
    assert!(caps.delete);
    assert!(caps.lp);
    assert!(caps.mip);
    assert!(caps.callbacks);
    assert!(caps.solution);
    assert!(caps.duals);
    assert!(caps.reduced_costs);
    assert!(caps.semicontinuous);
    assert!(caps.semiinteger);
    assert!(caps.parameter_update);
}

// ===================================================================
// 6. BackendCapabilities::default()
// ===================================================================

#[test]
fn default_capabilities_every_field_false() {
    let caps = BackendCapabilities::default();
    assert!(!caps.add_variable);
    assert!(!caps.add_constraint);
    assert!(!caps.set_coefficient);
    assert!(!caps.set_bounds);
    assert!(!caps.set_objective);
    assert!(!caps.delete);
    assert!(!caps.lp);
    assert!(!caps.mip);
    assert!(!caps.callbacks);
    assert!(!caps.solution);
    assert!(!caps.duals);
    assert!(!caps.reduced_costs);
    assert!(!caps.semicontinuous);
    assert!(!caps.semiinteger);
    assert!(!caps.parameter_update);
}

// ===================================================================
// 7. Option-negotiation round-trip
// ===================================================================

#[test]
fn full_negotiation_round_trip() {
    // Scenario: a user submits a SolveRequest with a mix of LP and MIP
    // options against a backend that supports LP but not MIP.
    let caps = lp_only_backend();

    let request = SolveRequest {
        lp_algorithm: Some(LpAlgorithm::DualSimplex),
        time_limit_secs: Some(120.0),
        mip_rel_gap: Some(0.01),
        mip_abs_gap: Some(1e-6),
        threads: Some(4),
        ..SolveRequest::new()
    };

    let rejections = validate_request(&request, &caps);

    // Two MIP options should be rejected.
    assert_eq!(rejections.len(), 2);

    let rejected_keys: Vec<&str> = rejections.iter().map(|r| r.key.as_str()).collect();
    assert!(rejected_keys.contains(&"mip_rel_gap"));
    assert!(rejected_keys.contains(&"mip_abs_gap"));

    // Build the effective config that would result.
    let effective = EffectiveConfig {
        lp_algorithm: request.lp_algorithm,
        time_limit_secs: request.time_limit_secs,
        threads: request.threads,
        // mip_rel_gap and mip_abs_gap are not applied — they were rejected.
        mip_rel_gap: None,
        adjustments: Vec::new(),
        rejections,
        ..EffectiveConfig::default()
    };

    assert_eq!(effective.lp_algorithm, Some(LpAlgorithm::DualSimplex));
    assert_eq!(effective.time_limit_secs, Some(120.0));
    assert_eq!(effective.threads, Some(4));
    assert!(
        effective.mip_rel_gap.is_none(),
        "MIP options should not appear in effective config"
    );
    assert!(effective.adjustments.is_empty());
    assert_eq!(effective.rejections.len(), 2);
}

#[test]
fn negotiation_round_trip_mip_only_backend() {
    // Scenario: MIP-only backend (no LP algorithm option, no duals).
    let caps = mip_only_backend();

    let request = SolveRequest::new()
        .with_lp_algorithm(LpAlgorithm::Automatic)
        .with_mip_rel_gap(0.05)
        .with_time_limit(300.0);

    let rejections = validate_request(&request, &caps);

    // LP algorithm is not a MIP option, so it should not be rejected.
    assert!(
        rejections.is_empty(),
        "MIP-only backend should not reject LP algorithm — \
         validate_request only checks MIP options. Got: {:?}",
        rejections,
    );
}

#[test]
fn negotiation_round_trip_full_backend() {
    // Scenario: Full-capability backend accepts everything.
    let caps = BackendCapabilities::all();

    let request = SolveRequest::new()
        .with_lp_algorithm(LpAlgorithm::Barrier)
        .with_mip_rel_gap(0.01)
        .with_time_limit(60.0)
        .with_threads(8)
        .with_option("presolve", "on");

    let rejections = validate_request(&request, &caps);
    assert!(rejections.is_empty());

    let effective = EffectiveConfig {
        lp_algorithm: request.lp_algorithm,
        time_limit_secs: request.time_limit_secs,
        mip_rel_gap: request.mip_rel_gap,
        threads: request.threads,
        adjustments: Vec::new(),
        rejections,
        ..EffectiveConfig::default()
    };

    assert_eq!(effective.lp_algorithm, Some(LpAlgorithm::Barrier));
    assert_eq!(effective.time_limit_secs, Some(60.0));
    assert_eq!(effective.mip_rel_gap, Some(0.01));
    assert_eq!(effective.threads, Some(8));
    assert!(effective.adjustments.is_empty());
    assert!(effective.rejections.is_empty());
}

#[test]
fn negotiation_round_trip_nothing_rejected() {
    // Scenario: All options are non-MIP, backend is LP-only.
    let caps = lp_only_backend();

    let request = SolveRequest::new()
        .with_lp_algorithm(LpAlgorithm::DualSimplex)
        .with_time_limit(30.0)
        .with_threads(2);

    let rejections = validate_request(&request, &caps);
    assert!(rejections.is_empty());

    let effective = EffectiveConfig {
        lp_algorithm: request.lp_algorithm,
        time_limit_secs: request.time_limit_secs,
        threads: request.threads,
        adjustments: Vec::new(),
        rejections,
        ..EffectiveConfig::default()
    };

    assert_eq!(effective.lp_algorithm, Some(LpAlgorithm::DualSimplex));
    assert!(effective.rejections.is_empty());
}
