//! Ergonomic solve options façade (API-01.3, plan Task 4).
//!
//! `SolveOptions` wraps the immutable [`SolveRequest`] contract with a
//! builder-style API. Builders map directly onto request fields; validation of
//! non-negative durations/gaps and positive thread counts happens before
//! synchronization, so a failed validation leaves the model and backend state
//! unchanged. The effective configuration (including any adjustments and
//! rejections from backend negotiation) is preserved on the returned solution
//! through [`SolveMetadata`](crate::SolveMetadata).

use std::time::Duration;

use crate::solver::request::SolveRequest;
use crate::solver::SolveError;

/// Ergonomically-built solve options for one solve attempt.
///
/// Defaults to an empty request (solver defaults apply).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SolveOptions {
    pub(crate) request: SolveRequest,
}

impl SolveOptions {
    /// Create an empty option set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a wall-clock time limit for the solve.
    pub fn time_limit(mut self, limit: Duration) -> Self {
        self.request.time_limit_secs = Some(limit.as_secs_f64());
        self
    }

    /// Set the MIP relative optimality gap tolerance.
    pub fn relative_gap(mut self, gap: f64) -> Self {
        self.request.mip_rel_gap = Some(gap);
        self
    }

    /// Set the MIP absolute optimality gap tolerance.
    pub fn absolute_gap(mut self, gap: f64) -> Self {
        self.request.mip_abs_gap = Some(gap);
        self
    }

    /// Set the maximum number of threads.
    pub fn threads(mut self, count: i32) -> Self {
        self.request.threads = Some(count);
        self
    }

    /// Enable or disable solver output/logging.
    pub fn output(mut self, enabled: bool) -> Self {
        self.request.enable_output = Some(enabled);
        self
    }

    /// Set the random seed for reproducible solves.
    pub fn random_seed(mut self, seed: i32) -> Self {
        self.request.random_seed = Some(seed);
        self
    }

    /// Add a backend-specific option as a key/value pair.
    pub fn backend_option(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.request.extra_options.push((key.into(), value.into()));
        self
    }

    /// Validate options before synchronization (plan Task 4).
    ///
    /// Rejects negative or non-finite gaps and non-positive thread counts.
    /// A [`Duration`] time limit is non-negative by construction. Returns
    /// [`SolveError::InvalidOptions`] on the first violation; the caller
    /// (the façade) validates before committing or touching the backend, so
    /// model and backend state are left unchanged.
    pub(crate) fn validate(&self) -> Result<(), SolveError> {
        if let Some(gap) = self.request.mip_rel_gap {
            if !(gap.is_finite() && gap >= 0.0) {
                return Err(SolveError::InvalidOptions(format!(
                    "relative_gap must be a non-negative finite value, got {gap}"
                )));
            }
        }
        if let Some(gap) = self.request.mip_abs_gap {
            if !(gap.is_finite() && gap >= 0.0) {
                return Err(SolveError::InvalidOptions(format!(
                    "absolute_gap must be a non-negative finite value, got {gap}"
                )));
            }
        }
        if let Some(threads) = self.request.threads {
            if threads <= 0 {
                return Err(SolveError::InvalidOptions(format!(
                    "threads must be positive, got {threads}"
                )));
            }
        }
        Ok(())
    }

    /// Convert into the underlying immutable request (validated by the caller).
    pub(crate) fn into_request(self) -> SolveRequest {
        self.request
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builders_map_onto_request_fields() {
        let opts = SolveOptions::new()
            .time_limit(Duration::from_secs(120))
            .relative_gap(0.05)
            .absolute_gap(0.001)
            .threads(8)
            .output(false)
            .random_seed(7)
            .backend_option("solver", "simplex");
        assert_eq!(opts.request.time_limit_secs, Some(120.0));
        assert_eq!(opts.request.mip_rel_gap, Some(0.05));
        assert_eq!(opts.request.mip_abs_gap, Some(0.001));
        assert_eq!(opts.request.threads, Some(8));
        assert_eq!(opts.request.enable_output, Some(false));
        assert_eq!(opts.request.random_seed, Some(7));
        assert_eq!(
            opts.request.extra_options,
            vec![("solver".to_string(), "simplex".to_string())]
        );
    }

    #[test]
    fn valid_options_validate_ok() {
        let opts = SolveOptions::new()
            .time_limit(Duration::from_secs(1))
            .relative_gap(0.0)
            .absolute_gap(0.5)
            .threads(1);
        assert!(opts.validate().is_ok());
    }

    #[test]
    fn invalid_options_validate_err() {
        for opts in [
            SolveOptions::new().relative_gap(-1.0),
            SolveOptions::new().relative_gap(f64::NAN),
            SolveOptions::new().absolute_gap(-0.1),
            SolveOptions::new().absolute_gap(f64::INFINITY),
            SolveOptions::new().threads(0),
            SolveOptions::new().threads(-3),
        ] {
            assert!(opts.validate().is_err(), "expected validation failure");
        }
    }

    #[test]
    fn default_options_validate_ok() {
        assert!(SolveOptions::default().validate().is_ok());
    }
}
