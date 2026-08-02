//! Conformance integration tests for HighsSession.
//!
//! Runs the shared synchronization conformance suite against
//! [`HighsFixture`], verifying that HiGHS correctly implements all
//! [`BackendSession`] lifecycle semantics alongside ReferenceBackend.

use roml::solver::conformance::run_sync_suite;
use roml::solver::session::BackendFixture;
use roml_highs::HighsFixture;

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
