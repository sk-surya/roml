//! Conformance integration tests for the ReferenceBackend.
//!
//! Runs the shared synchronization conformance suite against
//! [`RefBackendFixture`], verifying that the reference backend correctly
//! implements all [`BackendSession`] lifecycle semantics.

use roml::solver::conformance::run_sync_suite;
use roml::solver::reference::RefBackendFixture;

#[test]
fn conformance_reference_backend() {
    let fixture = RefBackendFixture;
    run_sync_suite(&fixture);
}
