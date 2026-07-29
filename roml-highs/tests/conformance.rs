//! Conformance integration tests for HighsSession.
//!
//! Runs the shared synchronization conformance suite against
//! [`HighsFixture`], verifying that HiGHS correctly implements all
//! [`BackendSession`] lifecycle semantics alongside ReferenceBackend.

use roml::solver::conformance::run_sync_suite;
use roml_highs::HighsFixture;

#[test]
fn conformance_highs_session() {
    let fixture = HighsFixture;
    run_sync_suite(&fixture);
}
