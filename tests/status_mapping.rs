//! P21 Task 1 — Unified status and metadata mapping tests.
//!
//! Every backend `TerminationStatus` variant is mapped into a `SolveStatus`
//! (returned inside an `Ok(Solution)`) or into a `SolveError` (returned as
//! `Err`). Mathematical outcomes stay in the status; inability to perform or
//! interpret the solve becomes an error (API-03.3).
//!
//! Also covers `SolveMetadata` (backend name, model revision, effective
//! configuration, synchronization mode) and the `SolverStatus` alias of
//! `SolveStatus`.

use roml::solver::backend::{BackendError, ErrorCategory, HealthEffect, TerminationStatus};
use roml::{SolveError, SolveMetadata, SolveStatus, SolverStatus, SynchronizationMode};

/// Exhaustive table: every current `TerminationStatus` variant maps to a
/// `SolveStatus` or to `Err(SolveError::Status(..))`.
#[test]
fn every_termination_status_maps_to_solve_status_or_error() {
    use TerminationStatus::*;

    // The 12 current variants, each with its expected mapping. Keeping the
    // full explicit list here is deliberate: it is the exhaustive table the
    // conversion must preserve (no wildcard in the implementation).
    let cases: &[(TerminationStatus, Result<SolveStatus, SolveError>)] = &[
        (Optimal, Ok(SolveStatus::Optimal)),
        (Feasible, Ok(SolveStatus::Feasible)),
        (Infeasible, Ok(SolveStatus::Infeasible)),
        (Unbounded, Ok(SolveStatus::Unbounded)),
        (
            InfeasibleOrUnbounded,
            Ok(SolveStatus::InfeasibleOrUnbounded),
        ),
        (TimeLimit, Ok(SolveStatus::TimeLimit)),
        (IterationLimit, Ok(SolveStatus::IterationLimit)),
        (NodeLimit, Ok(SolveStatus::NodeLimit)),
        (Interrupted, Ok(SolveStatus::Interrupted)),
        (NumericalIssue, Ok(SolveStatus::Numerical)),
        (Error, Err(SolveError::Status(Error))),
        (Unknown, Err(SolveError::Status(Unknown))),
    ];

    for (termination, expected) in cases {
        let got = SolveStatus::from_termination(*termination);
        match (expected, &got) {
            (Ok(status), Ok(got_status)) => {
                assert_eq!(
                    got_status, status,
                    "{termination:?} mapped to {got_status:?}"
                );
            }
            (Err(_), Err(got_err)) => {
                assert!(
                    matches!(got_err, SolveError::Status(_)),
                    "{termination:?} mapped to non-status error {got_err:?}"
                );
            }
            _ => panic!(
                "{termination:?} mapped to {:?}, expected {:?}",
                got, expected
            ),
        }
    }
}

/// Golden-path: optimal terminates in an `Ok(SolveStatus::Optimal)`.
#[test]
fn optimal_maps_to_optimal_status() {
    assert_eq!(
        SolveStatus::from_termination(TerminationStatus::Optimal),
        Ok(SolveStatus::Optimal)
    );
    assert!(SolveStatus::Optimal.is_optimal());
}

/// Feasible (feasible limit / MIP incumbent, not proven optimal) is its own
/// status distinct from optimal.
#[test]
fn feasible_limit_maps_to_feasible() {
    assert_eq!(
        SolveStatus::from_termination(TerminationStatus::Feasible),
        Ok(SolveStatus::Feasible)
    );
    assert!(!SolveStatus::Feasible.is_optimal());
}

/// Infeasible is a mathematical outcome: `Ok(SolveStatus::Infeasible)`.
#[test]
fn infeasible_maps_to_infeasible_status() {
    assert_eq!(
        SolveStatus::from_termination(TerminationStatus::Infeasible),
        Ok(SolveStatus::Infeasible)
    );
}

/// Unbounded is a mathematical outcome: `Ok(SolveStatus::Unbounded)`.
#[test]
fn unbounded_maps_to_unbounded_status() {
    assert_eq!(
        SolveStatus::from_termination(TerminationStatus::Unbounded),
        Ok(SolveStatus::Unbounded)
    );
}

/// Infeasible-or-unbounded preserves the solver's reported ambiguity.
#[test]
fn infeasible_or_unbounded_maps_to_ambiguous_status() {
    assert_eq!(
        SolveStatus::from_termination(TerminationStatus::InfeasibleOrUnbounded),
        Ok(SolveStatus::InfeasibleOrUnbounded)
    );
}

/// Interrupted (e.g. via callback) is a distinct status.
#[test]
fn interrupted_maps_to_interrupted_status() {
    assert_eq!(
        SolveStatus::from_termination(TerminationStatus::Interrupted),
        Ok(SolveStatus::Interrupted)
    );
}

/// Numerical difficulty is preserved as a distinct status with no primal
/// values (the backend reports no solution for it).
#[test]
fn numerical_maps_to_numerical_status() {
    assert_eq!(
        SolveStatus::from_termination(TerminationStatus::NumericalIssue),
        Ok(SolveStatus::Numerical)
    );
}

/// The three limit statuses are each preserved.
#[test]
fn limit_statuses_are_preserved() {
    assert_eq!(
        SolveStatus::from_termination(TerminationStatus::TimeLimit),
        Ok(SolveStatus::TimeLimit)
    );
    assert_eq!(
        SolveStatus::from_termination(TerminationStatus::IterationLimit),
        Ok(SolveStatus::IterationLimit)
    );
    assert_eq!(
        SolveStatus::from_termination(TerminationStatus::NodeLimit),
        Ok(SolveStatus::NodeLimit)
    );
}

