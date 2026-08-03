//! User-facing solve error (API-01.2, D4).
//!
//! `SolveError` is the error type returned by the user-facing solve path.
//! It distinguishes:
//!
//! - [`Commit`](SolveError::Commit) — model commit failed before any backend
//!   mutation (D5 constraint);
//! - [`InvalidOptions`](SolveError::InvalidOptions) — solve options failed
//!   validation before synchronization;
//! - [`NoActiveObjective`](SolveError::NoActiveObjective) — the model has no
//!   active objective to optimize;
//! - [`Synchronization`](SolveError::Synchronization) — backend synchronization
//!   failed;
//! - [`Solve`](SolveError::Solve) — the backend solve call failed;
//! - [`License`](SolveError::License) — the backend reported a license failure
//!   (terminal);
//! - [`Status`](SolveError::Status) — the backend returned an uninterpretable
//!   termination status (e.g. `Error` or `Unknown`), so no `Solution` can be
//!   produced (API-03.3).
//!
//! Backend errors are wrapped whole so their identity (message), operation,
//! [`ErrorCategory`], and [`HealthEffect`] remain inspectable (API-02.4).

use crate::compiler::backend_ir::CompilationId;
use crate::model::ModelError;
use crate::solver::backend::{BackendError, ErrorCategory, HealthEffect, TerminationStatus};
use crate::solver::overlay::OverlayError;

/// Error returned by the user-facing solve façade.
#[derive(Clone, Debug, PartialEq)]
pub enum SolveError {
    /// Model commit failed before backend mutation.
    Commit(ModelError),
    /// Solve options failed validation before synchronization.
    InvalidOptions(String),
    /// The model has no active objective.
    NoActiveObjective,
    /// Backend synchronization failed.
    Synchronization(BackendError),
    /// The backend solve call failed.
    Solve(BackendError),
    /// Backend license failure (terminal).
    License(BackendError),
    /// Backend terminated in an uninterpretable status.
    Status(TerminationStatus),
    /// The backend returned a result tagged with a `CompilationId` that does
    /// not match the compiled state the façade synchronized to (F2, SM-03.9).
    /// A result produced from a different compiled state is never accepted
    /// silently.
    CompilationMismatch {
        /// The compilation the façade synchronized to (the compiler's current
        /// compiled state id).
        expected: Option<CompilationId>,
        /// The compilation the result actually claimed (F5: `None` means the
        /// backend fabricated no compiled state at all — still a mismatch).
        actual: Option<CompilationId>,
    },
    /// The solve overlay failed to compile (P27 Task 10). The overlay is
    /// rejected BEFORE any backend mutation (SM-06.6) — stale compilation,
    /// assignment/band/value validation, or an unknown objective reference.
    Overlay(OverlayError),
    /// A backend error during the transactional overlay lifecycle (P27
    /// Task 10): apply, rollback, or post-rollback verification failed. The
    /// backend is marked `RequiresRebuild` so a partially overlaid state is
    /// never silently reused (SM-07.4, D7).
    Rollback(BackendError),
}

impl SolveError {
    /// Wrap a backend error, promoting license failures to the dedicated
    /// [`License`](SolveError::License) variant and otherwise keeping the
    /// synchronization/solve distinction chosen by the caller.
    ///
    /// `is_synchronization` selects [`Synchronization`](SolveError::Synchronization)
    /// versus [`Solve`](SolveError::Solve) for non-license errors.
    pub fn from_backend(is_synchronization: bool, error: BackendError) -> Self {
        if error.category == ErrorCategory::LicenseFailure {
            SolveError::License(error)
        } else if is_synchronization {
            SolveError::Synchronization(error)
        } else {
            SolveError::Solve(error)
        }
    }

    /// The backend error wrapped by this error, if any.
    pub fn backend(&self) -> Option<&BackendError> {
        match self {
            SolveError::Synchronization(e) | SolveError::Solve(e) | SolveError::License(e) => {
                Some(e)
            }
            SolveError::Rollback(e) => Some(e),
            _ => None,
        }
    }

