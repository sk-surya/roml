//! Typed cleanup/rebuild fault evidence for the P30 lifecycle.

use roml::solver::FeasibilityRelaxationError;

#[test]
fn cleanup_error_retains_primary_failure_and_rebuild_requirement() {
    let error = FeasibilityRelaxationError::Cleanup {
        primary: "solve returned malformed candidate".into(),
        cleanup: "rollback verification failed".into(),
        requires_rebuild: true,
    };
    match error {
        FeasibilityRelaxationError::Cleanup {
            primary,
            cleanup,
            requires_rebuild,
        } => {
            assert!(primary.contains("malformed"));
            assert!(cleanup.contains("rollback"));
            assert!(requires_rebuild);
        }
        other => panic!("wrong error classification: {other:?}"),
    }
}

#[test]
fn numerical_and_compile_failures_are_not_mathematical_outcomes() {
    assert!(matches!(
        FeasibilityRelaxationError::Numerical("non-finite candidate".into()),
        FeasibilityRelaxationError::Numerical(_)
    ));
    assert!(matches!(
        FeasibilityRelaxationError::Compile("dangling row".into()),
        FeasibilityRelaxationError::Compile(_)
    ));
}
