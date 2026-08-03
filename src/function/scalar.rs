//! Canonical scalar functions (design §6).
//!
//! M3 implements linear scalar functions only; later milestones may add
//! quadratic and nonlinear variants. The enum is `#[non_exhaustive]` so
//! extension is additive.

use crate::expr::LinExpr;

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
