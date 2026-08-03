//! Canonical scalar functions (design §6).
//!
//! M3 implements linear scalar functions only; later milestones may add
//! quadratic and nonlinear variants. The enum is `#[non_exhaustive]` so
//! extension is additive.

use crate::expr::LinExpr;
use crate::id::ParamId;

/// A canonical scalar function.
///
/// The enum is the nonlinear-ready extension seam: M3 implements only
/// [`ScalarFunction::Linear`] (SM-01.2).
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum ScalarFunction {
    /// A linear function `Σ a_i x_i + c`.
    Linear(LinExpr),
}

impl ScalarFunction {
    /// Derive the parameter dependencies of this function (F1).
    ///
    /// Dependencies are DERIVED from the function's symbolic `ValueExpr`
    /// coefficients, never stored: a consumer calls this instead of reading a
    /// cached field. For the linear form this delegates to
    /// [`LinExpr::parameter_dependencies`].
    pub fn parameter_dependencies(&self) -> Vec<ParamId> {
        match self {
            Self::Linear(expr) => expr.parameter_dependencies(),
        }
    }
}