/// Solver `Error` termination means the solve could not be interpreted —
/// it is an operational failure (`Err`), not a `Solution`.
#[test]
fn error_termination_maps_to_solve_error() {
    let err = SolveStatus::from_termination(TerminationStatus::Error)
        .expect_err("Error termination must produce Err(SolveError)");
    assert!(
        matches!(err, SolveError::Status(TerminationStatus::Error)),
        "unexpected error: {err:?}"
    );
}

/// Unknown termination cannot be interpreted — `Err(SolveError)`.
#[test]
fn unknown_termination_maps_to_solve_error() {
    let err = SolveStatus::from_termination(TerminationStatus::Unknown)
        .expect_err("Unknown termination must produce Err(SolveError)");
    assert!(
        matches!(err, SolveError::Status(TerminationStatus::Unknown)),
        "unexpected error: {err:?}"
    );
}

/// License failures from the backend surface as `SolveError::License`,
/// preserving category and terminal health effect (API-03.2, API-02.4).
#[test]
fn license_backend_error_maps_to_license_solve_error() {
    let backend = BackendError::license_failure("HiGHS license expired");
    let err = SolveError::from_backend(false, backend.clone());
    assert!(matches!(err, SolveError::License(_)));
    assert_eq!(err.category(), Some(ErrorCategory::LicenseFailure));
    assert_eq!(err.health_effect(), Some(HealthEffect::Terminal));
    assert_eq!(err.backend(), Some(&backend));
}

/// A non-license backend error maps to `SolveError::Solve` and retains the
/// backend's identity (message), operation label, category, and health effect
/// (API-02.4).
#[test]
fn backend_error_maps_to_solve_error_preserving_identity_and_category() {
    let backend = BackendError::with_code(
        "Highs_run failed with status -7",
        ErrorCategory::Internal,
        HealthEffect::Recoverable,
        -7,
    );
    let err = SolveError::from_backend(false, backend.clone());
    assert!(matches!(err, SolveError::Solve(_)));
    assert_eq!(err.category(), Some(ErrorCategory::Internal));
    assert_eq!(err.health_effect(), Some(HealthEffect::Recoverable));
    let preserved = err.backend().expect("backend error must be preserved");
    assert_eq!(preserved, &backend);
    assert_eq!(preserved.native_code, Some(-7));
    assert!(preserved.message.contains("Highs_run"));
}

/// Synchronization failures map to `SolveError::Synchronization`.
#[test]
fn synchronization_backend_error_maps_to_synchronization_variant() {
    let backend = BackendError::new(
        "delta batch base mismatch",
        ErrorCategory::InvalidInput,
        HealthEffect::Recoverable,
    );
    let err = SolveError::from_backend(true, backend);
    assert!(matches!(err, SolveError::Synchronization(_)));
}

/// `SolverStatus` is an alias of `SolveStatus` — the same type, so both
/// spellings work and compare equal.
#[test]
fn solver_status_is_alias_of_solve_status() {
    fn same_type(_: &dyn std::any::Any) {}
    // Compile-time proof: the two names name the same type.
    let s: SolverStatus = SolveStatus::Optimal;
    assert_eq!(s, SolveStatus::Optimal);
    let t: SolveStatus = SolverStatus::Infeasible;
    assert_eq!(t, SolveStatus::Infeasible);
    assert_eq!(SolverStatus::default(), SolveStatus::Unknown);
    same_type(&SolverStatus::Optimal);
    same_type(&SolveStatus::Optimal);
}

/// `Solution::status()` reports a `SolveStatus`, and `metadata()` exposes the
/// `SolveMetadata` (backend name, revision, effective config, sync mode).
#[test]
fn solution_exposes_solve_status_and_solve_metadata() {
    use roml::revision::ModelRevision;
    use roml::solver::request::EffectiveConfig;
    use roml::Solution;

    let metadata = SolveMetadata {
        backend_name: "HiGHS 1.15.0".to_string(),
        model_revision: ModelRevision::from_u64(3),
        effective_configuration: EffectiveConfig::default(),
        synchronization: SynchronizationMode::Delta,
        ..SolveMetadata::default()
    };

    let solution = Solution::new(SolveStatus::Optimal).with_metadata(metadata.clone());
    assert_eq!(solution.status(), SolveStatus::Optimal);
    assert!(solution.status().is_optimal());
    assert_eq!(solution.metadata(), &metadata);
    assert_eq!(
        solution.metadata().synchronization,
        SynchronizationMode::Delta
    );
    assert_eq!(
        solution.metadata().model_revision,
        ModelRevision::from_u64(3)
    );
    assert_eq!(solution.metadata().backend_name, "HiGHS 1.15.0");
}

/// `SolveMetadata` default is a valid no-op metadata value.
#[test]
fn solve_metadata_default_is_valid() {
    use roml::revision::ModelRevision;
    let m = SolveMetadata::default();
    assert_eq!(m.model_revision, ModelRevision::ZERO);
    assert_eq!(m.synchronization, SynchronizationMode::NoChange);
    assert!(m.effective_configuration.adjustments.is_empty());
    assert!(m.effective_configuration.rejections.is_empty());
}

/// All three synchronization modes exist and are distinct.
#[test]
fn synchronization_modes_are_distinct() {
    assert_ne!(SynchronizationMode::Delta, SynchronizationMode::Rebuild);
    assert_ne!(SynchronizationMode::Rebuild, SynchronizationMode::NoChange);
    assert_ne!(SynchronizationMode::NoChange, SynchronizationMode::Delta);
}