    /// Whether this error leaves the backend in a terminal state, in which
    /// case retrying (e.g. a snapshot rebuild) is meaningless — the plan's
    /// "terminal -> return without retry" branch (API-02.3).
    pub fn is_terminal(&self) -> bool {
        match self {
            SolveError::License(_) => true,
            SolveError::Synchronization(e) | SolveError::Solve(e) | SolveError::Rollback(e) => {
                e.health_effect == HealthEffect::Terminal
            }
            _ => false,
        }
    }

    /// The error category of this failure (API-02.4).
    pub fn category(&self) -> Option<ErrorCategory> {
        match self {
            SolveError::Synchronization(e) | SolveError::Solve(e) | SolveError::License(e) => {
                Some(e.category)
            }
            SolveError::Rollback(e) => Some(e.category),
            SolveError::Commit(_)
            | SolveError::InvalidOptions(_)
            | SolveError::NoActiveObjective
            | SolveError::CompilationMismatch { .. }
            | SolveError::Overlay(_) => Some(ErrorCategory::InvalidInput),
            SolveError::Status(_) => Some(ErrorCategory::Unknown),
        }
    }

    /// The effect on backend session health of this failure (API-02.4).
    pub fn health_effect(&self) -> Option<HealthEffect> {
        match self {
            SolveError::Synchronization(e) | SolveError::Solve(e) | SolveError::License(e) => {
                Some(e.health_effect)
            }
            SolveError::Rollback(e) => Some(e.health_effect),
            _ => None,
        }
    }

    /// A short human-readable summary of this failure.
    pub fn message(&self) -> String {
        match self {
            SolveError::Commit(e) => format!("model commit failed: {e}"),
            SolveError::InvalidOptions(m) => format!("invalid solve options: {m}"),
            SolveError::NoActiveObjective => "no active objective in model".to_string(),
            SolveError::Synchronization(e) | SolveError::Solve(e) | SolveError::License(e) => {
                format!("backend error: {e}")
            }
            SolveError::Status(t) => format!("solve terminated in uninterpretable status: {t:?}"),
            SolveError::CompilationMismatch { expected, actual } => format!(
                "solve result tagged with compilation {actual:?}, but the façade synchronized to \
                 compilation {expected:?}"
            ),
            SolveError::Overlay(e) => format!("solve overlay failed: {e:?}"),
            SolveError::Rollback(e) => format!("overlay rollback failed: {e}"),
        }
    }
}

impl std::fmt::Display for SolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message())
    }
}

impl std::error::Error for SolveError {}

impl From<ModelError> for SolveError {
    fn from(e: ModelError) -> Self {
        SolveError::Commit(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_mentions_failure_class() {
        let err = SolveError::InvalidOptions("threads must be positive".into());
        assert!(err.to_string().contains("invalid solve options"));
        let commit = SolveError::Commit(ModelError::InvalidBounds);
        assert!(commit.to_string().contains("model commit failed"));
        let status = SolveError::Status(TerminationStatus::Error);
        assert!(status.to_string().contains("uninterpretable"));
    }

    #[test]
    fn backend_accessor_returns_wrapped_error() {
        let be = BackendError::new("boom", ErrorCategory::Internal, HealthEffect::Terminal);
        let err = SolveError::Solve(be.clone());
        assert_eq!(err.backend(), Some(&be));
        assert_eq!(err.category(), Some(ErrorCategory::Internal));
        assert_eq!(err.health_effect(), Some(HealthEffect::Terminal));
    }

    #[test]
    fn non_backend_variants_have_no_backend_error() {
        assert_eq!(SolveError::NoActiveObjective.backend(), None);
        assert_eq!(
            SolveError::Commit(ModelError::InvalidBounds).category(),
            Some(ErrorCategory::InvalidInput)
        );
        assert_eq!(
            SolveError::Status(TerminationStatus::Unknown).category(),
            Some(ErrorCategory::Unknown)
        );
    }

    #[test]
    fn from_model_error_maps_to_commit() {
        let e = ModelError::RevisionOverflow;
        let err = SolveError::from(e);
        assert!(matches!(
            err,
            SolveError::Commit(ModelError::RevisionOverflow)
        ));
    }
}
