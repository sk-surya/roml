//! Immutable solve requests and results.
//!
//! Solver policy is supplied through an explicit `SolveRequest` rather
//! than stored in canonical `Model` state. A request is either applied,
//! adjusted, or rejected — never silently ignored.

use crate::solver::backend::BackendCapabilities;

/// Algorithm selection for LP optimization.
///
/// Each solver backend maps these to its own controls.
/// Unsupported options are reported as rejections (not silently ignored).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LpAlgorithm {
    /// Let the solver choose automatically (default).
    #[default]
    Automatic,
    /// Primal simplex.
    PrimalSimplex,
    /// Dual simplex.
    DualSimplex,
    /// Barrier / interior point method.
    Barrier,
}

/// A solve request — immutable solver policy for one solve attempt.
///
/// All fields are optional; the solver applies defaults for unset values.
/// Unsupported options produce explicit rejection or adjustment, never
/// silent ignorance.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SolveRequest {
    /// LP algorithm preference.
    pub lp_algorithm: Option<LpAlgorithm>,

    /// Time limit in seconds (None = no limit).
    pub time_limit_secs: Option<f64>,

    /// MIP relative optimality gap tolerance.
    pub mip_rel_gap: Option<f64>,

    /// MIP absolute optimality gap tolerance.
    pub mip_abs_gap: Option<f64>,

    /// Maximum number of threads (None = solver default).
    pub threads: Option<i32>,

    /// Enable solver-specific logging/output.
    pub enable_output: Option<bool>,

    /// Seed for random number generator (for reproducibility).
    pub random_seed: Option<i32>,

    /// Extra solver-specific options (key-value pairs).
    pub extra_options: Vec<(String, String)>,
}

impl SolveRequest {
    /// Create an empty request (all defaults).
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the LP algorithm.
    pub fn with_lp_algorithm(mut self, algo: LpAlgorithm) -> Self {
        self.lp_algorithm = Some(algo);
        self
    }

    /// Set the time limit.
    pub fn with_time_limit(mut self, secs: f64) -> Self {
        self.time_limit_secs = Some(secs);
        self
    }

    /// Set MIP relative gap.
    pub fn with_mip_rel_gap(mut self, gap: f64) -> Self {
        self.mip_rel_gap = Some(gap);
        self
    }

    /// Set thread count.
    pub fn with_threads(mut self, n: i32) -> Self {
        self.threads = Some(n);
        self
    }

    /// Add an extra solver option.
    pub fn with_option(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.extra_options.push((key.into(), value.into()));
        self
    }
}

/// Result of a solve attempt.
///
/// Contains the effective configuration (what was actually used),
/// termination status, and solution data if available.
#[derive(Clone, Debug, PartialEq)]
pub struct SolveResult {
    /// The effective configuration applied (may differ from request
    /// if the solver adjusted options within supported bounds).
    pub effective_configuration: EffectiveConfig,

    /// How the solve terminated.
    pub termination: crate::solver::backend::TerminationStatus,

    /// Solution data, if available.
    pub solution: Option<SolveSolution>,
}

/// The configuration that was actually applied by the solver.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EffectiveConfig {
    /// LP algorithm used.
    pub lp_algorithm: Option<LpAlgorithm>,
    /// Time limit applied.
    pub time_limit_secs: Option<f64>,
    /// MIP gap applied.
    pub mip_rel_gap: Option<f64>,
    /// Thread count used.
    pub threads: Option<i32>,
    /// Whether output was enabled.
    pub enable_output: Option<bool>,
    /// Options that were adjusted from the request (with reason).
    pub adjustments: Vec<ConfigAdjustment>,
    /// Options that were rejected.
    pub rejections: Vec<ConfigRejection>,
}

/// An option that was adjusted from the requested value.
#[derive(Clone, Debug, PartialEq)]
pub struct ConfigAdjustment {
    /// The option key.
    pub key: String,
    /// The requested value.
    pub requested: String,
    /// The applied value.
    pub applied: String,
    /// Why it was adjusted.
    pub reason: String,
}

/// An option that was rejected entirely.
#[derive(Clone, Debug, PartialEq)]
pub struct ConfigRejection {
    /// The option key.
    pub key: String,
    /// Why it was rejected.
    pub reason: String,
}

/// Solution data extracted from a solver.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SolveSolution {
    /// Variable values (by VarId).
    pub variable_values: Vec<(crate::id::VarId, f64)>,
    /// Objective value.
    pub objective_value: Option<f64>,
    /// Dual values for constraints (by ConId), if available.
    pub dual_values: Option<Vec<(crate::id::ConId, f64)>>,
    /// Reduced costs for variables (by VarId), if available.
    pub reduced_costs: Option<Vec<(crate::id::VarId, f64)>>,
}

/// Validate that a request is compatible with a backend's capabilities.
///
/// Returns a list of rejections for unsupported options.
/// Does not modify the request.
pub fn validate_request(
    request: &SolveRequest,
    capabilities: &BackendCapabilities,
) -> Vec<ConfigRejection> {
    let mut rejections = Vec::new();

    // MIP options require MIP capability
    if request.mip_rel_gap.is_some() && !capabilities.mip {
        rejections.push(ConfigRejection {
            key: "mip_rel_gap".into(),
            reason: "backend does not support MIP".into(),
        });
    }
    if request.mip_abs_gap.is_some() && !capabilities.mip {
        rejections.push(ConfigRejection {
            key: "mip_abs_gap".into(),
            reason: "backend does not support MIP".into(),
        });
    }

    rejections
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_request_is_empty() {
        let req = SolveRequest::new();
        assert!(req.lp_algorithm.is_none());
        assert!(req.time_limit_secs.is_none());
    }

    #[test]
    fn builder_pattern() {
        let req = SolveRequest::new()
            .with_lp_algorithm(LpAlgorithm::DualSimplex)
            .with_time_limit(60.0)
            .with_threads(4);

        assert_eq!(req.lp_algorithm, Some(LpAlgorithm::DualSimplex));
        assert_eq!(req.time_limit_secs, Some(60.0));
        assert_eq!(req.threads, Some(4));
    }

    #[test]
    fn validate_rejects_mip_options_on_lp_only_backend() {
        let mut caps = BackendCapabilities::all();
        caps.mip = false;

        let req = SolveRequest::new().with_mip_rel_gap(0.01);
        let rejections = validate_request(&req, &caps);
        assert_eq!(rejections.len(), 1);
        assert!(rejections[0].key.contains("mip"));
    }

    #[test]
    fn validate_accepts_all_on_full_backend() {
        let caps = BackendCapabilities::all();
        let req = SolveRequest::new()
            .with_lp_algorithm(LpAlgorithm::Barrier)
            .with_mip_rel_gap(0.01)
            .with_time_limit(30.0);
        let rejections = validate_request(&req, &caps);
        assert!(rejections.is_empty());
    }
}
