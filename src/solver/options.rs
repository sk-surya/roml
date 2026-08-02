//! Ergonomic solve options façade (API-01.3, plan Task 4).
//!
//! `SolveOptions` wraps the immutable [`SolveRequest`] contract with a
//! builder-style API. Builder methods are added in plan Task 4; this module
//! provides the shell the orchestration layer needs.

use crate::solver::request::SolveRequest;
use crate::solver::SolveError;

/// Ergonomically-built solve options for one solve attempt.
///
/// Defaults to an empty request (solver defaults apply). Builders (`time_limit`,
/// `relative_gap`, `absolute_gap`, `threads`, `output`, `random_seed`,
/// `backend_option`) are added by plan Task 4.
#[derive(Clone, Debug, Default)]
pub struct SolveOptions {
    pub(crate) request: SolveRequest,
}

impl SolveOptions {
    /// Create an empty option set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Validate options before synchronization. Currently accepts all inputs;
    /// plan Task 4 adds non-negative duration/gap and positive-threads checks.
    pub(crate) fn validate(&self) -> Result<(), SolveError> {
        Ok(())
    }

    /// Convert into the underlying immutable request (validated by the caller).
    pub(crate) fn into_request(self) -> SolveRequest {
        self.request
    }
}
