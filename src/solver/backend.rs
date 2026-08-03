//! Backend metadata, capabilities, and typed errors.
//!
//! Every solver backend reports its identity, version, supported
//! operations, and classifies failures into categories that the
//! synchronization coordinator can act on.
//!
//! # Capability model (P26 Task 6 migration)
//!
//! [`BackendCapabilities`] is the legacy flat capability record, retained for
//! M2 source compatibility (D27). The typed
//! [`BackendCapabilitySet`](crate::compiler::capability::BackendCapabilitySet)
//! is now the authoritative model for request validation and backend
//! capability declarations (D10, SM-04): `validate_request` gates on typed
//! [`BackendFeature`](crate::compiler::capability::BackendFeature)s, and
//! `roml-highs` builds a version-aware typed set. Backends that implement
//! [`BackendMetadata`](crate::solver::session::BackendMetadata) return the flat
//! [`BackendCapabilities`] compat view derived from their typed set.

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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminationStatus {
    /// Optimal solution found.
    Optimal,
    /// Proven infeasible.
    Infeasible,
    /// Proven infeasible or unbounded (preserved ambiguity).
    InfeasibleOrUnbounded,
    /// Proven unbounded.
    Unbounded,
    /// Feasible solution found (not proven optimal — MIP).
    Feasible,
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

    /// Characterize the legacy flat capability mapping onto the typed
    /// `BackendFeature` surface (P26 Task 6, D10). The flat fields with a typed
    /// equivalent map onto `Lp`, `Mip`, `IncrementalBounds`, `IncrementalRows`,
    /// and `IncrementalCoefficients`; the remaining flat fields have no typed
    /// equivalent and stay flat-only.
    #[test]
    fn characterize_legacy_flat_mapping_onto_typed_features() {
        use crate::compiler::capability::BackendFeature;

        let caps = BackendCapabilities::all();

        // Flat LP/MIP capability maps onto the typed LP/MIP features.
        assert!(caps.lp);
        assert!(caps.mip);
        // Incremental variable/constraint/coefficient/bound capability maps
        // onto the typed incremental features.
        assert!(caps.add_variable);
        assert!(caps.add_constraint);
        assert!(caps.set_coefficient);
        assert!(caps.set_bounds);
        assert!(caps.set_objective);

        // The typed feature surface names the same concepts.
        let _ = BackendFeature::Lp;
        let _ = BackendFeature::Mip;
        let _ = BackendFeature::IncrementalBounds;
        let _ = BackendFeature::IncrementalRows;
        let _ = BackendFeature::IncrementalCoefficients;

        // Flat-only fields (solution, duals, reduced_costs, callbacks,
        // delete, parameter_update, semicontinuous, semiinteger) have no typed
        // `BackendFeature` equivalent and are preserved flat-only.
        assert!(caps.solution);
        assert!(caps.duals);
        assert!(caps.reduced_costs);
        assert!(caps.callbacks);
        assert!(caps.delete);
    }
}
