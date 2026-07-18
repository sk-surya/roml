//! Backend metadata, capabilities, and typed errors.
//!
//! Every solver backend reports its identity, version, supported
//! operations, and classifies failures into categories that the
//! synchronization coordinator can act on.

/// Information about a solver backend.
#[derive(Clone, Debug, PartialEq)]
pub struct BackendInfo {
    /// Human-readable backend name (e.g., "HiGHS 1.9.0").
    pub name: String,
    /// Backend version string.
    pub version: String,
    /// Build/host information.
    pub build_info: String,
    /// Supported capabilities.
    pub capabilities: BackendCapabilities,
}

/// What this backend supports.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct BackendCapabilities {
    /// Supports incremental variable addition.
    pub add_variable: bool,
    /// Supports incremental constraint addition.
    pub add_constraint: bool,
    /// Supports incremental coefficient changes.
    pub set_coefficient: bool,
    /// Supports incremental bound changes.
    pub set_bounds: bool,
    /// Supports incremental objective changes.
    pub set_objective: bool,
    /// Supports variable/constraint deletion.
    pub delete: bool,
    /// Supports LP solving.
    pub lp: bool,
    /// Supports MIP solving.
    pub mip: bool,
    /// Supports callbacks during solve.
    pub callbacks: bool,
    /// Supports solution retrieval.
    pub solution: bool,
    /// Supports dual values.
    pub duals: bool,
    /// Supports reduced costs.
    pub reduced_costs: bool,
    /// Supports semi-continuous variables.
    pub semicontinuous: bool,
    /// Supports semi-integer variables.
    pub semiinteger: bool,
    /// Supports parameter/sensitivity updates without full rebuild.
    pub parameter_update: bool,
}

impl BackendCapabilities {
    /// Full capabilities (reference backend).
    pub const fn all() -> Self {
        Self {
            add_variable: true,
            add_constraint: true,
            set_coefficient: true,
            set_bounds: true,
            set_objective: true,
            delete: true,
            lp: true,
            mip: true,
            callbacks: true,
            solution: true,
            duals: true,
            reduced_costs: true,
            semicontinuous: true,
            semiinteger: true,
            parameter_update: true,
        }
    }
}

/// Categorised native error with adapter health implication.
#[derive(Clone, Debug, PartialEq)]
pub struct BackendError {
    /// Human-readable message.
    pub message: String,
    /// Error category (determines recovery behavior).
    pub category: ErrorCategory,
    /// Native error code, if available.
    pub native_code: Option<i32>,
    /// Effect on adapter health.
    pub health_effect: HealthEffect,
}

/// Category of backend error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorCategory {
    /// Invalid input (model data rejected by solver).
    InvalidInput,
    /// Operation not supported by this backend.
    Unsupported,
    /// Native library not found or failed to load.
    LibraryNotFound,
    /// License check failed.
    LicenseFailure,
    /// Solver-specific numerical issue.
    Numerical,
    /// Memory/resource exhaustion.
    OutOfMemory,
    /// Solver internal error.
    Internal,
    /// Timeout or iteration limit.
    Limit,
    /// Unknown/unclassified error.
    Unknown,
}

/// Effect of an error on the adapter session health.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HealthEffect {
    /// Backend is still healthy; operation was a no-op.
    None,
    /// Operation failed but backend is unchanged; recoverable.
    Recoverable,
    /// Backend needs a full rebuild from snapshot.
    RequiresRebuild,
    /// Backend session is terminally broken.
    Terminal,
}

impl BackendError {
    /// Create a new backend error.
    pub fn new(
        message: impl Into<String>,
        category: ErrorCategory,
        health_effect: HealthEffect,
    ) -> Self {
        Self {
            message: message.into(),
            category,
            native_code: None,
            health_effect,
        }
    }

    /// Create an error with a native code.
    pub fn with_code(
        message: impl Into<String>,
        category: ErrorCategory,
        health_effect: HealthEffect,
        native_code: i32,
    ) -> Self {
        Self {
            message: message.into(),
            category,
            native_code: Some(native_code),
            health_effect,
        }
    }

    /// An unsupported operation that requires rebuild.
    pub fn unsupported(op: impl Into<String>) -> Self {
        Self::new(
            format!("operation not supported: {}", op.into()),
            ErrorCategory::Unsupported,
            HealthEffect::RequiresRebuild,
        )
    }

    /// A library-not-found error (terminal).
    pub fn library_not_found(detail: impl Into<String>) -> Self {
        Self::new(
            detail.into(),
            ErrorCategory::LibraryNotFound,
            HealthEffect::Terminal,
        )
    }

    /// A license failure (terminal).
    pub fn license_failure(detail: impl Into<String>) -> Self {
        Self::new(
            detail.into(),
            ErrorCategory::LicenseFailure,
            HealthEffect::Terminal,
        )
    }
}

impl std::fmt::Display for BackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{:?}] {} (health: {:?})",
            self.category, self.message, self.health_effect
        )
    }
}

impl std::error::Error for BackendError {}

/// Precise termination status from a solve.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TerminationStatus {
    /// Optimal solution found.
    Optimal,
    /// Proven infeasible.
    Infeasible,
    /// Proven unbounded.
    Unbounded,
    /// Feasible solution found (not proven optimal — MIP).
    Feasible,
    /// Preserves ambiguity (HiGHS can return this instead of Infeasible/Unbounded).
    InfeasibleOrUnbounded,
    /// Time limit reached.
    TimeLimit,
    /// Iteration limit reached.
    IterationLimit,
    /// Node limit reached (MIP).
    NodeLimit,
    /// Solver interrupted (e.g., callback).
    Interrupted,
    /// Numerical difficulties.
    NumericalIssue,
    /// Solver error.
    Error,
    /// Unknown status.
    #[default]
    Unknown,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_error_unsupported() {
        let err = BackendError::unsupported("semi-continuous");
        assert_eq!(err.category, ErrorCategory::Unsupported);
        assert_eq!(err.health_effect, HealthEffect::RequiresRebuild);
    }

    #[test]
    fn backend_error_library_not_found() {
        let err = BackendError::library_not_found("libhighs.so not in LD_LIBRARY_PATH");
        assert_eq!(err.category, ErrorCategory::LibraryNotFound);
        assert_eq!(err.health_effect, HealthEffect::Terminal);
    }

    #[test]
    fn full_capabilities() {
        let caps = BackendCapabilities::all();
        assert!(caps.lp);
        assert!(caps.mip);
        assert!(caps.solution);
    }

    #[test]
    fn default_capabilities_are_all_false() {
        let caps = BackendCapabilities::default();
        assert!(!caps.lp);
        assert!(!caps.solution);
    }
}
